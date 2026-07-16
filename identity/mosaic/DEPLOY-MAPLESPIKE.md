---
last-reviewed: 2026-07-14
status: active
target: mosaic.maplespike.ca
operator-policy: NEVER deploy to zephyr; only the other nodes (per user directive 2026-07-14)
---

# Mosaic — Deploy on the local k3s cluster

> **Goal:** Mosaic (forked Haven, AGPL-3.0, Express + Socket.io + WebRTC) exposed
> at `https://mosaic.maplespike.ca` for human + invitee use, persistent on our
> own infra. Image goes via `nexus:5000` (the canonical MapleSpike registry
> per `quill/docs/decisions/0004-split-hostname-routing.md`); traffic goes
> through the existing `maplespike-k8s` Cloudflare Tunnel (UUID
> `66d76492-036b-4177-8fac-8682e0eda37e`).

## Why this path (not CF Pages, not Workers, not Render/Railway/Fly)

`mosaic/server.js` is a long-lived Node.js process that:

| Need | Why CF Pages doesn't fit | Why CF Workers/Containers beta doesn't fit (yet) | Why this path fits |
|---|---|---|---|
| Persistent `/data` volume (`haven.db` SQLite + uploads + certs + `.env`) | Pages is static; no writable filesystem | Workers Container beta: persistent disk not first-class | StatefulSet + PVC (the standard answer in k3s) |
| WebSocket connections (Socket.io, real-time presence) | Pages has no persistent sockets | CF Container supports WS but cold-start latency is bad for chat | k3s Service with cluster-internal hostname, never cold-started |
| Self-signed cert generation + `apple-app-site-association` + per-install custom .env (admin sets `SERVER_NAME`, `JWT_SECRET`, VAPID keys) | n/a | /data needs the certs and .env to live | k3s volume mount → image `docker-entrypoint.sh` reads/writes through |
| Admin-tier tools (upload, sticker management, contact form, ICE/STUN config, group limits, GIPHY proxy) | n/a | n/a | All run server-side; no CF Worker implementations exist |

CF Pages for the brand portal works because the portal is pure static. Mosaic
isn't, so we ship it to the cluster. Operator picked K8s cluster path over
Render/Railway/Fly on 2026-07-14: keeping all of `*.maplespike.ca` traffic on
Canadian infrastructure (the same reason we picked CF Tunnel over a public
ingress in the first place — `quill/docs/decisions/0004-split-hostname-routing.md`).

## Why not zephyr

Per operator policy (`we NEVER deploy to zephyr, only the other nodes`),
the StatefulSet pod has `nodeAffinity` that excludes the `zephyr` host. Other
k3s nodes (`nexus`, `sentry`, `forge`) are admissible. See Step 4 for the
manifest snippet.

---

## Step 1 — Build + push image (one-time, then on each release)

```bash
cd mosaic
docker build -t nexus:5000/mosaic:dev .
docker push nexus:5000/mosaic:dev
```

The Dockerfile already exists at `mosaic/Dockerfile`; it produces a
`node:22-alpine` image with `openssl` + `su-exec` for cert generation and
volume permission fixups. The image exposes `:3000` (TURN/STUN also `:3001`
but Mosaic uses `3000` for both chat+API). It expects `/data` mounted (the
StatefulSet's `volumeClaimTemplates` will provide it).

### Step 1.5 — Create the `nexus-registry-secret` imagePullSecret (one-time)

The `nexus:5000` registry is private. Without this Secret, pods fail
`ImagePullBackOff` within ~30s of being scheduled:

```bash
kubectl -n maplespike create secret docker-registry nexus-registry-secret \
  --docker-server=nexus:5000 \
  --docker-username='maplespike' \
  --docker-password='<REDACTED — see ~/.docker/config.json on nexus host>' \
  --docker-email='ops@maplespike.ca'
```

The StatefulSet below references this via
`spec.template.spec.imagePullSecrets[].name = nexus-registry-secret`. If
your cluster pulls from a public registry (e.g. `ghcr.io`), swap the
image reference to a public one AND drop the
`imagePullSecrets:` array from the StatefulSet.


> If you don't have `nexus.local` DNS or your `nexus` host doesn't run a
> registry on `:5000`, swap the registry prefix for whatever the cluster
> actually pulls from (`nexus:5000/quill-portal:dev` works the same way per
> `quill/scripts/deploy.sh`).

## Step 2 — Namespace + PVC (NFS / local-path / whatever your StorageClass is)

If you're using the `maplespike` namespace already (e.g. you have `quill-api`
running there), drop Mosaic in there too to share the cloudflared tunnel
configmap. Otherwise:

