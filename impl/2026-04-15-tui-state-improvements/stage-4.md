# Stage 4: Form Dirty-Data Warnings

## What Changes

Prevent the daemon from overwriting unsaved TUI edits without warning. If a tab has "dirty" changes (unsaved) and the daemon pushes new data for that tab, we skip the load and set a status message warning the user.

### `tux-tui/src/update.rs`
- Update `handle_data` for the following `DbusUpdate` variants:
    - `SettingsData`: Check `model.settings.form.dirty`.
    - `KeyboardData`: Check `model.keyboard.form.dirty`.
    - `ChargingData`: Check `model.charging.form.dirty`.
    - `PowerData`: Check `model.power.form_tab.form.dirty`.
    - `DisplayData`: Check `model.display.form.dirty`.
    - `WebcamData`: Check `model.webcam.form_tab.form.dirty`.
    - `FanCurve`: Check `model.fan_curve.dirty`.
    - `ProfileList`: Check if Profile Editor is open and `form.dirty`.

- If dirty:
    - Do NOT call `load_form_from_toml` / `load_curve`.
    - Set `state.status_message = Some("Daemon update skipped (unsaved changes)".into())`.
    - Log a debug event: `model.log_debug_event(...)`.

- Special case: `ProfileList`
    - If the user is currently editing a profile (`ProfilesMode::Editor`) and that profile's form is dirty, and the `ProfileList` contains an update for that specific profile ID:
        - We can't easily "skip" just one profile in the list, but we can prevent the editor from being refreshed with new data if it's dirty.
        - Currently `ProfileList` doesn't seem to update the editor form anyway (it only updates the list and checks if the edited profile was deleted). I should verify this.

## Verification Plan

### Automated Tests
- `cargo test -p tux-tui`
- Add a new test `update_skipped_when_dirty` in `update.rs`:
    1. Set a form field to dirty.
    2. Dispatch a `DbusUpdate` for that form.
    3. Verify `form.dirty` is still true and original value is preserved.
    4. Verify `status_message` contains the warning.

### Manual Verification (Simulated)
- Since we aren't on hardware, the automated tests are the primary verification.
