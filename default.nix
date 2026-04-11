# Non-flake entry point for tux-rs.
#
# This file accepts an attribute set of dependencies so that classic Nix
# consumers (nix-build, niv, npins, Hydra, ...) can inject their own pinned
# versions of nixpkgs and rust-overlay.
#
# Basic usage (flakes not required):
#
#   nix-build -A tux-daemon
#   nix-build -A tux-tui
#
# npins-style usage:
#
#   # Pin dependencies:
#   #   npins add github nixos nixpkgs
#   #   npins add github oxalica rust-overlay
#   #   npins add github tonky tux-rs
#   let
#     sources = import ./npins;
#     pkgs = import sources.nixpkgs {
#       overlays = [ (import sources.rust-overlay) ];
#     };
#     tux-rs = import sources.tux-rs {
#       inherit pkgs;
#       nixpkgs = sources.nixpkgs;
#       rust-overlay = sources.rust-overlay;
#     };
#   in {
#     imports = [ tux-rs.nixosModules.default ];
#     services.tux-daemon.enable = true;
#     # Optional: use the rust-overlay-pinned binary from `tux-rs` rather than
#     # the nixpkgs-rustPlatform one produced by the default overlay.
#     services.tux-daemon.package = tux-rs.tux-daemon;
#     services.tux-daemon.tui.package = tux-rs.tux-tui;
#   }
#
# Flakes layer on top of this file — see flake.nix.

{ nixpkgs ? <nixpkgs>
, rust-overlay ? null
, system ? builtins.currentSystem
, pkgs ? import nixpkgs {
    inherit system;
    overlays =
      if rust-overlay == null
      then [ ]
      else [ (if builtins.isFunction rust-overlay then rust-overlay else import rust-overlay) ];
  }
}:

let
  # If the caller provided pkgs without rust-overlay already applied, layer it
  # on so we can build a pinned rustPlatform. If pkgs already has rust-bin
  # (e.g. the caller applied the overlay themselves, as our flake does), this
  # branch is a no-op.
  pkgsWithRust =
    if rust-overlay == null || pkgs ? rust-bin
    then pkgs
    else pkgs.appendOverlays [
      (if builtins.isFunction rust-overlay then rust-overlay else import rust-overlay)
    ];

  rustPlatform =
    if pkgsWithRust ? rust-bin then
      let toolchain = pkgsWithRust.rust-bin.stable.latest.default;
      in pkgsWithRust.makeRustPlatform {
        rustc = toolchain;
        cargo = toolchain;
      }
    else
      pkgsWithRust.rustPlatform;
in
rec {
  tux-daemon = pkgsWithRust.callPackage ./nix/tux-daemon.nix { inherit rustPlatform; };
  tux-tui = pkgsWithRust.callPackage ./nix/tux-tui.nix { inherit rustPlatform; };
  tux-kmod = pkgsWithRust.callPackage ./nix/tux-kmod.nix {
    kernel = pkgsWithRust.linuxPackages_latest.kernel;
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