```bash
kubectl create ns mosaic   # or reuse the maplespike ns
```

Then the PVC:

```yaml
# mosaic/manifests/00-pvc.yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: mosaic-data
  namespace: maplespike
spec:
  accessModes: ["ReadWriteOnce"]
  storageClassName: local-path      # k3s default; swap to nfs/rook/etc if you have one
  resources:
    requests:
      storage: 10Gi                 # start at 10Gi; resize via `kubectl edit pvc mosaic-data -n maplespike`
```

```bash
kubectl apply -f mosaic/manifests/00-pvc.yaml
```

## Step 3 — Service (ClusterIP, internal-only)

```yaml
# mosaic/manifests/01-svc.yaml
apiVersion: v1
kind: Service
metadata:
  name: mosaic-svc
  namespace: maplespike
  labels:
    app: mosaic
spec:
  type: ClusterIP
  selector:
    app: mosaic
  ports:
    - port: 3000
      targetPort: 3000
      name: http
      protocol: TCP
    - port: 3001                  # ⚠️ informational only — cloudflared does NOT proxy UDP. Mosaic's
                                  # STUN/TURN on UDP/3001 is unreachable from the public internet via
                                  # this tunnel. If voice calls from off-LAN clients need STUN, either
                                  # (a) set up a separate UDP-capable tunnel
                                  # (`cloudflared tunnel --protocol quic`) for `turn.maplespike.ca`,
                                  # (b) point Mosaic at a public STUN server
                                  # (`STUN_URLS=stun:stun.cloudflare.com:3478,...` env var), or
                                  # (c) run a TURN relay with TCP fallback. For local-LAN-only calls,
                                  # no action needed — Mosaic's STUN bypass via direct ICE works.
      targetPort: 3001
      name: turn
      protocol: UDP
```

## Step 4 — StatefulSet (with `nodeAffinity` excluding `zephyr`)

```yaml
# mosaic/manifests/02-statefulset.yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: mosaic
  namespace: maplespike
  labels:
    app: mosaic
spec:
  serviceName: mosaic-svc
  replicas: 1                       # Single instance — Mosaic uses local SQLite; HA needs a different DB
  selector:
    matchLabels:
      app: mosaic
  template:
    metadata:
      labels:
        app: mosaic
    spec:
      # Per operator policy (2026-07-14): "we NEVER deploy to zephyr".
      # The NotIn operator is the most defensive — if zephyr reappears with
      # a different label assignment, this still keeps Mosaic off it.
      imagePullSecrets:
        - name: nexus-registry-secret       # created in Step 1.5; drop line if registry is public
      affinity:
        nodeAffinity:
          requiredDuringSchedulingIgnoredDuringExecution:
            nodeSelectorTerms:
              - matchExpressions:
                  - key: kubernetes.io/hostname
                    operator: NotIn
                    values: ["zephyr"]
      containers:
        - name: mosaic
          image: nexus:5000/mosaic:dev
          imagePullPolicy: IfNotPresent     # stable tag vs :dev / :latest — switch to Always if you tag with `:dev` and want each restart to grab the latest build
          ports:
            - containerPort: 3000
            - containerPort: 3001
              protocol: UDP
          env:
            - name: PORT
              value: "3000"
            - name: HOST
              value: "0.0.0.0"
            - name: NODE_ENV
              value: "production"
            - name: HAVEN_DATA_DIR
              value: "/data"
            # FORCE_HTTP=true so Mosaic's docker-entrypoint does NOT
            # self-generate a certificate — Cloudflare Tunnel terminates
            # TLS at the edge and forwards plain HTTP to the pod. Saving
            # us from cert-rotation drama every 90 days.
            - name: FORCE_HTTP
              value: "true"
          volumeMounts:
            - name: data
              mountPath: /data
          readinessProbe:
            httpGet:
              path: /api/health
              port: 3000
            initialDelaySeconds: 10
            periodSeconds: 30
            timeoutSeconds: 5
            failureThreshold: 3
          livenessProbe:
            httpGet:
              path: /api/health
              port: 3000
            initialDelaySeconds: 30
            periodSeconds: 60
            timeoutSeconds: 5
            failureThreshold: 6
          resources:
            requests:
              cpu: "200m"
              memory: "512Mi"
            limits:
              cpu: "1000m"
              memory: "2Gi"
      volumes:
        - name: data
          persistentVolumeClaim:
            claimName: mosaic-data
```

