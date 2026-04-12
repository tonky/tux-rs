# High-Level Plan: Remove kmod, use tuxedo-drivers

## Stage 1: Add tuxedo-drivers backend layer (NB05 first)

Start with NB05 since it's the cleanest case (Pulse 14 Gen 4 is the user's priority). tuxedo-drivers exposes proper sysfs attributes (`fan1_pwm`, `fan1_pwm_enable`) and hwmon (`temp1_input`, `fan1_input`).

- Create new `tux-daemon/src/platform/td_nb05.rs` backend using tuxedo-drivers' `tuxedo_nb05_fan_control` sysfs + `tuxedo_nb05_sensors` hwmon
- Update platform factory to select new backend when tuxedo-drivers modules are loaded
- Write integration tests with mock sysfs/hwmon
- Verify with real hardware if possible

## Stage 2: Clevo & Uniwill backends via tuxedo_io ioctl

tuxedo-drivers uses an ioctl chardev (`/dev/tuxedo_io`) for Clevo and Uniwill fan control — very different from our current sysfs approach.

- Create `tux-daemon/src/platform/tuxedo_io.rs` ioctl client (command structs, read/write wrappers)
- Create `td_clevo.rs` and `td_uniwill.rs` backends using the ioctl interface
- Port charging control (Clevo flexicharger, Uniwill charge profiles) to tuxedo_keyboard sysfs
- Integration tests

## Stage 3: Tuxi & NB04 backends

- Create `td_tuxi.rs` using `tuxedo_tuxi_fan_control` sysfs + hwmon
- Create `td_nb04.rs` using `tuxedo_nb04_sensors` hwmon + `tuxedo_nb04_power_profiles` sysfs
- NB04 keyboard via `tuxedo_nb04_kbd_backlight` LED subsystem

## Stage 4: Device table expansion & full TCC parity

- Expand device table to cover all SKUs from tuxedo-drivers' DMI match tables
- Map each device to the correct tuxedo-drivers backend
- Update `PlatformRegisters` to hold tuxedo-drivers-specific paths/constants
- Update `custom_devices.toml` schema if needed
- Ensure feature parity with TCC for all supported devices

## Stage 5: Remove tux-kmod & update packaging

- Delete `tux-kmod/` directory
- Remove kmod-related justfile recipes (`kmod-build`, `kmod-install`, `kmod-swap`)
- Update Nix packaging: remove `tux-kmod` package, update NixOS module to depend on tuxedo-drivers
- Update DKMS references
- Update systemd/dinit service files if needed
- Update documentation

## Stage 6: Cleanup & validation

- Remove old platform backends that talked to tux-kmod
- Remove unused `PlatformRegisters` fields
- Run full test suite
- Test on real hardware (NB05 Pulse 14 Gen 4 priority)
- Update README, docs

## Risks & considerations

- **ioctl interface stability**: tuxedo_io's ioctl commands may change between versions. We should version-check or handle gracefully.
- **Missing features**: tuxedo-drivers may not expose everything we need (e.g., raw EC RAM for advanced NB05 register access). We need to verify feature coverage per platform.
- **NB04 fan control**: tuxedo-drivers has no direct fan PWM for NB04 — only firmware profiles (low-power/balanced/performance). Our daemon's per-fan curve control won't work for NB04.
- **Transition period**: Users may have tux-kmod installed. Need clear migration docs. USER COMMENT: ignore this.
- **hwmon path discovery**: hwmon device numbers are dynamic (`hwmon0`, `hwmon1`, ...). Need robust discovery by `name` attribute.
