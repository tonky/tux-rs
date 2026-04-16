# Stage 2: Explicit Form Field Keys

## What Changes

Eliminate label-to-TOML-key normalization by making `FormField.key` mandatory.

### `tux-tui/src/model.rs`
- Update `FormField`: `key: Option<String>` → `key: String`
- Update all 30+ `FormField` initializations with explicit keys.
- Use current normalization logic for default keys:
  - "Name" → "name"
  - "Min Speed (%)" → "min_speed_percent"
  - "Fan Control" → "fan_control"
  - etc.

### `tux-tui/src/update.rs`
- Simplify `serialize_form_to_toml`: remove `if let Some(ref k) = field.key` block, use `field.key` directly.
- Simplify `load_form_from_toml`: remove key derivation logic, use `field.key` directly.
- Update unit tests in `update.rs` to use explicit keys.

## Verification Plan

### Automated Tests
- `cargo test -p tux-tui`
- Verify `charging_form_loads_from_toml_keys` specifically.
- Add a new test in `update.rs` to verify that serialization uses the explicit key, not the label.

### Manual Verification
1. Launch `tux-tui`.
2. Navigate to Settings/Keyboard/Webcam tabs.
3. Verify data loads (proves `load_form_from_toml` works with new keys).
4. Modify a value and save.
5. Verify daemon receives the change (proves `serialize_form_to_toml` works).
