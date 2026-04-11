# Stage 1: Packaging Rust Components

## Objective
Define `tux-daemon` and `tux-tui` in `flake.nix` using `rustPlatform.buildRustPackage`.

## Plan
1. Create `flake.nix` in the root of the repository.
2. Setup basic flake structure with `nixpkgs` and `rust-overlay` (optional, can use standard rustPlatform).
3. Define derivations for the two Rust crates.
4. Test the build with `nix build .#tux-daemon` and `nix build .#tux-tui`.

## References
- `tux-daemon/Cargo.toml`
- `tux-tui/Cargo.toml`
