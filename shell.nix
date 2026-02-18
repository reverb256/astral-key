# Simple shell.nix for development
{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    rustc
    cargo
    gcc
    pkg-config
    openssl
    protobuf
    sqlite
  ];

  LD_LIBRARY_PATH = "${pkgs.openssl.out}/lib:${pkgs.sqlite.out}/lib";
  PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
}
