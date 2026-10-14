# Passkey + Web3 Authentication Without Collected PII

Research and architecture brainstorm for Astral Key

**Date:** 2026-07-30
**Scope:** Evaluate Astral Key as the foundation for a system that authenticates only with WebAuthn/passkeys and Web3 signatures, while deliberately avoiding collection and retention of directly identifying personal information.

## Executive recommendation

Astral Key is a promising foundation, but the target should be stated precisely as:

> **Astral Key collects no directly identifying profile data and minimizes operational data; authentication is based only on public-key credentials and wallet signatures.**

This promise applies to Astral Key's application storage and documented telemetry—not automatically to reverse proxies, hosting providers, operating-system logs, backups, browser/platform providers, wallet providers, or RPC providers. A literal claim of anonymity or “no personal data” would be inaccurate. A wallet address, WebAuthn credential identifier, opaque account identifier, IP address, timestamps, and authentication history can become personal data when linkable to a person. Public blockchains also make wallet activity inherently observable and correlatable.

Recommended direction:

1. Keep WebAuthn/passkeys and SIWE as the only interactive authentication protocols.
2. Remove OAuth and all email/name/avatar/token fields.
3. Remove persisted IP address and device information from sessions and audit events.
4. Use opaque random internal subjects; never derive them from emails, wallet addresses, credential IDs, or usernames.
5. Treat passkeys and wallets as separate authentication methods that are linked only by an explicit user action.
6. Support recovery through multiple passkeys and optional explicit wallet linking—not through email, phone, or administrator identity proof.
7. Use short-lived, minimal sessions and keep operational logs free of addresses, signatures, credentials, request bodies, IPs, and user-agent data.

## What the current repository contains

The current Astral Key codebase already has the core primitives:

- WebAuthn/FIDO2 registration and authentication
- SIWE/Web3 nonce and signature verification
- SQLite persistence
- JWT access tokens and refresh-token rotation
- Passkey CRUD
- Additional identity, contact, API-key, capability, and JIT-token features

The current design is not yet aligned with the strict target:

- `src/api/routes.rs` wires GitHub OAuth login and callback routes.
- `migrations/004_oauth_accounts.sql` stores provider IDs, email, name, avatar URL, and OAuth tokens.
- `migrations/002_api_keys_and_sessions.sql` stores `device_info` and `ip_address`.
- `migrations/001_initial.sql` permits a free-form passkey `name`.
- Identity/contact labels and the wider identity graph can accept user-entered identifying text.
- `src/api/middleware/audit.rs` extracts and emits `X-Forwarded-For`/`X-Real-IP` as `client_ip` for every API request.
- The README currently suggests that passkeys are self-hosted and do not roam through cloud providers. That is too strong: the RP stores the credential public material, while the authenticator or password manager may sync a passkey according to the user's platform/provider.

## Terminology and privacy boundary

### Three useful product promises

**No PII collection:** The product does not ask for or persist email addresses, phone numbers, legal names, profile images, government identifiers, or OAuth profiles.

**Privacy-minimized:** The product also avoids persistent IP addresses, device fingerprints, user-agent logs, unnecessary analytics, address enrichment, ENS lookups, and free-form profile fields.

**Anonymous:** Do not use this as a blanket promise. A wallet address and a passkey-backed account can be stable pseudonymous identifiers, and the network layer still sees traffic. A self-hosted operator may also be able to correlate access with infrastructure logs.

The first two promises are realistic. The third depends on deployment, network, wallet funding history, browser/platform behavior, and external data sources that Astral Key cannot control.

## Findings from primary standards

### WebAuthn and passkeys

The W3C WebAuthn Level 3 specification requires the RP to provide a user handle (`user.id`) for an account; it is capped at 64 bytes and must not contain personally identifying information. A random opaque value is the recommended implementation and must not encode an email, username, wallet address, or other identifying value. Discoverable credentials let a user authenticate without first supplying a username; they are appropriate for a passwordless, no-PII flow.

Use these defaults:

