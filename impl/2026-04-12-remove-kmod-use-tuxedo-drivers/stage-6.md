# Stage 6: Cleanup & validation

## Context
Perform standard cleanup of obsolete backend code from `tux-daemon` that solely existed for interaction with `tux-kmod`. Validating real-life tests on standard supported hardware (specifically InfinityBook Pro 16 Gen 8).

## Files to modify
- **[DELETE] `tux-daemon/src/platform/nb05.rs`**: (Original version accessing EC directly)
- **[DELETE] `tux-daemon/src/platform/clevo.rs`**: (Original version)
- **[DELETE] `tux-daemon/src/platform/uniwill.rs`**: (Original version)
- **[DELETE] `tux-daemon/src/platform/tuxi.rs`**: (Original version)

## Details
- Remove dead references in `PlatformRegisters`.
- Run formatting `cargo fmt` and strict clippy loops as usual via `just check`.
