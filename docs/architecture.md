# tux-rs Architecture

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                    tux-tui (unprivileged)                │
│         ratatui TUI · 10 tabs · TEA architecture        │
└───────────────────────────┬─────────────────────────────┘
                            │ D-Bus (system bus)
┌───────────────────────────┴─────────────────────────────┐
│                 tux-daemon (root service)                │
│                                                         │
│  ┌──────────┐ ┌──────────┐ ┌────────┐ ┌─────────────┐  │
│  │Fan Curve │ │ Profile  │ │Keyboard│ │  CPU / GPU   │  │
│  │ Engine   │ │ Applier  │ │  HID   │ │  Backends    │  │
│  └────┬─────┘ └────┬─────┘ └───┬────┘ └──────┬──────┘  │
│       │             │           │              │         │
│  ┌────┴─────────────┴───────────┴──────────────┴──────┐  │
│  │          tux-core (shared library)                 │  │
│  │   Device table · Trait hierarchy · Platform types  │  │
│  └────────────────────────┬───────────────────────────┘  │
└───────────────────────────┼─────────────────────────────┘
                            │ sysfs / hidraw / procfs
┌───────────────────────────┴─────────────────────────────┐
│              tux-kmod (5 kernel shims)                   │
│                                                         │
│  tuxedo-ec · tuxedo-uniwill · tuxedo-clevo              │
│  tuxedo-nb04 · tuxedo-tuxi                              │
│                                                         │
│  Stateless passthrough — no policy in kernel             │
└─────────────────────────────────────────────────────────┘
```

**Data flow:** TUI → D-Bus method calls → daemon applies policy (fan curves,
profiles, safety limits) → reads/writes sysfs via `pread`/`pwrite` → kernel
shim translates to raw EC/WMI/ACPI operations → hardware responds. Sensor
data flows back the same path; the daemon broadcasts updates as D-Bus signals.

---

## tux-core — Hardware Model

Each TUXEDO device is described by a `DeviceDescriptor`. 40 named SKUs plus 5
platform fallbacks are in the device table:

```rust
struct DeviceDescriptor {
    name: &'static str,               // "TUXEDO Pulse 14 Gen4"
    product_sku: &'static str,        // DMI match key
    platform: Platform,               // NB05 | Uniwill | Clevo | NB04 | Tuxi
    fans: FanCapability,              // count, control type, scale
    keyboard: KeyboardType,           // None | White | Rgb1Zone | Rgb3Zone | PerKey
    sensors: SensorSet,               // available temps and RPMs
    charging: ChargingCapability,     // thresholds, profiles, priority
    tdp: Option<TdpBounds>,           // PL1/PL2/PL4 min/max per model
    registers: PlatformRegisters,     // EC addresses, WMI GUIDs
}
```

Adding a new laptop = adding a table entry. No new code paths unless the hardware
uses a genuinely new access method.

**Trait hierarchy** provides platform-independent interfaces:

| Trait              | Methods                                              |
|--------------------|------------------------------------------------------|
| `FanBackend`       | `read_temp`, `write_pwm`, `read_pwm`, `set_auto`, `read_rpm`, `num_fans` |
| `KeyboardBackend`  | `set_brightness`, `set_color`, `set_mode`, `zone_count` |
| `SensorBackend`    | `read_temperatures`, `read_fan_rpms`                 |
| `ChargingBackend`  | `get_thresholds`, `set_thresholds`                   |

Five platform implementations (NB05, Uniwill, Clevo, NB04, Tuxi) implement these
traits via platform-specific sysfs attributes.

ITE keyboard LEDs (8291, 8291-lb, 8297, 829x) are handled in userspace via
`/dev/hidraw*` — the daemon discovers HID devices by USB VID:PID and applies
per-model color scaling from the device table.

---

## tux-kmod — Kernel Shims

Five minimal C modules (~2900 LOC total):

| Module           | Hardware Access          | sysfs Interface             |
|------------------|--------------------------|-----------------------------|
| `tuxedo-ec`      | SuperIO port I/O (0x4e/0x4f) | Binary `ec_ram` attribute (64 KiB) |
| `tuxedo-uniwill` | ACPI EC + WMI BC (unified Uniwill platform) | Binary attributes per register + LED/fn_lock/charging |
| `tuxedo-clevo`   | WMI + ACPI DSM (dual transport) | Binary attributes per command |
| `tuxedo-nb04`    | WMI AB/BS methods        | Binary attributes per method |
| `tuxedo-tuxi`    | ACPI TFAN evaluation     | Binary attributes per fan   |

Design:
- **Stateless passthrough** — no fan curves, no LED logic, no profiles in kernel.
- **Binary sysfs attributes** — daemon uses `pread`/`pwrite` with offsets.
- **DKMS-ready** — each module has its own Makefile and dkms.conf.
- **No compatibility check module** — platform detection in userspace via DMI.

---

## tux-daemon — D-Bus Service

Root systemd service on the system bus (`com.tuxedocomputers.tccd`), with 10
D-Bus interfaces:

| Interface | Key Methods |
|-----------|------------|
| `Device` | `GetDevice()` — model info, capabilities |
| `Fan` | `GetFanStatus()`, `SetFanMode()`, `WriteFanCurve()`, `GetFanSpeeds()` |
| `Profile` | `GetProfiles()`, `SetProfile()`, `GetAssignments()`, `SetAssignments()` |
| `Keyboard` | `SetBrightness()`, `SetColor()`, `SetMode()` |
| `Charging` | `GetThresholds()`, `SetThresholds()` |
| `Cpu` | `GetGovernor()`, `SetGovernor()`, `GetCpuLoad()`, `GetPerCoreFrequencies()` |
| `GpuPower` | NVIDIA cTGP/dynamic boost control |
| `Settings` | Fn Lock, webcam, display settings |
| `System` | `GetSystemInfo()` — CPU/RAM/uptime |
| `TccCompat` | 58-method flat interface for backwards compatibility |

### Resource Efficiency

| Aspect              | Active (client connected)      | Idle (no client)               |
|---------------------|--------------------------------|--------------------------------|
| Fan curve engine    | Polls at 2s (configurable)     | Reduced to 10s                 |
| Sensor broadcast    | D-Bus signals at poll rate     | Disabled                       |
| Profile auto-switch | Monitors AC/battery (event-driven) | Same                      |
| CPU usage           | Minimal (one read + one write per tick) | Near zero             |

Client presence tracked via D-Bus `NameOwnerChanged` — no polling.

Event-driven where possible:
- **AC/battery** — inotify on `/sys/class/power_supply/`
- **USB HID** — udev monitor for ITE keyboard discovery
- **Suspend/resume** — systemd inhibitor locks

Configuration in `/etc/tux-daemon/config.toml`.

### Safety

- Fans auto-restore to hardware automatic mode on crash or shutdown.
- Minimum speed enforcement (default 25%) prevents silent fan stall.
- systemd `WatchdogSec` — daemon restarted if it stops responding.

### Profiles

- 4 built-in defaults: Max Energy Save, Quiet, Office, High Performance.
- Custom profiles: fan curve, CPU governor, TDP limits, brightness, keyboard, charging.
- AC/battery auto-switching — different profile per power state.
- TOML persistence in `/etc/tux-daemon/profiles/`.

---

## tux-tui — Terminal Interface

Built with ratatui and crossterm, following TEA (The Elm Architecture):
Model → Update → View.

**10 tabs:** Dashboard · Profiles · Fan Curve · Settings · Keyboard · Charging ·
Power · Display · Webcam · Info

- Runs unprivileged — all hardware writes go through D-Bus.
- Pure consumer — can be closed/reopened without affecting fan control or profiles.
- Number keys (1–9, 0) for direct tab access, `?` for help overlay.
- Form widget system for settings-style tabs with Text/Number/Bool/Select fields.
- Fan curve editor with interactive point navigation and insertion.
- CLI dump mode (`--dump-dashboard`, `--json`, etc.) for scripting.

A future GUI (e.g. GTK/Iced) would use the same D-Bus API with no daemon changes.
