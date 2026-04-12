# Worklog: TUI Event Log

## 2026-04-12
- Inspected current TUI architecture (`cli.rs`, `event.rs`, `model.rs`, `update.rs`, `view.rs`, `main.rs`).
- Identified a good first-pass integration point: keep the log entirely in `tux-tui` model/update/view layers, without daemon changes.
- Proposed a dedicated Event Log tab plus `l` shortcut, with follow mode enabled by default.
- Deferred implementation pending stage confirmation.

## Stage 1 — TUI Event Log model and rendering (completed)
- Added `Tab::EventLog` and included it in tab rotation/order.
- Added in-memory rolling event log state in `tux-tui/src/model.rs`:
	- `EventLogState` with bounded `VecDeque` retention.
	- structured `EventLogEntry` (`timestamp`, `source`, `summary`, optional `detail`).
	- `Model::log_event(...)` helper and startup event entry.
- Added new Event Log view:
	- `tux-tui/src/views/event_log.rs`
	- module export in `tux-tui/src/views/mod.rs`
	- routing in `tux-tui/src/view.rs`
- Added `l` keybinding to jump directly to Event Log tab and updated help/status hints.
- Added meaningful event logging in `tux-tui/src/update.rs`:
	- command emissions from key actions,
	- connection status transitions,
	- fan health changes,
	- material dashboard changes (temp delta/profile/power state),
	- profile operation success/failure,
	- form save success/failure.
- Added/updated tests:
	- bounded retention test,
	- `l` keybinding/tab selection + log entry assertion,
	- event-log render test,
	- tab-wrap expectations adjusted for new tab order.
- Validation:
	- `cargo test -p tux-tui` (129 passed, 0 failed)
	- `just test` (workspace tests passed)
	- `just ci` (fmt/clippy/check/test pipeline passed)

## Stage 2 — Debug filter and value-rich events (completed)
- Added toggleable debug filter to Event Log (`D` key), disabled by default.
- Added debug event level support in model (`show_debug_events`, debug-tagged entries).
- Event Log view now filters debug events in normal mode and shows all events in debug mode.
- Added richer event detail payloads with numeric values:
	- keyboard brightness and mode
	- display brightness
	- charging profile/priority and thresholds
	- power TGP offset
	- fan curve point/value details
- Added fan-change events from telemetry with duty/rpm and CPU temperature context.
- Added debug telemetry fan events that are visible only when debug mode is enabled.
- Updated help/status hints for debug toggle discoverability.
- Validation:
	- `cargo test -p tux-tui` (132 passed, 0 failed)