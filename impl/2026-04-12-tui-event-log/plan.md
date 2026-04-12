# High-Level Plan: TUI Event Log

## Stage 1: TUI event log model and rendering

- add an in-memory rolling event log to the TUI model
- define a structured log entry type with timestamp, source, summary, and optional detail
- add a new Event Log tab/view with follow mode enabled by default
- bind `l` to jump to the Event Log view
- log key state transitions in the update layer, including what changed and why

## Stage 2: CLI/headless access and debug coverage

- add a CLI flag to dump recent event log entries in JSON or plain text
- decide whether CLI mode should read a live in-process log snapshot only, or synthesize a trace from one polling cycle
- add tests for key binding, model log retention, follow mode behavior, and CLI parsing/output

## Open design choices

- tab vs overlay:
  - recommendation: a dedicated tab, because it is simpler, testable, and consistent with current TUI structure
- log detail granularity:
  - recommendation: log meaningful model mutations and outgoing commands, not every raw tick
- follow mode controls:
  - recommendation: follow by default, with a toggle key in the view for manual scroll later if needed