# Astral Key - Architecture Document

## Executive Summary

Astral Key is a next-generation authentication microservice designed for Web3, FIDO2, and Passkey authentication. Built on NixOS with cutting-edge Nix features, it provides a secure, declarative, and reproducible authentication infrastructure backed by Vaultwarden for credential management.

## System Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              ASTRAL KEY SYSTEM                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐             │
│  │   Web3 Auth     │  │  FIDO2/Passkey  │  │   API Gateway   │             │
│  │    Module       │  │     Module      │  │    (Axum)       │             │
│  └────────┬────────┘  └────────┬────────┘  └────────┬────────┘             │
│           │                    │                    │                       │
│           └────────────────────┼────────────────────┘                       │
│                                │                                            │
│                      ┌─────────▼─────────┐                                  │
│                      │  Auth Orchestrator │                                 │
│                      │   (Rust/Tokio)    │                                  │
│                      └─────────┬─────────┘                                  │
│                                │                                            │
│           ┌────────────────────┼────────────────────┐                       │
│           │                    │                    │                       │
│  ┌────────▼────────┐  ┌────────▼────────┐  ┌────────▼────────┐             │
│  │   Vaultwarden   │  │    PostgreSQL   │  │     Redis       │             │
│  │   (Secrets)     │  │   (User Data)   │  │    (Sessions)   │             │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘             │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────┐           │
│  │              NixOS Module & Systemd Services                 │           │
│  │         (Declarative Configuration & Security)              │           │
│  └─────────────────────────────────────────────────────────────┘           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Component Breakdown

#### 1. Web3 Authentication Module

**Purpose:** Handle blockchain-based authentication using Ethereum, Solana, and other EVM-compatible chains.

**Key Components:**
- **Wallet Connect Integration:** Support for WalletConnect v2 protocol
- **SIWE (Sign-In with Ethereum):** EIP-4361 compliant message signing
- **Multi-Chain Support:** Ethereum, Polygon, Arbitrum, Optimism, Solana
- **Signature Verification:** ECDSA and EdDSA signature validation

**Flow:**
```
1. Client requests nonce from /auth/web3/nonce
2. Client signs SIWE message with wallet
3. Client sends signature to /auth/web3/verify
4. Server verifies signature against blockchain
5. Server creates session and returns JWT
```

#### 2. FIDO2/Passkey Module

**Purpose:** Provide passwordless authentication using WebAuthn standard.

**Key Components:**
- **WebAuthn Server:** Relying party implementation
- **Credential Storage:** Integration with Vaultwarden for secure key storage
- **Attestation:** Support for direct, indirect, and none attestation types
- **Authenticator Selection:** Platform vs. roaming authenticator support

**Flow (Registration):**
```
1. Client requests registration options from /auth/fido2/register/options
2. Server generates challenge and user entity
3. Client creates credential using authenticator
4. Client sends attestation to /auth/fido2/register/verify
5. Server verifies attestation and stores credential
```

**Flow (Authentication):**
```
```
1. Client requests authentication options from /auth/fido2/authenticate/options
2. Server generates challenge and selects allowed credentials
3. Client generates assertion using authenticator
4. Client sends assertion to /auth/fido2/authenticate/verify
5. Server verifies assertion and creates session
```

#### 3. Vaultwarden Integration

**Purpose:** Secure credential storage and management.

**Architecture:**
```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Astral Key    │────▶│  Vaultwarden    │────▶│   SQLite/       │
│   API Server    │     │   API Client    │     │   PostgreSQL    │
└─────────────────┘     └─────────────────┘     └─────────────────┘
        │
        │ Secure credential operations:
        │ - Store WebAuthn credentials
        │ - Retrieve user credentials
        │ - Backup/restore keys
        │ - Organization management
