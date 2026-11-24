# Stage 3 — Uniwill EC TDP backend via `tuxedo_io`

## Context

- Everything above the backend already exists: `TuxProfile.tdp {pl1, pl2}`,
  `profile_apply` step 6, D-Bus `Cpu.{Get,Set}Pl{1,2}` + `GetTdpBounds`, the TUI
  profile editor's `tdp_pl1/tdp_pl2` fields (shown once bounds exist, 0 = unset),
  TCC import of `odmPowerLimits.tdpValues[0..2]`.
- RAPL is wrong for this board: `constraint_0_max_power_uw` = 28 W (base TDP)
  while the EC programs 90 W, so `RaplTdp` would cap PL1 at 28 W.
- Vendor path (tuxedo-drivers 4.25.0, `tuxedo_io.c`), board `X5AR45xS`:
  `tdp_min = {10,10,10}`, `tdp_max = {90,90,230}` W; index 0/1/2 = PL1/PL2/PL4
  at EC 0x0783/0x0784/0x0785 (PL4 doubled when `uniwill_has_double_pl4`).

### Driver semantics that shape the design

| ioctl | Behaviour |
|-------|-----------|
| `R_UW_TDP{0,1,2}` `_IOR(0xEF, 0x18..0x1a)` | value, or **negative errno copied as the value** (ioctl returns 0) |
| `R_UW_TDP{0,1,2}_MIN` `_IOR(0xEF, 0x1b..0x1d)` | min, or `-ENODEV` if the board has no TDP table |
| `R_UW_TDP{0,1,2}_MAX` `_IOR(0xEF, 0x1e..0x20)` | max, or `-ENODEV` |
| `W_UW_TDP{0,1,2}` `_IOW(0xF0, 0x15..0x17)` | out-of-range → `-EINVAL` **swallowed** (ioctl still returns 0); with `uniwill_custom_profile_mode_needed` (true for X5AR45xS) the EC perf profile is forced to *overboost* first |

⇒ negative reads are errors; every write is clamped first and **verified by
read-back**.

TCC (3.1.2) `ODMPowerLimitWorker` writes all TDP values on every profile
activation, defaulting to the maxima. tux-rs only writes when a profile sets
PL1/PL2 (unchanged policy).

## Steps (one commit each)

### 3.1 feat(tuxedo_io): TDP ioctl constants
`tux-daemon/src/platform/tuxedo_io.rs`: `R_UW_TDP{0,1,2}`, `R_UW_TDP{0,1,2}_{MIN,MAX}`,
`W_UW_TDP{0,1,2}` + a unit test deriving them from an `_IOC` helper so the
hex literals can't drift.

### 3.2 feat(tdp): `UniwillEcTdp`
`tux-daemon/src/cpu/tdp.rs`:
- `UniwillEcTdp::probe(io: Arc<dyn TuxedoIo>) -> Option<Self>`: bounds from
  MIN/MAX for PL1/PL2 (required), PL4 optional (`pl4_min/pl4_max`, read-only);
  requires a successful PL1 read. Driver bounds are authoritative (the driver
  enforces them); `descriptor.tdp` is not used (same as RAPL). *Deviation from
  plan.md ("fall back to descriptor.tdp"): a fallback cannot work because the
  driver rejects reads/writes on boards without its table.*
- `get_pl{1,2}`: negative value → `io::Error::from_raw_os_error(-v)`.
- `set_pl{1,2}`: clamp to bounds (like `EcTdp`/`RaplTdp`), write, read back,
  error if the EC didn't take it.
- Tests with an EC simulator implementing `TuxedoIo` that mimics the driver
  (bounds table, silent `-EINVAL` on out-of-range writes, negative-errno reads).

### 3.3 feat(device): `TdpSource::UniwillEc` + IBM15I10 opt-in
- `tux-core/src/device.rs`: new variant (serde-compatible for custom devices).
- `build_backend`: `UniwillEc` opens `/dev/tuxedo_io`, requires
  `is_uniwill_hardware()`; hermetic seam `build_backend_with(desc, open_io)`.
- `IBM15I10`: `tdp_source: UniwillEc` (`tdp: None`); device test asserts it.

### 3.4 test(live): IBM15I10 TDP
`LiveExpectations.tdp_control = true` for IBM15I10; bounds 10–90 W; PL1/PL2
set→read-back→restore round-trip (IBM15I10 only, via a `tdp_roundtrip` flag, so
the maintainer's RAPL machine keeps today's checks).

### 3.5 docs
`docs/hardware_support.md` TDP opt-in table + IBM15I10 row; feature matrix.

### 3.6 Live verification (sudo restart)
- Startup log shows the EC TDP backend with 10–90 W bounds.
- `just live-test` round-trip passes; cross-check RAPL
  `constraint_{0,1}_power_limit_uw` follows the EC values.
- TUI profile editor shows PL1/PL2 fields bounded at 90 W.

## Out of scope
PL4 write (trait has PL1/PL2 only) → follow-up; NB05/other Uniwill boards.

## Outcome (2026-11-24)

3.1–3.6 done. Live: EC TDP read-back and *enforcement* verified (PL 25 W →
package 25.0 W); RAPL registers do not mirror EC limits, so 3.6's "RAPL follows
the EC" check was replaced by a measured load test. See `worklog-3.md`.
