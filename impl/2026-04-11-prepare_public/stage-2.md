# Stage 2: Bugfixes and Polish

This document details the action plan for executing Phase 2.

## Context & Objectives

We need to address several lingering integration bugs before final release. These bugs span across D-Bus bindings, hardware persistence, Legacy GUI crash workarounds, and a flaky test.

## Execution Plan & Code References

### 1. `SetChargingSettings` DBus Method
**Bug:** TUI reports `UnknownMethod 'SetChargingSettings'` when interacting with the charging settings.
**Analysis:** In `tux-daemon/src/dbus/charging.rs`, `fn set_charging_settings(&self, ...)` might be failing to export correctly or is incorrectly routed in `zbus`. Some `zbus` versions map `set_*` single-argument methods to properties natively.
**Action:** Explicitly enforce the original method name logic via `#[zbus(name = "SetChargingSettings")]` atop `set_charging_settings` inside `tux-daemon/src/dbus/charging.rs`.

### 2. Charging Settings Persistence
**Bug:** TUI charging settings (e.g., profiles: "performance" / "high capacity") revert on reboot.
**Analysis:** Currently, `tux-daemon/src/config.rs` `DaemonConfig` does not persist anything globally or write out charging profiles. The daemon loads them but never persists them outside of the Profile assignments (`ProfileStore`).
**Action:** 
1. Expand `DaemonConfig` (in `tux-daemon/src/config.rs`) to include `pub charging: Option<tux_core::profile::ChargingSettings>`.
2. Add a `save()` wrapper to `DaemonConfig` allowing it to write explicitly to `/etc/tux-daemon/config.toml` when runtime alterations happen.
3. Update `ChargingInterface::set_charging_settings` to update `DaemonConfig` and trigger a `config.save()`.

### 3. TUI Keyboard Brightness Hardware Sync
**Bug:** TUI changes save the brightness slider levels, but the hardware doesn't illuminate.
**Analysis:** `KeyboardLed` implementors (e.g., `sysfs_kbd.rs`) correctly calculate brightness overrides but do not actively reinstate the hardware state into "ON". If the LED class was in an "OFF" state, changing the brightness scalar does nothing unless `guard.turn_on()` is called. 
**Action:** In `tux-daemon/src/dbus/settings.rs` inside `set_keyboard_state`:
- if `hw_brightness > 0`, call `guard.turn_on()`.
- if `hw_brightness == 0`, call `guard.turn_off()`.
- This ensures hardware sync accurately reacts to TUI slider adjustments.

### 4. Legacy TCC "CPU Power" Crash
**Bug:** Launching the legacy TCC app leaves zombie processes and crashes the UI.
**Analysis:** The legacy shim `get_cpu_power_values_json()` in `tux-daemon/src/dbus/tcc_compat.rs` returns `"{}"`. The legacy UI's JSON parser for CPU Power likely expects an array wrapper (e.g. `[]`) for iterative core values. 
**Action:** Modify `get_cpu_power_values_json()` to return `"[]".to_string()`. If an array does not fix the unmarshalling crash inside the JS client, we will supply a base object: `{"active": false}`.

### 5. `config_change_bypasses_hysteresis` Flaky Test
**Bug:** The test fails sporadically on slower test environments due to TOCTOU races between `watch::Receiver` ticks.
**Analysis:** Although `await_pwm` attempts to stabilize checks, `config_tx.send(new_config)` might fire while the loop is processing the previous interval, causing the fan engine to miss the curve drop entirely or read out of bounds.
**Action:** Strengthen the synchronization in `tests` by ensuring `settle().await` forces the internal Tokio task scheduler to drop control back to the engine task consistently before executing `await_pwm`.

---
**Status:** Pending User Approval