```bash
kubectl apply -f mosaic/manifests/02-statefulset.yaml
kubectl rollout status statefulset/mosaic -n maplespike --timeout=120s
```

The StatefulSet (not Deployment) gives us a stable `mosaic-0.mosaic-svc`
hostname inside the namespace, which makes the cloudflared ingress simpler —
point it at `http://mosaic-0.mosaic-svc.maplespike:3000` (or just
`http://mosaic-svc:3000` thanks to the Service).

## Step 5 — Patch `cloudflared-config` ConfigMap

Add the `mosaic.maplespike.ca` row above the `http_status:404` catch-all.
The current ConfigMap lives in the `maplespike` namespace.

```bash
kubectl -n maplespike get cm cloudflared-config -o yaml > /tmp/cloudflared-config.bak.yaml
# Edit: insert the mosaic row above the catch-all.
kubectl -n maplespike edit cm cloudflared-config
```

The new row (inserted before the `service: http_status:404` line):

```yaml
  - hostname: mosaic.maplespike.ca
    service: http://mosaic-svc:3000
```

Then restart `cloudflared` so it loads the new ingress:

```bash
kubectl -n maplespike rollout restart deploy cloudflared
kubectl -n maplespike rollout status deploy/cloudflared --timeout=60s
```

## Step 6 — DNS CNAME

