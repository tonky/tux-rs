# Worklog

## 2026-04-16
- Triage of [issue #19](https://github.com/tonky/tux-rs/issues/19) — reporter
  (@Ciugam) asks for RyzenAdj-style power-draw limiting and raises iGPU power
  as a follow-up.
- Codebase survey:
  - `TdpSettings` / `TdpBounds` / `TdpBackend` trait already in place; only
    implementor is `EcTdp` (NB05 EC RAM).
  - No device in `DEVICE_TABLE` currently has `tdp: Some(..)`, so the backend
    is effectively dormant today.
  - TUI Power tab exposes only `tgp_offset`; no PL1/PL2 editor fields exist.
  - Target machines (IBP16G8 MK2 = `Platform::Uniwill`; IBP14G9 not yet in
    table) are Intel; standard path is `/sys/class/powercap/intel-rapl:0`.
- Scope agreed with user: Intel RAPL backend + TUI fields, enabled on
  IBP16G8 MK2 and IBP14G9. AMD/RyzenAdj and iGPU control deferred.
- Feature directory `impl/2026-04-16-issue-19-power-draw-limit/` created
  with `description.md` + `plan.md`. Awaiting plan approval before authoring
  stage-N docs.
- Vendor-source investigation of IBP14G9 variants (against
  `vendor/tuxedo-drivers` and `vendor/tuxedo-control-center`):
  - AMD `IBP14A09MK1` / `IBP15A09MK1` exist but have no entry in
    `tuxedo_io.c:166` `uw_id_tdp()` — `-ENODEV`. TCC also lists them as
    unsupported cTGP devices (`TuxedoControlCenterDaemon.ts:542`).
  - No Intel Gen9 SKU (`IBP1xI09*`) is referenced by vendor code.
  - Only Gen8 Intel (`IBP1XI08MK2`) currently has vendor-supported TDP
    (bounds PL1 5-45 W, PL2 5-60 W, PL3 0-110 W in `tuxedo_io.c:184`).
- Updated plan to make TDP activation **strict opt-in** via a new
  `TdpSource { None, Ec, Rapl }` field on `DeviceDescriptor`. Stage 4 will
  flip only `IBP16I08MK2` and `IBP14I08MK2` to `Rapl`. Every other row stays
  `None`, including Gen9 AMD.
- Plan confirmed (units: integer watts). Authored `stage-1.md` with detailed
  `RaplTdp` design: single-file change in `tux-daemon/src/cpu/tdp.rs`,
  sysfs-backed with the existing `SysfsReader`, hermetic tempfile tests,
  firmware-lock (`EPERM`) path covered. Awaiting stage-1 confirmation.
- Stage 1 implemented + reviewed + gates green. Details in `worklog-1.md`
  and `review-1.md`. 373 daemon lib tests passing; 10 new RAPL tests cover
  get/set round-trips, floor clamping, bounds probing, wrong-`name` domain
  rejection, wrong `constraint_N_name` rejection, unparseable-bounds
  rejection, missing-base directory. Firmware-lock permission test
  intentionally skipped (root bypasses DAC); deferred to Stage 4 live-test.
  Seven review items carried forward in `follow_up.toml`.
