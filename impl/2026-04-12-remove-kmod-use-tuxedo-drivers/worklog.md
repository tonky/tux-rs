# Worklog: Remove kmod, use tuxedo-drivers

## 2026-04-12
- Investigated `tux-daemon` backend platform directory mappings and build scripts (`Justfile`, `flake.nix`).
- Created and initialized the 6 detailed stage plans (`stage-1.md` through `stage-6.md`) with context, targets, and file diffing expected for the migration from `tux-kmod` to `tuxedo-drivers`.

## Stage 1 — NB05 backend (td_nb05.rs)
- Created `tux-daemon/src/platform/td_nb05.rs` with `TdNb05FanBackend`.
- Paths: `tuxedo_nb05_fan_control` sysfs, `tuxedo_nb05_sensors` hwmon.
- `discover_hwmon()` walks dir for first `hwmon*` subdir.
- 9 unit tests using `TempDir` + `with_paths()` constructor.
- Updated `mod.rs` to prefer td backend, fall back to legacy.

## Stage 2 — Clevo & Uniwill via tuxedo_io (td_clevo.rs, td_uniwill.rs)
- Read full ioctl header `tuxedo_io_ioctl.h`; computed ioctl codes manually.
- Created `tuxedo_io.rs` with `TuxedoIo` trait + `TuxedoIoDevice` + `MockTuxedoIo`.
- Created `td_clevo.rs`: packed u32 fan info, Mutex-guarded write, `R/W_CL_FANINFO*`.
- Created `td_uniwill.rs`: EC scale 0-200 ↔ PWM 0-255 conversion, per-fan R/W.
- Updated `mod.rs` for Clevo/Uniwill prefer tuxedo-drivers first with hw check.
- 338 tests passing.

## Stage 3 — Tuxi & NB04 (td_tuxi.rs, td_nb04.rs)
- Key finding: tuxi driver registers as `tuxedo_fan_control` (not `tuxedo_tuxi_fan_control`).
- Created `td_tuxi.rs`: optional hwmon (older firmware may lack RPM/temp).
- Created `td_nb04.rs`: `Nb04Profile` enum, profile-only control; `write_pwm` = Unsupported.
- Updated `mod.rs`: Tuxi prefers td backend, NB04 still returns None from `init_fan_backend`.
- 356 tests passing.

## Review of Stages 1–4 (2026-04-12)
Two sub-agents reviewed the complete implementation. Issues addressed:

**Correctness fixed (MAJOR):**
- `td_uniwill.rs`: `ec_to_pwm` — used `ec as u16` which wraps on negative `i32`; replaced with `.clamp(0, EC_PWM_MAX as i32) as u16`.
- `td_nb04.rs`: `set_auto` had no bounds check on `fan_index`; added `check_fan_index`.
- `td_clevo.rs`: `write_pwm` only read `0..max_fans` fan slots, zeroing the third slot; now reads all `CLEVO_MAX_FANS` slots preserving EC values.

**Refactoring (MAJOR/MODERATE):**
- Extracted `discover_hwmon`, `check_fan_index`, `fan_attr`, `PWM_ENABLE_MANUAL/AUTO` to `sysfs.rs`.
- `td_nb05.rs`, `td_tuxi.rs`, `td_nb04.rs` updated to use shared utilities.

**API consistency (MODERATE):**
- `td_tuxi.rs`: `read_fan_rpm` was `Ok(0)` when hwmon absent; changed to `Err(Unsupported)` to match `read_temp` behaviour.
- Device table: removed dead `AURA14GEN4`/`AURA15GEN4` individual entries (never matched on real hardware); only combined entry remains.

**Test additions:**
- `dmi.rs`: Added `exact_sku_match_ibp16_gen8` (priority regression test for Gap 2 from stage 2 spec).
- `td_nb04.rs`: Added `set_auto` out-of-range assertion to `out_of_range_fan_index_errors`.

