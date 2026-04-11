# Worklog: NixOS Support

## Setup
- **Objective:** Provide native NixOS support, flake packages, and VM testing.
- **Started:** 2026-04-11

## Diary
- Defined implementation plan with 4 stages.
- Created tracking directory and initialized documents.
- **Stage 1 (Rust Packaging):**
    - Created `flake.nix` with `tux-daemon` and `tux-tui`.
    - Integrated `rust-overlay` for stable Rust toolchain.
    - Verified builds of both packages.
    - Note: Disabled `doCheck` for now as E2E tests require D-Bus in the sandbox which needs further configuration.
- **Stage 2 (Kernel Module Packaging):**
    - Created `tux-kmod` derivation in `flake.nix` using `stdenv.mkDerivation`.
    - Configured it to build against a provided kernel.
    - Verified build against `linuxPackages_latest`.
- **Stage 3 (NixOS Module and D-Bus):**
    - Created `nixos/default.nix` with options for daemon, TUI, and kernel modules.
    - Configured systemd service and D-Bus policy integration.
    - Exposed module as `nixosModules.default` in `flake.nix`.
    - Implemented a default overlay to provide `tux-*` packages to the module.
    - Updated `tux-daemon` derivation to install D-Bus policy file.
- **Stage 4 (Automated Local VM Testing):**
    - Implemented a `--mock` mode in `tux-daemon` for testing without hardware.
    - Added `checks.vmTest` to `flake.nix` using `nixosTest` framework.
    - Verified daemon startup, D-Bus registration, and TUI integration inside a NixOS VM.
    - Successfully validated the entire NixOS integration on the local dev machine.
- **Stage 5 (Packaging restructure — issue #3 feedback):**
    - Fixed README URLs: `tuxedocomputers/tux-rs` → `tonky/tux-rs` (flake input + `nix run` example).
    - Extracted per-package derivations into `nix/tux-daemon.nix`, `nix/tux-tui.nix`, `nix/tux-kmod.nix` as `callPackage`-style functions.
    - Added `nix/overlay.nix` (non-flake overlay using `final.callPackage`).
    - Added top-level `default.nix` exposing `{ tux-daemon, tux-tui, tux-kmod, overlays, nixosModules }` for classic Nix consumers.
    - Rewrote `flake.nix` as a thin wrapper around `nix/*.nix`; rust-overlay-pinned toolchain is injected via `pkgs.callPackage { inherit rustPlatform; }`.
    - Validated via `nix eval` + `nix flake show`; VM test still evaluates.
    - See `stage-5.md` and `worklog-5.md` for details.




