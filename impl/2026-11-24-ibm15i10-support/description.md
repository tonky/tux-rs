# Feature: TUXEDO InfinityBook Max 15 Gen10 Intel (IBM15I10) support

## Goal

Make tux-rs recognise and fully drive the **TUXEDO InfinityBook Max 15 Intel Gen10**
instead of falling back to the conservative "Unknown Uniwill Device" descriptor
(1 fan, no keyboard, no charging, no GPU power, no TDP).

## Target hardware (captured 2026-11-24 on the device itself)

| Item | Value |
|------|-------|
| `sys_vendor` | `TUXEDO` |
| `product_name` | `TUXEDO InfinityBook Max 15 Intel Gen10` |
| `product_sku` | `IBM15I10` |
| `product_family` | `INFINITYBOOK MAX` |
| `board_vendor` | `NB02` (Uniwill) |
| `board_name` | `X5AR45xS` |
| CPU | Intel Core Ultra 7 255H (Arrow Lake-H), `intel_pstate` with EPP |
| iGPU | Intel Arc 130T/140T (`8086:7d51`) |
| dGPU | NVIDIA GeForce RTX 5060 Mobile (`10de:2d19`) |
| Keyboard | ITE 8291 per-key RGB, USB `048D:600B` (kernel `ite_8291` bound, `/dev/hidraw0`) |
| Kernel / drivers | Linux 7.3.5-cachyos, tuxedo-drivers 4.25.0 (DKMS) |
| Loaded modules | `tuxedo_keyboard`, `uniwill_wmi`, `tuxedo_io`, `ite_8291`, `tuxedo_nb02_nvidia_power_ctrl`, `clevo_wmi` |

### Interfaces exposed by tuxedo-drivers on this machine

- `/dev/tuxedo_io` — Uniwill ioctls (fans, EC TDP, perf profile).
- `/sys/devices/platform/tuxedo_keyboard/charging_profile/{charging_profile,charging_profiles_available}`
  → `high_capacity balanced stationary` (no `charging_priority` group).
- `/sys/devices/platform/tuxedo_keyboard/{fn_lock,usb_powershare,ac_auto_boot}`.
- `/sys/devices/platform/tuxedo_nvidia_power_ctrl/ctgp_offset` (NB02 cTGP).
- `/sys/class/leds/rgb:kbd_backlight*` (126 per-key LEDs from `ite_8291`).
- `/sys/class/powercap/intel-rapl:0` — PL1=90 W, PL2=90 W, PL4=229 W, but
  `constraint_0_max_power_uw` reports only **28 W** (base TDP), so the generic
  RAPL backend would wrongly clamp PL1 to 28 W.

### Vendor driver facts for board `X5AR45xS` (tuxedo-drivers 4.25.0)

- `tuxedo_io.c`: EC TDP bounds `min = {10, 10, 10}`, `max = {90, 90, 230}` W (PL1/PL2/PL4).
- `uniwill_keyboard.h`:
  - in `universal_fan_control_exception` → uses legacy EC fan control path;
  - `uniwill_custom_profile_mode_needed = true` → writing TDP via `tuxedo_io`
    forces the EC "overboost" perf profile first;
  - supports `ac_auto_boot` + `usb_powershare`.
- `ite_8291.c`: `X5AR45xS` → **no colour scaling** (identity).

### Vendor TCC (3.1.2) facts for `IBM15I10`

- DMI SKU map: `IBM15I10` → `TUXEDODevice.IBM15I10`.
- `CpuWorker`: IBM15I10 is in `eppPerformanceQuirkDevices` — TCC forces
  `energy_performance_preference = performance` "to allow full frequency range".

## Problems found while investigating

1. **No device-table row** for `IBM15I10` → detection uses the Uniwill fallback.
2. **WMI GUID probe is broken on current kernels**: `/sys/bus/wmi/devices/`
   entries carry an instance suffix (`ABBC0F72-…-C90629100000-13`), so the
   exact-path check never matches. Detection only works here thanks to the
   `NB02` board-vendor fallback. Affects every Uniwill/Clevo/NB04 machine on
   recent kernels.
3. **ITE colour scaling** would apply the generic green/blue reduction to PID
   `0x600b`, whereas the vendor driver uses identity for this board.
4. **Hardware-dependent unit test**: `gpu::nb02::tests::new_returns_none_when_sysfs_missing`
   probes the real `/sys` path and fails on any NB02 machine with an NVIDIA dGPU.
5. **No Uniwill EC TDP backend**: vendor-sanctioned TDP for this board goes
   through `tuxedo_io` (`R/W_UW_TDP0..2`); tux-rs only has NB05-EC and RAPL.

## Out of scope (candidates for follow-up)

- Sibling SKUs (`IBM15A10`, `IBM16A10`, boards `X5KK45xS_X5SP45xS`,
  `X6KK45xU_X6SP45xU`, `X6AR55xU`) — cannot be tested here.
- `fn_lock`, `usb_powershare`, `ac_auto_boot` controls.
- Uniwill ODM perf profiles (`W_UW_PERF_PROF`) and PRIME switching.
