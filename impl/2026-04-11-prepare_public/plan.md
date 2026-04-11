# High Level Plan: Prepare public release

## State review
Code is clean, properly formatted, architecture sits well, tests pass. Minor format issues were fixed.
`TccCompat` interface provides a baseline structure that's good enough to keep basic compatibility and old UI stable.
The previous rewrite phase (Phase 8) left documentation and packaging open, as well as a few bugs like `SetChargingSettings` DBus missing, unpersisted charging configs, and keyboard lighting not physically triggering. We also want to simplify adding new hardware.

## Stages

### Stage 1: Hardware Extensibility & Documentation
- Implement `custom_devices.toml` overriding table to skip recompilation for new known-platform laptops.
- Add `--dump-hardware-spec` flags
- Create `docs/ADDING_HARDWARE.md`

### Stage 2: Bugfixes and Polish
- Fix missing `SetChargingSettings` DBus method
- Fix charging settings persistence
- Fix keyboard lighting sync to hardware
- Fix TCC "CPU Power" compatibility crash
- Fix flaky `config_change_bypasses_hysteresis` test

### Stage 3: UI & Enhancements
- Read and display battery cycle count in TUI
- Separate unsupported hardware capability flags from unsupported UI flags
- Improve `--debug` logging



### Stage 4: OSS Readiness
- Add `CONTRIBUTING.md`
- Add `LICENSE` (GPL-3.0)
- Update `README.md`
- Create `.github/ISSUE_TEMPLATE` and `PULL_REQUEST_TEMPLATE`
