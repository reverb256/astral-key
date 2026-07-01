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

    # Treefmt for formatting
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
            ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.libiconv
              darwin.apple_sdk.frameworks.Security
            ];
          };

          # Cargo artifacts for caching
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;

          # Main application package
          astral-key = craneLib.buildPackage (commonArgs // {
            inherit cargoArtifacts;
            pname = "astral-key";
            version = "0.1.0";

            # Runtime checks
            doCheck = true;

            # Post-install hooks
            postInstall = ''
              mkdir -p $out/share/astral-key
              if [ -d ./migrations ]; then
                cp -r ./migrations $out/share/astral-key/
              fi
            '';
          });

        in
        {
          # Packages
          packages = {
            default = astral-key;
            astral-key = astral-key;
          };

          # Development shells
          devShells = {
            default = pkgs.mkShell {
              name = "astral-key-dev";

              packages = with pkgs; [
                # Rust toolchain
                rustToolchain
                cargo-watch
                cargo-edit

                # Database tools
                postgresql
                redis
                sqlx-cli

                # API testing
                httpie

                # Build dependencies
                openssl
                pkg-config

                # Nix tools
                nil
              ];

              shellHook = ''
                echo "╔══════════════════════════════════════════╗"
                echo "║     Astral Key Development Environment   ║"
                echo "╚══════════════════════════════════════════╝"
                echo ""
                echo "  API:        http://localhost:8080"
                echo "  Health:     http://localhost:8080/health"
                echo ""
                echo "  Current Status: Prototype (~5% complete)"
                echo "  See STATUS.md for details"
                echo ""
                echo "  Commands:"
                echo "    cargo run       - Start development server"
                echo "    cargo test       - Run tests"
                echo "    cargo clippy     - Run linter"
                echo ""

                # Environment setup
                export DATABASE_URL="postgresql://astral:astral@localhost:5432/astral_key"
                export REDIS_URL="redis://localhost:6379"
                export RUST_LOG="debug,astral_key=trace"

                # Pre-commit hooks
                ${config.pre-commit.installationScript}
              '';
            };
          };

          # Applications (nix run)
          apps = {
            default = {
              type = "app";
              program = "${astral-key}/bin/astral-key";
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
          };

          # Formatter configuration
          treefmt.config = {
            projectRootFile = "flake.nix";
            programs = {
              nixpkgs-fmt.enable = true;
              rustfmt.enable = true;
            };
          };

          # Pre-commit hooks
          pre-commit.settings.hooks = {
            nixpkgs-fmt.enable = true;
            rustfmt.enable = true;
            clippy.enable = true;
          };
        };
    };
}
