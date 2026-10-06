#!/usr/bin/env bash
# Guarantee that one digest-pinned image exists in this node's containerd store
# under the NAME kubelet resolves, or say exactly why it cannot.
#
# The convention (nexus:5000 was decommissioned 2026-09-17): images are built
# on a build host, imported into a node's store, and referenced as
# `<repo>@sha256:<digest>` with pullPolicy: Never. Nothing re-pulls them. If
# the store loses the image — ctr garbage collection, a rebuilt node, a
# restored-from-backup node — the pod sits at ErrImageNeverPull until a human
# rebuilds, re-imports, AND re-adds the name. That third step is the one that
# gets skipped, and skipping it is the expensive part: the layers can all be
# present and the pod still will not start, because CRI resolves repo-digest
# names only. A `:tag` alone is not enough.
#
# So this script always ends by asserting, and if necessary creating, the
# `<repo>@sha256:<digest>` name — not just the content.
#
# Usage:
#   scripts/ensure-image.sh                                   # every pin in charts/astral-key/values.yaml
#   scripts/ensure-image.sh <repo> <sha256:...>               # one image
#
# Options:
#   --build       build from source when the store lacks the content
#   --from-tar F  import F (a `docker save` tar) when the store lacks it
#
# Safe to re-run: prints what it did, changes nothing when already correct.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VALUES="$REPO_ROOT/charts/astral-key/values.yaml"
CTR=(sudo -n k3s ctr -n k8s.io)
BUILD=false
TAR=""

die() { echo "error: $*" >&2; exit 1; }

# The store is per node, so this only means anything on a k3s node.
[[ -S /run/k3s/containerd/containerd.sock || -f /etc/rancher/k3s/k3s.yaml ]] ||
  die "not a k3s node — the containerd store to fix is the one on this host"

store_named() { "${CTR[@]}" images ls -q 2>/dev/null | grep -Fx -- "$1" >/dev/null; }

# Content present under ANY name? Column 3 of `ctr images ls` is the content
# digest, so this finds the layers even when the @digest name is gone — the
# common case after a GC, and the one that makes recovery cheap.
#
# Returns non-zero when absent, which is a normal answer, not a failure — so it
# is called in a condition, never bare under `set -e`.
store_has_content() { "${CTR[@]}" images ls 2>/dev/null | awk -v d="$1" '$3 == d { found = 1 } END { exit !found }'; }

# Any name in the store that already points at this content, used as the source
# for naming. Not restricted to `<repo>:` tags: a `docker save` of a short name
# imports as a bare `sha256:<digest>` reference, and that is a perfectly good
# thing to name from.
store_ref_for_digest() { "${CTR[@]}" images ls 2>/dev/null | awk -v d="$1" '$3 == d { print $1; exit }' || true; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --build)    BUILD=true; shift ;;
    --from-tar) TAR="$2"; shift 2 ;;
    --help|-h)  sed -n '2,26p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*)         die "unknown option: $1" ;;
    *)          break ;;
  esac
done

# No args: take the pins straight from the chart, so this never drifts from
# what is actually deployed.
if [[ $# -eq 0 ]]; then
  if command -v yq >/dev/null 2>&1; then
    repo=$(yq -r '.image.repository' "$VALUES")
    digest=$(yq -r '.images["astral-key"].digest' "$VALUES")
  elif command -v python3 >/dev/null 2>&1; then
    # Minimal extractor: this values file is flat, so a full YAML parser is not
    # worth a dependency just to read two scalars.
    read -r repo digest < <(python3 - "$VALUES" <<'PY'
import re, sys
text = open(sys.argv[1]).read()
repo = re.search(r'^\s*repository:\s*"?([^"\n]+)"?', text, re.M).group(1)
block = re.search(r'^\s*astral-key:\s*$.*?digest:\s*"?([^"\n]+)"?', text, re.M | re.S).group(1)
print(repo, block)
PY
)
  else
    die "neither yq nor python3 found; pass <repo> <digest> explicitly"
  fi
  [[ -n "$digest" && "$digest" != "null" ]] || die "no digest pinned in $VALUES"
  set -- "$repo" "$digest"
fi

[[ $# -eq 2 ]] || die "usage: scripts/ensure-image.sh <repo> <sha256:...>"
repo=$1 digest=$2
[[ "$digest" =~ ^sha256:[0-9a-f]{64}$ ]] || die "malformed digest: $digest"
named="$repo@$digest"

if store_named "$named"; then
  echo "ok      $named already present and named"
  exit 0
fi

echo "missing $named"

# Content is already in the store under some other name: re-add the name only.
# No rebuild, no re-import.
if store_has_content "$digest"; then
  src=$(store_ref_for_digest "$digest")
  [[ -n "$src" ]] || die "content $digest is in the store but reports no name to copy"
  "${CTR[@]}" images tag --force "$src" "$named"
  store_named "$named" || die "tag reported success but $named is still absent"
  echo "named   $src -> $named"
  exit 0
fi

echo "absent  content $digest is not in this store at all"

if [[ -n "$TAR" ]]; then
  [[ -f "$TAR" ]] || die "--from-tar: no such file: $TAR"
  echo "import  $TAR"
  "${CTR[@]}" images import "$TAR" >/dev/null
elif [[ "$BUILD" == true ]]; then
  command -v docker >/dev/null || die "--build needs docker on this host"
  # Containerfile, not the root Dockerfile — that one builds mosaic-identity.
  echo "build   docker build -f Containerfile -t $repo:ensure ."
  docker build -f "$REPO_ROOT/Containerfile" -t "$repo:ensure" "$REPO_ROOT"
  TMP_TAR=$(mktemp -d)/ensure.tar
  docker save "$repo:ensure" -o "$TMP_TAR"
  "${CTR[@]}" images import "$TMP_TAR" >/dev/null
  rm -rf "$(dirname "$TMP_TAR")"
else
  die "content is gone and no source was given: pass --from-tar <docker save tar> or --build.
       Rebuilding does NOT recreate $digest — a fresh build gets a new digest, so
       --build alone cannot satisfy this pin. It is here for the case where you
       intend to re-pin afterwards."
fi

# Importing restores content but not the name. This is the step that is easy to
# forget and that CI, kubectl and `ctr images ls` will all report as green
# without it.
if store_has_content "$digest"; then
  src=$(store_ref_for_digest "$digest")
  [[ -n "$src" ]] || die "imported content matches but reports no name to copy"
  "${CTR[@]}" images tag --force "$src" "$named"
fi

store_named "$named" || die "still no $named — the imported content is a different digest than $digest"
echo "ok      $named present and named"