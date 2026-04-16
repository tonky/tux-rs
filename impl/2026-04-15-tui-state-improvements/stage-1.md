# Stage 1: BoundedIndex Wrapper

## What Changed

New type `BoundedIndex` replaces raw `usize` indices in 3 state types:

### New file: `tux-tui/src/bounded_index.rs`
- `BoundedIndex(usize)` with `new`, `get`, `set`, `next(len)`, `prev(len)`, `clamp_to(len)`
- `len` passed per-call (not stored) since backing collections change independently
- 10 unit tests covering wrapping, clamping, empty-len safety

### Updated state types in `model.rs`
- `ProfilesState.selected_index: usize` → `BoundedIndex`
- `FanCurveState.selected_index: usize` → `BoundedIndex`
- `WebcamState.selected_device: usize` → `BoundedIndex`
- `select_next/select_prev` methods now delegate to `BoundedIndex`
- `FanCurveState.delete_point()` uses `clamp_to()` instead of manual clamping

### Updated `update.rs`
- Profile list clamping (5 lines) → `model.profiles.selected_index.clamp_to(len)` (1 line)
- Webcam device clamping (5 lines) → `model.webcam.selected_device.clamp_to(len)` (1 line)
- `log_commands` uses `.get()` for fan curve index

### Updated views
- `fan_curve.rs`, `profiles.rs`, `webcam.rs` — `.selected_index` reads → `.selected_index.get()`

### NOT changed (by design)
- `Form.selected_index` stays `usize` — has special skip-disabled-field navigation

## Test results
- All 152 tests pass
- clippy clean
- fmt clean
