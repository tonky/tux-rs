# Supported Hardware

| Capability          | Clevo              | Uniwill           | NB04              | NB05              | Tuxi         |
|---------------------|--------------------|--------------------|--------------------|--------------------|--------------| 
| Fan control         | WMI/ACPI DSM (≤3 fans) | EC RAM (2 fans) | Profile-only       | EC port I/O (1–2 fans) | ACPI (2 fans) |
| Temperature sensors | Parsed from FANINFO u32 | EC RAM        | WMI BS             | EC port I/O        | ACPI         |
| Fan RPM             | Parsed from FANINFO u32 | Not available  | WMI BS             | EC port I/O        | ACPI         |
| Keyboard backlight  | White / RGB / 3-zone | White / RGB      | Per-key RGB (WMI AB) | White (3 levels)  | —            |
| Power profiles      | WMI command        | EC register (1–3)  | WMI BS (3 modes)   | WMI (3 modes)      | —            |
| Charging control    | ACPI flexicharger  | EC (profile + priority) | —             | —                  | —            |
| TDP control         | —                  | EC (PL1/PL2/PL4)  | —                  | —                  | —            |
| NVIDIA GPU power    | —                  | EC (cTGP, DB) NB02 only | —             | —                  | —            |
| ITE keyboard LEDs   | USB HID (hidraw)   | USB HID (hidraw)   | USB HID (hidraw)   | —                  | —            |

40 named SKUs are directly supported out of the box (Pulse, InfinityBook, Polaris, Stellaris, Sirius, InfinityFlex, Aura, Omnia) plus 5 platform fallback descriptors.

For detailed breakdown, please see the `Platform` structs situated under `tux-core/src/platforms`.