```

**Integration Points:**
- **Admin API:** Organization and user management
- **Encrypted Export:** Backup credentials in encrypted format
- **Sync Protocol:** Real-time credential synchronization

#### 4. API Gateway (Axum)

**Purpose:** High-performance HTTP API with middleware stack.

**Features:**
- **Rate Limiting:** Token bucket algorithm with Redis backend
- **CORS:** Configurable cross-origin resource sharing
- **Request Validation:** JSON Schema validation
- **Metrics:** Prometheus-compatible metrics export
- **Tracing:** OpenTelemetry integration

**Middleware Stack:**
```rust
// Conceptual middleware order
1. TracingLayer          // Request tracing
2. MetricsLayer          // Prometheus metrics
3. CompressionLayer      // gzip/brotli compression
4. CorsLayer             // CORS headers
5. RateLimitLayer        // Rate limiting
6. AuthLayer             // JWT validation
7. ValidationLayer       // Request validation
```

## API Design

### RESTful Endpoints

#### Health & Discovery
```http
GET /health
GET /ready
GET /metrics
GET /api/v1/openapi.json
```

#### Web3 Authentication
```http
POST /api/v1/auth/web3/nonce
POST /api/v1/auth/web3/verify
POST /api/v1/auth/web3/chains
GET  /api/v1/auth/web3/sessions
DELETE /api/v1/auth/web3/sessions/{id}
```

#### FIDO2/Passkey Authentication
```http
POST /api/v1/auth/fido2/register/options
POST /api/v1/auth/fido2/register/verify
POST /api/v1/auth/fido2/authenticate/options
POST /api/v1/auth/fido2/authenticate/verify
GET  /api/v1/auth/fido2/credentials
DELETE /api/v1/auth/fido2/credentials/{id}
```

#### Session Management
```http
POST /api/v1/sessions/refresh
DELETE /api/v1/sessions/current
GET  /api/v1/sessions
```

#### User Management
```http
GET    /api/v1/users/me
PATCH  /api/v1/users/me
DELETE /api/v1/users/me
GET    /api/v1/users/me/security-keys
```

### WebSocket API

**Endpoint:** `wss://api.astral-key.local/v1/ws`

**Events:**
```json
// Authentication request
{
  "type": "auth_request",
  "id": "uuid",
  "method": "web3|fido2",
  "data": {}
}

// Authentication response
{
  "type": "auth_response",
  "id": "uuid",
  "status": "success|pending|failed",
  "token": "jwt_token"
}
```

## Nix Flake Structure

### Flake.nix Design

