# Stage 1: Add tuxedo-drivers backend layer (NB05 first)

## Context
The goal of this stage is to migrate NB05 (Pulse 14 Gen 4, etc.) platform backend from the custom `tux-kmod` (direct binary EC RAM manipulation) to `tuxedo-drivers`, utilizing standard `hwmon` and `sysfs` files.

## Files to modify
- **[NEW] `tux-daemon/src/platform/td_nb05.rs`**: Create a new FanBackend for NB05 utilizing the `tuxedo-drivers` sysfs and hwmon interface.
    - Path mapping: `tuxedo_nb05_fan_control` (sysfs: `fan1_pwm`, `fan1_pwm_enable`) and `tuxedo_nb05_sensors` (hwmon: `temp1_input`, `fan1_input`).
- **`tux-daemon/src/platform/mod.rs`**: Update `init_fan_backend` to use the new `TdNb05FanBackend` when the proper sysfs/hwmon interfaces are found.

## Details
- Investigate `tuxedo-drivers` NB05 hwmon location mechanism. The backend should look up the `hwmon` device dynamically based on standard naming.
- Ensure integration tests use mocked sysfs/hwmon files.
