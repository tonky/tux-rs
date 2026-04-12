# Remove kernel drivers, use tuxedo-drivers

**Issue:** https://github.com/tonky/tux-rs/issues/4

## Motivation

The original tux-kmod kernel modules were a proof-of-concept showing that modern LLM/agents can write kernel drivers. Now that this has been proven, maintaining custom kernel modules for one laptop model isn't practical. The official [tuxedo-drivers](https://github.com/tuxedocomputers/tuxedo-drivers) repo already supports the full TUXEDO laptop lineup.

By switching to tuxedo-drivers:
- tux-rs supports **all** TUXEDO hardware that TCC supports (not just a few models)
- No more C code maintenance burden
- Users get driver updates from TUXEDO directly
- This repo focuses on what it does better: a lightweight Rust daemon + TUI

## Scope

1. **Remove** `tux-kmod/` directory (5 C kernel modules, ~1400 LOC)
2. **Rewrite** daemon platform backends to talk to tuxedo-drivers' sysfs/ioctl/hwmon interfaces
3. **Update** device table to cover all hardware supported by tuxedo-drivers
4. **Update** packaging (Nix, DKMS removal, justfile recipes)
5. **Ensure** full hardware coverage — all devices legacy TCC supports should work, including Pulse 14 Gen 4

## Key Interface Differences (tux-kmod → tuxedo-drivers)

| Platform | tux-kmod interface | tuxedo-drivers interface |
|----------|-------------------|--------------------------|
| NB05 | Binary sysfs `ec_ram` (pread/pwrite) | No raw EC exposed. `tuxedo_nb05_fan_control` sysfs (`fan1_pwm`, `fan1_pwm_enable`) + `tuxedo_nb05_sensors` hwmon |
| Uniwill | Text sysfs (`cpu_temp`, `fan0_pwm`, `fan_mode`) | ioctl chardev `/dev/tuxedo_io` + `tuxedo_keyboard` sysfs |
| Clevo | Text sysfs (`fan0_info`, `fan_speed`, `fan_auto`) | ioctl chardev `/dev/tuxedo_io` |
| Tuxi | Text sysfs (`fan0_pwm`, `fan_mode`, `cpu_temp`) | `tuxedo_tuxi_fan_control` sysfs + hwmon |
| NB04 | Text sysfs (single module) | Split: `tuxedo_nb04_sensors` hwmon + `tuxedo_nb04_power_profiles` sysfs + `tuxedo_nb04_kbd_backlight` LED |

## User note

> daemon and TUI should support all the hardware that legacy TCC supports. for example the NixOS requester had 'Pulse 14 Gen 4'. 'InfinityBook Pro 16 Gen8' should be a priority, it's the hardware we're tesing on.

Pulse 14 Gen 4 is NB05 platform — currently supported via tux-kmod's `tuxedo-ec`. With tuxedo-drivers, this becomes `tuxedo_nb05_ec` + `tuxedo_nb05_fan_control` + `tuxedo_nb05_sensors`.
