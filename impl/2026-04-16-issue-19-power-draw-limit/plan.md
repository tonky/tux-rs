# High-Level Plan

Four stages. Each stage is gated on user confirmation per AGENTS.md.

## Stage 1 — RAPL backend in `tux-daemon`

Add a new `RaplTdp` struct implementing `cpu::tdp::TdpBackend` that talks to
`/sys/class/powercap/intel-rapl:0/`.

Key files:
- `tux-daemon/src/cpu/tdp.rs` — extend with `RaplTdp` alongside `EcTdp`.

Behaviours:
- `RaplTdp::probe()` returns `Option<RaplTdp>`:
  - requires `intel-rapl:0/name` to contain `package-0`;
  - reads `constraint_0_max_power_uw` / `constraint_1_max_power_uw` to build
    `TdpBounds` (watts). Minimums default to 1 W unless firmware exposes
    `constraint_*_min_power_uw`.
- `get_pl1 / get_pl2` read `constraint_{0,1}_power_limit_uw`, divide by 1e6,
  round to `u32` watts.
- `set_pl1 / set_pl2` clamp to bounds, multiply by 1e6, write as decimal
  microwatts. Wrap `EPERM` / `EACCES` into a clearly-typed error so upper
  layers can surface "RAPL locked by firmware" without propagating panic.
- `bounds()` returns the bounds cached at `probe()` time.

Tests (hermetic, tempfile-based):
- Happy-path round-trip: set/get across PL1/PL2 with values inside bounds.
- Clamping above `max_power_uw` and below min.
- Locked-domain path: write returns `PermissionDenied` -> typed error surfaces.
- Bounds are read correctly from sysfs fixture.

Follow the existing `EcTdp` test style (tempfile sysfs tree).

## Stage 2 — Backend selection + profile wiring

Goal: have the daemon pick the right backend per device and keep
`profile_apply` unchanged. Selection is **strictly opt-in per device row** —
we never probe RAPL blindly on laptops whose vendor drivers don't sanction
TDP control.

Key files:
- `tux-daemon/src/cpu/tdp.rs` — add a `build_backend(device)` factory.
- `tux-daemon/src/main.rs:302` — replace the inline EC-only construction with
  the factory.
- `tux-core/src/device.rs` — add an enum describing which backend a device
  uses:
  ```rust
  pub enum TdpSource {
      None,   // no TDP control (default; existing behaviour)
      Ec,    // EC RAM (NB05, existing EcTdp backend)
      Rapl,  // Intel RAPL sysfs (new backend in Stage 1)
  }
  ```
  Add `tdp_source: TdpSource` to `DeviceDescriptor` with default `None`.
  Every existing row stays `None`; we only flip specific rows in Stage 4.

Selection rules (in `build_backend`):
1. `TdpSource::None` → no backend (current behaviour for every device today).
2. `TdpSource::Ec` → require `descriptor.tdp` bounds present; try
   `EcTdp::new(bounds)`. If construction fails, log a warning and return
   `None` (unchanged error path).
3. `TdpSource::Rapl` → try `RaplTdp::probe()`. Bounds are read from sysfs, so
   `descriptor.tdp` is ignored. If the intel-rapl tree is missing (e.g. user
   installed on an unsupported kernel, or forced-swapped CPU), log a clear
   warning and return `None` — do NOT fall through to another backend.
4. No `Auto` mode. Never probe RAPL on a device whose row doesn't opt in.

Profile apply stays as-is — it already operates through `dyn TdpBackend`.

Tests:
- Factory returns an `RaplTdp` when `TdpSource::Rapl` + sysfs fixture is
  present.
- Factory returns `None` when `TdpSource::Rapl` but sysfs is missing, and
  emits a warning (no panic, no fallthrough).
- Factory returns an `EcTdp` when `TdpSource::Ec` + bounds present.
- Factory returns `None` for `TdpSource::None` regardless of sysfs state
  (safety net for AMD boxes that happen to expose some powercap tree).

