# Non-flake entry point for tux-rs.
#
# This file accepts an attribute set of dependencies so that classic Nix
# consumers (nix-build, niv/npins, Hydra, ...) can inject their own pinned
# nixpkgs.
#
# Basic usage (flakes not required):
#
#   nix-build -A tux-daemon
#   nix-build -A tux-tui
#
# NixOS module usage:
#
#   let
#     tux-rs = import (builtins.fetchTarball {
#       url = "https://github.com/tonky/tux-rs/archive/main.tar.gz";
#     }) { inherit pkgs; };
#   in {
#     imports = [ tux-rs.nixosModule ];
#     services.tux-daemon.enable = true;
#   }

{ nixpkgs ? <nixpkgs>
, system ? builtins.currentSystem
, pkgs ? import nixpkgs { inherit system; }
}:

let
  rustPlatform = pkgs.rustPlatform;
in
rec {
  tux-daemon = pkgs.callPackage ./nix/tux-daemon.nix { inherit rustPlatform; };
  tux-tui = pkgs.callPackage ./nix/tux-tui.nix { inherit rustPlatform; };

  overlay = import ./nix/overlay.nix;

  nixosModule = { ... }: {
    imports = [ ./nix/nixos.nix ];
    nixpkgs.overlays = [ overlay ];
  };

  # Compatibility aliases for older configs.
  overlays.default = overlay;
  nixosModules.default = nixosModule;
}
