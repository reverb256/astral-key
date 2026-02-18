# Astral Key - NixOS Module
# { config, pkgs, lib, ... }:

{
  config = {
    # Astral Key service configuration
    services.astral-key = {
      enable = true;
      package = pkgs.astral-key;

      # Server configuration
      host = "0.0.0.0";
      port = 8080;

      # Database configuration
      database = {
        url = "postgresql://postgres@/astral_key";
        maxConnections = 10;
        minConnections = 2;
      };

      # Redis configuration
      redis = {
        url = "redis://localhost:6379";
        maxConnections = 10;
        minConnections = 2;
      };

      # JWT configuration
      jwt = {
        # Read from file for security
        secretFile = "/etc/astral-key/jwt-secret";
        accessTokenTtl = 900;      # 15 minutes
        refreshTokenTtl = 604800;   # 7 days
      };

      # FIDO2 configuration
      fido2 = {
        rpId = "example.com";
        rpName = "Astral Key";
        origin = "https://example.com";
      };

      # Vaultwarden integration
      vaultwarden = {
        url = "http://localhost:80";
        adminTokenFile = "/etc/astral-key/vaultwarden-token";
      };
    };

    # PostgreSQL database
    services.postgresql = {
      enable = true;
      ensureDatabases = [ "astral_key" ];
      ensureUsers = [
        {
          name = "astral-key";
          ensureDBOwnership = true;
        }
      ];
    };

    # Redis
    services.redis.servers."".enable = true;

    # Nginx reverse proxy
    services.nginx = {
      enable = true;
      virtualHosts."example.com" = {
        forceSSL = true;
        enableACME = true;
        locations."/" = {
          proxyPass = "http://localhost:8080";
        };
      };
    };

    # Firewall
    networking.firewall.allowedTCPPorts = [ 80 443 ];

    # Secrets management (use agenix or sops)
    # age.secrets.astral-key-jwt = {
    #   file = ./secrets/jwt-secret.age;
    #   mode = "440";
    #   owner = "astral-key";
    # };
  };
}
