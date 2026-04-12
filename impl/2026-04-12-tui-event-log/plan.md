# High-Level Plan: TUI Event Log

## Stage 1: TUI event log model and rendering

- add an in-memory rolling event log to the TUI model
- define a structured log entry type with timestamp, source, summary, and optional detail
- add a new Event Log tab/view with follow mode enabled by default
- bind `l` to jump to the Event Log view
- log key state transitions in the update layer, including what changed and why

## Stage 2: Debug filter and high-detail events

- add a toggleable debug filter (default off) for Event Log
- keep normal mode concise while allowing full-detail/debug events when enabled
- enrich user and daemon log entries with numeric values (brightness %, fan %, temp C, charging profile/priority, thresholds)
- log fan change events with explicit duty/rpm/temperature context
- add tests for toggle behavior and enriched event output paths

## Open design choices

- tab vs overlay:
  - recommendation: a dedicated tab, because it is simpler, testable, and consistent with current TUI structure
- log detail granularity:
  - recommendation: log meaningful model mutations and outgoing commands, not every raw tick
- follow mode controls:
  - recommendation: follow by default, with a toggle key in the view for manual scroll later if needed