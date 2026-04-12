# Stage 1: TUI Event Log Model and View

## Objective

Implement the core event log inside `tux-tui` without changing daemon APIs.

## Planned changes

- `tux-tui/src/model.rs`
  - add `Tab::EventLog`
  - add `EventLogState`
  - add `EventLogEntry`
  - keep a bounded rolling buffer

- `tux-tui/src/update.rs`
  - append log entries when model state changes in meaningful ways
  - append log entries when commands are emitted from key actions

- `tux-tui/src/view.rs`
  - render the new tab in the tab bar and route tab content
  - add `l` global keybinding to jump to Event Log
  - update help overlay

- `tux-tui/src/views/`
  - add a dedicated event log renderer

## Proposed logging scope for first pass

- connection state changes
- dashboard telemetry updates that materially change displayed values
- fan health changes
- profile operations and active-profile changes
- form save success/failure
- user-triggered commands from key bindings

## Verification

- unit tests for `l` binding and tab selection
- tests for bounded log retention
- render test for the new tab
- `just test`
- `just ci`