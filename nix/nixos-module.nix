# Astral Key — NixOS Module
#
# Declarative service configuration for the auth sidecar.
# The package option has no real default — the flake wrapper sets it
# via mkDefault. Consumers not using the flake must set:
#   services.astral-key.package = /path/to/astral-key;

{ config, lib, pkgs, ... }:

with lib;

let
  cfg = config.services.astral-key;
in
{
  options.services.astral-key = {
    enable = mkEnableOption "Astral Key auth sidecar";

    package = mkOption {
      type = types.package;
      description = "The astral-key package to use. Must be set explicitly or via flake wrapper.";
    };

    environmentFile = mkOption {
      type = types.nullOr types.path;
      default = null;
      example = "/run/secrets/astral-key-env";
      description = ''
        Path to an env file (KEY=VALUE lines) loaded as the systemd service
        EnvironmentFile. Use this for the JWT_SECRET and other sensitive config.
      '';
    };

    environment = mkOption {
      type = types.attrsOf types.str;
      default = { };
      example = {
        SERVER_HOST = "0.0.0.0";
        SERVER_PORT = "8080";
        DATABASE_URL = "sqlite:/var/lib/astral-key/astral-key.db?mode=rwc";
        FIDO2_RP_ID = "auth.example.com";
        FIDO2_ORIGINS = "https://auth.example.com";
      };
      description = "Environment variables passed to the astral-key service.";
    };

    databaseDir = mkOption {
      type = types.path;
      default = "/var/lib/astral-key";
      description = "Directory for the SQLite database and related data files.";
    };

    listenAddress = mkOption {
      type = types.str;
      default = "127.0.0.1";
      description = "IP address to bind the HTTP server to.";
    };

    port = mkOption {
      type = types.port;
      default = 8080;
      description = "TCP port for the HTTP server.";
    };

    openFirewall = mkOption {
      type = types.bool;
      default = false;
      description = "Whether to open the configured port in the firewall.";
    };

    verbose = mkOption {
      type = types.bool;
      default = false;
      description = "Enable verbose (debug-level) logging.";
    };
  };

  config = mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    systemd.tmpfiles.settings."astral-key" = {
      "${cfg.databaseDir}" = {
        d = {
          user = "astral-key";
          group = "astral-key";
          mode = "0750";
        };
      };
    };

    systemd.services.astral-key = {
      description = "Astral Key — Passkey + Web3 Auth Sidecar";
      after = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];

      serviceConfig = {
        Type = "simple";
        User = "astral-key";
        Group = "astral-key";
        StateDirectory = "astral-key";
        WorkingDirectory = cfg.databaseDir;
        Restart = "on-failure";
        RestartSec = "5s";
        LimitNOFILE = 65536;

        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        MemoryDenyWriteExecute = true;
        LockPersonality = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        RemoveIPC = true;
      };

      environment =
        let
          baseEnv = {
            SERVER_HOST = cfg.listenAddress;
            SERVER_PORT = toString cfg.port;
            DATABASE_URL = "sqlite:${cfg.databaseDir}/astral-key.db?mode=rwc";
            RUST_LOG = if cfg.verbose then "debug,astral_key=debug" else "info,astral_key=info";
          };
        in
        baseEnv // cfg.environment;

      path = with pkgs; [ openssl ];
    } // optionalAttrs (cfg.environmentFile != null) {
      serviceConfig.EnvironmentFile = cfg.environmentFile;
    };

    users.users.astral-key = {
      description = "Astral Key service user";
      isSystemUser = true;
      group = "astral-key";
      home = cfg.databaseDir;
      createHome = true;
    };

    users.groups.astral-key = { };

    networking.firewall.allowedTCPPorts = optional cfg.openFirewall cfg.port;
  };
}