```nix
{
  description = "Astral Key - Web3 & FIDO2 Authentication Microservice";

  inputs = {
    # Core Nix inputs
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    
    # Flake utilities
    flake-parts.url = "github:hercules-ci/flake-parts";
    flake-parts.inputs.nixpkgs-lib.follows = "nixpkgs";
    
    # Rust toolchain
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    
    # Crane for Rust builds
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    
    # Pre-commit hooks
    pre-commit-hooks = {
      url = "github:cachix/pre-commit-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    
    # Vaultwarden for integration testing
    vaultwarden = {
      url = "github:dani-garcia/vaultwarden";
      flake = false;
    };
    
    # Nix formatting
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs@{ self, nixpkgs, flake-parts, fenix, crane, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      
      imports = [
        inputs.treefmt-nix.flakeModule
        inputs.pre-commit-hooks.flakeModule
      ];
      
      perSystem = { config, self', inputs', pkgs, system, ... }: 
        let
          # Rust toolchain with specific version
          rustToolchain = inputs'.fenix.packages.complete.withComponents [
            "cargo"
            "clippy"
            "rust-src"
            "rustc"
            "rustfmt"
            "llvm-tools-preview"
          ];
          
          # Crane library
          craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rustToolchain;
          
          # Common arguments for crane builds
          commonArgs = {
            src = craneLib.cleanCargoSource ./.;
            strictDeps = true;
            buildInputs = with pkgs; [
              openssl
              pkg-config
              protobuf
            ] ++ lib.optionals stdenv.isDarwin [
              libiconv
              darwin.apple_sdk.frameworks.Security
            ];
          };
          
          # Cargo artifacts for caching
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          
          # Main application package
          astral-key = craneLib.buildPackage (commonArgs // {
            inherit cargoArtifacts;
            pname = "astral-key";
            version = self.rev or "dev";
            
            # Additional build configuration
            cargoExtraArgs = "--features production";
            
            # Runtime checks
            doCheck = true;
            cargoTestExtraArgs = "--all-features";
            
            # Post-install hooks
            postInstall = ''
              mkdir -p $out/share/astral-key
              cp -r ./migrations $out/share/astral-key/
              cp -r ./static $out/share/astral-key/
            '';
          });
          
        in {
          # Packages
          packages = {
            default = astral-key;
            astral-key = astral-key;
            
            # Container image
            container = pkgs.dockerTools.buildLayeredImage {
              name = "astral-key";
              tag = self.rev or "latest";
              contents = [ astral-key pkgs.cacert ];
              config = {
                Cmd = [ "${astral-key}/bin/astral-key" ];
                ExposedPorts = {
                  "8080/tcp" = {};
                };
                Env = [
                  "RUST_LOG=info"
                  "RUST_BACKTRACE=1"
                ];
              };
            };
            
            # NixOS VM for testing
            vm = self'.nixosConfigurations.test-vm.config.system.build.vm;
          };
          
          # Development shells
          devShells = {
            default = pkgs.mkShell {
              name = "astral-key-dev";
              
              inputsFrom = [ astral-key ];
              
              packages = with pkgs; [
                # Rust toolchain
                rustToolchain
                cargo-watch
                cargo-edit
                cargo-deny
                cargo-audit
                cargo-tarpaulin
                cargo-nextest
                
                # Database tools
                postgresql_16
                redis
                sqlx-cli
                
                # API testing
                httpie
                websocat
                
                # Nix tools
                nil
                nix-tree
                nix-diff
                
                # Documentation
                mdbook
                plantuml
              ];
              
              shellHook = ''
                echo "╔══════════════════════════════════════════╗"
                echo "║     Astral Key Development Environment   ║"
                echo "╚══════════════════════════════════════════╝"
                echo ""
                echo "  API:        http://localhost:8080"
                echo "  Docs:       http://localhost:8080/docs"
                echo "  Metrics:    http://localhost:8080/metrics"
                echo ""
                echo "  Commands:"
                echo "    just dev        - Start development server"
                echo "    just test       - Run all tests"
                echo "    just db-up      - Start database services"
                echo "    just migrate    - Run database migrations"
                echo ""
                
                # Environment setup
                export DATABASE_URL="postgresql://astral:astral@localhost:5432/astral_key"
                export REDIS_URL="redis://localhost:6379"
                export VAULTWARDEN_URL="http://localhost:8000"
                export RUST_LOG="debug,astral_key=trace"
                
                # Pre-commit hooks
                ${config.pre-commit.installationScript}
              '';
            };
            
            # CI shell with minimal dependencies
            ci = pkgs.mkShell {
              name = "astral-key-ci";
              packages = [ rustToolchain ] ++ commonArgs.buildInputs;
            };
          };
          
          # Applications (nix run)
          apps = {
            default = {
              type = "app";
              program = "${astral-key}/bin/astral-key";
            };
            
            migrate = {
              type = "app";
              program = pkgs.writeShellScriptBin "migrate" ''
                export DATABASE_URL="''${DATABASE_URL:-postgresql://astral:astral@localhost:5432/astral_key}"
                exec ${pkgs.sqlx-cli}/bin/sqlx migrate run --source ${astral-key}/share/astral-key/migrations
              '';
            };
          };
          
          # Checks (nix flake check)
          checks = {
            # Unit tests
            test = craneLib.cargoTest (commonArgs // {
              inherit cargoArtifacts;
            });
            
            # Clippy linting
            clippy = craneLib.cargoClippy (commonArgs // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-features -- --deny warnings";
            });
            
            # Formatting check
            fmt = craneLib.cargoFmt commonArgs;
            
            # Audit
            audit = craneLib.cargoAudit {
              inherit (commonArgs) src;
              advisory-db = inputs.advisory-db;
            };
            
            # Deny license check
            deny = craneLib.cargoDeny {
              inherit (commonArgs) src;
            };
          };
          
          # Formatter configuration
          treefmt.config = {
            projectRootFile = "flake.nix";
            programs = {
              nixpkgs-fmt.enable = true;
              rustfmt.enable = true;
              taplo.enable = true;
              prettier.enable = true;
            };
          };
          
          # Pre-commit hooks
          pre-commit.settings.hooks = {
            nixpkgs-fmt.enable = true;
            rustfmt.enable = true;
            clippy.enable = true;
            typos.enable = true;
          };
        };
      
      # NixOS configurations
      flake.nixosModules = {
        astral-key = import ./nix/nixos-module.nix;
        default = self.nixosModules.astral-key;
      };
      
      # NixOS configurations for testing
      flake.nixosConfigurations = {
        test-vm = nixpkgs.lib.nixosSystem {
          system = "x86_64-linux";
          modules = [
            self.nixosModules.astral-key
            ./nix/test-vm.nix
          ];
        };
      };
    };
}
```

### Cutting-Edge Nix Features Used

1. **Flake Parts:** Modular flake structure for better composability
2. **Crane:** Incremental Rust builds with artifact caching
3. **Fenix:** Rust toolchain management with specific versions
4. **Treefmt:** Unified code formatting across languages
5. **Pre-commit Hooks:** Automated quality checks
6. **Build Layered Image:** Optimized container images
7. **NixOS VM:** Reproducible test environments

## NixOS Module Design

### Module Structure

