# Feature Support Matrix

Tracks per-device capability across three layers:
- **tuxedo-drivers** — upstream kernel modules from TUXEDO Computers
- **tccd / TCC** — upstream TUXEDO Control Center daemon and GUI
- **tux-rs** — this project (tux-daemon + tux-tui)

Legend: ✅ implemented · ⚠️ partial / known limitation · ❌ not implemented · — not applicable

---

## Platform overview

| Platform     | Kernel module(s) used                                      | Devices in table |
|-------------|-------------------------------------------------------------|-----------------|
| **NB05**    | `tuxedo_nb05_fan_control`, `tuxedo_nb05_sensors`, `tuxedo_nb05_ec`, `tuxedo_nb05_power_profiles` | 4 |
| **Uniwill** | `tuxedo_io` (ioctl chardev)                                 | 21 |
| **Clevo**   | `tuxedo_io` (ioctl chardev), `tuxedo_keyboard`              | 3 |
| **NB04**    | `tuxedo_nb04_sensors`, `tuxedo_nb04_power_profiles`, `tuxedo_nb04_wmi_*` | 2 |
| **Tuxi**    | `tuxedo_tuxi` → `tuxedo_fan_control` platform dev          | 2 |

---

## Device table

### NB05 platform

| Device | SKU | Fans | Charging | TDP | Keyboard |
|--------|-----|------|----------|-----|----------|
| TUXEDO Pulse 14 Gen3 | PULSE1403 | 2 | — | — | ITE8291 |
| TUXEDO Pulse 14 Gen4 | PULSE1404 | 2 | — | — | ITE8291 |
| TUXEDO Pulse 15 Gen2 | PULSE1502 | 2 | — | — | ITE8291 |
| TUXEDO InfinityFlex 14 Gen1 | IFLX14I01 | 1 | — | — | ITE8291 |

### Uniwill platform

| Device | SKU | Fans | Charging | GPU dGPU pwr | Keyboard |
|--------|-----|------|----------|---------|----------|
| Stellaris 15 Gen3 Intel | STELLARIS1XI03 | 2 | EcProfilePriority | Nb02Nvidia | ITE8291 |
| Stellaris 15 Gen3 AMD | STELLARIS1XA03 | 2 | EcProfilePriority | Nb02Nvidia | ITE8291 |
| Stellaris 15 Gen4 Intel | STELLARIS1XI04 | 2 | EcProfilePriority | Nb02Nvidia | ITE8291 |
| Stellaris/Polaris Gen4 AMD | STEPOL1XA04 | 2 | EcProfilePriority | Nb02Nvidia | ITE8291 |
| Stellaris 15 Gen5 Intel | STELLARIS1XI05 | 2 | EcProfilePriority | Nb02Nvidia | ITE8291 |
| Stellaris 15 Gen5 AMD | STELLARIS1XA05 | 2 | EcProfilePriority | Nb02Nvidia | ITE8291 |
| Stellaris 16 Gen6 Intel | STELLARIS16I06 | 2 | EcProfilePriority | Nb02Nvidia | ITE8291 |
| Stellaris Slim 15 Gen6 Intel | STELLSL15I06 | 2 | EcProfilePriority | Nb02Nvidia | ITE8291 |
| Stellaris Slim 15 Gen6 AMD | STELLSL15A06 | 2 | EcProfilePriority | Nb02Nvidia | ITE8291 |
| Stellaris 17 Gen6 Intel | STELLARIS17I06 | 2 | EcProfilePriority | Nb02Nvidia | ITE8291 |
| Stellaris 16 Gen7 Intel | STELLARIS16I07 | 2 | EcProfilePriority | Nb02Nvidia | ITE8291 |
| Stellaris 16 Gen7 AMD | STELLARIS16A07 | 2 | EcProfilePriority | Nb02Nvidia | ITE8291 |
| Polaris 15 Gen2 Intel | POLARIS1XI02 | 2 | EcProfilePriority | Nb02Nvidia | RGB 3-zone |
| Polaris 15 Gen2 AMD | POLARIS1XA02 | 2 | EcProfilePriority | Nb02Nvidia | RGB 3-zone |
| Polaris 15 Gen3 Intel | POLARIS1XI03 | 2 | EcProfilePriority | Nb02Nvidia | ITE8291 |
| Polaris 15 Gen3 AMD | POLARIS1XA03 | 2 | EcProfilePriority | Nb02Nvidia | ITE8291 |
| Polaris 15 Gen5 AMD | POLARIS1XA05 | 2 | EcProfilePriority | Nb02Nvidia | ITE8291 |
| InfinityBook Pro Gen7 MK1 | IBP1XI07MK1 | 2 | EcProfilePriority | — | White |
| InfinityBook Pro Gen7 MK2 | IBP1XI07MK2 | 2 | EcProfilePriority | — | White |
| InfinityBook Pro Gen8 MK1 | IBP1XI08MK1 | 2 | EcProfilePriority | — | White |
| InfinityBook Pro Gen8 MK2 | IBP1XI08MK2 | 2 | EcProfilePriority | — | White |
| InfinityBook Pro 14 Gen8 MK2 | IBP14I08MK2 | 2 | EcProfilePriority | — | ITE8291 |
| InfinityBook Pro 16 Gen8 MK2 | IBP16I08MK2 | 2 | EcProfilePriority | — | ITE8291 |
| OMNIA Gen8 MK2 | OMNIA08IMK2 | 2 | EcProfilePriority | — | ITE8291 |
| InfinityBook S 14 Gen8 | IBS14I08 | 1 | EcProfilePriority | — | White |
| InfinityBook S 15 Gen8 | IBS15I08 | 1 | EcProfilePriority | — | White |
| InfinityBook Pro 14 Gen7 | IBP14I07 | 2 | EcProfilePriority | — | White |
| InfinityBook Pro 15 Gen7 | IBP15I07 | 2 | EcProfilePriority | — | White |

