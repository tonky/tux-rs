# Stage 2: Debug Filter and Value-Rich Event Logging

## Objective

Add a toggleable debug filter (off by default) and improve event payload detail so operators can switch from concise logs to full telemetry detail during troubleshooting.

## Scope

- add debug-tagged events in model
- add Event Log debug filter toggle (`D`)
- default to filtered mode (hide debug events)
- show all debug events when enabled
- enrich event messages with values for:
  - keyboard brightness/mode
  - display brightness
  - charging profile/priority/thresholds
  - power TGP offset
  - fan curve points
  - fan duty/rpm/temp change telemetry

## Files

- `tux-tui/src/model.rs`
- `tux-tui/src/update.rs`
- `tux-tui/src/views/event_log.rs`
- `tux-tui/src/view.rs`

## Validation

- `cargo test -p tux-tui`
