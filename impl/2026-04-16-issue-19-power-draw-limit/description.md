# Feature Description: Power Draw (TDP) Limit via Intel RAPL

Addresses [GitHub issue #19](https://github.com/tonky/tux-rs/issues/19) (reporter: @Ciugam).

The reporter asks whether `tux-rs` can cap CPU power draw in watts — similar
in spirit to [FlyGoat/RyzenAdj](https://github.com/FlyGoat/RyzenAdj) — and
notes interest in iGPU power control as well.

## Scope (for this feature)

- **In scope**: Intel RAPL-based PL1/PL2 control, exposed through the existing
  `TdpBackend` trait, wired into the TUI profile editor. Enabled **only on
  devices where vendor `tuxedo-drivers` already sanctions TDP control**:
  - `IBP16I08MK2` (InfinityBook Pro 16 Gen8 MK2) — primary dev machine;
    `tuxedo_io.c:184` registers TDP for this SKU.
  - `IBP14I08MK2` (InfinityBook Pro 14 Gen8 MK2) — same Gen8 Intel family,
    same vendor TDP support.
- **Explicitly NOT enabled** (vendor does not expose TDP, so we won't either):
  - `IBP14A09MK1` / `IBP15A09MK1` — AMD InfinityBook Pro 14/15 Gen9. Confirmed
    in `tuxedo_io.c:166` (`uw_id_tdp()` returns `-ENODEV` for these). Also
    listed as an unsupported cTGP device in
    `TuxedoControlCenterDaemon.ts:542`.
  - Any Intel InfinityBook Pro 14 Gen9 variant — the reporter in issue #19's
    `brave soul` note confirmed `tux-rs` works on "IBP14G9", but that is
    almost certainly the AMD variant above. No Intel Gen9 SKU appears in
    vendor drivers or TCC. Unless we get a confirmed Intel Gen9 DMI SKU +
    evidence vendor supports TDP there, we will **not** enable it.
  - `IBP14A10MK1` / `IBP15A10MK1` / `IBP15I10MK1` — Gen10 variants, no vendor
    TDP support.
- **Out of scope (deferred)**:
  - AMD TDP control (needs MSR / `ryzen_smu` / libryzenadj port — much larger
    effort; tracked as follow-up).
  - iGPU (AMD or Intel) power draw control.
  - EC-based TDP rollout beyond NB05 (already present but ungated).

### Policy: opt-in only

RAPL exists on virtually any Intel laptop as a generic CPU feature. That does
**not** mean we should surface it everywhere. We'll gate RAPL activation on
an explicit `TdpSource` field on each `DeviceDescriptor` so that a new device
row must be authored deliberately after cross-checking vendor drivers/TCC.
This mirrors the existing convention: `charging`, `fans`, `gpu_power`, and
`tdp` bounds are all per-row capability declarations. Adding `TdpSource`
keeps the table the single source of truth for "what this laptop supports".

## Investigation Findings

### What already exists

1. **Profile model** (`tux-core/src/profile.rs:164`)
   ```rust
   pub struct TdpSettings {
       pub pl1: Option<u32>,  // watts
       pub pl2: Option<u32>,  // watts
   }
   ```
   Embedded in `TuxProfile.tdp: Option<TdpSettings>`.
2. **Hardware capability** (`tux-core/src/device.rs:99`)
   `TdpBounds { pl1_min, pl1_max, pl2_min, pl2_max, pl4_min, pl4_max }` stored
   in `DeviceDescriptor.tdp: Option<TdpBounds>`.
3. **Backend trait** (`tux-daemon/src/cpu/tdp.rs:20`)
   ```rust
   pub trait TdpBackend: Send + Sync {
       fn get_pl1(&self) -> io::Result<u32>;
       fn set_pl1(&self, watts: u32) -> io::Result<()>;
       fn get_pl2(&self) -> io::Result<u32>;
       fn set_pl2(&self, watts: u32) -> io::Result<()>;
       fn bounds(&self) -> &TdpBounds;
   }
   ```
   Only implementor today: `EcTdp` (NB05, EC RAM offsets `0x0783`/`0x0784`).
4. **Profile apply** (`tux-daemon/src/profile_apply.rs:361`) already writes
   PL1/PL2 via the optional `tdp_backend` when the profile specifies them.
5. **D-Bus** (`tux-daemon/src/dbus/cpu.rs:77`) already exposes `get_pl1`,
   `set_pl1`, `get_pl2`, `set_pl2`, `get_tdp_bounds`.

### What's missing

1. **No RAPL backend** — Intel systems (incl. IBP16G8, IBP14G9) cannot control
   TDP via `tux-rs` today.
2. **No TUI editor fields** for PL1/PL2. The Power tab only exposes
   `tgp_offset` for the dGPU.
3. **No device rows enable TDP**: every entry has `tdp: None`, so even the
   existing `EcTdp` backend never gets constructed. `IBP14I09` row does not
   exist in the device table yet and will be added if needed.

### Why Intel RAPL (not RyzenAdj) first

- Both target machines are Intel Core (Raptor Lake / Meteor Lake class) — RAPL
  applies directly.
- RAPL is the standard, vendor-documented sysfs interface at
  `/sys/class/powercap/intel-rapl:0/` (package domain). No kernel module
  outside of mainline `intel_rapl_common` is required.
- Bounds (`constraint_0_max_power_uw`, `constraint_1_max_power_uw`) come from
  firmware/hardware — no need to hard-code them per SKU. This means RAPL
  support generalises to many Intel Tuxedo laptops cheaply.
- RyzenAdj-equivalent for AMD is a separate initiative (tracked as follow-up).

### Notable gotchas for RAPL

- sysfs unit is **microwatts**; profile field is **watts**. Convert at the
  backend boundary.
- `constraint_0_*` = long-term (PL1), `constraint_1_*` = short-term (PL2).
- Some firmwares lock RAPL limits (MSR `MSR_PKG_POWER_LIMIT` bit 63).
  Writes return `EPERM` / `EACCES`; surface clearly without crashing the
  daemon. A read-only banner in the TUI is acceptable for locked systems.
- Only the `package-0` domain is controlled; core/uncore sub-domains are
  ignored for now.
- Existing `EcTdp` and `RaplTdp` are mutually exclusive at a point in time.
  Selection precedence: if `/sys/class/powercap/intel-rapl:0` exists and the
  device row opts in (or no EC TDP is declared), use RAPL; otherwise fall
  back to EC.
