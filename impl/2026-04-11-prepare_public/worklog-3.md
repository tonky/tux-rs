# Phase 3 Worklog

## Summary of Changes
- Adjusted TUI Webcam tab verbiage to explicitly indicate that webcam controls are "unavailable in the TUI", directing the user to use a GUI application instead of asserting it's unsupported by the underlying hardware.
- Refined the daemon's `--debug` flag so that it limits debug output to only the `tux_daemon` and `tux_core` crates (specifically using `EnvFilter::new("tux_daemon=debug,tux_core=debug")`). This strips out the heavy noise from dependencies like `zbus`, leading to a much cleaner debugging experience.
- Tracked down a bug where the TUI displayed a `0` battery cycle count despite the hardware maintaining one.

## Decisions Made
- **Battery Cycle Location Read:** Discovered that the `tux-kmod` shims (specifically `tuxedo_uniwill`) maps `raw_cycle_count` directly into `/sys/devices/platform/tuxedo-uniwill/raw_cycle_count` as a `DEVICE_ATTR_RO`. Our previous fallback merely checked the standard `/sys/class/power_supply/BAT0` directory. I've augmented `tux-daemon::dbus::system` to unconditionally prioritize reading directly from the platform path before attempting the `BAT0` ACPI fallback path, ensuring cycles are correctly pulled. 

All automated test suites and linters pass cleanly. Phase 3 is completed.
