# tux-rs Implementation Roadmap

Unified Rust rewrite of TUXEDO laptop drivers + control center.
Merges patterns from two validated prototypes: **tuxedo-drivers-rs** and **tcc-rs**.

## Architecture

```
┌──────────────────────────────────────────────────────┐
│  tux-tui              Binary crate (ratatui)         │
│  (future: tux-gui)    Runs unprivileged              │
├──────────────────────────────────────────────────────┤
│              D-Bus: com.tuxedocomputers.tccd          │
├──────────────────────────────────────────────────────┤
│  tux-daemon           Binary crate                   │
│                       Root service, systemd-managed   │
├──────────────────────────────────────────────────────┤
│  tux-core             Library crate                  │
│  platform/ hw/ backend/ fan/ profile/ keyboard/      │
├──────────────────────────────────────────────────────┤
│  tux-kmod             5 C kernel shims               │
│  tuxedo-ec · tuxedo-uw-fan · tuxedo-clevo            │
│  tuxedo-nb04 · tuxedo-tuxi                           │
├──────────────────────────────────────────────────────┤
│  Hardware   EC ports · WMI · ACPI DSM · USB HID      │
└──────────────────────────────────────────────────────┘
```

## Phases Overview

| Phase | Name | Days | Sub-phases | Depends on |
|-------|------|------|------------|------------|
| 1 | [Project Scaffolding](phase-1-scaffolding/) | 1 | 1 | — |
| 2 | [tux-core: Hardware Model](phase-2-tux-core/) | 4 | 4 | Phase 1 |
| 3 | [tux-kmod: Kernel Shims](phase-3-tux-kmod/) | 3 | 3 | Phase 2 |
| 4 | [Fan Control + D-Bus](phase-4-fan-control-dbus/) | 4 | 4 | Phase 2, 3 |
| 5 | [Profile Management](phase-5-profile-management/) | 3 | 3 | Phase 4 |
| 6 | [Terminal Interface](phase-6-tui/) | 5 | 5 | Phase 4 |
| 7 | [Feature Parity](phase-7-feature-parity/) | 5 | 5 | Phase 4 |
| 8 | [Polish & Release](phase-8-polish-packaging/) | 9 | 9 | Phase 7 |
| | **Total** | **34** | **34** | |

## Dependency Graph

```
Phase 1 ──→ Phase 2 ──→ Phase 3 ──→ Phase 4 ──┬──→ Phase 5
                                                ├──→ Phase 6 ──┐
                                                └──→ Phase 7 ──┴──→ Phase 8
```

## Key Decisions

- **Domain modeling:** Use domain types and language everywhere — APIs, config, internal code. No magic constants, minimum bare primitive values. Enums over strings, newtypes over raw integers.
- **Kernel shims:** C only (proven ~1200 LOC from tuxedo-drivers-rs). Rust rewrite deferred.
- **Config format:** TOML everywhere (daemon config + profiles).
- **D-Bus bus:** System bus for production, session bus for development.
- **TCC compatibility:** Compat shim (Phase 8.6) exposes the original TCC flat D-Bus interface (58 methods, JSON) so the Angular GUI works as a drop-in.
- **Test hardware:** InfinityBook (Uniwill) as primary validation target.
- **Day scope:** 6 hours per sub-phase.

## Prototype References

| Source | Key Patterns |
|--------|-------------|
| **tuxedo-drivers-rs** | FanBackend trait, 5 platform modules, HID keyboard (4 ITE families), D-Bus server, TOML config, 56 tests |
| **tcc-rs** | TUI (10 tabs, TEA architecture), profiles (CRUD + auto-switch), D-Bus client (30+ methods), form widgets, 126 tests |

## Progress

See [PROGRESS.md](PROGRESS.md) for current status at a glance.
