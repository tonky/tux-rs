# Plan: AMD APU detection + hide unsupported Power-tab elements

Companion to `description.md`. Iterate with the user before opening
stage files.

## High-level approach

Two narrow daemon-side fixes (correct GPU classification, derive
`gpu_control`), one TUI-side change (gate Power form on capabilities), one
small surface-area decision on the dashboard power-draw line. No new
hardware backends, no AMD TDP work.

## Proposed stages

### Stage 1 — GPU info pipeline: classification + D-Bus contract

This stage fixes three coupled bugs that together prevent the iGPU/dGPU
panels from lighting up. Splitting them across stages would mean shipping
half-fixes that are not user-visible — keep them together.

**Bug A — `amdgpu` always classified as Discrete.**
File: `tux-daemon/src/gpu/hwmon.rs`. Replace the static
`("amdgpu", GpuType::Discrete)` row with a runtime classifier using
`boot_vga` (kernel-marked primary VGA), read via the hwmon's `device/`
symlink: `<hwmon_dir>/device/boot_vga` → `1` means primary VGA → on a
single-GPU APU laptop this is the integrated Radeon. `nvidia` / `i915` /
`xe` keep their static classification — only `amdgpu` needs the runtime
check. If `boot_vga` is missing entirely (very old kernels), `amdgpu`
falls back to the legacy `Discrete` mapping; document the limitation in
`follow_up.toml` (Stage 5).

**Bug B — `GetGpuInfo` D-Bus method always returns an error.**
File: `tux-daemon/src/dbus/system.rs:83-86`. The current code does
`toml::to_string(&Vec<GpuInfo>)`, which returns `Err("unsupported rust
type")` because TOML requires a table at the root. Verified with a
standalone probe in `tmp/toml_probe/` (2026-04-19). Fix: serialize a
proper top-level struct. Use the existing
`GpuInfoResponse { gpus: Vec<GpuData> }` from
`tux-core/src/dbus_types.rs:54`. Convert from `hwmon::GpuInfo` to
`GpuData` at the boundary (carry `gpu_type` as `"discrete"` /
`"integrated"`).

**Bug C — TUI consumer parses a contract that the daemon never spoke.**
File: `tux-tui/src/update.rs:1099-1119`. Current code reads flat
`dgpu_name` / `igpu_name` / `dgpu_temp` / `dgpu_usage` / `dgpu_power` /
`igpu_usage` keys at the top level; daemon was supposed to send a
`GpuInfoResponse`. Fix: deserialize `GpuInfoResponse`, then pivot into
`model.power.dgpu_*` / `igpu_*` based on `GpuData.gpu_type`. Multiple
GPUs of the same type: prefer the first entry, log a debug event for
extras (rare on Tuxedo hardware).

**Tests:**
- `tux-daemon/src/gpu/hwmon.rs`:
  - Split existing `discover_amdgpu` into two: integrated
    (`boot_vga = 1`) and discrete (`boot_vga = 0`).
  - `discover_amdgpu_missing_boot_vga_falls_back_to_discrete`.
  - Hybrid case: `amdgpu` discrete + `nvidia` discrete + `i915`
    integrated → 3 entries with correct types.
- `tux-core/src/dbus_types.rs`: reuse the existing roundtrip test
  (`GpuInfoResponse` ↔ TOML), already at line 271 — no change needed.
- `tux-tui/src/update.rs`: replace the existing mock at line 2283 with
  a `GpuInfoResponse`-shaped TOML; assert `model.power.dgpu_*` /
  `igpu_*` populate correctly. Add a hybrid (1 dGPU + 1 iGPU) case.
- Add a daemon integration test that `GetGpuInfo` now returns a
  parseable `GpuInfoResponse` (regression for Bug B).

**Acceptance:** on a synthetic sysfs tree mimicking IBP14G9 (single
`amdgpu` with `boot_vga = 1`), the TUI Power tab shows the iGPU name in
the iGPU panel and the dGPU panel collapses (collapse logic is Stage 3,
but the pipeline data is correct after Stage 1).

### Stage 2 — Derive `gpu_control` capability from NB02 backend presence

- File: `tux-daemon/src/dbus/settings.rs:139` (and its caller).
- Plumb the constructed `Option<Box<dyn GpuPowerBackend>>` (already built
  in the daemon) into `SettingsInterface::new`, mirroring how
  `tdp_available` is handled today (see `tdp_control:tdp_available` at
  line 137).
- Update `CapabilitiesResponse { gpu_control: gpu_available, ... }`.
- Tests:
  - Add `capabilities_report_gpu_control_when_backend_present` /
    `..._false_when_absent` integration tests in
    `tux-daemon/tests/integration.rs`, modelled on the two existing
    `tdp_control` tests at lines 494 / 567.

### Stage 3 — TUI: gate Power form fields and the whole tab on capabilities

