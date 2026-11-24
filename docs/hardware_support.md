# Supported Hardware

| Capability          | Clevo              | Uniwill           | NB04              | NB05              | Tuxi         |
|---------------------|--------------------|--------------------|--------------------|--------------------|--------------| 
| Fan control         | WMI/ACPI DSM (≤3 fans) | EC RAM (2 fans) | Profile-only       | EC port I/O (1–2 fans) | ACPI (2 fans) |
| Temperature sensors | Parsed from FANINFO u32 | EC RAM        | WMI BS             | EC port I/O        | ACPI         |
| Fan RPM             | Parsed from FANINFO u32 | Not available  | WMI BS             | EC port I/O        | ACPI         |
| Keyboard backlight  | White / RGB / 3-zone | White / RGB      | Per-key RGB (WMI AB) | White (3 levels)  | —            |
| Power profiles      | WMI command        | EC register (1–3)  | WMI BS (3 modes)   | WMI (3 modes)      | —            |
| Charging control    | ACPI flexicharger  | EC (profile + priority; priority optional per board) | —             | —                  | —            |
| TDP control         | —                  | EC (PL1/PL2/PL4) or Intel RAPL (PL1/PL2) | —     | —                  | —            |
| NVIDIA GPU power    | —                  | EC (cTGP, DB) NB02 only; offset bounded by `nvidia-smi` max − default limit | —             | —                  | —            |
| GPU telemetry       | hwmon + kernel `boot_vga` (AMD); Intel i915/xe sysfs + RAPL uncore (iGPU); NVIDIA via NVML, never querying a runtime-suspended dGPU | same | same | same | same |
| Package power draw  | Intel RAPL (`intel-rapl:0`) with AMD `amd_energy` hwmon fallback | same | same | same | same |
| ITE keyboard LEDs   | USB HID (hidraw)   | USB HID (hidraw)   | USB HID (hidraw)   | —                  | —            |

40 named SKUs are directly supported out of the box (Pulse, InfinityBook Pro/S/Max, Polaris, Stellaris, Sirius, InfinityFlex, Aura, Omnia) plus 5 platform fallback descriptors.

## Devices run on real hardware

| SKU | Device | Notes |
|-----|--------|-------|
| IBP16I08MK2 | TUXEDO InfinityBook Pro 16 Gen8 MK2 | Maintainer's machine |
| IBP14A09MK1 / IBP15A09MK1 | TUXEDO InfinityBook Pro 14 Gen9 AMD | Community-confirmed (platform fallback) |
| IBM15I10 | TUXEDO InfinityBook Max 15 Gen10 Intel | Board `X5AR45xS`; Core Ultra 7 255H + RTX 5060. `just live-test` passes: 2 fans (curve/manual/auto), ITE 8291 per-key keyboard, charging profiles (this EC has no charge priority), cTGP 0–65 W (needs `nvidia-smi`), display backlight, EC TDP PL1/PL2 10–90 W (verified under load), EPP `performance` quirk on AC only (battery keeps profile EPP), iGPU/dGPU telemetry, power limits re-applied after resume. |

WMI-based platform probing accepts both bare (`<GUID>`) and instance-suffixed (`<GUID>-<n>`) device names under `/sys/bus/wmi/devices/`, as registered by current kernels.

## TDP (RAPL) opt-in policy

Intel RAPL TDP control (PL1/PL2 via `/sys/class/powercap/intel-rapl:0/`) is enabled on a strict per-device basis. Only vendor-sanctioned Gen8 Intel SKUs are opted in:

| SKU           | Device                                  | TDP backend | Notes                        |
|---------------|-----------------------------------------|-------------|------------------------------|
| IBP1XI08MK1   | TUXEDO InfinityBook Pro Gen8 MK1        | Intel RAPL  | PL2 max not published by firmware; falls back to PL1 max |
| IBP16I08MK2   | TUXEDO InfinityBook Pro 16 Gen8 MK2     | Intel RAPL  |                              |
| IBP14I08MK2   | TUXEDO InfinityBook Pro 14 Gen8 MK2     | Intel RAPL  |                              |
| IBM15I10      | TUXEDO InfinityBook Max 15 Gen10 Intel  | Uniwill EC (`tuxedo_io`) | See below; RAPL advertises a 28 W PL1 max here and must not be used |

### Uniwill EC TDP (`TdpSource::UniwillEc`)

PL1/PL2 are written through `/dev/tuxedo_io` (`W_UW_TDP0/1`), the path TUXEDO's own
driver and Control Center use. Bounds come from the driver's per-board table
(board `X5AR45xS`: PL1/PL2 10–90 W, PL4 10–230 W read-only). Notes:

- The driver drops out-of-range writes silently; tux-rs clamps to the bounds and
  verifies every write by reading it back.
- On this board the driver switches the EC performance profile to *overboost*
  before each TDP write, and it stays there (TCC behaves the same).
- The EC enforces the limit itself: the RAPL registers keep showing 90 W, but under
  an all-core load with PL1=PL2=25 W the package settles at 25.0 W (measured).
- The EC does not restore TDP after suspend; tux-rs does not re-apply it yet
  (follow-up).

All other devices remain at `TdpSource::None`. In particular, Gen9 AMD SKUs (`IBP14A09MK1`, `IBP15A09MK1`) are **not** supported — vendor drivers do not sanction TDP control for those platforms.

AMD laptops still get:
- iGPU detection in the Power tab (via `boot_vga` kernel flag applied to `amdgpu` hwmon entries).
- Dashboard package-power reading (via `amd_energy` hwmon driver, falling back from the Intel-specific `intel-rapl:0` counter).
- Capability-gated Power form: the `TGP Offset` slider is hidden on platforms without an NB02 backend, and the whole Power tab shows a "not available" placeholder when neither `gpu_control` nor `tdp_control` is present.

For detailed breakdown, please see the `Platform` structs situated under `tux-core/src/platforms`.
