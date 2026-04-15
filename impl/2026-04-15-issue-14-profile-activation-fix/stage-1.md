# Stage 1: Implement fixes for Issue #14

- Modify `tux-tui/src/dbus_task.rs` to fetch existing fan configuration prior to saving a new curve.
- Modify `tux-daemon/src/dbus/profile.rs` to persist profile assignment changes to the global configuration file.
- Update tests in `tux-daemon/src/dbus/profile.rs` to pass `daemon_config`.