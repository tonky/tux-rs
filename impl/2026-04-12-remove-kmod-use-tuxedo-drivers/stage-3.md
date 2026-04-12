# Stage 3: Tuxi & NB04 backends

## Context
Port the remaining supported platforms, Tuxi and NB04, to `tuxedo-drivers`. Tuxi uses a mix of sysfs + hwmon. NB04 doesn't support fine manual fan curve control but uses firmware profiles.

## Files to modify
- **[NEW] `tux-daemon/src/platform/td_tuxi.rs`**: Backend that queries `tuxedo_tuxi_fan_control` (sysfs) + hwmon device.
- **[NEW] `tux-daemon/src/platform/td_nb04.rs`**: Backend using `tuxedo_nb04_sensors` parameters and `tuxedo_nb04_power_profiles` for general power usage instead of pure fan curves. Note: requires dealing with the missing raw PWM support.

## Details
- Explore the exact mapping of NB04 fan modes to daemon curves.
- Integrate the NB04 keyboard control using the standard `tuxedo_nb04_kbd_backlight` LED subsystem.
