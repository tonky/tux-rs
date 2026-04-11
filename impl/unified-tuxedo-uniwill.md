# Plan: Unified tuxedo-uniwill Kernel Module

## TL;DR
Merge vendor `tuxedo_keyboard` + `uniwill_wmi` + `uniwill_keyboard.h` + `uniwill_leds.h` into our `tuxedo-uw-fan`, renamed to `tuxedo-uniwill`. Single self-contained Uniwill platform module with full parity. Eliminates the module conflict that prevents keyboard backlight from working alongside fan control.

## Context
- Our `tuxedo_uw_fan` and vendor `uniwill_wmi` fight over the same EC registers
- `tuxedo_keyboard` depends on `uniwill_wmi` for kbd backlight LED registration
- Result: keyboard backlight doesn't work when our fan module is active
- Solution: one module that does everything — fan, keyboard, LEDs, charging, battery, events

Our module already has the EC access layer (WMI + ACPI INOU), fan control, and charging. Need to add ~800 LOC for remaining features.

## Steps

### Phase 1: Rename & Restructure
1. Rename `tux-kmod/tuxedo-uw-fan/` → `tux-kmod/tuxedo-uniwill/`, source → `tuxedo_uniwill.c`
2. Update Makefile, dkms.conf (top-level + per-module), Justfile examples
3. Platform device name → `tuxedo_uniwill`
4. Verify: builds and loads, fan control still works

### Phase 2: Device Feature Detection (~100 LOC)
5. Port `uniwill_device_features_t` — EC register flags for capability detection
6. Port `uniwill_get_device_features()` — reads EC to determine hw capabilities
7. DMI-based support checks for AC auto boot / USB power share
8. Use feature flags to conditionally expose sysfs attributes

### Phase 3: LED Class Devices (~300 LOC)
9. **White kbd backlight**: `led_classdev` as `white:kbd_backlight` (EC 0x078C, brightness 0-2 or 0-4)
10. **RGB kbd backlight**: `led_classdev_mc` as `rgb:kbd_backlight` (0x0767 mode, 0x0769-0x076B color, 0-255→0-50 range)
11. **Lightbar RGB**: 3 `led_classdev` (R/G/B at 0x0749-0x074B) + animation toggle (0x0748 bit 7)
12. Feature-gate LED types based on detected hw

### Phase 4: fn_lock & Input Device (~150 LOC)
13. **fn_lock sysfs**: `DEVICE_ATTR` on platform device, EC reg 0x074E
14. **Input device**: sparse keymap (~15 Fn key events)
15. **WMI event handler**: `wmi_install_notify_handler()` on event GUIDs, route to input/LED/charging

### Phase 5: Charging & Power Features (~150 LOC)
16. Charging feature detection (0x0742 bit 5, 0x078e bit 3) before exposing sysfs
17. AC auto boot (0x0726 bit 3) sysfs attr
18. USB power share (0x0767 bit 4) sysfs attr

### Phase 6: Battery & Misc (~150 LOC)
19. Battery ACPI hook: `raw_cycle_count` (0x04A6/0x04A7), `raw_xif1/2` (0x0402-0x0405)
20. Touchpad toggle: i8042 filter for Ctrl+Super+Zenkaku
21. Mini-LED local dimming: sysfs attr via WMI function 5

### Phase 7: Daemon Updates (~10 LOC)
22. Update fn_lock paths in `tux-daemon/src/dbus/system.rs` and `tcc_compat.rs`
23. Update platform detection path in daemon (tuxedo-uw-fan → tuxedo_uniwill sysfs)

### Phase 8: Integration & Testing
24. Build, load, verify dmesg, LED sysfs, fn_lock, fan
25. Manual test kbd brightness from TUI
26. `just test` passes

### Phase 9: Cleanup
27. Remove old `tux-kmod/tuxedo-uw-fan/`
28. Update FOLLOW_UP.md, WORKLOG, PROGRESS.md

## Key Files
- `tux-kmod/tuxedo-uw-fan/` → `tux-kmod/tuxedo-uniwill/tuxedo_uniwill.c` (main change)
- `tux-kmod/dkms.conf`, `tux-kmod/Makefile` (build system)
- Vendor reference (read-only): `vendor/tuxedo-drivers/src/uniwill_{leds,keyboard,interfaces,wmi}.{h,c}`
- `tux-daemon/src/dbus/system.rs` (~line 148), `tcc_compat.rs` (~line 384) — fn_lock path
- `tux-core/src/dmi.rs` — platform detection sysfs path
- `Justfile` — kmod recipe examples

## Decisions
- Uniwill only first; other platforms extend later
- Rename to `tuxedo-uniwill`
- Full vendor feature parity
- Platform device name: `tuxedo_uniwill` (update 2 daemon refs)
- Skip `tuxedo_is_compatible()` CPU/DMI gating
- Keep ACPI INOU as primary EC path (better than vendor's WMI-only)
- Start w/ EC feature bits for AC auto boot / USB power share; add DMI gating only if needed for safety
