# Stage 2 — Make sd-notify optional via cargo feature

## Changes

### 1. `Cargo.toml` (workspace root)
- Keep `sd-notify` in workspace dependencies (unchanged)

### 2. `tux-daemon/Cargo.toml`
- Add `systemd` feature (default) that enables `sd-notify`
- Change `sd-notify` dependency to `optional = true`

### 3. `tux-daemon/src/main.rs`
- Gate all 3 sd-notify call sites with `#[cfg(feature = "systemd")]`:
  - Line 461: `sd_notify::notify(&[NotifyState::Ready])`
  - Lines 465-473: watchdog ping loop
  - Line 502: `sd_notify::notify(&[NotifyState::Stopping])`
