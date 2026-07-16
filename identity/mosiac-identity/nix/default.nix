# packages/mosiac-identity.nix
#
# Builds the Mosiac identity service as a Nix package.
# Drop into the nixos-config repo's packages/ directory.
#
# Usage from flake.nix:
#   mosiac-identity = pkgs.callPackage ./packages/mosiac-identity.nix {};
#
# Output: a store path with server.js + node_modules + public/
#
# Then run with:
#   ${mosiac-identity}/bin/node ${mosiac-identity}/server.js
# Or wrap as a systemd service (see modules/services/mosiac-identity.nix)

{ lib
, pkgs
, nodejs ? pkgs.nodejs_22
, ...
}:
pkgs.buildNpmPackage {
  pname = "mosiac-identity";
  version = "0.1.0";

  src = lib.cleanSource ../.;

  # Exclude OCI and Nix files from the build
  srcFilter = path: type:
    (lib.hasPrefix (toString ./..) path)
    && ! lib.hasSuffix ".nix" path
    && ! lib.hasPrefix (toString ../oci) path;

  npmDepsHash = lib.fakeHash; # Set after first build

  # Don't build native modules — we have sql.js fallback
  npmFlags = [ "--ignore-scripts" "--legacy-peer-deps" ];
  makeCacheWritable = true;
  dontNpmBuild = true;

  installPhase = ''
    runHook preInstall
    mkdir -p $out
    cp -r node_modules $out/node_modules
    cp server.js start.sh $out/
    cp -r src $out/src
    cp -r public $out/public
    runHook postInstall
  '';

  meta = {
    description = "Mosiac identity service — Ed25519 keys, WebAuthn, QR exchange";
    homepage = "https://github.com/reverb256/Mosaic";
    license = lib.licenses.agpl3Only;
    platforms = lib.platforms.all;
    maintainers = [ ];
  };
}
