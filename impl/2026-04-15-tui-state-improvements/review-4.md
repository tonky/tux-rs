# Review 4: Stage 4 (Form Dirty-Data Warnings)

## Status: APPROVED

### a) Conformance to Specification
- **Implementation:** `handle_data` now checks the `.dirty` flag for all form-backed tabs and the fan curve.
- **Robustness:** Daemon updates are correctly skipped when local unsaved changes exist, preventing accidental data loss.
- **UX:** Added `status_message` to `FanCurveState` and updated its view to display warnings, ensuring consistency across all editable tabs.

### b) Correctness of Logic
- All `DbusUpdate` variants that refresh form data (`SettingsData`, `KeyboardData`, `ChargingData`, `PowerData`, `DisplayData`, `WebcamData`, `FanCurve`, `ProfileList`) are correctly guarded.
- Status messages are set only when updates are skipped due to local changes.
- Debug events are logged for all skipped updates to assist in troubleshooting.

### c) Testing
- Added `daemon_updates_skipped_when_form_dirty` test case which exhaustively verifies the skip logic for every affected tab.
- Verified that existing tests still pass, ensuring no regressions in normal data loading.

### d) Code Quality
- Centralized `status_message` handling in `handle_data`.
- Idiomatic use of `let` chains for complex state matching (e.g., Profile Editor).
- Clean separation between data loading and user warning logic.
