# Review 1: Stage 1 (BoundedIndex Wrapper)

## Status: APPROVED

### a) Conformance to Specification
- **Implementation:** `BoundedIndex` correctly implements `next`, `prev`, and `clamp_to` with the specified "length-per-call" strategy.
- **Integration:** Successfully replaced raw `usize` in `ProfilesState`, `WebcamState`, and `FanCurveState`.
- **Simplification:** `update.rs` shows significant reduction in boilerplate for clamping (e.g., in `DbusUpdate::ProfileList`).

### b) Improvements & Refactoring
- **Form State:** Leaving `Form.selected_index` as `usize` is correct for now, as its `select_next/prev` methods involve complex "skip-disabled" logic that doesn't fit the pure modulo wrapping of `BoundedIndex`.
- **FieldType::Select:** The `selected` field within `FieldType::Select` (used for dropdowns) still uses manual modulo logic in `model.rs`. While out of scope for Stage 1, this is a prime candidate for `BoundedIndex` in a future stage.
- **Log Commands:** In `update.rs:168`, `log_commands` still uses `.min(points.len().saturating_sub(1))`. While safe, this could eventually be unified if the command structure passed a `BoundedIndex` instead of a raw index.

### c) Edge Cases
- **Empty Collections:** The implementation is robust. `next` and `prev` are no-ops when `len == 0`, and `clamp_to(0)` resets the index to `0`. This prevents "index out of bounds" panics during asynchronous data updates where a list might momentarily be empty.
- **Shrinking Collections:** `clamp_to` correctly handles the case where the selected index becomes invalid after a collection shrinks (e.g., deleting the last item), which was a previous source of manual clamping bugs.

### d) Code Style and Idiomatic Rust
- The abstraction is surgical and clean. It correctly handles `usize` wrapping and empty collections.
- The design allows for easy expansion (e.g., non-wrapping movement) and is decoupled from the collection size, which is ideal for the TUI's reactive data model.
- Excellent unit test coverage for edge cases, specifically `len == 0`.
- The refactor is applied uniformly across the model, update, and view layers.
