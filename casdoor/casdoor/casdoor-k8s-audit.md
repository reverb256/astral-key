# Casdoor Kubernetes Security Audit

**Date**: 2026-05-03
**Namespace**: `auth`
**Cluster**: CachyOS K3s (local)
**Managed by**: easykubenix
**Auditor**: Claude Code automated assessment

---

## Executive Summary

Casdoor (v3.49.0) is deployed as an identity provider in the `auth` namespace alongside a PostgreSQL 18.3 database. **The deployment is currently non-functional** — the pod is stuck in `Unknown` state due to a missing ConfigMap reference. Beyond this blocking issue, the audit found **3 critical, 3 high, 3 medium, and 2 low severity findings** covering secret leakage, missing container hardening, network exposure, and availability gaps.

---

## Current State

| Component | Resource | Status |
|---|---|---|
| Casdoor | Deployment (1 replica) | **Broken** — pod `Unknown`, ConfigMap mount failing |
| PostgreSQL | StatefulSet (1 replica) | Running |
| Casdoor Service | NodePort 32106 | Exposed, no TLS |
| Postgres Service | ClusterIP 5432 | Internal only |
| Network Policies | 3 policies | Present (ingress/egress for casdoor + postgres) |
| Ingress | None | No TLS termination |
| oauth2-proxy | Secrets present | Separate component in same namespace |

---

## Findings

### CRITICAL

#### C1. Casdoor Pod is Broken — Missing ConfigMap Reference
- **Resource**: Pod `casdoor-78dfdd6698-wh48t`
- **Symptom**: Pod stuck in `Unknown` state for 5+ hours
- **Root cause**: Deployment volume references ConfigMap `casdoor-config-new`, but only `casdoor-config` exists
- **Evidence**: `MountVolume.SetUp failed for volume "config" : configmap "casdoor-config-new" not found` (repeated 149+ times)
- **Impact**: Casdoor is completely down. No authentication is possible for dependent services.
- **Fix**: Update the deployment's volume definition to reference `casdoor-config` instead of `casdoor-config-new`, or create the missing ConfigMap.

#### C2. Database Password Leaked in Annotation
- **Resource**: Secret `casdoor-postgres-secret`
- **Password**: `casdoor_password_12345` (extremely weak)
- **How leaked**: The secret was originally applied using `stringData`, which caused Kubernetes to store the plaintext password in the `kubectl.kubernetes.io/last-applied-configuration` annotation. This annotation is readable by anyone with `get` access to the secret.
- **Evidence**: Base64 `Y2FzZG9vcl9wYXNzd29yZF8xMjM0NQ==` → `casdoor_password_12345`
- **Impact**: Anyone with read access to the secret object (not just the data) can see the plaintext password.
- **Fix**: 1) Rotate the password to something strong (e.g., 32+ random chars). 2) Re-apply the secret using `data` (pre-encoded base64) instead of `stringData` to avoid annotation leakage. 3) Delete the old secret object to remove the annotation history.

#### C3. oauth2-proxy Secrets Also Leaked via Annotations
- **Resource**: Secret `oauth2-proxy-secrets`
- **Fields exposed**: `client-secret` and `cookie-secret` in `last-applied-configuration`
- **Impact**: OAuth2 client credentials and session cookie signing key are recoverable from annotation metadata.
- **Fix**: Same as C2 — rotate credentials, re-apply with `data` field only.

---

### HIGH

