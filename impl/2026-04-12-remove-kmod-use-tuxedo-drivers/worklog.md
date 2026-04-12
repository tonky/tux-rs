# Worklog: Remove kmod, use tuxedo-drivers

## 2026-04-12 — Investigation & Planning

- Read issue #4: remove kernel code, reuse tuxedo-drivers, repo becomes daemon+TUI only
- Explored full tux-rs codebase architecture (3 Rust crates, 5 C kernel modules)
- Researched tuxedo-drivers repo: module structure, sysfs/ioctl/hwmon interfaces
- Key finding: interface mismatch is significant — tuxedo-drivers uses ioctl chardev for Clevo/Uniwill (not sysfs), hwmon for sensors, LED subsystem for keyboards
- NB05 is cleanest migration path: tuxedo-drivers has proper sysfs fan control + hwmon sensors
- Created 6-stage plan prioritizing NB05 (Pulse 14 Gen 4 user request)
- User confirmed: must support all hardware legacy TCC supports
