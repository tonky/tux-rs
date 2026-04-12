# Stage 2: Clevo & Uniwill backends via tuxedo_io ioctl

## Context
Unlike the custom text sysfs interface in `tux-kmod`, `tuxedo-drivers` uses an `ioctl` chardev at `/dev/tuxedo_io` for Clevo and Uniwill platforms. We need an `ioctl` client to handle basic data commands.

## Files to modify
- **[NEW] `tux-daemon/src/platform/tuxedo_io.rs`**: Ioctl client that defines standard structs and read/write wrappers mapping commands to `/dev/tuxedo_io`.
- **[NEW] `tux-daemon/src/platform/td_clevo.rs`**: Backend for Clevo platform replacing `clevo.rs`. It will utilize `tuxedo_io`.
- **[NEW] `tux-daemon/src/platform/td_uniwill.rs`**: Backend for Uniwill platform utilizing `tuxedo_io`.
- **`tux-daemon/src/platform/mod.rs`**: Hook up `TdClevoFanBackend` and `TdUniwillFanBackend`.

## Details
- Implement charging control (Clevo flexicharger and Uniwill charge profiles) via `tuxedo_keyboard` sysfs interfaces.
- Add robust tests mocking the ioctl calls using a dummy device or traits.
- **Priority Testing:** Ensure full compatibility and functionality specifically for the InfinityBook Pro 16 Gen 8 (Uniwill platform), as it will be the primary test hardware.