- Configure `residentKey: required` when the product requires discoverable passkeys; this is a recommended policy for username-less login, not a universal requirement for every credential described as a passkey.
- Set `userVerification: required` as this product's security policy for login and account-recovery-sensitive operations; it is not a universal requirement for every passkey deployment.
- `attestation: none` for consumer privacy unless a narrowly defined enterprise requirement justifies otherwise.
- A random, stable per-account user handle generated by the server.
- No passkey display name supplied by the user, or a constrained system label such as `Passkey 1`.
- Store only credential ID, public key, counter/sign-in state, required transport/authenticator metadata, opaque user ID, and timestamps.

Passkeys have an important tradeoff: some are device-bound and some are synced or backed up by a platform/password manager. A synced passkey improves recovery and portability but expands the user's trust boundary to that provider. Astral Key cannot truthfully claim that all passkey private keys remain exclusively on one physical device.

Backup eligibility/state flags can be used for risk decisions, but they should not be used as a user fingerprint or copied into logs. Direct/enterprise attestation should be avoided because authenticator identity can create tracking and device-model leakage.

Primary source: [W3C WebAuthn Level 3](https://www.w3.org/TR/webauthn-3/)

### SIWE and Web3 authentication

EIP-4361 defines a human-readable Sign-In with Ethereum message. The verifier must validate the complete message, not merely recover an address from a signature:

- exact expected domain/authority
- expected URI/origin
- version `1`
- supported EIP-155 chain ID
- fresh, server-generated, single-use nonce
- issued-at and expiration bounds
- signature matches the claimed address

For smart-contract wallets, support EIP-1271 rather than assuming every address is an EOA. Contract signature verification depends on current contract state and RPC availability, so the implementation needs explicit RPC timeouts, chain selection, failure behavior, and a policy for session validity after wallet ownership changes. Do not treat a successful verification as permanently valid if the account's on-chain authorization state can later change.

Do not resolve ENS names, avatars, text records, transaction history, balances, or funding provenance as part of login. Those lookups add third-party correlation and are unnecessary for authentication.

A wallet address is pseudonymous, not anonymous. Reusing the same address across services creates a global correlation handle. The product should explain this plainly and avoid linking a wallet to a passkey automatically.

Primary sources:

- [EIP-4361: Sign-In with Ethereum](https://eips.ethereum.org/EIPS/eip-4361)
- [EIP-1271: Standard Signature Validation Method for Contracts](https://eips.ethereum.org/EIPS/eip-1271)
- [EIP-155: Replay protection and chain IDs](https://eips.ethereum.org/EIPS/eip-155)
- [CAIP-122: Sign in with X](https://namespaces.chainagnostic.org/caips/caip-122)

### Sessions and tokens

The authentication proof can be privacy-minimal while the resulting session is still vulnerable to theft or replay. Keep those concerns separate.

For browser sessions, prefer an opaque, high-entropy, server-side session cookie:

- `Secure`
- `HttpOnly`
- `SameSite=Strict` where compatible, otherwise `Lax`
- `__Host-` prefix where deployment permits
- short idle and absolute expiry
- explicit revocation and refresh-token rotation
- no wallet address, credential ID, IP, device data, or authentication method in the cookie value

For downstream APIs that require JWTs, use minimal claims: an opaque subject, issuer, audience, token type, issued-at, expiry, session ID or token ID, and narrowly scoped permissions. Enforce an explicit algorithm allow-list, issuer, audience, expiry, and token type. Consider EdDSA/Ed25519 for a service-oriented deployment rather than distributing an HMAC secret to every verifier.

DPoP-style sender-constrained tokens are a worthwhile hardening option when browser/mobile tokens are exposed to multiple services, but they are not an MVP prerequisite. First fix cookie handling, XSS defenses, token lifetime, revocation, and JWT validation.

Primary sources:

- [OWASP Session Management Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html)
- [NIST SP 800-63B / 800-63-4 session guidance](https://pages.nist.gov/800-63-4/sp800-63b/session/)
- [RFC 8725: JWT Best Current Practices](https://datatracker.ietf.org/doc/html/rfc8725)
- [RFC 9449: DPoP](https://datatracker.ietf.org/doc/html/rfc9449)

## Identity model options

### Option A: One account with linked methods — recommended default

Create an opaque account ID, then attach one or more passkey credentials and one or more wallet authenticators. Linking requires an already authenticated session and a fresh ceremony with the second method. The account ID, credentials, wallet address, timestamps, and linkage record remain potentially personal data in context even when no profile fields are collected.

**Advantages:** familiar account model, multiple passkeys, wallet recovery, simple downstream authorization.

**Risks:** linking a wallet and passkey creates a durable correlation record. A database compromise reveals that relationship. Make linking explicit, visible, and reversible; do not auto-link based on timing, browser state, or wallet address.

### Option B: Separate principals, no linking — strongest privacy

Passkeys and wallets create independent principals. A service can authorize both, but Astral Key never stores that they belong to the same person.

**Advantages:** limits cross-protocol correlation and reduces the impact of a database compromise.

**Risks:** poor recovery and UX; users cannot naturally manage “their” methods together; downstream services must handle multiple subjects.

### Option C: Wallet as mandatory second factor — not recommended for MVP

Require both passkey and wallet for every account or sensitive action.

**Advantages:** strong dual-control semantics and recovery potential.

**Risks:** excludes users without wallets, exposes wallet linkability on every login, increases transaction/signing friction, and creates a larger failure surface for RPC and smart-contract-wallet support.

### Suggested policy

Use Option A for normal self-hosted deployments. If a privacy mode is offered, define it as a separate-subject mode—not merely a setting that hides a linking button: passkey authentication and wallet authentication must create distinct account subjects, and the database, sessions, audit logs, rate-limit state, backups, and deletion records must not provide an indirect linking channel. A simpler first release can instead offer one explicit policy: linked methods are opt-in, visible, and reversible. Do not call “linking disabled” a privacy mode unless the underlying subject model actually prevents correlation.

## Recovery without email or phone

A no-PII system cannot offer “reset by email.” Recovery must be designed as key management:

1. Encourage registration of at least two independent passkeys, ideally across different authenticator types.
2. Allow a currently authenticated user to add a wallet after a fresh passkey verification.
3. Allow a currently authenticated wallet to add a passkey after a fresh SIWE verification, if the account has explicitly opted into wallet recovery.
4. For higher assurance, support an explicit threshold policy—for example, 2-of-3 approvals from independently registered passkeys, or 2-of-3 across passkeys and a linked wallet. Store the policy and authenticator references, but no recovery names or free-form notes. Define whether adding/removing an authenticator requires the threshold, require the existing threshold to change the threshold itself, expire recovery approvals quickly, and consume each approval exactly once. Wallet approvals must specify whether EOA recovery or EIP-1271 contract-wallet recovery is supported.
5. Present recovery consequences clearly: losing all passkeys and the linked wallet may mean permanent account loss.

Do not quietly introduce recovery codes, admin resets, or “support verification” while claiming that the system only uses passkeys and Web3 authentication. Those can be separate, explicitly named products or capabilities, but they are additional trust mechanisms.

## Threat model

Prioritize these threats:

- **Phishing:** WebAuthn RP/origin binding and strict SIWE domain/URI validation.
- **Replay:** one-time WebAuthn challenges and SIWE nonces; consume them atomically.
- **Cross-chain confusion:** allow-list chain IDs and bind them into the SIWE message and verifier policy.
- **Token theft:** secure cookies, short TTLs, rotation, revocation, CSP/XSS defenses; DPoP later.
- **Account enumeration:** uniform responses and timing for unknown credential/address cases.
- **Wallet linkability:** no ENS enrichment, no automatic method linking, clear consent before linking.
- **EIP-1271 state changes:** decide whether contract-wallet verification is checked only at login or periodically during a session; define behavior for RPC disagreement, chain reorganization, contract upgrades, and revoked wallet authorization.
- **Database compromise:** no directly collected profile PII, no private keys, hashed refresh tokens, encrypted/permissioned database backups, documented deletion.
- **Log leakage:** never log IPs, user agents, request bodies, SIWE messages, signatures, wallet addresses, credential IDs, or bearer tokens.
- **Abuse/Sybil attacks:** do not use token balances or transaction history as a default identity signal. Rate-limit transiently at the edge or with short-lived, non-exported buckets; accept that a no-PII service has less durable abuse memory.
- **Operational correlation:** scrub reverse-proxy, load-balancer, access, crash, and analytics logs as well as application tables. A database-only scrub is not enough.

## Target schema shape

A minimal successor schema could contain:

- `accounts(id, created_at, updated_at, deleted_at)`
- `passkey_credentials(id, account_id, credential_id, public_key, counter, backup_state, created_at, last_used_at)`
- `wallet_credentials(id, account_id, namespace, address, chain_id, created_at, last_used_at)`
- `sessions(id, account_id, refresh_token_hash, created_at, expires_at, revoked_at)`
- `auth_challenges(id, type, opaque_subject, challenge_hash, expires_at, consumed_at)`

Avoid free-form names, emails, profile fields, IPs, device info, OAuth tables, contact labels, and identity labels. Keep challenge state ephemeral where practical. If compatibility requires migrations rather than a clean schema replacement, add a privacy migration and verify that old columns and backups are handled according to the deployment's deletion policy.

## MVP roadmap

### Phase 0 — contract and policy

- Adopt the precise “no directly collected PII” promise.
- Define whether method linking is enabled by default.
- Define retention, deletion, backups, operator access, and log policy.
- Write an explicit data inventory and threat model.

### Phase 1 — remove non-conforming surfaces

- Remove OAuth routes, handlers, configuration, models, and migration from the active schema through a forward migration; do not simply delete historical migration files. Plan controlled cleanup of existing OAuth rows, old session columns, exports, and backups.
- Remove session IP/device fields.
- Remove audit `client_ip` and any request-body/token logging.
- Remove or constrain user-entered names and labels.
- Remove ENS and profile enrichment.

### Phase 2 — harden both ceremonies

- Enforce WebAuthn opaque user handles, discoverable credentials, UV policy, and privacy-safe attestation.
- Enforce SIWE domain, URI, chain, nonce, issued-at, expiry, and atomic nonce consumption.
- Add EIP-1271 support or document EOA-only scope explicitly.
- Add uniform errors and rate limits without durable identity profiling.

### Phase 3 — sessions and recovery

- Move browser flows to secure opaque cookies where possible.
- Minimize JWT claims and enforce RFC 8725 validation.
- Add multi-passkey enrollment and explicit wallet linking/recovery.
- Add account deletion that covers database rows, logs, exports, and backups according to the operator's policy.

### Phase 4 — verification and release

- Add integration tests for complete passkey registration/authentication.
- Add EOA and EIP-1271 SIWE tests, replay tests, wrong-domain tests, wrong-chain tests, and nonce-race tests.
- Add a schema/privacy test that rejects forbidden columns and forbidden serialized fields.
- Add log tests proving IPs, user agents, signatures, wallet addresses, credential IDs, and request bodies are absent.
- Run dependency/security audits and document deployment-level log controls.

## Decisions to lock before coding

These are proposed MVP defaults rather than universal requirements:

- **Challenge retention:** 5-minute maximum lifetime; consume atomically and purge expired challenges continuously or at least on every challenge write.
- **Session retention:** 15-minute access-token lifetime, 30-day maximum refresh lifetime, revocation on logout, and scheduled deletion of expired/revoked session rows after 24 hours.
- **Audit retention:** do not persist application audit events by default. If an operator enables security events, retain only event type, coarse result, and opaque request ID for 24–48 hours, with deployment-level controls preventing IP/user-agent capture.
- **Rate-limit state:** keep in memory or in a short-lived edge store; expire buckets within 24 hours and never export raw network identifiers to the application database.
- **Backups:** document that database backups may contain pseudonymous identifiers and linkage records; encrypt them, restrict access, set an operator-defined expiry, and include them in account-deletion procedures.
- **EIP-1271:** defer smart-contract-wallet login to a separately tested capability unless the launch deployment has a reliable chain/RPC policy. If enabled at launch, verify at login against the requested chain, fail closed on RPC disagreement/timeouts, and do not silently treat a later chain reorganization or authorization change as irrelevant.
- **Threshold recovery:** require approvals to carry an account, policy version, authenticator reference, expiry, and unique operation ID; consume each approval once. Adding/removing authenticators and changing the threshold require the existing threshold, not one ordinary session.

## Decision summary

**Build it, but narrow the promise and the surface area.** Astral Key's core public-key authentication model is viable. The biggest architectural mistake would be to treat “no email” as equivalent to “no PII” or “anonymous.” The strongest product is a privacy-minimized, self-hosted authentication service whose only login ceremonies are passkey and SIWE, with explicit method linking, no enrichment, no persistent network/device identifiers, and recovery through additional authenticators rather than Web2 identity.
