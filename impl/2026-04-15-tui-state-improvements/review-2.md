# Review 2: Stage 2 (Explicit Form Field Keys)

## Status: APPROVED

### a) Conformance to Specification
- **Implementation:** `FormField.key` is now a mandatory `String`.
- **Consistency:** All 30+ form initializations in `model.rs` and tests have been updated with explicit keys.
- **Simplification:** `serialize_form_to_toml` and `load_form_from_toml` in `update.rs` now use `field.key` directly, removing all derivation/normalization logic.
- **Robustness:** `apply_form_to_profile` in `model.rs` was refactored to use key-based matching instead of label-based matching.

### b) Correctness of Keys
- D-Bus compatibility verified for Charging and Display forms.
- Keys correctly align with `ChargingSettingsResponse` and `DisplayState` in `tux-core`.
- Unit tests `charging_form_loads_from_toml_keys` and `charging_saved_logs_profile_values` pass with new keys.

### c) Code Quality
- Refactored `summarize_` functions in `update.rs` to use new `form_string`, `form_int`, and `form_bool` helpers.
- Code is cleaner and less brittle to label changes (e.g., adding unit symbols to labels no longer breaks D-Bus serialization).
- Keyboard capability logic in `update.rs` (DbusUpdate::Capabilities) refactored to use keys.

### d) Cleanup
- Removed `#[allow(dead_code)]` from `Form`, `FormField`, and `FieldType`.
- Performed general dead-code cleanup in `BoundedIndex`, `Command`, and `DbusUpdate`.
- *Note:* Some minor dead code warnings remain in variants/methods that are legitimately unused in the current test suite or by design (e.g., `BoundedIndex::new` when `default()` is preferred).
