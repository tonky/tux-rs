# Worklog

### 2026-04-11: Initial Review & Planning
- Ran `cargo check`, discovered `cargo fmt` errors and fixed them via `cargo fmt --all`.
- Checked tests, all 492 passed.
- Examined `.github/workflows/ci.yml` — looks fully functional running clippy and tests.
- Reviewed `tcc_compat` codebase. It acts correctly as a partial stub.
- Created plans for adding missing OSS boilerplate (CONTRIBUTING.md, templates, LICENSE).

### 2026-04-11: Stage 1 - Hardware Extensibility
- Added `custom_device.rs` to parse `custom_devices.toml` overrides, using `Box::leak()` to bridge between dynamically loaded configuration and the core static `DeviceDescriptor` table safely.
- Implemented `tux-daemon --dump-hardware-spec` to output `DmiInfo` allowing end-users to easily craft custom override blocks.
- Documented both simple TOML overrides and complex Rust C-Shim adding in `docs/ADDING_HARDWARE.md`.
- Code integrated into `device_table.rs` such that any dynamic override is searched before the static compiled-in `DEVICE_TABLE`.

### 2026-04-11: Stage 2 - Bugfixes and Polish
- **Issue 1:** Enforced DBus `SetChargingSettings` casing via `zbus` explicit name parameter to fix TUI UnknownMethod exceptions.
- **Issue 2:** Implemented `DaemonConfig` modifications to persist charging settings properly to disk (`/etc/tux-daemon/config.toml`). Included daemon entry point loading/application for preserved charging capabilities.
- **Issue 3:** Corrected sysfs `set_keyboard_state` loop to forcefully issue `turn_on` and `turn_off` commands mirroring brightness values.
- **Issue 4:** Changed `get_cpu_power_values_json` empty object return to empty list string representations (`[]` instead of `{}`) inside TCC compat.
- **Issue 5:** Solved race conditions within `fan_engine::config_change_bypasses_hysteresis` TOCTOU loops, solidifying its test outcomes across CI runners using yields and long timeouts.
