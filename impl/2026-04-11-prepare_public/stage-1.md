# Stage 1: Hardware Extensibility & Documentation

## Description
In this stage, we are decoupling the strict static typing requirement for adding existing hardware variants. We will support a TOML file overrides map, implement helper commands for discovering hardware parameters, and document how to add completely new hardware models.

## Tasks
- [x] Investigate and parse `custom_devices.toml` overriding `device_table.rs` logic.
- [x] Add a CLI switch to `tux-daemon` for `--dump-hardware-spec` (dumps raw product SKU/board params).
- [x] Create `docs/ADDING_HARDWARE.md` providing two scenarios: editing the TOML, and implementing a new hardware platform in Rust.
