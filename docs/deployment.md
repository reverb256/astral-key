# Astral Key - Deployment Guide

## Overview

This guide covers deploying Astral Key in various environments.

## NixOS Deployment

### Using the NixOS Module

Add to your NixOS configuration:

```nix
{ config, pkgs, ... }:

{
  imports = [
    # Import the Astral Key module
    (builtins.fetchTarball {
      url = "https://github.com/reverb256/astral-key/archive/main.tar.gz";
    } + "/nix/nixos-module.nix")
  ];

  services.astral-key = {
    enable = true;
    
    # Server configuration
    host = "0.0.0.0";
    port = 8080;
    workers = 4;
    
    # Database
    database.url = "postgresql://astral:secret@localhost/astral_key";
    
    # Redis
    redis.url = "redis://localhost:6379";
    
    # Vaultwarden
    vaultwarden.url = "http://localhost:8000";
    vaultwarden.adminTokenFile = "/run/secrets/vaultwarden-admin-token";
    
    # FIDO2/WebAuthn
    fido2.rpId = "auth.example.com";
    fido2.origin = "https://auth.example.com";
    
    # JWT
    jwt.secretFile = "/run/secrets/jwt-secret";
    
    # Firewall
    openFirewall = true;
  };
  
  # Required services
  services.postgresql = {
    enable = true;
    ensureDatabases = [ "astral_key" ];
    ensureUsers = [{
      name = "astral";
      ensureDBOwnership = true;
    }];
  };
  
  services.redis.servers.astral-key = {
    enable = true;
    bind = "127.0.0.1";
  };
  
  # Secrets (using sops-nix or agenix)
  sops.secrets.jwt-secret = {
    owner = "astral-key";
    group = "astral-key";
  };
  
  sops.secrets.vaultwarden-admin-token = {
    owner = "astral-key";
    group = "astral-key";
  };
}
```

### Deploy with Colmena

```nix
# hive.nix
{
  meta.nixpkgs = import <nixpkgs> {};

  server = { config, pkgs, ... }: {
    deployment.targetHost = "server.example.com";
    
    imports = [ ./astral-key-module.nix ];
    
    services.astral-key = {
      enable = true;
      # ... configuration
    };
  };
}
```

```bash
colmena apply
```

## Container Deployment

### Docker

```bash
# Build container image
nix build .#container

# Load into Docker
docker load < result

# Run
docker run -d \
  --name astral-key \
  -p 8080:8080 \
  -e DATABASE_URL="postgresql://..." \
  -e REDIS_URL="redis://..." \
  -e VAULTWARDEN_URL="http://..." \
  astral-key:latest
```

### Docker Compose

```yaml
# docker-compose.yml
version: '3.8'

services:
  astral-key:
    image: astral-key:latest
    ports:
      - "8080:8080"
    environment:
      - DATABASE_URL=postgresql://astral:secret@postgres:5432/astral_key
      - REDIS_URL=redis://redis:6379
      - VAULTWARDEN_URL=http://vaultwarden:8000
      - FIDO2_RP_ID=auth.example.com
      - FIDO2_ORIGIN=https://auth.example.com
    depends_on:
      - postgres
      - redis
      - vaultwarden
    secrets:
      - jwt_secret

  postgres:
    image: postgres:16-alpine
    environment:
      - POSTGRES_USER=astral
      - POSTGRES_PASSWORD=secret
      - POSTGRES_DB=astral_key
    volumes:
      - postgres_data:/var/lib/postgresql/data

  redis:
    image: redis:7-alpine
    volumes:
      - redis_data:/data

  vaultwarden:
    image: vaultwarden/server:latest
    environment:
      - WEB_VAULT_ENABLED=true
      - ADMIN_TOKEN_FILE=/run/secrets/admin_token
    volumes:
      - vaultwarden_data:/data
    secrets:
      - admin_token

volumes:
  postgres_data:
  redis_data:
  vaultwarden_data:

secrets:
  jwt_secret:
    file: ./secrets/jwt_secret
  admin_token:
    file: ./secrets/admin_token
```

### Kubernetes

```yaml
# k8s/deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: astral-key
spec:
  replicas: 3
  selector:
    matchLabels:
      app: astral-key
  template:
    metadata:
      labels:
        app: astral-key
    spec:
      containers:
      - name: astral-key
        image: astral-key:latest
        ports:
        - containerPort: 8080
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: astral-key-secrets
              key: database-url
        - name: REDIS_URL
          valueFrom:
            secretKeyRef:
              name: astral-key-secrets
              key: redis-url
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
---
apiVersion: v1
kind: Service
metadata:
  name: astral-key
spec:
  selector:
    app: astral-key
  ports:
  - port: 80
    targetPort: 8080
  type: ClusterIP
```

## Secrets Management

### sops-nix

```yaml
# .sops.yaml
keys:
  - &admin_age age1...
creation_rules:
  - path_regex: secrets/[^/]+\.yaml$
    key_groups:
    - age:
      - *admin_age
```

```yaml
# secrets/astral-key.yaml
jwt_secret: ENC[AES256_GCM,data:...,iv:...,tag:...,type:str]
sops:
  # ... sops metadata
```

### agenix

```nix
# secrets.nix
let
  user = "ssh-ed25519 AAAAC3NzaC...";
in
{
  "jwt_secret.age".publicKeys = [ user ];
  "vaultwarden_token.age".publicKeys = [ user ];
}
```

## Monitoring

### Prometheus Metrics

Astral Key exposes metrics at `/metrics`:

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'astral-key'
    static_configs:
      - targets: ['localhost:8080']
```

### Health Checks

- `GET /health` - Liveness probe
- `GET /ready` - Readiness probe (checks dependencies)

## SSL/TLS

### With Nginx

```nginx
server {
    listen 443 ssl http2;
    server_name auth.example.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    location / {
        proxy_pass http://localhost:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### With Traefik

```yaml
# docker-compose.yml
labels:
  - "traefik.enable=true"
  - "traefik.http.routers.astral-key.rule=Host(`auth.example.com`)"
  - "traefik.http.routers.astral-key.tls=true"
  - "traefik.http.routers.astral-key.tls.certresolver=letsencrypt"
```

## Backup and Recovery

### Database Backup

```bash
# Backup PostgreSQL
pg_dump -h localhost -U astral astral_key > backup.sql

# Backup Vaultwarden
tar -czf vaultwarden-backup.tar.gz /var/lib/vaultwarden
```

### Automated Backups

```nix
# NixOS configuration
services.postgresqlBackup = {
  enable = true;
  location = "/var/backup/postgresql";
  startAt = "*-*-* 02:00:00";
};
```

## Troubleshooting

### Check Service Status

```bash
# NixOS
systemctl status astral-key
journalctl -u astral-key -f

# Docker
docker logs astral-key

# Kubernetes
kubectl logs -l app=astral-key
```

### Common Issues

1. **Database connection refused**
   - Check PostgreSQL is running
   - Verify connection string
   - Check firewall rules

2. **Vaultwarden authentication failed**
   - Verify admin token
   - Check Vaultwarden URL
   - Review Vaultwarden logs

3. **FIDO2 registration fails**
   - Verify rp_id matches domain
   - Check origin is HTTPS in production
   - Ensure authenticator is supported
