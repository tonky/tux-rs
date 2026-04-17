# Stage 3 — TUI profile editor fields for PL1/PL2

## Summary

Added PL1/PL2 TDP fields to the TUI profile editor. Fields are conditionally
shown only when the daemon reports TDP bounds (i.e., a TDP backend is active).
Uses the same `0=Unset → None` convention as CPU frequency fields.

## Changes

### `tux-tui/src/dbus_client.rs`
- Added `CPU_IFACE` constant (`com.tuxedocomputers.tccd.Cpu`).
- New `get_tdp_bounds()` method that calls `GetTdpBounds` on the Cpu interface.

### `tux-tui/src/event.rs`
- New `DbusUpdate::TdpBounds(TdpBounds)` variant.

### `tux-tui/src/dbus_task.rs`
- Fetch TDP bounds at startup (one-time), right after CPU hw limits.
  Empty string from daemon (= no TDP backend) is silently ignored.

### `tux-tui/src/update.rs`
- Handler for `DbusUpdate::TdpBounds` stores in `model.profiles.tdp_bounds`.
- Updated `build_editor_form` call site to pass TDP bounds.

### `tux-tui/src/model.rs`
- `ProfilesState` gains `tdp_bounds: Option<TdpBounds>` field.
- `build_editor_form` takes new `tdp: Option<&TdpBounds>` parameter.
  When `Some`, appends two `Number` fields:
  - `PL1 W (0=Unset)` — max from `tdp.pl1_max`, step 1.
  - `PL2 W (0=Unset)` — max from `tdp.pl2_max`, step 1.
  When `None`, fields are omitted entirely.
- `apply_form_to_profile` handles `tdp_pl1` / `tdp_pl2` keys:
  `0 → None`, `>0 → Some(value)`.
- 4 new tests:
  - `tdp_fields_hidden_without_bounds`
  - `tdp_fields_shown_with_bounds` (also checks max bounds)
  - `tdp_fields_roundtrip`
  - `tdp_zero_maps_to_none`

## Quality gates
- `cargo check` — clean.
- `cargo clippy -p tux-core -p tux-daemon` — clean.
  (Pre-existing `collapsible_match` in tux-tui unrelated to this change.)
- `cargo fmt -- --check` — clean.
- `cargo test -p tux-core -p tux-daemon -p tux-tui` — all pass.