## Stage 3 — TUI profile editor fields for PL1/PL2

Key files:
- `tux-tui/src/model.rs` — extend `ProfilesState::build_editor_form` and
  `apply_form_to_profile` with two new fields.

Form fields:
- `PL1 W (0=Unset)` — `Number { min: 0, max: pl1_hw_max, step: 1 }`.
- `PL2 W (0=Unset)` — `Number { min: 0, max: pl2_hw_max, step: 1 }`.
- `0 -> None`, `>0 -> Some(value)` — mirrors the CPU-freq convention.

Bounds:
- Fetch via existing D-Bus `get_tdp_bounds` at startup and cache in
  `ProfilesState::tdp_bounds: Option<TdpBounds>` (new field).
- If bounds are absent (no backend), hide both fields from the form.

Tests:
- Round-trip: set values via form -> back into profile -> back into form.
- `0` maps to `None` for both fields.
- Form-field visibility toggles correctly on bounds presence/absence.

## Stage 4 — Device table + docs + live tests

Key files:
- `tux-core/src/device_table.rs` — set `tdp_source: TdpSource::Rapl` on the
  two vendor-sanctioned Gen8 Intel rows:
  - `IBP16I08MK2` (row at line 551) — primary dev machine.
  - `IBP14I08MK2` (row at line 531–550 area) — same Gen8 Intel family,
    shares the same `uw_id_tdp()` DMI match in `tuxedo_io.c:184`.
  All other rows stay at `TdpSource::None`. In particular, do **not** add
  Gen9 AMD (`IBP14A09MK1`, `IBP15A09MK1`) or any Intel IBP14G9 variant until
  vendor support is confirmed.
- `docs/hardware_support.md` — add a "TDP (RAPL)" column/note reflecting the
  strict opt-in policy above.
- `README.md` — bullet in features list; call out the Gen9 AMD exclusion so
  the reporter's `brave soul` note doesn't set wrong expectations.
- `Justfile` — add `just live-test-tdp` if helpful (runs the PL1/PL2
  regression against a live daemon).
- `tux-daemon/tests/integration_tdp_rapl.rs` (new) — integration test that
  stands up a fake RAPL sysfs tree and drives profile apply end-to-end.

Tests / gates:
- `just check` (fmt + clippy + test) is green.
- `just live-test` gets a new regression matching the CPU-freq pattern.
- Two sub-agent reviews per stage per AGENTS.md (Opus 4.6 + Gemini 3.1 Pro).

## Cross-cutting notes

- `TdpSettings` struct already matches this feature (no schema change).
- No D-Bus API change is needed; existing methods cover PL1/PL2/bounds.
- `follow_up.toml` will track:
  - AMD / RyzenAdj backend (future, separate feature)
  - iGPU power control (future, separate feature)
  - RAPL `package-1` (dual-socket, not applicable to laptops but keep as TODO)
  - UX: surface "RAPL locked by firmware" in the Power tab status line.

## Out-of-stage risks

- **Firmware lockout**: if both targets have MSR bit 63 set, the set path is
  a no-op. Stage 1 must make this a clear, non-fatal error so we can ship
  the read-only UX while users investigate BIOS settings.
- **IBP14G9 variant ambiguity**: the reporter's note mentions "IBP14G9". The
  only Gen9 SKUs in vendor sources are AMD (`IBP14A09MK1` /
  `IBP15A09MK1`) and vendor does NOT sanction TDP there. Stage 4 explicitly
  skips them. If a confirmed Intel IBP14G9 SKU surfaces later (plus vendor
  TDP support), it can be added as a follow-up row without reopening
  Stages 1–3.
- **RAPL present on unsanctioned hardware**: Any Intel laptop exposes
  `/sys/class/powercap/intel-rapl:0`, so adding RAPL per-row (not blanket
  probe) is essential. The Stage-2 factory enforces this.
