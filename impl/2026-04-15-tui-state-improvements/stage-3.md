# Stage 3: Centralized Text-Edit State

## What Changes

Centralize the knowledge of "is any form currently editing a text field" into a single field on the `Model`. This eliminates the need for the 7-branch `is_inline_text_edit_active` check on every key press.

### `tux-tui/src/model.rs`
-   Add `pub editing_text_in: Option<Tab>` to `Model`.
-   Initialize it to `None` in `Model::new()`.

### `tux-tui/src/update.rs`
-   Refactor key handlers to return `(Vec<Command>, bool)` where `bool` is the current state of text editing for that tab:
    -   `handle_text_edit_key` -> `bool`
    -   `handle_form_tab_key` -> `(Vec<Command>, bool)`
    -   `handle_webcam_key` -> `(Vec<Command>, bool)`
    -   `handle_profiles_key` -> `(Vec<Command>, bool)`
    -   `handle_profiles_editor_key` -> `(Vec<Command>, bool)`
-   `dispatch_tab_key` -> `(Vec<Command>, bool)`:
    -   Updates `model.editing_text_in = if editing { Some(model.current_tab) } else { None };`
-   `handle_key`:
    -   Replace `if is_inline_text_edit_active(model)` with `if model.editing_text_in.is_some()`.
-   Delete `is_inline_text_edit_active`.

## Verification Plan

### Automated Tests
-   `cargo test -p tux-tui`
-   Add a new test in `update.rs` to verify that while `editing_text_in` is `Some`, global hotkeys (like tab switching) are ignored.
-   Add a test to verify that `editing_text_in` is correctly set after `KeyCode::Enter` on a Text field, and cleared after `KeyCode::Esc` or `KeyCode::Enter`.