```nix
# nix/nixos-module.nix
{ config, lib, pkgs, ... }:

let
  cfg = config.services.astral-key;
  
  # TOML configuration generation
  configFile = pkgs.writeText "astral-key.toml" (lib.generators.toINI {} {
    server = {
      host = cfg.host;
      port = cfg.port;
      workers = cfg.workers;
    };
    
    database = {
      url = cfg.database.url;
      max_connections = cfg.database.maxConnections;
      min_connections = cfg.database.minConnections;
    };
    
    redis = {
      url = cfg.redis.url;
      pool_size = cfg.redis.poolSize;
    };
    
    vaultwarden = {
      url = cfg.vaultwarden.url;
      admin_token_file = cfg.vaultwarden.adminTokenFile;
    };
    
    web3 = {
      chains = cfg.web3.chains;
      rpc_endpoints = cfg.web3.rpcEndpoints;
    };
    
    fido2 = {
      rp_id = cfg.fido2.rpId;
      rp_name = cfg.fido2.rpName;
      origin = cfg.fido2.origin;
      attestation = cfg.fido2.attestation;
    };
    
    jwt = {
      secret_file = cfg.jwt.secretFile;
      access_token_ttl = cfg.jwt.accessTokenTtl;
      refresh_token_ttl = cfg.jwt.refreshTokenTtl;
    };
    
    rate_limit = {
      requests_per_minute = cfg.rateLimit.requestsPerMinute;
      burst_size = cfg.rateLimit.burstSize;
    };
  });
  
in {
  options.services.astral-key = {
    enable = lib.mkEnableOption "Astral Key authentication service";
    
    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.astral-key;
      description = "Astral Key package to use";
    };
    
    host = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1";
      description = "Host to bind the server to";
    };
    
    port = lib.mkOption {
      type = lib.types.port;
      default = 8080;
      description = "Port to listen on";
    };
    
    workers = lib.mkOption {
      type = lib.types.ints.positive;
      default = 4;
      description = "Number of worker threads";
    };
    
    database = {
      url = lib.mkOption {
        type = lib.types.str;
        default = "postgresql://astral:astral@localhost:5432/astral_key";
        description = "Database connection URL";
      };
      
      maxConnections = lib.mkOption {
        type = lib.types.ints.positive;
        default = 10;
        description = "Maximum database connections";
      };
      
      minConnections = lib.mkOption {
        type = lib.types.ints.positive;
        default = 2;
        description = "Minimum database connections";
      };
    };
    
    redis = {
      url = lib.mkOption {
        type = lib.types.str;
        default = "redis://localhost:6379";
        description = "Redis connection URL";
      };
      
      poolSize = lib.mkOption {
        type = lib.types.ints.positive;
        default = 10;
        description = "Redis connection pool size";
      };
    };
    
    vaultwarden = {
      url = lib.mkOption {
        type = lib.types.str;
        default = "http://localhost:8000";
        description = "Vaultwarden instance URL";
      };
      
      adminTokenFile = lib.mkOption {
        type = lib.types.path;
        description = "Path to Vaultwarden admin token file";
      };
    };
    
    web3 = {
      chains = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ "ethereum" "polygon" "arbitrum" "optimism" ];
        description = "Supported blockchain chains";
      };
      
      rpcEndpoints = lib.mkOption {
        type = lib.types.attrsOf lib.types.str;
        default = {};
        description = "RPC endpoints for each chain";
      };
    };
    
    fido2 = {
      rpId = lib.mkOption {
        type = lib.types.str;
        description = "Relying Party ID (domain)";
      };
      
      rpName = lib.mkOption {
        type = lib.types.str;
        default = "Astral Key";
        description = "Relying Party display name";
      };
      
      origin = lib.mkOption {
        type = lib.types.str;
        description = "Allowed origin for WebAuthn";
      };
      
      attestation = lib.mkOption {
        type = lib.types.enum [ "none" "indirect" "direct" "enterprise" ];
        default = "indirect";
        description = "Attestation conveyance preference";
      };
    };
    
    jwt = {
      secretFile = lib.mkOption {
        type = lib.types.path;
        description = "Path to JWT secret key file";
      };
      
      accessTokenTtl = lib.mkOption {
        type = lib.types.ints.positive;
        default = 900; # 15 minutes
        description = "Access token TTL in seconds";
      };
      
      refreshTokenTtl = lib.mkOption {
        type = lib.types.ints.positive;
        default = 604800; # 7 days
        description = "Refresh token TTL in seconds";
      };
    };
    
    rateLimit = {
      requestsPerMinute = lib.mkOption {
        type = lib.types.ints.positive;
        default = 60;
        description = "Maximum requests per minute per IP";
      };
      
      burstSize = lib.mkOption {
        type = lib.types.ints.positive;
        default = 10;
        description = "Burst size for rate limiting";
      };
    };
    
    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Open firewall port for Astral Key";
    };
  };
  
  config = lib.mkIf cfg.enable {
    # User and group
    users.users.astral-key = {
      isSystemUser = true;
      group = "astral-key";
      home = "/var/lib/astral-key";
      createHome = true;
      description = "Astral Key service user";
    };
    
    users.groups.astral-key = {};
    
    # Data directory
    systemd.tmpfiles.rules = [
      "d /var/lib/astral-key 0750 astral-key astral-key - -"
      "d /var/lib/astral-key/data 0750 astral-key astral-key - -"
      "d /var/log/astral-key 0750 astral-key astral-key - -"
    ];
    
    # Systemd service
    systemd.services.astral-key = {
      description = "Astral Key Authentication Service";
      wantedBy = [ "multi-user.target" ];
      after = [ "network-online.target" "postgresql.service" "redis.service" "vaultwarden.service" ];
      wants = [ "network-online.target" ];
      
      serviceConfig = {
        Type = "notify";
        User = "astral-key";
        Group = "astral-key";
        WorkingDirectory = "/var/lib/astral-key";
        
        ExecStart = "${cfg.package}/bin/astral-key --config ${configFile}";
        ExecReload = "${pkgs.coreutils}/bin/kill -HUP $MAINPID";
        
        # Security hardening
        NoNewPrivileges = true;
        PrivateTmp = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        ReadWritePaths = [ "/var/lib/astral-key" "/var/log/astral-key" ];
        
        # Capabilities
        AmbientCapabilities = [ "CAP_NET_BIND_SERVICE" ];
        CapabilityBoundingSet = [ "CAP_NET_BIND_SERVICE" ];
        
        # Resource limits
        LimitNOFILE = 65536;
        MemoryHigh = "512M";
        MemoryMax = "1G";
        CPUQuota = "200%";
        
        # Restart policy
        Restart = "on-failure";
        RestartSec = 5;
        StartLimitBurst = 3;
        StartLimitIntervalSec = 60;
        
        # Notify systemd when ready
        NotifyAccess = "all";
        
        # Logging
        StandardOutput = "journal";
        StandardError = "journal";
        SyslogIdentifier = "astral-key";
        
        # Sandboxing
        PrivateDevices = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        RestrictAddressFamilies = [ "AF_INET" "AF_INET6" "AF_UNIX" ];
        RestrictNamespaces = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        SystemCallFilter = [ "@system-service" "~@privileged" ];
        SystemCallErrorNumber = "EPERM";
      };
      
      environment = {
        RUST_LOG = "info,astral_key=debug";
        RUST_BACKTRACE = "1";
      };
    };
    
    # Firewall
    networking.firewall = lib.mkIf cfg.openFirewall {
      allowedTCPPorts = [ cfg.port ];
    };
    
    # Database initialization
    systemd.services.astral-key-migrations = {
      description = "Astral Key Database Migrations";
      requiredBy = [ "astral-key.service" ];
      before = [ "astral-key.service" ];
      after = [ "postgresql.service" ];
      
      serviceConfig = {
        Type = "oneshot";
        User = "astral-key";
        Group = "astral-key";
        
        ExecStart = "${cfg.package}/bin/astral-key-migrate --database-url ${cfg.database.url}";
        
        RemainAfterExit = true;
      };
    };
  };
}
```

