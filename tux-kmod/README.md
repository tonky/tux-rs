# tux-kmod: Kernel Shims

Minimal C kernel modules for TUXEDO laptop hardware access.

Five modules, each ~200–300 lines of stateless passthrough code:

| Module           | Hardware Access              | sysfs Interface                 |
|------------------|------------------------------|---------------------------------|
| `tuxedo-ec`      | SuperIO port I/O (0x4e/0x4f) | Binary `ec_ram` attribute       |
| `tuxedo-uw-fan`  | ACPI EC read/write methods   | Binary attributes per register  |
| `tuxedo-clevo`   | WMI + ACPI DSM              | Binary attributes per command   |
| `tuxedo-nb04`    | WMI AB/BS methods            | Binary attributes per method    |
| `tuxedo-tuxi`    | ACPI TFAN evaluation         | Binary attributes per fan       |

Implementation begins in Phase 3.
