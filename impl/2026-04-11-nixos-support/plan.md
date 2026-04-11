# Plan: NixOS Support (Refined)

This plan details the addition of NixOS support to `tux-rs` following the repository workflow in `AGENTS.md`.

## Objective
Provide a seamless NixOS integration for `tux-rs` with automated local testing (executable on any Linux distro, like Ubuntu, using Nix) and easy usage from any NixOS machine via Flakes.

## Key Files & Context
- `flake.nix`: Root entry point for Nix, exposing packages, the NixOS module, and VM tests.
- `nixos/default.nix`: The NixOS module definition.
- `impl/2026-04-11-nixos-support/`: Directory for tracking implementation progress.

## Proposed Solution

### 1. `flake.nix`
Create a `flake.nix` that:
- Uses `nixpkgs` and `rust-overlay`.
- Provides `packages`:
    - `tux-daemon`: The Rust daemon.
    - `tux-tui`: The terminal UI.
    - `tux-kmod`: Derivation building all kernel modules.
- Provides a `nixosModules.default` (or `tux-daemon`) that exports the NixOS module.
- Provides a `devShell` for developers.
- Supports being run directly via `nix run github:tuxedocomputers/tux-rs#tux-tui`.

### 2. NixOS Module (`nixos/default.nix`)
A module with options to:
- Enable `tux-daemon` service.
- Configure kernel module loading.
- Integrate with D-Bus.

### 3. Automated Local Testing
Use the NixOS testing framework (`nixosTests`) exposed via Flake `checks` to:
- Launch a NixOS VM locally (this runs seamlessly on the developer's Ubuntu machine via Nix).
- Install the `tux-daemon` and `tux-tui`.
- Verify the service is running.
- Verify D-Bus accessibility.
- Test the TUI inside the VM by executing `tux-tui --json` (which acts as a headless test validating D-Bus communication and daemon state dumping).
- (No CI integration required for this NixOS test as per user request).

### 4. Workflow Compliance
Create the following files in `impl/2026-04-11-nixos-support/`:
- `description.md`: Feature overview.
- `plan.md`: High-level plan (matching this one).
- `worklog.md`: Implementation diary.
- `stage-1.md`: Packaging Rust components.
- `stage-2.md`: Packaging Kernel modules.
- `stage-3.md`: NixOS module and D-Bus integration.
- `stage-4.md`: Automated local VM testing.
- `follow_up.toml`: Tracking future tasks.

## Implementation Steps

### Stage 1: Packaging Rust components
- Define `tux-daemon` and `tux-tui` in `flake.nix` using `rustPlatform.buildRustPackage`.
- Verify build with `nix build .#tux-daemon` and `nix build .#tux-tui`.

### Stage 2: Packaging Kernel modules
- Define `tux-kmod` in `flake.nix` using `linux.moduleBuild`.
- Verify build against a default kernel version.

### Stage 3: NixOS Module and D-Bus
- Create `nixos/default.nix`.
- Integrate D-Bus policy from `dist/com.tuxedocomputers.tccd.conf`.
- Define systemd service.

### Stage 4: Automated Local VM Testing
- Add `checks.x86_64-linux.vmTest` to `flake.nix`.
- Define a Python test script (`machine.succeed(...)`) to verify daemon startup and D-Bus registration inside the NixOS VM.
- Execute `tux-tui --json` (or other dump commands like `--dump-dashboard`) within the VM test to parse and validate the daemon's JSON response, ensuring full end-to-end integration works headless.

## Verification & Testing
- Automated `nix build` of all packages locally.
- Execute local VM tests via `nix build .#checks.x86_64-linux.vmTest` to validate the NixOS module locally on Ubuntu.
