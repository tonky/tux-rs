# Plan

1. Investigate and confirm the root cause of the bug in `tux-tui` that overwrites `min_speed_percent` when saving the fan curve.
2. Investigate and confirm the root cause of the bug in `tux-daemon` that loses `ProfileAssignments` (AC/BAT settings) on reboot.
3. Implement a fix in `tux-tui` to read the existing `FanConfig` before overriding the curve.
4. Implement a fix in `tux-daemon`'s `set_active_profile_inner` to write the updated assignments back to `config.toml`.
5. Run the existing test suite and `just check` to ensure correctness.