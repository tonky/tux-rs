# tux-rs Progress

> Last updated: 2026-04-06

## Overview

| Phase | Name | Status | Progress |
|-------|------|--------|----------|
| 1 | Project Scaffolding | Done | 1/1 |
| 2 | tux-core: Hardware Model | Done | 4/4 |
| 3 | tux-kmod: Kernel Shims | Done | 3/3 |
| 4 | Fan Control + D-Bus | Done | 4/4 |
| 5 | Profile Management | Done | 3/3 |
| 6 | Terminal Interface | Done | 5/5 |
| 7 | Feature Parity | Done | 5/5 |
| 8 | Polish & Release | In progress | 6/9 |
| | **Total** | | **36/38** |

## Detailed Status

### Phase 1 — Project Scaffolding
- [x] 1.1 Workspace + Build Infrastructure

### Phase 2 — tux-core: Hardware Model System
- [x] 2.1 Platform Types + DeviceDescriptor
- [x] 2.2 Trait Hierarchy + Mock Backend
- [x] 2.3 Device Table (40+ SKUs)
- [x] 2.4 DMI Platform Detection

### Phase 3 — tux-kmod: Kernel Shims
- [x] 3.1 Uniwill + Tuxi Shims
- [x] 3.2 EC + Clevo Shims
- [x] 3.3 NB04 Shim + DKMS Packaging

### Phase 4 — Fan Control + D-Bus Core
- [x] 4.1 Fan Curve Engine
- [x] 4.2 Platform Backends (Uniwill focus)
- [x] 4.3 D-Bus Server + systemd
- [x] 4.4 Safety + Graceful Shutdown

### Phase 5 — Profile Management
- [x] 5.1 Profile Data Model + TOML Persistence
- [x] 5.2 AC/Battery Auto-Switching
- [x] 5.3 Profile D-Bus API

### Phase 6 — Terminal Interface
- [x] 6.1 TEA Architecture + Shell
- [x] 6.2 Dashboard + Info Tabs
- [x] 6.3 Fan Curve Editor + Form Widget
- [x] 6.4 Profiles Tab
- [x] 6.5 Remaining Tabs (Settings, Keyboard, Charging, Power, Display, Webcam)

### Phase 7 — Feature Parity
- [x] 7.1 ITE Keyboard Backlight (HID)
- [x] 7.2 Charging Thresholds
- [x] 7.3 TDP + CPU Governor Control
- [x] 7.4 Suspend/Resume + WMI Events
- [x] 7.5 NVIDIA GPU Power Control

### Phase 8 — Polish & Release
- [x] 8.1 Integration Test Harness
- [x] 8.2 End-to-End Tests
- [x] 8.3 Whole Repo Review
- [x] 8.4 Feature Parity Evaluation
- [x] 8.5 Implementation of Missing Pieces
- [x] 8.6 TCC Compatibility Shim
- [ ] 8.7 Documentation & Migration
- [ ] 8.8 Packaging (deb/rpm/PKGBUILD)
- [ ] 8.9 Discovered Items & Polish