## Directory Structure

```
astral-key/
├── Cargo.toml                 # Rust workspace configuration
├── Cargo.lock                 # Dependency lock file
├── flake.nix                  # Nix flake definition
├── flake.lock                 # Flake lock file
├── shell.nix                  # Legacy shell.nix (wraps flake)
├── default.nix                # Legacy default.nix (wraps flake)
├── justfile                   # Task runner configuration
├── README.md                  # Project documentation
├── ARCHITECTURE.md            # This document
├── LICENSE                    # License file
│
├── src/                       # Source code
│   ├── main.rs               # Application entry point
│   ├── lib.rs                # Library exports
│   ├── config.rs             # Configuration management
│   ├── error.rs              # Error types
│   ├── state.rs              # Application state
│   │
│   ├── api/                  # API layer
│   │   ├── mod.rs
│   │   ├── routes.rs         # Route definitions
│   │   ├── middleware/       # Axum middleware
│   │   │   ├── mod.rs
│   │   │   ├── auth.rs
│   │   │   ├── rate_limit.rs
│   │   │   ├── cors.rs
│   │   │   └── tracing.rs
│   │   └── handlers/         # Request handlers
│   │       ├── mod.rs
│   │       ├── health.rs
│   │       ├── web3.rs
│   │       ├── fido2.rs
│   │       └── session.rs
│   │
│   ├── auth/                 # Authentication modules
│   │   ├── mod.rs
│   │   ├── web3/             # Web3 authentication
│   │   │   ├── mod.rs
│   │   │   ├── siwe.rs       # EIP-4361 implementation
│   │   │   ├── verifier.rs   # Signature verification
│   │   │   └── types.rs
│   │   ├── fido2/            # FIDO2/WebAuthn
│   │   │   ├── mod.rs
│   │   │   ├── register.rs
│   │   │   ├── authenticate.rs
│   │   │   ├── types.rs
│   │   │   └── challenge.rs
│   │   └── jwt/              # JWT handling
│   │       ├── mod.rs
│   │       ├── issuer.rs
│   │       ├── verifier.rs
│   │       └── types.rs
│   │
│   ├── vaultwarden/          # Vaultwarden integration
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   ├── types.rs
│   │   └── sync.rs
│   │
│   ├── db/                   # Database layer
│   │   ├── mod.rs
│   │   ├── pool.rs           # Connection pool
│   │   ├── models/           # Database models
│   │   │   ├── mod.rs
│   │   │   ├── user.rs
│   │   │   ├── credential.rs
│   │   │   └── session.rs
│   │   └── migrations/       # SQL migrations
│   │       ├── 001_initial.sql
│   │       └── ...
│   │
│   ├── cache/                # Redis cache layer
│   │   ├── mod.rs
│   │   ├── pool.rs
│   │   └── operations.rs
│   │
│   └── utils/                # Utilities
│       ├── mod.rs
│       ├── crypto.rs
│       ├── validation.rs
│       └── logging.rs
│
├── tests/                    # Integration tests
│   ├── integration_tests.rs
│   ├── web3_tests.rs
│   ├── fido2_tests.rs
│   └── fixtures/
│       └── ...
│
├── benches/                  # Benchmarks
│   └── auth_benchmark.rs
│
├── nix/                      # Nix-specific files
│   ├── nixos-module.nix      # NixOS module
│   ├── test-vm.nix           # Test VM configuration
│   ├── container.nix         # Container build config
│   └── ci.nix                # CI/CD configuration
│
├── docs/                     # Documentation
│   ├── api.md               # API documentation
│   ├── deployment.md        # Deployment guide
│   └── development.md       # Development guide
│
├── static/                   # Static assets
│   └── ...
│
└── migrations/               # Database migrations (copied to output)
    └── ...
```

