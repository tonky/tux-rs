# Stage 2 — Backend selection + profile wiring

## Summary

Added `TdpSource` enum and `build_backend()` factory so the daemon picks
the right TDP backend per device row. Selection is strictly opt-in —
no blind RAPL probing.

## Changes

### `tux-core/src/device.rs`
- New `TdpSource` enum: `None`, `Ec`, `Rapl`.
- New field `tdp_source: TdpSource` on `DeviceDescriptor`.
- All test constructors updated.

### `tux-core/src/device_table.rs`
- Every device row and all 5 fallback descriptors now have
  `tdp_source: TdpSource::None`.

### `tux-core/src/custom_device.rs`
- `CustomDeviceDescriptor` gets `tdp_source: TdpSource` field +
  forwarding in `leak()`.

### `tux-daemon/src/cpu/tdp.rs`
- New `pub fn build_backend(descriptor: &DeviceDescriptor) -> Option<Arc<dyn TdpBackend>>`.
  - `TdpSource::None` → `None`.
  - `TdpSource::Ec` → requires `descriptor.tdp` bounds; tries `EcTdp::new`.
  - `TdpSource::Rapl` → `RaplTdp::probe()`; bounds from firmware. No fallthrough.
- Updated module doc comment.
- 4 new factory tests:
  - `factory_none_returns_none`
  - `factory_none_ignores_bounds`
  - `factory_ec_without_bounds_returns_none`
  - `factory_rapl_without_sysfs_returns_none`

### `tux-daemon/src/main.rs`
- Replaced 17-line inline EC-only construction with one-liner:
  `cpu::tdp::build_backend(device.descriptor)`.

### `tux-daemon/src/dbus/settings.rs`
- Test helper updated with `tdp_source` field.

## Quality gates
- `cargo check` — clean.
- `cargo clippy -p tux-core -p tux-daemon --all-targets -- -D warnings` �� clean.
- `cargo fmt -- --check` — clean.
- `cargo test -p tux-core -p tux-daemon` — all pass (375+ tests).
