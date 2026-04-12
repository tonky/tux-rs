# Worklog: TUI Event Log

## 2026-04-12
- Inspected current TUI architecture (`cli.rs`, `event.rs`, `model.rs`, `update.rs`, `view.rs`, `main.rs`).
- Identified a good first-pass integration point: keep the log entirely in `tux-tui` model/update/view layers, without daemon changes.
- Proposed a dedicated Event Log tab plus `l` shortcut, with follow mode enabled by default.
- Deferred implementation pending stage confirmation.