# Worklog Stage 2

## 2026-04-12

- Implemented debug-tagged event support and filter state in TUI model.
- Added `D` key binding to toggle debug filter in Event Log.
- Updated Event Log renderer to hide debug entries by default and show full stream in debug mode.
- Added richer command and save-event details with numeric values.
- Added fan-change telemetry events including duty/rpm/temp context.
- Added tests for debug toggle and detailed save/fan event behavior.
- Validation: `cargo test -p tux-tui` passed (132 tests).
