# Stage 3 — CI check for non-systemd build

## Task

Add a `cargo check` and `cargo clippy` for the `--no-default-features --features tcc-compat` combination to the `ci` recipe. This prevents regressions where someone adds an ungated `sd_notify` call.

## Changes

### `justfile`
Add to the `ci` recipe:
```
cargo check -p tux-daemon --no-default-features --features tcc-compat
cargo clippy -p tux-daemon --no-default-features --features tcc-compat -- -D warnings
```
