# Supported Hardware

| Capability          | Clevo              | Uniwill           | NB04              | NB05              | Tuxi         |
|---------------------|--------------------|--------------------|--------------------|--------------------|--------------| 
| Fan control         | WMI/ACPI DSM (≤3 fans) | EC RAM (2 fans) | Profile-only       | EC port I/O (1–2 fans) | ACPI (2 fans) |
| Temperature sensors | Parsed from FANINFO u32 | EC RAM        | WMI BS             | EC port I/O        | ACPI         |
| Fan RPM             | Parsed from FANINFO u32 | Not available  | WMI BS             | EC port I/O        | ACPI         |
| Keyboard backlight  | White / RGB / 3-zone | White / RGB      | Per-key RGB (WMI AB) | White (3 levels)  | —            |
| Power profiles      | WMI command        | EC register (1–3)  | WMI BS (3 modes)   | WMI (3 modes)      | —            |
| Charging control    | ACPI flexicharger  | EC (profile + priority) | —             | —                  | —            |
| TDP control         | —                  | EC (PL1/PL2/PL4) or Intel RAPL (PL1/PL2) | —     | —                  | —            |
| NVIDIA GPU power    | —                  | EC (cTGP, DB) NB02 only | —             | —                  | —            |
| GPU detection       | hwmon + kernel `boot_vga` (AMD APU iGPU, NVIDIA/Intel dGPU) | same | same | same | same |
| Package power draw  | Intel RAPL (`intel-rapl:0`) with AMD `amd_energy` hwmon fallback | same | same | same | same |
| ITE keyboard LEDs   | USB HID (hidraw)   | USB HID (hidraw)   | USB HID (hidraw)   | —                  | —            |

40 named SKUs are directly supported out of the box (Pulse, InfinityBook, Polaris, Stellaris, Sirius, InfinityFlex, Aura, Omnia) plus 5 platform fallback descriptors.

## TDP (RAPL) opt-in policy

Intel RAPL TDP control (PL1/PL2 via `/sys/class/powercap/intel-rapl:0/`) is enabled on a strict per-device basis. Only vendor-sanctioned Gen8 Intel SKUs are opted in:

| SKU           | Device                                  | TDP backend | Notes                        |
|---------------|-----------------------------------------|-------------|------------------------------|
| IBP1XI08MK1   | TUXEDO InfinityBook Pro Gen8 MK1        | Intel RAPL  | PL2 max not published by firmware; falls back to PL1 max |
| IBP16I08MK2   | TUXEDO InfinityBook Pro 16 Gen8 MK2     | Intel RAPL  |                              |
| IBP14I08MK2   | TUXEDO InfinityBook Pro 14 Gen8 MK2     | Intel RAPL  |                              |

All other devices remain at `TdpSource::None`. In particular, Gen9 AMD SKUs (`IBP14A09MK1`, `IBP15A09MK1`) are **not** supported — vendor drivers do not sanction TDP control for those platforms.

AMD laptops still get:
- iGPU detection in the Power tab (via `boot_vga` kernel flag applied to `amdgpu` hwmon entries).
- Dashboard package-power reading (via `amd_energy` hwmon driver, falling back from the Intel-specific `intel-rapl:0` counter).
- Capability-gated Power form: the `TGP Offset` slider is hidden on platforms without an NB02 backend, and the whole Power tab shows a "not available" placeholder when neither `gpu_control` nor `tdp_control` is present.

For detailed breakdown, please see the `Platform` structs situated under `tux-core/src/platforms`.