### Clevo platform

| Device | SKU | Fans | Charging | Keyboard |
|--------|-----|------|----------|----------|
| Aura 14 Gen3 | AURA14GEN3 | 2 | Flexicharger | RGB 3-zone |
| Aura 15 Gen3 | AURA15GEN3 | 2 | Flexicharger | RGB 3-zone |
| Aura 14/15 Gen4 | AURA14GEN4 / AURA15GEN4 | 2 | Flexicharger | RGB 3-zone |

### NB04 platform

| Device | SKU | Fans | Fan control | Charging | Keyboard |
|--------|-----|------|-------------|----------|----------|
| Sirius 16 Gen1 | SIRIUS1601 | 2 | Profile-only | — | ITE829x |
| Sirius 16 Gen2 | SIRIUS1602 | 2 | Profile-only | — | ITE829x |

### Tuxi platform

| Device | SKU | Fans | Charging | Keyboard |
|--------|-----|------|----------|----------|
| Aura 15 Gen1 | AURA15GEN1T | 1 | — | White |
| Aura 15 Gen2 | AURA15GEN2T | 1 | — | White |

---

## Feature support matrix

### Fan control

| Feature | tuxedo-drivers | tccd / TCC | tux-rs |
|---------|---------------|------------|--------|
| Fan PWM write (NB05) | ✅ `tuxedo_nb05_fan_control` sysfs | ✅ | ✅ `td_nb05.rs` |
| Fan PWM write (Uniwill) | ✅ `tuxedo_io` ioctl `W_UW_FANSPEED` | ✅ | ✅ `td_uniwill.rs` |
| Fan PWM write (Clevo) | ✅ `tuxedo_io` ioctl `W_CL_FANSPEED` packed u32 | ✅ | ✅ `td_clevo.rs` |
| Fan PWM write (NB04) | — firmware-managed | ✅ profile-based | ⚠️ `set_auto` only (maps to Balanced profile) |
| Fan PWM write (Tuxi) | ✅ `tuxedo_fan_control` sysfs | ✅ | ✅ `td_tuxi.rs` |
| Fan auto mode (NB05) | ✅ sysfs `pwm_enable=2` | ✅ | ✅ |
| Fan auto mode (Uniwill) | ✅ ioctl `W_UW_FANAUTO` (_IO, no-arg) | ✅ | ✅ |
| Fan auto mode (Clevo) | ✅ ioctl `W_CL_FANAUTO` | ✅ | ✅ |
| Fan auto mode (Tuxi) | ✅ sysfs `pwm_enable=2` | ✅ | ✅ |
| Fan curve (user-defined) | — | ✅ configurable | ✅ configurable per profile |
| Fan RPM read (NB05) | ✅ hwmon `fan[1,2]_input` | ✅ | ✅ |
| Fan RPM read (Uniwill) | ✅ ioctl `R_UW_FANSPEED*` | ✅ | ✅ |
| Fan RPM read (Clevo) | ✅ ioctl `R_CL_FANINFO*` bits[31:16] | ✅ | ✅ |
| Fan RPM read (NB04) | ✅ `tuxedo_nb04_sensors` hwmon | ✅ | ✅ |
| Fan RPM read (Tuxi) | ⚠️ optional hwmon on newer firmware | ✅ | ⚠️ returns Unsupported if hwmon absent |
| 3-fan Clevo support | ✅ 3 slots in packed u32 | ✅ | ✅ all 3 slots preserved in write |