**Conformance gaps not fixed (tracked in follow_up.toml):**
- f002/f005: Charging control sysfs paths not updated for tuxedo-drivers (tux-kmod paths still in charging/*.rs). Must be done before Stage 5 kmod removal.
- f003: NB04 keyboard backlight via `tuxedo_nb04_kbd_backlight` LED subsystem not implemented.
- f004: Device table SKU sweep vs tuxedo-drivers DMI tables incomplete; no reconciliation artifact.

All tests passing (648+), zero clippy warnings.

## Stage 4 — Device table & DMI detection for tuxedo-drivers
- Research: most "missing" vendor SKUs (POLARIS1501*, POLARIS1701*, PULSE1401/1501, AURA1501)
  use `DMI_BOARD_NAME` not `DMI_PRODUCT_SKU` — fall to platform fallback, no new entries needed.
- Critical fix: added `"AURA14GEN4 / AURA15GEN4"` combined SKU entry (the actual string hardware reports).
  Existing separate AURA14GEN4 / AURA15GEN4 entries kept for documentation.
- dmi.rs: Added `CLEVO_WMI_EVENT_GUID` (0F6B) and `UNIWILL_WMI_EVENT_GUID_2` (0F72) constants.
- `detect_platform()`: Clevo/Uniwill now also detected via WMI GUIDs (tuxedo-drivers path).
  Tuxi now also detected via `/sys/devices/platform/tuxedo_fan_control/`.
- NB04 shim check: now accepts both `/tuxedo-nb04/` (tux-kmod) and `/tuxedo_nb04_sensors/` (tuxedo-drivers).
- Added 7 new tests covering all tuxedo-drivers detection paths.
- All tests pass (647+), zero clippy warnings.

## Review of Stages 1–4 (2026-04-12)
Two sub-agents reviewed all implemented changes for conformance and code quality.

**Correctness bugs fixed (MAJOR):**
- `td_uniwill.rs` `ec_to_pwm`: `ec as u16` wraps silently on negative hardware values; replaced with `.clamp(0, EC_PWM_MAX as i32) as u16`.
- `td_nb04.rs` `set_auto`: accepted out-of-range `fan_index`; added bounds check for API consistency.
- `td_clevo.rs` `write_pwm`: only read `0..max_fans` fan slots for packed i32; now reads all `CLEVO_MAX_FANS` slots to preserve EC values for unmanaged fans.

**Refactoring (MAJOR/MODERATE):**
- Extracted `discover_hwmon`, `check_fan_index`, `fan_attr`, `PWM_ENABLE_MANUAL/AUTO` to `sysfs.rs` as shared platform utilities; removed triplicated copies from `td_nb05.rs`, `td_tuxi.rs`, `td_nb04.rs`.
- Fixed redundant closure `|p| SysfsReader::new(p)` → `SysfsReader::new` in `td_tuxi.rs`.

**API consistency (MODERATE):**
- `td_tuxi.rs` `read_fan_rpm`: changed from `Ok(0)` to `Err(Unsupported)` when hwmon absent, consistent with `read_temp`.
- Device table: removed dead `AURA14GEN4`/`AURA15GEN4` individual entries (never matched on real hardware).

**Documentation:**
- `td_clevo.rs` `set_auto`: documented why `write_i32` rather than `ioctl_noarg` (W_CL_FANAUTO is `_IOW`, not `_IO`).

**New tests:**
- `dmi.rs`: `exact_sku_match_ibp16_gen8` — regression test for InfinityBook Pro 16 Gen 8 (stage 2 priority test gap).
- `td_nb04.rs`: `out_of_range_fan_index_errors` extended to also test `set_auto`.

**Conformance gaps tracked in follow_up.toml (not fixed now):**
- f002/f005: Charging paths not updated for tuxedo-drivers (must be done before Stage 5).
- f003: NB04 keyboard backlight (`tuxedo_nb04_kbd_backlight`) not implemented.
- f004: Device table SKU sweep vs tuxedo-drivers incomplete.

All tests passing (648+), zero clippy warnings after all fixes.
