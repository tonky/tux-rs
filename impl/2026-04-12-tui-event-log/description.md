# Feature: TUI Event Log

Add an in-app event log to `tux-tui` for debugging model changes and their causes.

Goals:
- show a rolling log of significant state changes and commands
- make it accessible from the TUI via `l`
- default to follow mode so the newest entries stay visible
- expose the same log through a CLI/headless mode for debugging and bug reports

Non-goals for the first pass:
- persistent on-disk log storage
- daemon-side journald integration
- full structured tracing across process boundaries