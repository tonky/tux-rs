# Adding Hardware to tux-rs

`tux-rs` is designed to cleanly separate the hardware logic from the UI and backend logic. We strictly treat **Hardware Models as Data**.

If you have an unsupported TUXEDO computer (or compatible OEM chassis), there are two ways to add support depending on the underlying hardware controller.

## Scenario A: Known Platform (Fast Path, No Compilation)

Most TUXEDO laptops belong to standard ODM groups like **Clevo, Uniwill**, or custom architectures like **NB04 (Sirius)** or **NB05 (Pulse/InfinityFlex)**. The daemon already knows how to control these platforms if the correct kernel modules are loaded (`tuxedo-keyboard`, `tuxedo-nb04`, etc.).

If your laptop belongs to an existing platform but just has an unknown SKU string, you can add it dynamically without touching Rust code.

### 1. Dump your Hardware Specs
Run the daemon in dump mode to extract your laptop's unique identifiers:
```sh
sudo tux-daemon --dump-hardware-spec
```
You will see output like:
```text
--- TUXEDO Hardware Spec ---
board_vendor    = TUXEDO
board_name      = NB05_BOARD
product_sku     = NEWGEN1402
sys_vendor      = TUXEDO
product_name    = TUXEDO Pulse 14 Gen5
product_version = Rev2
----------------------------
```

### 2. Create the Override File
Create a `custom_devices.toml` file in `/etc/tux-daemon/`:
```sh
sudo touch /etc/tux-daemon/custom_devices.toml
```

### 3. Add your Device Descriptor
Using the `product_sku` extracted earlier, add your laptop's capabilities to the file. Below is an example.

```toml
[[device]]
name = "TUXEDO Pulse 14 Gen5"
productSku = "NEWGEN1402"
platform = "Nb05" # Must be one of: Nb05, Nb04, Uniwill, Clevo, Tuxi

[device.fans]
count = 2
control = "Direct"
pwmScale = 255 # Usually 200 for Uniwill, 255 for everything else

[device.keyboard]
type = "WhiteLevels" # None, White, WhiteLevels, Rgb1Zone, Rgb3Zone, RgbPerKey
[device.keyboard.value] # Only needed for enums with values like WhiteLevels(3)
WhiteLevels = 3

[device.sensors]
cpuTemp = true
gpuTemp = false
fanRpm = [true, true]

[device.charging]
type = "None" # None, Flexicharger, EcProfilePriority

[device.gpuPower]
type = "None"

[device.registers]
type = "Nb05"
num_fans = 2
fanctl_onereg = false
```

Restart `tux-daemon` and your device will instantly be tracked correctly without any Rust recompilation!
If you find a layout that perfectly supports your model, please submit a PR to `tux-core/src/device_table.rs` so it is officially supported out-of-the-box!

---

## Scenario B: Entirely New Platform (Rust Integration)

If your laptop relies on completely new sysfs mappings, a new kernel shim (like a newly developed `tuxedo-foo`), and does not fit any existing backend, you will need to add it to the Rust codebase.

### 1. Add Platform Enum
Add your new platform to `tux-core/src/platform.rs`:
```rust
pub enum Platform {
    // ...
    NewPlatformFoo,
}
```

### 2. Define Register Paths
Add a struct in `tux-core/src/registers.rs` referencing sysfs paths or logic.
```rust
pub struct FooRegisters {
    pub sysfs_base: &'static str,
}
```

### 3. Create the Backend Implementations
Create a new module in `tux-daemon/src/fan/foo.rs` or `tux-daemon/src/charging/foo.rs`.
Implement the control traits:
- `FanBackend` (in `tux-daemon/src/platform/traits.rs`)
- `ChargingBackend` (in `tux-daemon/src/charging/traits.rs`)

### 4. Link the Backend to the Daemon
Inside `tux-daemon/src/platform/mod.rs`, map your exact platform to your newly implemented `FanBackend`.

### 5. Add it to the Device Table
Finally, add your laptop's `DeviceDescriptor` block natively to `tux-core/src/device_table.rs`.

If you have any questions along the way, open an Issue!