Re-use the one-liner from
[`quill/DEPLOY-CHECKLIST.md` Step 1d](./../quill/DEPLOY-CHECKLIST.md#1d-one-liner-to-wire-dns-run-from-quill-so-the-cloudflare-sdk-is-on-disk) — just append `'mosaic'` to the `targets` array:

```bash
cd /home/j_kro/Projects/quill && cat > /tmp/wire-dns.mts <<'TSX'
import { Cloudflare } from 'cloudflare'; import fs,os,path from 'node:fs';
(async () => {
  const t = fs.readFileSync(path.join(os.homedir(),'.config/.wrangler/config/default.toml'),'utf8')
    .match(/oauth_token\s*=\s*["']([^"']+)["']/i)![1];
  const cf = new Cloudflare({ oauthToken: t });
  const zone = (await cf.zones.list({ name: 'maplespike.ca' })).result[0];
  const tgt = `${process.env.TUNNEL}.cfargotunnel.com`;
  const ex = (await cf.dns.records.list({ zone_id: zone.id, type: 'CNAME', name: 'mosaic.maplespike.ca' })).result;
  if (ex.length > 0) {
    await cf.dns.records.update(ex[0].id, { zone_id: zone.id, type: 'CNAME', name: ex[0].name, content: tgt, proxied: true });
    console.log(`✓ updated ${ex[0].name} → ${tgt}`);
  } else {
    await cf.dns.records.create({ zone_id: zone.id, type: 'CNAME', name: 'mosaic.maplespike.ca', content: tgt, proxied: true });
    console.log(`✓ created mosaic.maplespike.ca → ${tgt}`);
  }
})();
TSX
TUNNEL=66d76492-036b-4177-8fac-8682e0eda37e npx tsx /tmp/wire-dns.mts
sleep 60   # cert provisioning
```

> If the wrangler OAuth token doesn't have `zone:dns:edit`, fall back to the
> Dashboard route documented in the DEPLOY-CHECKLIST.

## Step 7 — Verify

End-to-end probes (must all pass before declaring victory):

```bash
# Cluster-side: pod is up, health endpoint answers
kubectl -n maplespike get pods -l app=mosaic
kubectl -n maplespike exec -it mosaic-0 -- curl -s http://localhost:3000/api/health | jq .
# → { "status": "online", "name": "...", "icon": null, "fingerprint": null }

# Tunnel-side: cloudflared picked up the new ingress
kubectl -n maplespike logs deploy/cloudflared --tail=20 | grep -i mosaic
# → expect "Updated to no longer route through cloudflared" or similar for the new hostname

# Public-side: TLS works through Cloudflare
curl -sS -o /dev/null -w 'mosaic → HTTP %{http_code}\n' https://mosaic.maplespike.ca/
curl -sS -o /dev/null -w 'mosaic health → HTTP %{http_code}\n' https://mosaic.maplespike.ca/api/health
# → both 200

# Browser smoke (manual): open https://mosaic.maplespike.ca/ in a browser,
# check the cert (Cloudflare-issued origin pull should match), and confirm
# the HTML loads without console errors.
```

If `202 response codes /api/health` works but the browser shows a
`helmet` CSP violation: `helmet()` in `mosaic/server.js` defaults to a
strict policy with no remote origins; Cloudflare Tunnel terminates TLS at
the edge and forwards plain HTTP, so `script-src` etc. won't allow
self-hosted admin-panel scripts unless relaxed. See Step 9.

## Step 8 — Time

Provisioning wall-clock from a cold start:

| Stage | Time |
|-------|------|
| Step 1 (build + push image) | ~5 min |
| Step 2 (PVC) | <10s |
| Step 3 (Service) | <5s |
| Step 4 (StatefulSet + image pull) | ~1 min |
| Step 5 (ConfigMap edit + cloudflared rollout) | ~30s |
| Step 6 (DNS + cert provisioning) | ~60s |
| Step 7 (E2E probes) | ~10s |
| **Total cold start** | **~8 min** |

Hot redeploys (rebuilt image, same version): ~30s.

## Step 9 — CSP / per-tenant relaxation

`mosaic/server.js` hardcodes a helmet CSP that allows:

- `script-src 'self' 'unsafe-eval' 'wasm-unsafe-eval' blob: https://www.youtube.com https://w.soundcloud.com https://unpkg.com`
- `connect-src 'self' ws: wss: https:`   ← already loose, so the Cloudflare Insights beacon, Sentry-style webhook relay, etc., work as expected.

If you later add a "register via OAuth provider" feature that reloads content
from a `cdn.maplespike.ca` origin, update both the CSP directive in
`server.js` AND the portal's `connect-src` to allow it.

## Step 10 — Backup + restore

Mosaic has its own backup endpoint (admin-only): `GET /api/admin/backup` —
streams a zip with channels/roles/users/settings/messages/uploads
(selectable). Schedule a CronJob in the cluster to pull backups daily.

### Step 10.1 — Create the `mosaic-admin-token` Secret (one-time)

The backup CronJob below reads the admin token from
`/var/run/secrets/mosaic/admin-token`. Obtain + create it BEFORE applying
the CronJob:

1. After Mosaic is up at `https://mosaic.maplespike.ca` (post Step 5–6),
   open the URL in a browser and **register the first account with username
   `admin`** (the `ADMIN_USERNAME` env var can be overridden via
   `SERVER_NAME` admin setting; default is `admin`). This becomes the
   server owner.
2. Sign in, navigate to **Settings → API Keys**, and create an admin-scoped
   API key (or extract the JWT token from `localStorage.maplespike_token`
   in DevTools — either works as the `Authorization: Bearer ...` header
   value).
3. Create the K8s Secret:
   ```bash
   TOKEN=$(echo -n '<paste the API key or JWT here>')
   kubectl -n maplespike create secret generic mosaic-admin-token \
     --from-literal=admin-token="$TOKEN"
   ```
   The CronJob below mounts this Secret at
   `/var/run/secrets/mosaic/admin-token` and reads it with `set -eu`
   (fail-fast on a missing token is intentional — better than running with
   an empty Bearer header which Mosaic would silently 401 every second
   until humans notice).

### Step 10.2 — Apply the daily backup CronJob

```yaml
# backups/mosaic-backup-cron.yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: mosaic-backup-daily
  namespace: maplespike
spec:
  schedule: "0 4 * * *"
  jobTemplate:
    spec:
      template:
        spec:
          containers:
            - name: backup
              image: curlimages/curl:8.10.1
              args:
                - /bin/sh
                - -c
                - |
                  set -eu
                  TOKEN="$(cat /var/run/secrets/mosaic/admin-token)"
                  mkdir -p /backup
                  curl -fsS -H "Authorization: Bearer ${TOKEN}" \
                       -o /backup/mosaic-$(date +%F).zip \
                       "http://mosaic-svc:3000/api/admin/backup?include=channels,users,settings,messages,files"
              volumeMounts:
                - name: backup
                  mountPath: /backup
                - name: mosaic-admin-token
                  mountPath: /var/run/secrets/mosaic
                  readOnly: true
          restartPolicy: OnFailure
          volumes:
            - name: backup
              persistentVolumeClaim:
                claimName: mosaic-backups
            - name: mosaic-admin-token
              secret:
                secretName: mosaic-admin-token    # K8s Secret to create first: `kubectl create secret generic mosaic-admin-token --from-literal=admin-token=... -n maplespike`. The token comes from Mosaic's `settings.server_settings` 'admin_password_reset_enabled' / initial admin registration; treat as sensitive.
```

(Pair with a separate `mosaic-backups` PVC; rotate 30d using a cron-driven
cleanup script.)

## Operator reference — what to check first when something breaks

| Symptom | Likely cause | First check |
|---------|--------------|-------------|
| Browser shows `530` from Cloudflare edge | `cloudflared` pod can't reach the Service | `kubectl -n maplespike logs deploy/cloudflared --tail=50` — look for `connection refused` or `dial tcp i/o timeout` |
| Browser shows `521` from Cloudflare edge | `cloudflared` pod is not running | `kubectl -n maplespike get pod -l app=cloudflared` |
| Browser shows `502` from Cloudflare edge | `cloudflared` reached the Service but the pod was killed / readiness probe failed | `kubectl -n maplespike get pod -l app=mosaic` |
| Browser shows the brand site or 404 | DNS is pointing at the tunnel but the tunnel ingress doesn't have the new row yet OR DNS isn't wired at all | `kubectl -n maplespike get cm cloudflared-config -o yaml | grep mosaic` and `dig +short mosaic.maplespike.ca CNAME` |
| `/api/health` returns 200 inside the pod but 530 outside | Cloudflare cert not yet provisioned for the new hostname | wait 60s after DNS attach; `curl -vI https://mosaic.maplespike.ca/` should show `server: cloudflare` and a Cloudflare-issued cert |
| Pod won't start, `CrashLoopBackOff` | `/data` mount permission or volume not yet bound | `kubectl describe pod mosaic-0 -n maplespike` — look at the `Events:` block |
| Pod scheduled onto `zephyr` (policy violation) | `nodeAffinity` not applied yet | `kubectl describe pod mosaic-0 -n maplespike | grep Node-Selectors` — should show `kubernetes.io/hostname NotIn [zephyr]` |

---

## Provenance

This recipe was established 2026-07-14 after operator decision ("K8s cluster
via dev-up.sh" + "K3s cluster + cloudflared tunnel" answers to two-step
clarification). Cross-references:

- `quill/DEPLOY-CHECKLIST.md` Step 7 — paired checklist entry
- `quill/ROADMAP.md` Known Drift — 2026-07-14 reconciliation entry
- `quill/docs/ROUTING.md` — the split-hostname routing rationale
- `quill/docs/decisions/0004-split-hostname-routing.md` — ADR for the canonical routing policy
