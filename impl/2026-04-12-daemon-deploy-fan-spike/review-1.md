# Review 1: Daemon Deploy Fan Spike

## Result

- Two independent review passes found the code change coherent with the existing startup/shutdown/profile-apply flow.
- No blocking code issues were identified in the removal itself.

## Findings

- Remaining high-priority risk is hardware-only validation on real Uniwill firmware during `just deploy-daemon`.
- Unit tests and linting cannot prove EC behavior during the stop/start window.

## Validation

- `cargo test -p tux-daemon auto_switch_`
- `cargo test -p tux-daemon`
- `cargo fmt --all --check`
- `cargo clippy -p tux-daemon -- -D warnings`