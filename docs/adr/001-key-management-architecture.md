# ADR-001: Key Management Architecture — Mnemonic-Derived HD Keys

**Status:** Accepted (2026-07-25)
**Context:** [docs/foundation.md](../foundation.md)

---

## Decision

Drop Vaultwarden as a required or recommended component for the Mosaic
Identity Foundation key store. Instead, adopt a **BIP-39 mnemonic phrase →
SLIP-10 hierarchical deterministic (HD) sub-keys** architecture, with age +
YubiKey as the encrypted backup mechanism.

## Background

The Mosaic Identity Foundation (MIS + bridges + auth sidecar) holds Ed25519
and ML-DSA-65 private keys on behalf of the user. These keys must survive
device loss, be portable across machines, and support selective disclosure
(one sub-key per protocol).

We initially evaluated Vaultwarden (self-hosted Bitwarden server, already
deployed in the homelab) as an encrypted key store. After investigation,
we concluded that Vaultwarden's client-side encryption model (master-password
derived vault key) makes it a poor fit for machine-to-machine key storage.

## Evaluated alternatives

| Approach | Recovery | Multi-device | Offline | Complexity |
|---|---|---|---|---|
| Random keys in SQLite (current) | ❌ DB loss = all keys lost | ❌ | ✅ | Minimal |
| Vaultwarden encrypted sync | ⚠️ Needs master password + API key | ✅ | ❌ Depends on server | High |
| BIP-39 mnemonic + HD derivation | ✅ Phrase = all keys recoverable | ✅ Same phrase = same keys | ✅ Derivation is local | Medium |
| Age-encrypted seed file | ✅ Age key decrypts seed | ✅ Copy file | ✅ | Low |

## Chosen architecture

```
┌─────────────────┐
│  24-word phrase  │  ← BIP-39 mnemonic (printed paper = single source of truth)
│  (BIP-39, EN)   │
└────────┬────────┘
         │ PBKDF2 ("mnemonic" + passphrase)
         ▼
┌─────────────────┐
│  512-bit master  │  ← never stored, never leaves process memory
│  seed            │
└────────┬────────┘
         │ SLIP-10 / BIP32-Ed25519
         ├────────────────────┬────────────────────┐
         ▼                    ▼                    ▼
   ┌────────────┐       ┌────────────┐       ┌────────────┐
   │ Ed25519    │       │ secp256k1  │       │ ML-DSA-65  │
   │ sub-key    │       │ (atproto)  │       │ (custom)   │
   │ protocol=* │       │ protocol=  │       │ protocol=  │
   │ path=...   │       │ atproto    │       │ pq         │
   └──────┬─────┘       └──────┬─────┘       └──────┬─────┘
          │ cached in          │ cached in          │ cached in
          ▼                    ▼                     ▼
   ╔══════════════════════════════════════════════════════╗
   ║              SQLite (fast cache)                      ║
   ║  Derived keys cached for sub-ms signing               ║
   ║  Bindings, rotation history                           ║
   ║  Re-built from mnemonic on loss                      ║
   ╚══════════════════════════════════════════════════════╝
```

### Layers

1. **Recovery source** (canonical): 24-word BIP-39 mnemonic on paper.
   Optionally age-encrypted for digital backup: `age -o mis-backup.age`
2. **Master seed**: PBKDF2(mnemonic, salt="mnemonic" + passphrase,
   2048 rounds) → 512 bits. Derived fresh on MIS start, held in memory
   only during key derivation, then zeroed.
3. **Sub-keys**: SLIP-10 (Ed25519) / BIP32-Ed25519 (Cardano variant) /
   custom derivation for ML-DSA. One key per protocol per relationship.
   `key_id` is the derivation path (e.g. `m/44'/mosaic'/atproto'/0`).
4. **SQLite cache**: Derived sub-keys cached for fast signing. If the DB
   is lost, re-derive from mnemonic. The DB is disposable.
5. **Optional YubiKey binding**: The mnemonic alone derives the master
   seed. An optional YubiKey can serve as a hardware salt — the mnemonic
   alone is useless without the YubiKey (or vice versa), similar to BIP-38.

### What changes in the code

| Component | Change |
|---|---|
| `crypto.rs` | Add `generate_mnemonic()`, `mnemonic_to_seed()`, `derive_subkey()`, `derive_mldsa_subkey()`. |
| `api.rs` | `POST /keys/generate` accepts optional `?mnemonic=&passphrase=`. If omitted, random key (current). If provided, deterministic derivation. |
| `storage.rs` | Store `derivation_path` alongside `key_id`. Sub-keys are cached; on DB rebuild, re-derive all from mnemonic. |
| Key import | `POST /keys/import` accepts mnemonic instead of raw private key hex. |

### What stays the same

- Bridge daemons (unchanged — they talk to MIS via `/sign`, `/verify`,
  `/bindings/claim`, `/resolve`)
- Auth sidecar (unchanged — issues JIT tokens using the Ed25519 JIT key)
- SQLite schema (adds `derivation_path` column, keeps everything else)
- All existing random keys continue to work (legacy path)

## Consequences

- **Positive**: Keys survive device loss. The identity is the mnemonic,
  not the database.
- **Positive**: Multi-device. Same mnemonic on two MIS instances = same
  identity on both.
- **Positive**: Selective disclosure. Compromise of the atproto sub-key
  doesn't compromise the Matrix or Nostr sub-key.
- **Positive**: No Vaultwarden dependency. One less service to deploy and
  secure.
- **Positive**: Age + YubiKey is already used for SOPS in the homelab.
  Same tooling.
- **Negative**: Additional ~200 lines of crypto code. BIP-39 wordlist
  (2048 English words) must be bundled.
- **Negative**: ML-DSA HD derivation has no standard. We'll use
  `SHA512("mosaic-mldsa-65" || seed)` and truncate to 4032 bytes. Not
  interoperable with any other wallet.
- **Negative**: Mnemonic management burden. User must securely store 24
  words. Tradeoff accepted — this is the baseline for self-sovereign
  identity.

## Related

- [ADR template](https://github.com/joelparkerhenderson/architecture-decision-record)
- [BIP-39](https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki)
- [SLIP-10](https://github.com/satoshilabs/slips/blob/master/slip-0010.md)
- [BIP32-Ed25519](https://input-output-hk.github.io/adrestia/static/Ed25519_BIP.pdf)
- [docs/foundation.md](../foundation.md)
