#!/usr/bin/env bash
# Static validation of the astral-key network policies.
#
# Renders the chart the way ArgoCD does (master -> charts/astral-key) and
# asserts the breach-containment properties verified live on 2026-09-25:
# a breached astral-key pod must not reach nodes, the LAN, or the tailnet,
# while quill's auth calls and RPC internet egress keep working.
set -euo pipefail

cd "$(dirname "$0")/.."
CHART=charts/astral-key
RENDER=$(mktemp)
NETPOLS=$(mktemp)
trap 'rm -f "$RENDER" "$NETPOLS"' EXIT

HELM=${HELM:-helm}
command -v "$HELM" >/dev/null || { echo "helm not found" >&2; exit 2; }

fail=0
check() { # check <description> <expected-count> <pattern>
  local desc=$1 want=$2 pat=$3 got
  got=$(grep -cF -- "$pat" "$NETPOLS" || true)
  if [ "$got" = "$want" ]; then
    echo "ok   $desc"
  else
    echo "FAIL $desc (expected $want x '$pat', found $got)" >&2
    fail=1
  fi
}
present() { check "$1" 1 "$2"; }

echo "== render =="
"$HELM" lint "$CHART" >/dev/null
"$HELM" template astral-key "$CHART" --namespace astral-key > "$RENDER"
# scope all checks to NetworkPolicy documents (services/deployments also have ports)
awk 'BEGIN{RS="---"} /kind: NetworkPolicy/{print "---" $0}' "$RENDER" > "$NETPOLS"
echo "ok   chart renders"

echo "== policy set =="
check "exactly 4 NetworkPolicies" 4 "kind: NetworkPolicy"
present "default-deny exists" "name: astral-key-default-deny"
present "dns allow exists" "name: astral-key-allow-dns"
present "quill ingress allow exists" "name: astral-key-allow-ingress-quill"
present "internet egress allow exists" "name: astral-key-allow-internet-egress"

echo "== internet egress excludes every private range =="
present "excludes 10/8" "10.0.0.0/8"
present "excludes 172.16/12" "172.16.0.0/12"
present "excludes 192.168/16" "192.168.0.0/16"
present "excludes 100.64/10 (tailnet)" "100.64.0.0/10"
present "excludes 169.254/16" "169.254.0.0/16"
present "excludes 127/8" "127.0.0.0/8"
check "0.0.0.0/0 appears exactly once (internet egress only)" 1 "cidr: 0.0.0.0/0"

echo "== ingress allowances: quill namespace + node subnet only =="
check "node subnet appears exactly once (pre-DNAT svc flows)" 1 "cidr: 10.1.1.0/24"
present "quill namespace allowed" "kubernetes.io/metadata.name: maplespike"
present "auth port 8080" "port: 8080"

echo "== dns =="
present "dns via kube-dns only" "k8s-app: kube-dns"

if [ "$fail" -ne 0 ]; then
  echo "NETPOL VALIDATION FAILED" >&2
  exit 1
fi
echo "all network policy assertions passed"