- Files: `tux-tui/src/model.rs`, `tux-tui/src/update.rs`,
  `tux-tui/src/views/power.rs`.
- Today `PowerState::new()` builds a single hardcoded `TGP Offset` field
  and sets `form_tab.supported = true`. Two changes:
  1. Build the field list dynamically from the capabilities frame the TUI
     already receives. Mirror the pattern used for `model.display.supported`
     and `model.webcam.form_tab.supported` (both set in
     `tux-tui/src/update.rs` from daemon responses).
  2. Set `power.form_tab.supported = caps.gpu_control || caps.tdp_control
     || any other future Power-tab capability`.
- The existing `if !state.form_tab.supported` branch in
  `views/power.rs:14-20` already prints the right message.
- **Collapse the dGPU panel entirely** when no dGPU is reported (i.e.
  `state.dgpu_name` is empty AND no telemetry fields are populated). The
  iGPU panel takes the full width in that layout. View code in
  `tux-tui/src/views/power.rs:34-69` switches from a fixed 50/50
  horizontal split to a conditional layout. Symmetric collapse for the
  iGPU panel when only a dGPU is present (e.g. desktops, hypothetical
  dGPU-only laptops) is a free-roll once the conditional layout is in.
- Tests:
  - Adjust `power_state_renders_without_gpu_data` and add a variant for
    `caps.gpu_control = false`.
  - Add an `update.rs` test that, given a capabilities frame with both
    flags false, leaves `power.form_tab.fields` empty and `supported =
    false`.

### Stage 4 — Dashboard package-power line: probe + AMD fallback

- File: `tux-daemon/src/dbus/system.rs:180-239`.
- `EnergySampler::sample()` already returns `None` when the file can't be
  read; `get_package_power_w()` swallows that to `0.0`. Change the surface
  to honestly signal absence: return `Option<f64>` (or omit the field on
  the TOML frame) and have the TUI render the line only when present.
- **AMD fallback**: probe `/sys/class/hwmon/*/name == "amd_energy"` and use
  its `energy*_input` counter (microjoules) when `intel-rapl:0` is missing.
  Same delta-over-dt formula as `EnergySampler`. If neither source exists,
  do not publish `package_power_w` at all (TUI hides the line).
- Tests:
  - Daemon: tests for (a) intel-rapl present → publish, (b) only amd_energy
    present → publish, (c) neither → omit.
  - TUI: a unit test that the dashboard status line omits the "Pkg" field
    when the value is absent.

### Stage 5 — Docs + follow-ups + branch wrap-up

- Update `docs/hardware_support.md` to clearly list AMD APU iGPU detection
  as a supported runtime feature (and AMD TDP control as not).
- Update `docs/feature_support_matrix.md` row for cTGP to note runtime
  capability gating, and add a row for AMD package-power via `amd_energy`.
- Add follow-up entries to `follow_up.toml`:
  - AMD `ryzen_smu` / RyzenAdj-equivalent backend (deferred from #19).
  - `tgp_offset` `i8`-vs-`u8` sign-mismatch fix.
- README hardware-support note: clarify that "works on IBP14G9" now also
  means iGPU shows up and unsupported Power-tab fields are hidden.
- This stage closes the branch. User will draft the GitHub-comment reply
  to @Ciugam separately, after the PR is up.

## Sequencing & risk

- Stages 1, 2 are independent and can land separately.
- Stage 3 depends on Stage 2 (needs `gpu_control` to be honest).
- Stage 4 is independent of all of the above; can land first if convenient.
- Stage 5 is documentation / bookkeeping; do last.

Risk surface is small: every change is gated either by capability flags
already in the contract or by sysfs existence checks. No vendor-driver
poking, no new write paths, no new device rows.

## Resolved decisions (2026-04-19)

1. GPU classification signal — agent's call. **Resolved**: use
   `boot_vga` (kernel-marked primary VGA) read via the hwmon's `device/`
   symlink, applied only to `amdgpu` entries. `nvidia`/`i915`/`xe` keep
   their static classification. If `boot_vga` is missing entirely (older
   kernels), `amdgpu` falls back to the legacy `Discrete` mapping —
   document the limitation in `follow_up.toml`. A second pass that uses
   "only one amdgpu device on the system → integrated" is rejected as
   over-engineering for the current target hardware.
2. **Resolved**: add `amd_energy` hwmon fallback for the dashboard
   package-power line. If neither `intel-rapl:0` nor `amd_energy` is
   present, the field is omitted from the daemon frame and the TUI
   hides the "Pkg" segment.
3. **Resolved**: collapse the dGPU panel when no dGPU is reported.
   Conditional layout in `views/power.rs`.
4. **Resolved**: Stage 5 closes the branch. User drafts the GitHub
   reply separately after the PR is up.

Plan is locked. Next: draft `stage-1.md` (GPU classification) for
review and confirmation per AGENTS.md before any code changes.