### Temperature sensors

| Feature | tuxedo-drivers | tccd / TCC | tux-rs |
|---------|---------------|------------|--------|
| CPU temp (NB05) | ✅ `tuxedo_nb05_sensors` hwmon | ✅ | ✅ |
| CPU temp (Uniwill) | ✅ via EC ioctl `R_UW_FAN_TEMP` | ✅ | ✅ |
| CPU temp (Clevo) | ✅ fan_info bits[15:8] | ✅ | ✅ |
| CPU temp (NB04) | ✅ `tuxedo_nb04_sensors` hwmon | ✅ | ✅ |
| CPU temp (Tuxi) | ⚠️ optional hwmon | ✅ | ⚠️ returns Unsupported if hwmon absent |
| GPU temp (Stellaris/Polaris Uniwill) | ✅ | ✅ | ✅ `gpu/hwmon.rs` |
| GPU temp (NB04) | ✅ `tuxedo_nb04_sensors` | ✅ | ✅ |
| GPU temp (NB05/Clevo/Tuxi) | — no dGPU on these models | — | — |
| AMD APU iGPU detection | ✅ `amdgpu` hwmon | — | ✅ classified via `device/boot_vga` kernel flag (`gpu/hwmon.rs`) |
| Package power draw (Intel RAPL) | ✅ `/sys/class/powercap/intel-rapl:0` | ✅ | ✅ `dbus/system.rs::EnergySampler` |
| Package power draw (AMD `amd_energy`) | ✅ `amd_energy` hwmon | — | ✅ fallback probe when intel-rapl absent |

### Charging control

| Feature | tuxedo-drivers | tccd / TCC | tux-rs |
|---------|---------------|------------|--------|
| Start/end threshold (Clevo Flexicharger) | ✅ `tuxedo_keyboard` ACPI | ✅ | ✅ `charging/clevo.rs` — `charge_control_{start,end}_threshold` under `/sys/devices/platform/tuxedo_keyboard` |
| EC profile (Uniwill) | ✅ `tuxedo_keyboard` | ✅ | ✅ `charging/uniwill.rs` — `charging_profile/charging_profile` subgroup |
| EC priority (Uniwill) | ✅ `tuxedo_keyboard` | ✅ | ✅ `charging/uniwill.rs` — `charging_priority/charging_prio` subgroup |
| Charging control (NB05) | — | — | — |
| Charging control (NB04) | — | — | — |
| Charging control (Tuxi) | — | — | — |

### TDP / Power limits

| Feature | tuxedo-drivers | tccd / TCC | tux-rs |
|---------|---------------|------------|--------|
| PL1 read/write (NB05 EC) | ✅ `tuxedo_nb05_ec` ec_ram | ✅ | ⚠️ backend implemented (`cpu/tdp.rs`), no device table TDP fields populated for current SKUs |
| PL2 read/write (NB05 EC) | ✅ `tuxedo_nb05_ec` | ✅ | ⚠️ same |
| Power profiles (NB05) | ✅ `tuxedo_nb05_power_profiles` | ✅ | ❌ not exposed |
| Power profiles (NB04) | ✅ `tuxedo_nb04_power_profiles` | ✅ | ⚠️ mapped to fan Balanced via `td_nb04.rs`; no ODM profile API |
| ODM profiles (Stellaris/Uniwill) | — (handled by EC firmware) | ✅ "enthusiast" / "balanced" / "quiet" | ⚠️ stored in profile as string, passed through TCC compat; not applied to hardware |

