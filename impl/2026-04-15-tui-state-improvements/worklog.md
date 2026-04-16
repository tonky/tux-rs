# Worklog

## 2026-04-15 — Planning

- Investigated Statum typestate library as potential Elm replacement
- Concluded incompatible: Elm needs single Model type, Statum makes each state a different type; transitions consume self vs &mut; TUI states aren't lifecycle phases
- Identified 4 concrete improvements from the investigation
- Created 4-stage plan ordered by risk

## 2026-04-15 — Stage 1: BoundedIndex Wrapper

- Created `BoundedIndex` type in `tux-tui/src/bounded_index.rs`
- Replaced `selected_index: usize` in ProfilesState, FanCurveState, WebcamState
- Replaced manual clamping in update.rs with `.clamp_to(len)` calls
- Updated all view files and tests to use `.get()`/`.set()`
- `Form.selected_index` intentionally left as `usize` (skip-disabled logic)
- All 152 tests pass, clippy clean, fmt clean
- Reviews launched and completed: Status APPROVED (see review-1.md)
- Created follow_up.toml with 2 minor items
- Stage 1 COMPLETE

## 2026-04-15 — Stage 2: Explicit Form Field Keys

- Drafted stage-2.md
- Updated `FormField` with mandatory `key: String`
- Updated 30+ form initializations in `model.rs` and tests
- Refactored `serialize_form_to_toml` and `load_form_from_toml` to use keys
- Refactored `summarize_` functions in `update.rs` with key-based helpers
- Aligned Charging and Display form keys with D-Bus/Daemon expectations
- Refactored `apply_form_to_profile` to use keys
- Fixed keyboard capability logic to use keys
- Cleanup: removed many `#[allow(dead_code)]` attributes
- All tests pass, clippy clean, fmt clean
- Reviews completed: Status APPROVED (see review-2.md)
- Stage 2 COMPLETE

## 2026-04-15 — Stage 3: Centralized Text-Edit State

- Drafted stage-3.md
- Added `editing_text_in: Option<Tab>` to `Model`
- Refactored all tab key handlers to return `(Vec<Command>, bool)` indicating edit state
- Updated `dispatch_tab_key` to manage `model.editing_text_in` automatically
- Simplified `handle_key` global hotkey suppression logic
- Deleted redundant `is_inline_text_edit_active` helper
- Implemented `Default` for `TuxProfile` and related settings in `tux-core`
- Added comprehensive unit test for hotkey suppression
- All 153 tests pass, clippy clean, fmt clean
- Reviews completed: Status APPROVED (see review-3.md)
- Stage 3 COMPLETE

## 2026-04-15 — Stage 4: Form Dirty-Data Warnings

- Drafted stage-4.md
- Implemented dirty checks in `handle_data` for all form-backed tabs and fan curve
- Added `status_message` to `FanCurveState` and updated its view
- Implemented warning messages when daemon updates are skipped
- Added comprehensive unit test for update skip logic
- All 154 tests pass, clippy clean, fmt clean
- Reviews completed: Status APPROVED (see review-4.md)
- Stage 4 COMPLETE
- FEATURE COMPLETE
