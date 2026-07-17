# Cluster Recovery Runbook

## Scenario: Nexus unreachable after cluster-reset

Nexus is pingable but SSH (port 22) is refused. All k3s services down.

**Root cause:** `k3s server --cluster-reset` on nexus reset etcd to single
member. Subsequent `systemctl start k3s` may have OOM-killed sshd or the
network stack was disrupted by the reset.

### Recovery steps (at nexus console / IPMI)

```bash
# 1. Check if k3s is consuming all resources
top
systemctl status k3s

# 2. If k3s is stuck, stop it
systemctl stop k3s

# 3. Restart SSH
systemctl start sshd

# 4. Verify SSH is running
ss -tlnp | grep :22

# 5. Load MIS + bridge images into containerd
docker save nexus:5000/mosaic-identity:v0.1.0 > /tmp/mis.tar
sudo ctr -n k8s.io images import /tmp/mis.tar

docker save nexus:5000/mosaic-bridges:v0.1.0 > /tmp/bridges.tar
sudo ctr -n k8s.io images rm nexus:5000/mosaic-bridges:v0.1.0 2>/dev/null
sudo ctr -n k8s.io images import /tmp/bridges.tar

# 6. Restart k3s
systemctl start k3s

# 7. After k3s is Ready, deploy MIS + bridges
kubectl -n orchestration delete pods --all
```

## Scenario: k3s unit has no ExecStart

Sentry's k3s unit file has no `ExecStart` line, causing
`Failed to start k3s.service: Unit k3s.service has a bad unit file setting.`

**Root cause:** Upstream nixpkgs `services.k3s` module bug when
`role = "server"` and `clusterInit = false`.

### Workaround (systemd drop-in)

```bash
sudo mkdir -p /run/systemd/system/k3s.service.d
sudo tee /run/systemd/system/k3s.service.d/override.conf << 'EOF'
[Service]
ExecStart=
ExecStart=/nix/store/<hash>-k3s-with-agent/bin/k3s server \
  --server https://10.1.1.100:6443 \
  --token-file /persistent/etc/k3s-cluster-token \
  --node-name=<hostname> \
  --node-ip=<ip> \
  --disable=traefik --disable=metrics-server \
  --cluster-cidr=10.42.0.0/16 \
  --service-cidr=10.43.0.0/16 \
  --cluster-dns=10.43.0.10 \
  --flannel-backend=none
EOF
sudo systemctl daemon-reload
sudo systemctl start k3s
```

## Scenario: Stale etcd data prevents node rejoin

After cluster-reset on nexus, forge and sentry have stale etcd data
that prevents them from rejoining.

### Fix

```bash
# On each rejoining node:
sudo rm -rf /var/lib/rancher/k3s/server/db/etcd
sudo systemctl restart k3s
```

The node will detect no local etcd data, connect to the `--server` address,
and rejoin the cluster fresh.

## Scenario: Bridge pods CrashLoopBackOff

All bridge pods show `Error` or `CrashLoopBackOff`.

### Debug

```bash
# Check logs
kubectl -n orchestration logs <pod-name>

# Common fixes:
# 1. Permission denied → check runAsUser matches container UID
kubectl -n orchestration get pod <pod> -o yaml | grep runAsUser

# 2. Wrong entrypoint → check BRIDGE_TYPE env var matches pod name
kubectl -n orchestration get pod <pod> -o yaml | grep BRIDGE_TYPE

# 3. Image not refreshed → reload into containerd
docker save nexus:5000/mosaic-bridges:v0.1.0 | sudo ctr -n k8s.io images import -
kubectl -n orchestration delete pods --all
```