#### H1. NodePort Exposes Casdoor Without TLS
- **Resource**: Service `casdoor` (type: NodePort, port 32106)
- **Issue**: Casdoor is accessible via HTTP on port 32106 on every cluster node. No TLS termination exists (no Ingress resource).
- **Impact**: All authentication traffic (credentials, tokens, cookies) transmitted in plaintext. Any network-adjacent attacker can intercept credentials.
- **Config inconsistency**: `app.conf` sets `origin = https://auth.lan`, but there's no HTTPS endpoint.
- **Fix**: Replace NodePort with an Ingress resource backed by a TLS certificate (cert-manager + Let's Encrypt, or self-signed for LAN).

#### H2. No Container-Level Security Context on Casdoor
- **Resource**: Deployment `casdoor`, container `casdoor`
- **Issue**: The Casdoor container has zero security hardening — no `runAsNonRoot`, no `readOnlyRootFilesystem`, no `capabilities` drop, no `runAsUser`.
- **Contrast**: The Postgres StatefulSet properly sets `runAsNonRoot: true`, `runAsUser: 999`, `runAsGroup: 999`, `fsGroup: 999`.
- **Impact**: If Casdoor is compromised, the attacker runs as root inside the container with full capabilities.
- **Fix**: Add to the Casdoor container:
  ```yaml
  securityContext:
    runAsNonRoot: true
    runAsUser: 1000
    runAsGroup: 1000
    readOnlyRootFilesystem: true
    allowPrivilegeEscalation: false
    capabilities:
      drop: ["ALL"]
  ```
  Note: `readOnlyRootFilesystem` may require an `emptyDir` volume for `/tmp` if Casdoor writes temp files.

#### H3. Single Replica with No Availability Guarantees
- **Resource**: Deployment `casdoor` (replicas: 1), StatefulSet `casdoor-postgres` (replicas: 1)
- **Issues**:
  - No PodDisruptionBudget (PDB) — node maintenance can take Casdoor offline
  - No HorizontalPodAutoscaler (HPA)
  - Node pinned to `zephyr` via `nodeSelector` — single point of failure
  - PostgreSQL uses `local-path` storage — data is bound to one node
- **Impact**: Any node failure, maintenance, or pod eviction takes the entire auth system offline.
- **Fix**: For production, consider multiple replicas with anti-affinity rules, a PDB (`minAvailable: 1`), and for Postgres either a managed database service or a replicated setup (e.g., Patroni).

---

### MEDIUM

#### M1. Egress Network Policy Blocks External HTTPS
- **Resource**: NetworkPolicy `casdoor-egress`
- **Issue**: Egress only allows DNS (port 53) and PostgreSQL (port 5432). No HTTPS (port 443) egress is permitted.
- **Impact**: Casdoor cannot communicate with external identity providers (Google, GitHub, LDAP, SAML IdPs). Features requiring outbound HTTP/S calls will silently fail.
- **Fix**: Add an egress rule for port 443/TCP to destinations Casdoor needs to reach (specific IdP endpoints preferred over `0.0.0.0/0`).

#### M2. Overly Permissive Ingress Network Policy
- **Resource**: NetworkPolicy `casdoor-ingress`
- **Issue**: Allows ingress from:
  - `10.244.0.0/16` — entire pod CIDR (all pods in cluster)
  - `10.1.1.0/24` — entire LAN subnet
- **Impact**: Any pod in the cluster and any device on the LAN can reach Casdoor's auth endpoint.
- **Fix**: Restrict to specific pod selectors (e.g., `oauth2-proxy` in `auth` namespace, ingress controller namespace) and specific LAN IPs.

#### M3. PostgreSQL Connection Uses `sslmode=disable`
- **Resource**: ConfigMap `casdoor-config`, key `app.conf.template`
- **Issue**: `dataSourceName` contains `sslmode=disable`
- **Impact**: Database credentials and query data transmitted in plaintext between Casdoor and Postgres. Within the cluster pod network this is lower risk, but still violates defense-in-depth.
- **Fix**: Enable TLS on Postgres and set `sslmode=require` or `sslmode=verify-full`.

---

### LOW

#### L1. `imagePullPolicy: IfNotPresent` on Tagged Images
- **Resources**: Both Casdoor and Postgres containers
- **Issue**: Using `IfNotPresent` means if the same tag is pushed with different content (supply chain concern), the node won't pull the updated image.
- **Fix**: Use `Always` for mutable tags, or pin to SHA256 digests for immutability.

#### L2. Aggressive Revision History Limit
- **Resource**: Deployment `casdoor`, `revisionHistoryLimit: 2`
- **Issue**: Only 2 old ReplicaSets retained. Limits rollback depth.
- **Fix**: Increase to at least 5–10 for operational flexibility.

---

### INFORMATIONAL

#### I1. Service Account Permissions — Good
- The `casdoor` service account has only default API discovery permissions. No custom RoleBindings or ClusterRoleBindings exist. This is well-scoped.

#### I2. Probes — Good
- All three probe types are configured: startup, liveness, and readiness with appropriate thresholds and timing.

#### I3. Resource Limits — Adequate
- Casdoor: 500m CPU / 1Gi RAM (limits), 100m / 256Mi (requests)
- Postgres: 500m / 512Mi (limits), 250m / 256Mi (requests)

#### I4. Postgres Security Context — Good
- Postgres StatefulSet properly runs as non-root with `runAsUser: 999`, `fsGroup: 999`, and `seccompProfile: RuntimeDefault`.

---

## Priority Remediation Order

| Priority | Finding | Effort | Risk Reduced |
|---|---|---|---|
| 1 | **C1**: Fix ConfigMap reference | Low (5 min) | Service restored |
| 2 | **H1**: Add Ingress + TLS | Medium | Credential interception |
| 3 | **C2+C3**: Rotate all leaked secrets | Medium | Credential theft |
| 4 | **H2**: Add container security context | Low | Container escape |
| 5 | **M1**: Allow HTTPS egress | Low | External IdP integration |
| 6 | **M2**: Tighten ingress network policy | Low | Lateral movement |
| 7 | **H3**: Add PDB + multi-replica | Medium | Availability |
| 8 | **M3**: Enable Postgres TLS | Medium | DB credential interception |

---

## Immediate Fix Commands

### Fix the broken pod (C1):
```bash
# Option A: Update deployment to reference existing configmap
kubectl patch deployment casdoor -n auth --type=json \
  -p='[{"op":"replace","path":"/spec/template/spec/volumes/0/configMap/name","value":"casdoor-config"}]'
```

### Rotate the database password (C2):
```bash
# Generate a strong password
NEW_PASS=$(openssl rand -base64 32)

# Recreate the secret using 'data' (not 'stringData') to avoid annotation leakage
kubectl delete secret casdoor-postgres-secret -n auth
kubectl create secret generic casdoor-postgres-secret \
  -n auth \
  --from-literal=POSTGRES_PASSWORD="$NEW_PASS"

# Then update the password in PostgreSQL itself
kubectl exec -n auth casdoor-postgres-0 -- \
  psql -U casdoor -c "ALTER USER casdoor PASSWORD '$NEW_PASS';"

# Restart Casdoor to pick up the new secret
kubectl rollout restart deployment/casdoor -n auth
```
