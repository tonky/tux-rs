# Stage 4: Device table expansion & full TCC parity

## Context
Align our internal `custom_devices.toml` and device tables with all SKUs exposed by the upstream `tuxedo-drivers` DMI match tables to achieve full feature parity.

## Files to modify
- **`tux-core/src/dmi/devices.rs`**: Expand mapped SKUs or create automatic logic to map DMI product details into the correct `td_*` backend platform.
- **`tux-core/src/registers/`**: Update device definition and clean up missing references to EC registers that are no longer accessible without `tux-kmod`.

## Details
- Verify fallback handling if a specific chassis name isn't formally found but its `tuxedo-drivers` module loaded successfully.