### CPU governor

| Feature | tuxedo-drivers | tccd / TCC | tux-rs |
|---------|---------------|------------|--------|
| CPU scaling governor | — (standard kernel) | ✅ | ✅ `cpu/governor.rs` |
| CPU energy/performance preference (EPP) | — (standard kernel) | ✅ | ✅ |
| Turbo boost enable/disable | — (standard kernel) | ✅ | ✅ |

### Keyboard backlight

| Feature | tuxedo-drivers | tccd / TCC | tux-rs |
|---------|---------------|------------|--------|
| ITE8291 per-key RGB (Pulse, IBP, Stellaris Gen3+, Sirius) | ✅ `ite_8291` hidraw | ✅ | ✅ `hid/ite8291.rs` brightness + color + mode |
| ITE8291 lightbar variant | ✅ `ite_8291_lb` | ✅ | ✅ `hid/ite8291_lb.rs` |
| ITE8297 RGB lightbar | ✅ `ite_8297` | ✅ | ✅ `hid/ite8297.rs` |
| ITE829x per-key (Sirius NB04) | ✅ `ite_829x` | ✅ | ✅ `hid/ite829x.rs` |
| White/single-color backlight (IBP Gen7, Aura Tuxi) | ✅ sysfs LED | ✅ | ✅ `hid/sysfs_kbd.rs` brightness |
| RGB 3-zone (Polaris Gen2, Aura Gen3) | ✅ `tuxedo_keyboard` / `clevo_keyboard` | ✅ | ❌ Clevo 3-zone not implemented (no ITE chip on these) |
| NB05 keyboard backlight | ✅ `tuxedo_nb05_keyboard` / `tuxedo_nb05_kbd_backlight` | ✅ | ❌ not implemented (NB05 Pulse has ITE8291 via hidraw; nb05-specific sysfs kbd-backlight path not wired) |
| NB04 keyboard backlight | ✅ `tuxedo_nb04_kbd_backlight` → `rgb:kbd_backlight` LED | ✅ | ✅ handled via `hid/discover.rs` `discover_sysfs_keyboards()` + `SysfsRgbKeyboard` (sysfs fallback when no ITE HID found) |

### GPU power control (Nvidia dGPU)

| Feature | tuxedo-drivers | tccd / TCC | tux-rs |
|---------|---------------|------------|--------|
| cTGP offset read/write (Stellaris/Polaris Uniwill) | ✅ `tuxedo_nb02_nvidia_power_ctrl` | ✅ | ✅ `gpu/nb02.rs` |
| GPU power on other platforms | — | — | — |
| Runtime capability gating of TGP Offset UI | — | ⚠️ device-table gated | ✅ slider hidden when no NB02 backend; Power tab placeholder when neither `gpu_control` nor `tdp_control` is present |

### Display / Screen

| Feature | tuxedo-drivers | tccd / TCC | tux-rs |
|---------|---------------|------------|--------|
| Display brightness read/write | — (standard `sysfs /sys/class/backlight`) | ✅ | ✅ `display.rs` |
| Refresh rate control | — | ✅ | ❌ not implemented |
| Screen on/off | — | ✅ (webcam privacy) | ⚠️ webcam view in TUI (stub) |

### Profile management

