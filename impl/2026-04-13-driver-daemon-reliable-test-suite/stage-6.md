# Stage 6 (Bonus): TUI Runtime Controls and Optional API Additions

## Objective

As a second bonus stage, extend Uniwill TUI runtime controls after core reliability
stages are complete and accepted.

## Scope

- Add fn-lock status/toggle UX in TUI.
- Add keyboard metadata display and multi-zone control UX.
- Add daemon API fields only if required for these bonus controls.

## Target Files

- tux-tui/src/dbus_client.rs
- tux-tui/src/dbus_task.rs
- tux-tui/src/event.rs
- tux-tui/src/command.rs
- tux-tui/src/model.rs
- tux-tui/src/update.rs
- tux-tui/src/views/keyboard.rs
- tux-tui/src/views/info.rs
- tux-core/src/dbus_types.rs
- tux-daemon/src/dbus/settings.rs
- tux-daemon/src/dbus/charging.rs

## Tasks

1. Wire fn-lock polling and toggle commands through TUI state transitions.
2. Surface keyboard hardware details from daemon metadata.
3. Add per-zone control UX with capability-safe fallbacks.
4. If needed, add additive capability and option fields to daemon payloads.
5. Add focused tests for new reducers, D-Bus parsing, and live regression flows.

## Risks

- Bonus scope can grow quickly; keep runtime controls focused.
- Added API fields must remain additive and default-safe.

## Verification

- Targeted TUI unit tests pass for new runtime controls.
- Live regression checks validate runtime controls on Uniwill hardware.

## Exit Criteria

- Bonus runtime controls are functional and do not regress core reliability suite.