## Implementation Recommendations

### Nix Features to Use

1. **Flakes with Flake Parts:**
   - Modular, composable flake structure
   - Per-system configuration
   - Easy integration with other flakes

2. **Crane for Rust:**
   - Incremental builds with artifact caching
   - Automatic dependency vendoring
   - Cross-compilation support

3. **Fenix for Rust Toolchain:**
   - Pin specific Rust versions
   - Component management (clippy, rustfmt, etc.)
   - Cross-compilation toolchains

4. **NixOS Module System:**
   - Declarative service configuration
   - Type-safe options
   - Automatic systemd unit generation

5. **sops-nix for Secrets:**
   - Encrypted secrets in git
   - Runtime decryption
   - Integration with NixOS

6. **nix-direnv:**
   - Automatic environment activation
   - Fast shell switching
   - Cache development shells

### Vaultwarden Integration Strategy

```
┌─────────────────────────────────────────────────────────────┐
│                    Vaultwarden Integration                  │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. Organization per Application                           │
│     - Create org: "astral-key-{environment}"               │
│     - Users map 1:1 with Astral Key users                  │
│                                                             │
│  2. Collection Structure                                   │
│     - web3-credentials: Wallet addresses, chain info       │
│     - fido2-credentials: Passkey metadata (not keys)       │
│     - user-secrets: Encrypted user data                    │
│                                                             │
│  3. Sync Strategy                                          │
│     - Webhook on Vaultwarden events                        │
│     - Periodic full sync (backup)                          │
│     - Conflict resolution: Timestamp-based                 │
│                                                             │
│  4. Security Model                                         │
│     - Admin API access only from Astral Key                │
│     - Token rotation every 24 hours                        │
│     - Audit logging of all credential access               │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Web3 Authentication Flow

```
┌──────────┐                                    ┌──────────────┐
│  Client  │                                    │  Astral Key  │
└────┬─────┘                                    └──────┬───────┘
     │                                                 │
     │  1. GET /auth/web3/nonce                        │
     │ ───────────────────────────────────────────────▶│
     │                                                 │
     │  2. { nonce, message_template, domain }         │
     │ ◀───────────────────────────────────────────────│
     │                                                 │
     │  3. Sign message with wallet                    │
     │     (SIWE format)                               │
     │                                                 │
     │  4. POST /auth/web3/verify                      │
     │    { message, signature, chain_id }             │
     │ ───────────────────────────────────────────────▶│
     │                                                 │
     │  5. Verify signature                            │
     │     Check nonce not replayed                    │
     │     Verify chain support                        │
     │                                                 │
     │  6. { access_token, refresh_token, user }       │
     │ ◀───────────────────────────────────────────────│
     │                                                 │
