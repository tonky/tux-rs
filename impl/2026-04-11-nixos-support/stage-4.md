# Stage 4: Automated Local VM Testing

## Objective
Create an automated test suite verifying the daemon and TUI in a NixOS VM.

## Plan
1. Add a `checks.<system>.vmTest` to `flake.nix` using `nixosTests`.
2. Define a NixOS system running the module.
3. Write a Python test script interacting with the VM.
4. Ensure the test calls `machine.succeed("tux-tui --json")` and verifies the JSON output.
5. Run the test with `nix flake check` or `nix build .#checks.x86_64-linux.vmTest`.

## References
- `tux-tui/src/cli.rs`
