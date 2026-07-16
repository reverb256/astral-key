#!/usr/bin/env bash
# build-push-mosaic.sh — build the Mosaic image and push to the
# in-cluster local registry (nexus:5000).
#
# IMPORTANT (root-cause fix): push with `--format docker`, NOT the
# default OCI format. Pushing OCI-format manifests to the plain-HTTP
# docker-distribution registry at nexus:5000 corrupts the manifest
# (registry returns HTTP 500 on manifest GET-by-digest). Docker-format
# manifests are what distribution registries handle reliably.
#
# The /etc/nixos kubernetes module (kubernetes/modules/haven.nix)
# pins the deployed image. After a successful push, roll out:
#   kubectl -n haven rollout restart deployment/haven
#
# Run on a host with podman + LAN reachability to nexus:5000 and
# internet (for the node:22-alpine base pull). 10.1.1.100 qualifies.
set -euo pipefail

REPO_DIR="${MOSAIC_REPO:-/home/j_kro/Projects/mosaic}"
REG="nexus:5000"
DATE_TAG="$(date -u +%Y%m%d)"
LATEST_IMG="${REG}/mosaic:latest"
VERSIONED_IMG="${REG}/mosaic:${DATE_TAG}-clean"
PLATFORM="${PLATFORM:-linux/amd64}"

cd "$REPO_DIR" || { echo "FATAL: cannot cd to $REPO_DIR"; exit 1; }

echo "==> repo:   $REPO_DIR"
echo "==> targets: $LATEST_IMG  +  $VERSIONED_IMG"
echo "==> platform: $PLATFORM"

# 1. Build with podman (daemonless). --format docker is REQUIRED for
#    the plain-HTTP nexus:5000 registry (OCI format corrupts manifest).
echo "==> [1/4] building image (docker format)..."
podman build \
  --platform "$PLATFORM" \
  --format docker \
  --label "org.opencontainers.image.source=https://github.com/reverb256/Mosaic" \
  --tag "$LATEST_IMG" \
  --file Dockerfile \
  .

# tag the same build with a dated, immutable version
podman tag "$LATEST_IMG" "$VERSIONED_IMG"

# 2. Push both tags. nexus:5000 is plain HTTP -> --tls-verify=false.
echo "==> [2/4] pushing $LATEST_IMG ..."
podman push --tls-verify=false --format docker "$LATEST_IMG" "$LATEST_IMG"
echo "==> [3/4] pushing $VERSIONED_IMG ..."
podman push --tls-verify=false --format docker "$VERSIONED_IMG" "$VERSIONED_IMG"

# 3. Verify the manifest resolves BY DIGEST (the real corruption check).
#    A 200 here means the registry stored a valid manifest; a 500 would
#    mean the push corrupted it.
echo "==> [4/4] verifying registry manifest by digest..."
DIGEST=$(curl -sS --max-time 15 -H "Accept: application/vnd.docker.distribution.manifest.v2+json" \
  "http://${REG}/v2/mosaic/manifests/latest" 2>/dev/null \
  | grep -oE '"digest":"sha256:[a-f0-9]{64}"' | head -1 | sed -E 's/.*"sha256:([a-f0-9]{64})".*/\1/')
if [ -z "$DIGEST" ]; then
  echo "WARN: could not read :latest digest from registry. Check push output above."
  exit 1
fi
HTTP=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 15 \
  -H "Accept: application/vnd.docker.distribution.manifest.v2+json" \
  "http://${REG}/v2/mosaic/manifests/sha256:${DIGEST}" || true)
if [ "$HTTP" = "200" ]; then
  echo "OK: $LATEST_IMG manifest valid (digest sha256:${DIGEST}, HTTP 200)."
else
  echo "ERROR: manifest GET-by-digest returned HTTP $HTTP (expected 200). Push corrupted the manifest."
  exit 1
fi

echo "DONE."
echo "  pinned image: $VERSIONED_IMG"
echo "  next: point haven.nix at $VERSIONED_IMG, commit, then:"
echo "    kubectl -n haven set image deployment/haven haven=${VERSIONED_IMG} && kubectl -n haven rollout restart deployment/haven"
