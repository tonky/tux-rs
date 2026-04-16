# Worklog

- Investigated issue #14.
- Discovered that `tux-tui` constructs a `FanConfig` with default `min_speed_percent` (25) when saving a curve, losing the user's custom minimum speed.
- Discovered that `tux-daemon` updates profile assignments in memory but does not save them to `config.toml`, causing them to be lost on reboot.
- Fixed `tux-tui` `execute_save_fan_curve` to merge points into the existing config.
- Fixed `tux-daemon` `ProfileInterface::set_active_profile_inner` to write the new configuration to disk.
- Ran all tests (`cargo test --workspace`) and linters successfully.