| Feature | tuxedo-drivers | tccd / TCC | tux-rs |
|---------|---------------|------------|--------|
| Named profiles with per-feature settings | — | ✅ | ✅ `profile_store.rs` |
| Profile persistence (TOML/JSON) | — | ✅ JSON | ✅ TOML |
| TCC profile import | — | — | ✅ `tcc_import.rs` reads TCC JSON profiles |
| TCC D-Bus compat interface | — | ✅ tccd native | ✅ `dbus/tcc_compat.rs` (TCC GUI can connect) |
| Power monitoring (AC/battery) | — | ✅ | ✅ `power_monitor.rs` |
| Auto-profile on AC/battery switch | — | ✅ | ✅ |
| Sleep/resume restore | — | ✅ | ✅ `sleep.rs` |
| Custom device overrides (TOML) | — | — | ✅ `custom_device.rs` |

### Platform detection

| Feature | tuxedo-drivers | tccd / TCC | tux-rs |
|---------|---------------|------------|--------|
| DMI product SKU lookup | — | ✅ | ✅ `dmi.rs` + `device_table.rs` |
| WMI GUID detection (Clevo/Uniwill) | ✅ | ✅ | ✅ |
| Fallback platform detection (sysfs probe) | — | ✅ | ✅ |
| Unknown device fallback descriptor | — | — | ✅ per-platform conservative fallback |

---

## Known gaps / open follow-ups

| ID | Feature | Status | Notes |
|----|---------|--------|-------|
| f002 | Clevo charging sysfs path | ✅ done | `charging/clevo.rs` uses `/sys/devices/platform/tuxedo_keyboard/charge_control_{start,end}_threshold`. |
| f003 | NB04 keyboard backlight | ✅ done | `tuxedo_nb04_kbd_backlight` exposes `rgb:kbd_backlight` LED; already handled by `discover_sysfs_keyboards()` → `SysfsRgbKeyboard`. No daemon changes needed. |
| f004 | Device table SKU sweep | ✅ done | All 33 vendor `DMI_PRODUCT_SKU` entries are in `device_table.rs`. Extra entries (IBP14I07, IBP15I07, AURA15GEN1T/2T) are pre-tuxedo-drivers legacy models with correct platform fallback. |
| f005 | Uniwill charging sysfs path | ✅ done | `charging/uniwill.rs` uses `charging_profile/charging_profile` and `charging_priority/charging_prio` subgroups; priority value corrected from `"charge"` to `"charge_battery"`. |
| —   | Clevo RGB 3-zone keyboard | ❌ not done | Aura Gen3/Gen4 use `tuxedo_keyboard` / `clevo_keyboard` with 3-zone LED class, not ITE HID. Not wired into `hid/`. |
| —   | NB05 keyboard backlight | ❌ not done | `tuxedo_nb05_keyboard` + `tuxedo_nb05_kbd_backlight` — NB05 Pulse/InfinityFlex have ITE8291, already handled via `hid/ite8291.rs`, but the nb05-specific sysfs kbd-backlight (WhiteLevels) path is not wired. |
| —   | NB05 power profiles | ❌ not done | `tuxedo_nb05_power_profiles` platform sysfs not integrated. |
| —   | Display refresh rate | ❌ not done | TCC supports this via kernel DRM; not planned. |

---

## Test coverage by feature area

| Area | Unit tests | Integration tests |
|------|-----------|------------------|
| Fan backends (all 5 platforms) | ✅ `tux-daemon/src/platform/td_*.rs` | ✅ `tux-daemon/tests/integration.rs` |
| Platform detection / DMI | ✅ `tux-core/src/dmi.rs` | — |
| Device table completeness | ✅ `tux-core/src/device_table.rs` | — |
| Fan curve engine | ✅ `tux-core/src/fan_curve.rs` | — |
| Charging backends | ✅ `charging/clevo.rs`, `charging/uniwill.rs` | — |
| CPU governor | ✅ `cpu/governor.rs` | — |
| TDP backend | ✅ `cpu/tdp.rs` | — |
| GPU power (Nb02) | ✅ `gpu/nb02.rs` | — |
| ITE keyboard HID | ✅ per-controller unit tests | — |
| TCC import | ✅ `tcc_import.rs` (90+ cases) | — |
| Profile store | ✅ `profile_store.rs` | — |
| D-Bus TCC compat | ✅ `dbus/tcc_compat.rs` | ✅ `tests/e2e.rs` |
| Live regression | — | ✅ `tux-tui/tests/live_regression.rs` (manual) |
