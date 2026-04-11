# Non-flake entry point for tux-rs.
#
# Usage (classic Nix, no flakes required):
#
#   nix-build -A tux-daemon
#   nix-build -A tux-tui
#
# NixOS consumers can import the module like this:
#
#   { pkgs, ... }:
#   let tux-rs = import (fetchTarball "https://github.com/tonky/tux-rs/archive/main.tar.gz") {
#     inherit pkgs;
#   };
#   in {
#     imports = [ tux-rs.nixosModules.default ];
#     services.tux-daemon.enable = true;
#   }
#
# Flakes layer on top of this file — see flake.nix.

{ pkgs ? import <nixpkgs> { } }:

rec {
  tux-daemon = pkgs.callPackage ./nix/tux-daemon.nix { };
  tux-tui = pkgs.callPackage ./nix/tux-tui.nix { };
  tux-kmod = pkgs.callPackage ./nix/tux-kmod.nix {
    kernel = pkgs.linuxPackages_latest.kernel;
  };

  overlays = {
    default = import ./nix/overlay.nix;
  };

  nixosModules = {
    default = { ... }: {
      imports = [ ./nixos/default.nix ];
      nixpkgs.overlays = [ overlays.default ];
    };
  };
}
