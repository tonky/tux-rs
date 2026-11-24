# Stage 1 — Detection + device descriptor

No root required. One commit per step.

## 1.1 fix(dmi): match instance-suffixed WMI device names

Context: `tux-core/src/dmi.rs:61` — `SysFsDmiSource::wmi_guid_exists` checks
`/sys/bus/wmi/devices/{GUID}`. Since the kernel WMI bus started naming devices
`{GUID}-{instance}` (observed on 7.3: `ABBC0F72-8EA1-11D1-00A0-C90629100000-13`),
the exact path never exists.

Change:
- Extract `fn wmi_guid_present_in(dir: &Path, guid: &str) -> bool` that scans
  the directory and returns true when an entry equals `guid` or is
  `guid` + `-` + ASCII digits (case-insensitive on the GUID part).
- `SysFsDmiSource::wmi_guid_exists` delegates with `/sys/bus/wmi/devices`.

Tests (tempdir, `tempfile` is already a tux-core dependency):
- bare GUID dir matches (old kernels);
- `GUID-13` matches (regression for this machine);
- lower-case dir name matches upper-case constant;
- `GUID-foo` / different GUID sharing a prefix / missing dir → false.

## 1.2 test(gpu): hermetic NB02 probe test

Context: `tux-daemon/src/gpu/nb02.rs:92` probes real `/sys`; fails on any NB02
machine with the `tuxedo_nb02_nvidia_power_ctrl` module loaded.

Change: add `Nb02GpuPower::probe(path)` (used by `new()`), test against a
non-existent tempdir path and a tempdir without `ctgp_offset`, plus a positive
probe test.

## 1.3 feat(device): IBM15I10 descriptor

Context: `tux-core/src/device_table.rs` (Uniwill section, after IBP Gen8 / OMNIA).

Row (values justified in `description.md`):
```rust
DeviceDescriptor {
    name: "TUXEDO InfinityBook Max 15 Gen10 Intel",
    product_sku: "IBM15I10",
    platform: Platform::Uniwill,
    fans: FanCapability { count: 2, control: FanControlType::Direct, pwm_scale: 200 },
    keyboard: KeyboardType::IteHid(IteModel::Ite8291),
    sensors: SensorSet { cpu_temp: true, gpu_temp: true, fan_rpm: &[true, true] },
    charging: ChargingCapability::EcProfilePriority,
    tdp: None,
    tdp_source: TdpSource::None,
    gpu_power: GpuPowerCapability::Nb02Nvidia,
    registers: PlatformRegisters::Uniwill,
}
```
Tests:
- `device_table`: lookup returns the row with the expected capabilities.
- `dmi`: exact SKU match using real DMI strings (`NB02`, `X5AR45xS`, product name).

## 1.4 fix(hid): identity colour scaling for IBM15I10

Context: `tux-daemon/src/hid/color_scaling.rs:36`. Vendor `ite_8291.c` uses no
scaling for board `X5AR45xS`; ours falls into the generic `0x600b` arm.
Add `("IBM15I10", 0x600b) => IDENTITY` before the generic arm + unit test.

## 1.5 docs

- `docs/hardware_support.md`: SKU count 40 → 41; note IBM15I10.
- `README.md`: mention InfinityBook Max 15 Gen10 Intel as tested.

## Exit criteria

- `just check` green (fmt, clippy `-D warnings`, tests).
- `./target/debug/tux-daemon --dump-startup-diagnostics` reports
  `probe.wmi.uniwill_guid_2=true`.
