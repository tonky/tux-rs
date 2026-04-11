# Worklog: Stage 5 — Nix packaging restructure

## Motivation
Issue #3 review from `balintbarna`:
1. README points at `github:tuxedocomputers/tux-rs`, but the actual remote is `github:tonky/tux-rs`.
2. Everything is flake-only; flakes are still experimental, so classic Nix / niv / npins users are locked out.
3. Packages live inlined in `flake.nix` — separate files would be cleaner.
4. Packages should be functions so consumers can inject their own dependency versions.
5. Flakes should act as an optional wrapper to avoid code duplication.

## Changes

### New files
- `default.nix` — non-flake entry point. Returns `{ tux-daemon, tux-tui, tux-kmod, overlays, nixosModules }`. Works with `nix-build -A tux-daemon`.
- `nix/tux-daemon.nix`, `nix/tux-tui.nix`, `nix/tux-kmod.nix` — function-based derivations usable via `pkgs.callPackage`. Consumers can override `rustPlatform`, `dbus`, `kernel`, etc.
- `nix/overlay.nix` — generic overlay for non-flake consumers; uses `final.callPackage` with ambient `rustPlatform`.
- `impl/2026-04-11-nixos-support/stage-5.md` — design + plan.

### Modified
- `flake.nix` — now a thin wrapper. `pkgs.callPackage ./nix/tux-daemon.nix { inherit rustPlatform; }` injects the rust-overlay-pinned toolchain. The flake-side `overlays.default` still references `self.packages.${system}.*` so flake consumers of `nixosModules.default` get pinned binaries unchanged.
- `README.md` — URL corrections (two places) plus a new "NixOS (classic Nix)" section showing non-flake usage.

### Unchanged
- `nixos/default.nix` — module signature was already clean; no changes needed.
- VM test derivation — still `self.nixosModules.default` + `--mock`.

## Design notes

### Two overlays, not one
- `nix/overlay.nix` (non-flake): `final.callPackage ./tux-daemon.nix {}` → ambient `rustPlatform`.
- Flake-internal overlay in `flake.nix`: `self.packages.${final.system}.tux-daemon` → pinned toolchain.

Both point at the same underlying derivation files, so only the overlay wiring is
duplicated (3 lines × 2). Trying to collapse this into a single overlay would
require either globally overriding `rustPlatform` (affects every package in the
tree — nasty side effects) or making the overlay take parameters (awkward since
`overlays.default` is system-independent but `rustPlatform` is per-system).

### Relative source paths
`nix/*.nix` lives one directory below the repo root:
- `src = ../.;` for Rust workspace packages
- `cargoLock.lockFile = ../Cargo.lock;`
- `src = ../tux-kmod;` for the kernel module package

## Validation
- `nix eval .#packages.x86_64-linux.{tux-daemon,tux-tui,tux-kmod}.drvPath` — all produced valid derivation paths.
- `nix-instantiate --eval -E '(import ./default.nix {}).{tux-daemon,tux-tui,tux-kmod}.drvPath'` — non-flake path evaluates cleanly.
- `nix eval .#checks.x86_64-linux.vmTest.drvPath` — VM test still evaluates.
- `nix flake show` — all outputs resolve on all systems.

Derivation bodies are structurally identical to the previous inline versions
(same `src`, `cargoLock`, `buildInputs`, `cargoBuildFlags`, `postInstall`), so
eval-passing is strong evidence the full build still works.

## Out of scope
- Submitting to nixpkgs upstream
- Hydra / CI integration

## Follow-up: expanded default.nix signature (balintbarna's updated comment)

After posting stage-5, balintbarna clarified the non-flake ergonomics he wanted.
Quoted requirement: *"The top level module which defines the packages and nixos
module, should be a function that takes an attribute set of its dependencies."*
His concrete usage:

```nix
tux-rs = sources.tux-rs.nixosModule { inherit pkgs nixpkgs rust-overlay; };
```

The initial `default.nix` only accepted `{ pkgs }` and had no rust-overlay
integration on the non-flake path. Updated signature:

```nix
{ nixpkgs ? <nixpkgs>
, rust-overlay ? null         # path or overlay function
, system ? builtins.currentSystem
, pkgs ? import nixpkgs { inherit system; overlays = [ ... ]; }
}:
```

Semantics:
- `{}` — uses ambient `<nixpkgs>`, no rust-overlay, ambient `rustPlatform`.
- `{ pkgs }` — uses caller's `pkgs`, ambient `rustPlatform`.
- `{ nixpkgs }` — imports `nixpkgs` itself, no rust-overlay.
- `{ pkgs, rust-overlay }` — detects whether `pkgs ? rust-bin`; if not,
  `pkgs.appendOverlays [ rust-overlay-imported ]` and builds a pinned
  `rustPlatform` from `rust-bin.stable.latest.default`.
- `rust-overlay` accepts either a path (npins-style) or an overlay function
  (flake-style) via `builtins.isFunction` dispatch.

`flake.nix` now delegates to `./default.nix` — it pre-applies rust-overlay to
`pkgs` before passing it in, so rust-bin is already present and no double
application happens. This removes ~15 lines of duplicated `callPackage`
wiring from the flake.

The flake-side `overlays.default` still references `self.packages.${system}.*`
(rather than `./nix/overlay.nix`) so flake consumers keep getting pinned
binaries through `inputs.tux-rs.nixosModules.default` without having to pass
`rust-overlay` through themselves.

### Updated README

New "NixOS (classic Nix, no flakes)" section shows:
1. Simple `fetchTarball` + `{ inherit pkgs; }` usage
2. Full npins example with `nixpkgs` + `rust-overlay` pinned via npins
3. Per-package override via `callPackage ./nix/tux-daemon.nix { rustPlatform = ...; }`