```

**SIWE Message Format:**
```
app.astral-key.local wants you to sign in with your Ethereum account:
0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb

Sign in to Astral Key

URI: https://app.astral-key.local
Version: 1
Chain ID: 1
Nonce: abc123def456
Issued At: 2026-02-02T02:36:02.488Z
Expiration Time: 2026-02-02T02:51:02.488Z
```

### FIDO2/Passkey Implementation Approach

**Recommended Libraries:**
- **Rust:** `webauthn-rs` - Full-featured WebAuthn server library
- **Frontend:** `@github/webauthn-json` - Client-side WebAuthn helpers

**Implementation Flow:**

```rust
// Registration flow
pub async fn start_registration(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
) -> Result<Json<RegistrationOptions>, AuthError> {
    let challenge = state.webauthn.generate_challenge();
    
    let options = RegistrationOptions {
        challenge: BASE64.encode(&challenge),
        rp: RelyingParty {
            name: "Astral Key".to_string(),
            id: state.config.fido2.rp_id.clone(),
        },
        user: PublicKeyCredentialUserEntity {
            id: user.id.as_bytes().to_vec(),
            name: user.email.clone(),
            display_name: user.display_name.clone(),
        },
        pub_key_cred_params: vec![
            PublicKeyCredentialParameters {
                type_: "public-key".to_string(),
                alg: COSEAlgorithm::ES256,
            },
            PublicKeyCredentialParameters {
                type_: "public-key".to_string(),
                alg: COSEAlgorithm::RS256,
            },
        ],
        authenticator_selection: AuthenticatorSelectionCriteria {
            authenticator_attachment: None, // Platform or roaming
            resident_key: ResidentKeyRequirement::Preferred,
            user_verification: UserVerificationRequirement::Preferred,
        },
        attestation: AttestationConveyancePreference::Indirect,
    };
    
    // Store challenge in Redis with TTL
    state.cache.set_challenge(&user.id, &challenge, 300).await?;
    
    Ok(Json(options))
}
```

**Credential Storage:**
- Public keys stored in PostgreSQL
- Private key metadata stored in Vaultwarden
- Challenge state in Redis (5 minute TTL)
- Backup codes in Vaultwarden (encrypted)

## Security Considerations

### Threat Model

| Threat | Mitigation |
|--------|------------|
| Replay attacks | Nonce-based SIWE, challenge-response for FIDO2 |
| Credential stuffing | Rate limiting, device fingerprinting |
| Session hijacking | Short-lived JWTs, secure httpOnly cookies |
| MITM | TLS 1.3, certificate pinning |
| Key exfiltration | Hardware security modules (HSM) support |
| Database breach | Encrypted at rest, no plaintext secrets |

### Security Hardening

1. **NixOS Level:**
   - Systemd sandboxing (implemented in module)
   - Immutable infrastructure
   - Automatic security updates via flakes

2. **Application Level:**
   - Constant-time comparison for signatures
   - Memory-safe Rust implementation
   - Input validation with strict schemas

3. **Network Level:**
   - mTLS between services
   - Network segmentation
   - DDoS protection

## Deployment Architecture

### Production Deployment

```
┌─────────────────────────────────────────────────────────────────┐
│                        Production Stack                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐         │
│  │   Nginx     │────│  Astral Key │────│  PostgreSQL │         │
│  │  (Reverse   │    │   (3 nodes) │    │  (Primary/  │         │
│  │   Proxy)    │    │             │    │  Replica)   │         │
│  └─────────────┘    └──────┬──────┘    └─────────────┘         │
│                            │                                    │
│                     ┌──────┴──────┐                            │
│                     │             │                            │
│               ┌─────▼─────┐ ┌─────▼─────┐                      │
│               │   Redis   │ │Vaultwarden│                      │
│               │  (Cluster)│ │  (HA)     │                      │
│               └───────────┘ └───────────┘                      │
│                                                                 │
│  NixOS Configuration:                                           │
│  - Declarative deployment via nixos-rebuild                   │
│  - Automatic failover with keepalived                         │
│  - Encrypted secrets via sops-nix                             │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Monitoring & Observability

