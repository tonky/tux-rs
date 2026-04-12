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

## Stage 5 — Remove tux-kmod, update packaging
- Deleted `tux-kmod/` directory entirely (5 C shim kernel modules no longer needed).
- Deleted `nix/tux-kmod.nix` Nix package derivation.
- Removed all `kmod-*` recipes from `Justfile` (kmod-build, kmod-build-one, kmod-clean, kmod-install, kmod-remove, kmod-load, kmod-unload, kmod-reload, kmod-swap, plus `kmod_version`/`kmod_src` variables).
- Updated `default.nix`: removed `tux-kmod = pkgsWithRust.callPackage ./nix/tux-kmod.nix {...}`.
- Updated `nix/overlay.nix`: removed `tux-kmod` entry.
- Updated `flake.nix`: removed `tux-kmod` from `inherit (tux-rs)` packages and from the flake overlay export.
- Updated `nixos/default.nix`:
  - `kernelModules.package` default changed from `pkgs.tux-kmod` → `pkgs.linuxPackages.tuxedo-drivers`.
  - `boot.kernelModules` list changed from tux-kmod module names (tuxedo_ec, tuxedo_clevo, etc.) to tuxedo-drivers names (tuxedo_io, tuxedo_nb05_fan_control, tuxedo_nb05_sensors, tuxedo_nb04_sensors, tuxedo_nb04_power_profiles, tuxedo_fan_control).
- Updated `README.md`: removed "Kernel Modules" section, removed "### Kernel module development" with kmod-* commands, removed "### 1. Kernel modules (DKMS)" installation section; added tuxedo-drivers prerequisite note; renumbered Installation sections.
- All tests passing (648+), zero errors.

## Stage 6 — Cleanup & validation
- Deleted old tux-kmod fan backends: `nb05.rs`, `clevo.rs`, `uniwill.rs`, `tuxi.rs`.
- Updated `platform/mod.rs`: removed `mod`/`use` declarations for old backends; removed all fallback branches from `init_fan_backend` — now uses tuxedo-drivers td_* backends exclusively.
- Simplified `PlatformRegisters` enum to unit variants: removed all structs (`Nb05Registers`, `UniwillRegisters`, `ClevoRegisters`, `Nb04Registers`, `TuxiRegisters`) and all dead fields (`sysfs_base`, `num_fans`, `fanctl_onereg`, `max_fans`). All tux-kmod-specific addressing was in these fields, and the td_* backends use hard-coded tuxedo-drivers sysfs paths.
- Updated all callsites: `device.rs` tests, `device_table.rs` (all device entries, via perl multi-line regex), `custom_device.rs`, `dbus/settings.rs` test.
- Fixed `device_table.rs` test `nb05_infinityflex_has_one_fan`: replaced `fanctl_onereg` check with `platform` + `registers` equality assertions.
- `CustomPlatformRegisters` in `custom_device.rs` simplified to unit variants (config schema no longer requires `sysfs_base` or other tux-kmod path fields).
- Fixed unused import warnings introduced by `cargo fix` clobbering test-only imports (`PathBuf`, `HashMap`, `Mutex`) — restored them under `#[cfg(test)]`.
- Added `Default` impl for `MockTuxedoIo` (clippy suggestion, `#[cfg(test)]`).
- All 607 tests passing (reduction from 648 expected: deleted old backend test suites), zero clippy warnings in library code.

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

## Stage 7 — Post-migration polish (2026-04-12)

All 4 phases from external review implemented. 629 tests passing (up from 608), 0 clippy warnings.

**Phase B (stale comment):**
- `td_uniwill.rs` doc comment: removed "consistent with the legacy sysfs-based `UniwillFanBackend`" — that backend is gone.

