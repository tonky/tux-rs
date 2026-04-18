# Stage 4 — Device table + docs + live tests

## Summary

Enabled Intel RAPL TDP control on the two vendor-sanctioned Gen8 Intel SKUs,
updated docs, and added hermetic RAPL integration tests to `profile_apply`.

## Changes

### `tux-core/src/device_table.rs`
- `IBP14I08MK2`: `tdp_source: TdpSource::None` → `TdpSource::Rapl`
- `IBP16I08MK2`: `tdp_source: TdpSource::None` → `TdpSource::Rapl`
- All other rows remain `TdpSource::None`.

### `tux-daemon/src/cpu/tdp.rs`
- `RaplTdp::probe_at` promoted from private to `pub(crate)` so the two new
  profile-apply integration tests can construct a hermetic RAPL backend.
- `factory_rapl_without_sysfs_returns_none` annotated
  `#[cfg_attr(target_os = "linux", ignore)]`: on real Linux hardware the
  genuine RAPL sysfs tree is present, so the test was falsely failing.
  The absent-path contract is already covered hermetically by
  `rapl_probe_missing_dir_returns_none`.

### `tux-daemon/src/profile_apply.rs`
- `apply_rapl_tdp_roundtrip`: sets PL1=25 / PL2=40 via `ProfileApplier::apply`
  backed by a `RaplTdp` pointed at a tempfile tree; asserts get_{pl1,pl2}
  return the written values.
- `apply_rapl_tdp_none_is_noop`: profile with `tdp: None` must not overwrite
  the backend; asserts original sysfs values (15 W / 28 W) survive.

### `docs/hardware_support.md`
- TDP row updated to mention `Intel RAPL (PL1/PL2)` alongside EC.
- New "TDP (RAPL) opt-in policy" section lists `IBP16I08MK2` and
  `IBP14I08MK2` as the only opted-in SKUs, and explicitly excludes Gen9
  AMD SKUs.

### `Justfile`
- New `live-test-tdp` recipe running the six RAPL-specific hermetic tests.
  No daemon or real hardware required.

## Quality gates
- `cargo fmt --all -- --check` — clean.
- `cargo clippy --workspace --tests -- -D warnings` — clean.
- `cargo test -p tux-core -p tux-daemon -p tux-tui` — 378 daemon lib tests
  + full workspace pass; 1 ignored (factory_rapl_without_sysfs_returns_none
  on Linux, deliberate).