**Metrics (Prometheus):**
- Authentication success/failure rates
- Latency percentiles
- Active sessions
- Rate limit hits

**Tracing (OpenTelemetry):**
- Distributed tracing across services
- Correlation IDs for request flows
- Performance profiling

**Alerting:**
- High error rates
- Unusual authentication patterns
- Certificate expiration

## Migration Strategy

### From Existing Systems

1. **Phase 1: Parallel Deployment**
   - Deploy Astral Key alongside existing auth
   - Gradual traffic shifting (canary)

2. **Phase 2: User Migration**
   - Import users to Vaultwarden
   - Prompt for passkey registration
   - Maintain legacy auth as fallback

3. **Phase 3: Legacy Deprecation**
   - Remove old authentication paths
   - Full Web3/FIDO2-only mode

## Conclusion

Astral Key represents a modern, secure, and Nix-native approach to authentication. By leveraging:

- **Rust** for memory safety and performance
- **Nix** for reproducible, declarative infrastructure
- **Web3** for decentralized identity
- **FIDO2/Passkey** for passwordless security
- **Vaultwarden** for secure credential management

This architecture provides a robust foundation for next-generation authentication systems.

## Appendix

### A. NixOS Configuration Example

```nix
# /etc/nixos/astral-key.nix
{ config, pkgs, ... }:

{
  imports = [ /path/to/astral-key/nix/nixos-module.nix ];

  services.astral-key = {
    enable = true;
    host = "0.0.0.0";
    port = 8080;
    openFirewall = true;
    
    database = {
      url = config.sops.secrets.astral-key-database-url.path;
      maxConnections = 20;
    };
    
    redis = {
      url = "redis://localhost:6379";
    };
    
    vaultwarden = {
      url = "http://localhost:8000";
      adminTokenFile = config.sops.secrets.vaultwarden-admin-token.path;
    };
    
    fido2 = {
      rpId = "auth.astral-key.local";
      origin = "https://app.astral-key.local";
      attestation = "direct";
    };
    
    jwt = {
      secretFile = config.sops.secrets.astral-key-jwt-secret.path;
      accessTokenTtl = 900;
      refreshTokenTtl = 604800;
    };
  };
  
  # Vaultwarden service
  services.vaultwarden = {
    enable = true;
    config = {
      DOMAIN = "https://vault.astral-key.local";
      SIGNUPS_ALLOWED = false;
      ADMIN_TOKEN_FILE = config.sops.secrets.vaultwarden-admin-token.path;
    };
  };
  
  # PostgreSQL
  services.postgresql = {
    enable = true;
    ensureDatabases = [ "astral_key" ];
    ensureUsers = [{
      name = "astral";
      ensureDBOwnership = true;
    }];
  };
  
  # Redis
  services.redis.servers.astral-key = {
    enable = true;
    bind = "127.0.0.1";
    port = 6379;
  };
}
```

### B. Environment Variables

| Variable | Description | Required |
|----------|-------------|----------|
| `ASTRAL_KEY_DATABASE_URL` | PostgreSQL connection string | Yes |
| `ASTRAL_KEY_REDIS_URL` | Redis connection string | Yes |
| `ASTRAL_KEY_VAULTWARDEN_URL` | Vaultwarden API URL | Yes |
| `ASTRAL_KEY_JWT_SECRET` | JWT signing secret | Yes |
| `ASTRAL_KEY_FIDO2_RP_ID` | WebAuthn relying party ID | Yes |
| `RUST_LOG` | Log level filter | No |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | OpenTelemetry endpoint | No |

### C. References

- [EIP-4361: Sign-In with Ethereum](https://eips.ethereum.org/EIPS/eip-4361)
- [WebAuthn Level 2 Specification](https://www.w3.org/TR/webauthn-2/)
- [FIDO2 Overview](https://fidoalliance.org/fido2/)
- [Nix Flakes](https://nixos.wiki/wiki/Flakes)
- [Crane Documentation](https://github.com/ipetkov/crane)
- [Vaultwarden Wiki](https://github.com/dani-garcia/vaultwarden/wiki)