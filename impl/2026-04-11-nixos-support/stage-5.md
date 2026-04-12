# Stage 5: Restructure Nix Packaging (Issue #3 Feedback)

## Context

Initial NixOS support (stages 1–4) inlined every derivation directly into `flake.nix`.
In [issue #3](https://github.com/tonky/tux-rs/issues/3) `balintbarna` pointed out that:

1. `README.md` advertises `github:tuxedocomputers/tux-rs`, but the actual remote is
   `github:tonky/tux-rs`. Both the flake input and the `nix run` example are wrong.
2. Packaging is flake-only. Flakes remain experimental, so users on plain `nix-build`
   / `niv` / `npins` / Hydra pipelines cannot consume the packages at all.
3. Each package should live in its own `.nix` file (TUI, daemon, kernel module).
4. There should be a top-level `default.nix` exposing packages + NixOS modules for
   non-flake consumers.
5. Modules should be function-based so consumers can inject custom dependency
   versions (e.g. swap `rustPlatform`, override `kernel`).
6. Flakes should act as an optional wrapper, avoiding code duplication.

## Objective

Land a non-flake-first Nix packaging layout without regressing flake consumers or the
existing VM test.

## Proposed layout

```
default.nix              # non-flake entry: { pkgs ? import <nixpkgs> {} }: { tux-daemon, tux-tui, overlay, nixosModule }
nix/
  tux-daemon.nix         # { lib, rustPlatform, pkg-config, dbus }: rustPlatform.buildRustPackage { ... }
  tux-tui.nix            # { lib, rustPlatform, pkg-config, dbus }: ...
  tux-kmod.nix           # { lib, stdenv, kernel }: stdenv.mkDerivation { ... }
  overlay.nix            # final: prev: { tux-daemon = final.callPackage ./tux-daemon.nix {}; ... }
  nixos.nix              # NixOS module
flake.nix                # thin wrapper; sources packages from ./nix via callPackage,
                         # exposes devShells + VM check
```

## Design decisions

### Function-based packages via `callPackage`

Each `nix/*.nix` file takes its dependencies as arguments, which lets consumers do:

```nix
pkgs.callPackage ./nix/tux-daemon.nix {
  rustPlatform = myPinnedRustPlatform;
}
```

No code duplication between flake and non-flake paths — both use the same functions.

### Two overlays, one underlying set of derivations

- `nix/overlay.nix` (non-flake path): uses `final.callPackage ./tux-daemon.nix {}`,
  picking up the *ambient* `rustPlatform` from the consumer's nixpkgs. This is the
  normal/expected behaviour for downstream packagers and lets them pin the toolchain
  in their own nixpkgs overlay if they want to.
- `flake.nix` inline overlay: continues to reference `self.packages.${system}.*`
  so flake consumers get matching binaries. Preserves the existing
  behaviour of `imports = [ inputs.tux-rs.nixosModule ]`.

Only the overlay wiring is duplicated — not the derivation code, which is the
"avoid duplication" goal from balintbarna's feedback.

### Source paths

`nix/*.nix` lives one directory below the repo root, so paths become:

- `src = ../.;` (for Rust packages that need the whole workspace)
- `cargoLock.lockFile = ../Cargo.lock;`
- `src = ../tux-kmod;` (for the kernel module package)

Tested via `builtins.filterSource` — nothing tricky, just relative-path bookkeeping.

### VM test

`checks.vmTest` in `flake.nix` keeps using `self.nixosModule` and
`--mock`. No behavioural change.

## Files to touch

### New
- `impl/2026-04-11-nixos-support/stage-5.md` (this file)
- `nix/tux-daemon.nix`
- `nix/tux-tui.nix`
- `nix/tux-kmod.nix`
- `nix/overlay.nix`
- `default.nix` (top-level)

### Modified
- `flake.nix` — trim inline derivations, delegate to `nix/*.nix`
- `README.md` — fix URLs (`tuxedocomputers` → `tonky`) and add a non-flake usage example
- `impl/2026-04-11-nixos-support/plan.md` — add stage-5 to the index
- `impl/2026-04-11-nixos-support/worklog.md` — append stage-5 diary entry

### Unchanged
- `nix/nixos.nix` — module signature is already clean

## Verification

1. `nix build .#tux-daemon` — flake path, pinned rust
2. `nix build .#tux-tui` — flake path, pinned rust
3. `nix-build -A tux-daemon` — non-flake path, ambient rust
4. `nix-build -A tux-tui` — non-flake path, ambient rust
5. `nix flake check` — VM test still passes

## Out of scope

- Hydra jobset config
- Pinning rust toolchain in non-flake mode (callers can do it via their own overlay)
- Submitting upstream to nixpkgs