**Phase A (fan telemetry accuracy):**
- `tux-core/src/dbus_types.rs`: added `rpm_available: bool` (#[serde(default)]) to `FanData`; added new `FanHealthResponse { status, consecutive_failures }` struct.
- `tux-daemon/src/dbus/fan.rs`: added `FanInterface::failure_counter` field (`Arc<AtomicU32>`); added `get_fan_data(fan_index) -> String` (TOML-encoded FanData with duty + rpm_available); added `get_fan_health() -> String` (TOML-encoded FanHealthResponse); 4 new tests.
- `tux-tui/src/dbus_client.rs`: added `get_fan_data(fan)` and `get_fan_health()` client methods.
- `tux-tui/src/event.rs`: added `fan_duties: Vec<u8>` and `fan_rpm_available: Vec<bool>` to `DashboardTelemetry`; added `FanHealth(String)` variant.
- `tux-tui/src/model.rs`: added `duty_percent: u8`, `rpm_available: bool` to `FanData`; added `fan_health: Option<String>` to `DashboardState`.
- `tux-tui/src/dbus_task.rs`: polls `get_fan_data(i)` instead of `get_fan_speed(i)` (with `get_fan_speed` as fallback for older daemons); polls `get_fan_health()` per tick.
- `tux-tui/src/update.rs`: `speed_percent` now derived from `duty_percent * 100 / 255` (PWM-authoritative), not `rpm / max_rpm`; handles `FanHealth` variant; 3 new tests.
- `tux-tui/src/views/dashboard.rs`: fan gauge label shows `"Fan N (~XX%)"` when `rpm_available == false`, else `"Fan N (MMMM RPM)"`; shows yellow/red health warning line in status block.

**Phase C (error injection):**
- `tux-daemon/src/platform/tuxedo_io.rs`: added `fail_reads: AtomicBool`, `fail_writes: AtomicBool` to `MockTuxedoIo`; setters `set_fail_reads()` / `set_fail_writes()`; read/write/noarg impls check flags; 5 new tests.
- `tux-daemon/src/platform/td_clevo.rs`: 4 new error path tests (read_temp failure, write_pwm failure, partial failure, set_auto failure).
- `tux-daemon/src/platform/td_uniwill.rs`: 3 new error path tests (read_temp, write_pwm, set_auto failures).

**Phase D (fan engine health):**
- `tux-daemon/src/fan_engine.rs`: `FanCurveEngine` gains `consecutive_failures: Arc<AtomicU32>`; incremented on `read_temp` error, reset to 0 on success; `failure_counter()` getter; 2 new tokio tests.
- `tux-daemon/src/dbus/fan.rs`: `FanInterface::failure_counter` wired from engine via `failure_counter()` call in main.rs before engine is moved into task; `DbusConfig` gained `fan_failure_counter` field.
- `tux-daemon/src/dbus/mod.rs`, `main.rs`, `tests/common/mod.rs`: plumbed `fan_failure_counter` through the wiring.
- Thresholds: ≥5 → "degraded", ≥30 → "failed" (hardcoded per spec).

## Stage 7 follow-up — Charging EIO hardening (2026-04-12)
- Investigated live-regression charging failures via daemon debug logs. Observed repeated `Input/output error (os error 5)` on Uniwill charging sysfs, often after rapid profile-apply cycles.
- Hardened `ProfileApplier` charging writes to avoid unnecessary EC writes:
  - Reads current profile/priority first.
  - Skips write if value is already current.
  - If current-value read fails, still attempts write (with warning) so profile activation remains effective under transient read errors.
- Increased Uniwill charging sysfs retry budget from `3x50ms` to `10x100ms` for transient `ErrorKind::Other` (`EIO`) read/write failures.
- Added regression tests in `profile_apply.rs`:
  - `apply_charging_skips_redundant_profile_priority_writes`.
  - `apply_charging_attempts_writes_when_read_fails`.
- Validation run:
  - New unit tests pass.
  - Full privileged live regression could not be rerun from this session because non-interactive `sudo` is unavailable (`sudo-rs: interactive authentication is required`).

## Stage 7 follow-up — TCC/tccd alignment (2026-04-12)
- Reviewed upstream TCC/tccd charging flow in vendor sources:
  - `ChargingWorker` applies charging settings as best-effort and returns boolean success.
  - `GetCurrentChargingProfile/GetCurrentChargingPriority` return daemon settings state, not strict hardware readback each call.
  - Errors are logged and handled gracefully in client-facing paths.
- Adjusted tux-rs charging handling accordingly:
  - Kept profile-apply write guards and fallback write attempt on read errors.
  - Broadened transient EIO detection to include errno 5 paths (`raw_os_error() == Some(5)`).
  - `GetChargingSettings` now retries whole-read and falls back to cached/config settings instead of hard-failing on transient EIO.
  - Live regression Uniwill charging section now uses retry helper for initial charging snapshot fetch.
- Validation:
  - Targeted daemon charging tests pass.
  - `just live-test` now passes fully on IBP Gen8 (including Charging — Uniwill Profiles and final PASSED banner).

## Stage 7 follow-up — Keyboard brightness regression guard in live workflow (2026-04-12)
- Added daemon keyboard regression tests to `just live-test` preflight so keyboard state and hardware-forwarding behavior are always checked before hardware live regression:
  - `keyboard_state_roundtrip`
  - `set_keyboard_state_forwards_color_and_mode_to_hardware`
  - `apply_scales_profile_keyboard_brightness_to_hardware`
- Added `profile_apply.rs` unit test `apply_scales_profile_keyboard_brightness_to_hardware` to lock profile keyboard brightness semantics at 0-100% input mapped to 0-255 hardware scale with flush.

## Stage 7 follow-up — Keyboard 50% does not illuminate (2026-04-12)
- Root cause: on ITE keyboard backends, after an explicit off state, `set_brightness()` updates internal value but does not re-enable LEDs. Removing `turn_on()` caused nonzero brightness writes to keep keyboard dark.
- Fixed in both runtime paths:
  - `dbus/settings.rs::set_keyboard_state`: for nonzero brightness do `set_brightness -> turn_on -> set_brightness -> flush`; for zero do `turn_off -> flush`.
  - `profile_apply.rs`: same on/off sequencing when applying profile keyboard brightness.
- Added/updated tests:
  - `set_keyboard_state_forwards_color_and_mode_to_hardware` now asserts turn-on path for nonzero brightness.
  - New `set_keyboard_state_zero_turns_off_hardware` test.
  - Existing `apply_scales_profile_keyboard_brightness_to_hardware` still passes with new sequencing.

## Stage 7 follow-up — Battery cycle count 0 regression (2026-04-12)
- Reproduced on host sysfs: `BAT0/raw_cycle_count=36` while `BAT0/cycle_count=0`.
- Root cause: daemon preferred `/sys/devices/platform/tuxedo_keyboard/raw_cycle_count` then fell back directly to `BAT*/cycle_count`; it did not read `BAT*/raw_cycle_count`.
- Fix in `dbus/system.rs::read_battery_info`:
  - prefer `BAT*/raw_cycle_count` when >0
  - else try `/sys/devices/platform/tuxedo_keyboard/raw_cycle_count`
  - else fallback to `BAT*/cycle_count`
- Added regression test `battery_info_prefers_bat_raw_cycle_count`.
- Targeted tests passing:
  - `battery_info_cycle_count_fallback`
  - `battery_info_prefers_bat_raw_cycle_count`
  - `battery_info_from_sysfs`

## Stage 7 follow-up — White keyboard 50% no illumination (2026-04-12)
- Reproduced with live checks: on this host `max_brightness=2`, previous mapping wrote brightness `1` for 50%, which may remain visually dark on some firmware.
- Fix in `hid/sysfs_kbd.rs` (`SysfsWhiteKeyboard::set_brightness`):
  - keep `0 -> 0` (off)
  - for `max_brightness <= 2`, treat any nonzero request as `max_brightness` (binary on/off behavior)
  - keep rounded scaling for higher-step devices.
- Updated test `white_brightness_scales_with_rounding` to reflect low-step binary mapping.
- Live verification after deploy:
  - `SetKeyboardState brightness=50` -> `/sys/class/leds/white:kbd_backlight/brightness=2`
  - `SetKeyboardState brightness=0` -> brightness `0`
  - `GetBatteryInfo` reports `cycle_count=36`.

## Stage 7 follow-up — Kernel-level keyboard control validation (2026-04-12)
- Manual kernel interface writes performed directly:
  - `trigger=none`
  - `brightness=0 -> 2`
  - Sysfs readback changed accordingly, but keyboard remained physically dark.
- Enabled `tuxedo_keyboard` dynamic debug and captured kernel logs during direct and DBus-triggered writes.
- Confirmed root cause is kernel-side on this host:
  - repeated `uniwill_wmi: WMI read error: 0x1808/0x078c, data: 0xfe`
  - `tuxedo_keyboard: uniwill_leds_set_brightness(): uniwill_write_kbd_bl_white() failed`
- Conclusion: daemon/TUI path reaches kernel node, but firmware/driver WMI EC access for white keyboard brightness fails on this machine.
- Safety UX improvement added in daemon: `SetKeyboardState` now propagates hardware write failures rather than silently succeeding, so UI can show actionable error when kernel writes fail.

## Stage 7 follow-up — Restore 2-stage white brightness behavior (2026-04-12)
- User feedback: keyboard on/off works, but no visual distinction between 50% and 100%.
- Adjusted `SysfsWhiteKeyboard` mapping for `max_brightness=2` to preserve two hardware stages:
  - `0 -> 0`
  - `1..127 -> 1`
  - `128..255 -> 2`
- Updated `white_brightness_scales_with_rounding` test accordingly.
- Live verification via D-Bus and sysfs readback after deploy:
  - `brightness=50%` -> `after50=1`
  - `brightness=100%` -> `after100=2`

## Stage 7 follow-up — Final keyboard root cause and persistence (2026-04-12)
- User hardware feedback: no illumination unless `uniwill_wmi` direct EC mode is enabled.
- Confirmed working runtime fix:
  - `/sys/module/uniwill_wmi/parameters/ec_direct_io = Y`
  - keyboard illumination works with distinct 2-stage levels.
- Added `Justfile` recipe `enable-uniwill-ec-direct` to persist and apply:
  - writes `/etc/modprobe.d/99-tuxedo-uniwill-ec-direct.conf` with `options uniwill_wmi ec_direct_io=1`
  - reloads Uniwill/tuxedo modules
  - restarts daemon
  - prints effective `ec_direct_io` state.

## Stage 7 follow-up — TUI cycle count stale output fix (2026-04-12)
- Investigated user report that Info tab showed an implausible cycle count (`32804`).
- Confirmed data path: Info view renders `battery.cycle_count` from `DbusUpdate::BatteryInfo`.
- Root cause: `tux-tui` fetched battery info only once in `fetch_info_data()` at startup, so Info tab could display stale values until reconnect/restart.
- Fix: added periodic battery refresh in `dbus_task::poll_dashboard_checked()` by polling `GetBatteryInfo` and emitting `DbusUpdate::BatteryInfo` every tick.
- Added headless verification helper in CLI mode:
  - new `--dump-battery-info` command in `tux-tui/src/cli.rs`.
  - prints parsed `BatteryInfoResponse` as JSON for quick live checks.
- Validation:
  - `cargo test -p tux-tui parse_dump_battery_info` passed.
  - `cargo run -q -p tux-tui -- --dump-battery-info` shows `"cycle_count": 36` on host.

## Stage 7 follow-up — Cycle count oscillation (36 vs 12836) fix (2026-04-12)
- Reproduced live oscillation in sysfs and daemon output:
  - `/sys/class/power_supply/BAT0/raw_cycle_count` intermittently returned `36`, `50`, `52`, `12836`, `13348`.
  - `GetBatteryInfo` mirrored these values, confirming this was not a TUI formatting issue.
- Root cause: firmware/driver exposes unstable packed/noisy values via `raw_cycle_count` on this platform.
- Hardening in `tux-daemon/src/dbus/system.rs`:
  - added `normalize_raw_cycle_count(raw)` to decode packed values by low-byte fallback (`12836 -> 36`, `13348 -> 36`).
  - added `read_battery_raw_cycle_count(bat_path)` that samples `raw_cycle_count` 5 times and picks the minimum non-zero normalized sample to reject transient spikes.
  - `read_battery_info()` now uses this robust sampled raw value.
- Added regression test `battery_info_normalizes_large_bat_raw_cycle_count`.
- Validation:
  - `cargo test -p tux-daemon battery_info_prefers_bat_raw_cycle_count` passed.
  - `cargo test -p tux-daemon battery_info_normalizes_large_bat_raw_cycle_count` passed.

## Stage 7 follow-up — Nix packaging polish from external feedback (2026-04-12)
- Reviewer feedback was relevant and actionable for packaging/docs ergonomics.
- `flake.nix`:
  - removed unused `rust-overlay` input.
  - switched to singular public exports `overlay` and `nixosModule`.
  - retained compatibility aliases `overlays.default` and `nixosModules.default`.
  - updated VM test import to `self.nixosModule`.
  - dev shell now uses `pkgs.rustc` + `pkgs.cargo` directly.
- NixOS module path cleanup:
  - moved module source from top-level `nixos/default.nix` to `nix/nixos.nix`.
  - removed now-empty top-level `nixos/` directory.
- `default.nix`:
  - simplified non-flake interface by dropping `rust-overlay` argument and related branching.
  - exports now match flake naming (`overlay`, `nixosModule`) with compatibility aliases.
  - imports module from `./nix/nixos.nix`.
- `README.md` Nix docs cleanup:
  - switched examples to `inputs.tux-rs.nixosModule` / `tux-rs.nixosModule`.
  - removed npins/rust-overlay/per-package-override sections that were incorrect or unnecessary for current setup.
  - clarified that NixOS integration builds/loads `tuxedo-drivers` for the configured kernel package.
- Validation:
  - `nix --extra-experimental-features 'nix-command flakes' flake show --no-write-lock-file` succeeds and exposes both new singular attrs and compatibility aliases.

## Stage 7 follow-up — Nix naming sync in docs + full test run (2026-04-12)
- Updated remaining implementation notes under `impl/2026-04-11-nixos-support/` to align with current naming and layout:
  - singular exports: `nixosModule` / `overlay`
  - module path: `nix/nixos.nix`
- Updated one historical wording line in this worklog to avoid stale `overlays.default` reference in Stage 5 summary.
- Validation:
  - `just test` passed across the full workspace.
  - Aggregate results: 657 passed, 0 failed, 2 ignored.

## Stage 7 follow-up — fmt/clippy remediation (2026-04-12)
- Addressed clippy failure (`too_many_arguments`) in `tux-daemon/src/dbus/fan.rs` by introducing `FanInterfaceDeps` and changing `FanInterface::new` to accept the deps struct.
- Updated fan interface construction in `tux-daemon/src/dbus/mod.rs` accordingly.
- Ran `just fmt-fix` to apply rustfmt normalization and then re-ran checks.
- Validation:
  - `just fmt` passed.
  - `just clippy` passed (`-D warnings`).

## Stage 7 follow-up — CI fmt mismatch (`init_system.rs`) (2026-04-12)
- Investigated GitHub Actions formatting failure on `tux-daemon/tests/init_system.rs` (`ExecStart` assertion line wrapping).
- Confirmed local file now matches rustfmt single-line output for the reported block.
- Validation:
  - `cargo fmt --all -- --check` passed.

## Stage 7 follow-up — Fan fault fallback softened on IBP Gen8 (2026-04-12)
- Investigated periodic fan bursts on InfinityBook Pro 16 Gen8 (Uniwill path).
- Confirmed fan engine safety behavior was too aggressive for transient temperature-read faults.
- Updated `tux-daemon/src/fan_engine.rs`:
  - keep the previously computed PWM for the first 4 consecutive temp-read failures
  - on the 5th consecutive failure, ramp to 60% safety PWM instead of 100%
- Added/updated regression tests for:
  - transient read failure keeps prior PWM
  - persistent read failure ramps to reduced safety speed
- Validation:
  - `cargo test -p tux-daemon transient_temp_read_failure_keeps_last_pwm`
  - `cargo test -p tux-daemon repeated_temp_read_failure_sets_reduced_safety_speed`
  - `cargo clippy -p tux-daemon -- -D warnings`
  - `just test`
  - `just ci`

## Stage 7 follow-up — Implausible temp filter with CPU load gate (2026-04-12)
- Added implausible-temperature handling in `tux-daemon/src/fan_engine.rs` for Uniwill-style spikes (e.g. 120°C).
- Policy:
  - requires 5 consecutive implausible samples before acting on them;
  - if CPU load is below 30%, skips acting on the implausible sample and keeps previous PWM.
- Reused existing daemon CPU sampler (`/proc/stat`) via an internal fan-engine load source.
- Added regression tests:
  - `implausible_temp_with_low_cpu_load_keeps_previous_pwm`
  - `implausible_temp_requires_five_consecutive_with_high_cpu_load`
- Validation:
  - `cargo test -p tux-daemon fan_engine -- --nocapture` (13 passed)

## Stage 7 follow-up — Prefer coretemp/hwmon CPU temp for fan curve (2026-04-12)
- Updated fan control temperature source in `tux-daemon/src/fan_engine.rs`:
  - Prefer CPU hwmon sensors (`coretemp`, `k10temp`, `zenpower`, `cpu_thermal`) for control-loop temperature.
  - If hwmon read fails, fall back to backend temperature (`tuxedo-uw-fan` / `tuxedo_io` path).
- Added `HwmonCpuTempSource` with sensor discovery and label preference (`Package id`/`Tdie`/`Tctl` labels preferred when available).
- Existing implausible-temp filtering and CPU-load gating continue to apply to the selected control temperature.
- Added regression tests:
  - `read_control_temp_prefers_hwmon_source`
  - `read_control_temp_falls_back_to_backend`
  - `hwmon_cpu_temp_source_prefers_package_label`
- Ensured tests are deterministic by disabling real host hwmon probing in test default source unless explicitly mocked.
- Validation:
  - `cargo test -p tux-daemon fan_engine -- --nocapture` (16 passed)

## Stage 7 follow-up — Default fan polling semantics and hysteresis cleanup (2026-04-12)
- Corrected `FanConfig::default()` so changing temperatures poll faster than stable ones:
  - `active_poll_ms = 1000`
  - `idle_poll_ms = 2000`
- Raised default `hysteresis_degrees` from `3` to `10` to reduce needless fan curve churn on small temperature fluctuations.
- Added assertions in `tux-core/src/fan_curve.rs` tests to lock the intended default semantics.
- Validation:
  - `just test`
  - `just ci`
