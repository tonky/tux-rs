# Stage 1 — Intel RAPL backend (`RaplTdp`)

Add a second implementor of `cpu::tdp::TdpBackend` that reads/writes
PL1/PL2 through `/sys/class/powercap/intel-rapl:0/`.

This stage is pure backend + unit tests. It does **not** touch device table,
factory selection, D-Bus wiring, or TUI. Existing behaviour is unaffected.

## Context

### Existing code to mirror
- `tux-daemon/src/cpu/tdp.rs` (full file) — `TdpBackend` trait and `EcTdp`.
- `tux-daemon/src/platform/sysfs.rs` — `SysfsReader` helper. RAPL attributes
  are plain decimal text, so `read_str` / `write_str` / `read_u32` are all
  we need (no `pread`/`pwrite`).
- Test style: hermetic, `tempfile::tempdir()`, `with_path(...)` constructor,
  no global fixtures.

### RAPL sysfs layout (Intel)
```
/sys/class/powercap/
└── intel-rapl:0/                       # package-0 domain
    ├── name                            # "package-0"
    ├── enabled                         # "0" or "1"
    ├── constraint_0_name               # "long_term"
    ├── constraint_0_power_limit_uw     # PL1 in microwatts (rw)
    ├── constraint_0_max_power_uw       # firmware-advertised upper bound (ro)
    ├── constraint_0_time_window_us     # (rw, we leave untouched)
    ├── constraint_1_name               # "short_term"
    ├── constraint_1_power_limit_uw     # PL2 in microwatts (rw)
    ├── constraint_1_max_power_uw       # upper bound (ro)
    └── ...
```

Notes:
- Units are microwatts; profile is watts. Convert at the backend boundary.
- `constraint_*_min_power_uw` is **not** universally exposed; when absent we
  use a `1 W` floor (RAPL will clamp anyway).
- `name` distinguishes `package-0` from MMIO or sub-zones — we match on it.
- Firmware-locked systems (MSR_PKG_POWER_LIMIT bit 63 set) return `EPERM`
  / `EACCES` on write. We surface this as a typed warning, not a panic.

## File changes

All changes in `tux-daemon/src/cpu/tdp.rs` (single file, extending existing
module):

1. Add constants for the RAPL base path and attribute names near the
   existing `SYSFS_BASE` / `EC_RAM_ATTR` constants:
   ```rust
   const RAPL_BASE: &str = "/sys/class/powercap/intel-rapl:0";
   const RAPL_NAME_ATTR: &str = "name";
   const RAPL_EXPECTED_NAME: &str = "package-0";
   const RAPL_PL1_LIMIT: &str = "constraint_0_power_limit_uw";
   const RAPL_PL1_MAX:   &str = "constraint_0_max_power_uw";
   const RAPL_PL2_LIMIT: &str = "constraint_1_power_limit_uw";
   const RAPL_PL2_MAX:   &str = "constraint_1_max_power_uw";
   const RAPL_FLOOR_W: u32 = 1; // conservative lower bound when firmware doesn't publish min
   ```

2. New struct:
   ```rust
   /// Intel RAPL-based TDP control (`/sys/class/powercap/intel-rapl:0`).
   pub struct RaplTdp {
       sysfs: SysfsReader,
       bounds: TdpBounds,
   }
   ```

3. Constructors:
   - `pub fn probe() -> Option<Self>`:
     - `SysfsReader::new(RAPL_BASE)`.
     - Return `None` if `available()` is false or `exists(RAPL_NAME_ATTR)` is
       false.
     - Read `name`. If it does not equal `RAPL_EXPECTED_NAME`, return `None`
       with a `debug!` log line (future-proof against MMIO-only zones).
     - Call `Self::read_bounds(&sysfs)` (below). If the read fails, return
       `None` with a `warn!` and the `io::Error`.
   - `#[cfg(test)] pub fn with_path(path, bounds) -> Self` — mirrors `EcTdp`.

4. Bounds probing helper:
   ```rust
   fn read_bounds(sysfs: &SysfsReader) -> io::Result<TdpBounds> {
       let pl1_max_uw = sysfs.read_u32(RAPL_PL1_MAX)?;
       let pl2_max_uw = sysfs.read_u32(RAPL_PL2_MAX)?;
       Ok(TdpBounds {
           pl1_min: RAPL_FLOOR_W,
           pl1_max: (pl1_max_uw / 1_000_000).max(RAPL_FLOOR_W),
           pl2_min: RAPL_FLOOR_W,
           pl2_max: (pl2_max_uw / 1_000_000).max(RAPL_FLOOR_W),
           pl4_min: None,
           pl4_max: None,
       })
   }
   ```
   - Divide µW → W with integer division. This matches the user-facing unit
     (integer watts) and errs on the conservative side (a 28.5 W firmware
     bound becomes 28 W; RAPL itself will still clamp internally).
   - Both min values default to 1 W. If firmware later exposes
     `constraint_*_min_power_uw`, that's a small follow-up.

5. `TdpBackend` impl:
   ```rust
   impl TdpBackend for RaplTdp {
       fn get_pl1(&self) -> io::Result<u32> {
           let uw = self.sysfs.read_u32(RAPL_PL1_LIMIT)?;
           Ok(uw_to_w(uw))
       }
       fn set_pl1(&self, watts: u32) -> io::Result<()> {
           let w = watts.clamp(self.bounds.pl1_min, self.bounds.pl1_max);
           self.sysfs.write_u32(RAPL_PL1_LIMIT, w.saturating_mul(1_000_000))
       }
       fn get_pl2(&self) -> io::Result<u32> {
           let uw = self.sysfs.read_u32(RAPL_PL2_LIMIT)?;
           Ok(uw_to_w(uw))
       }
       fn set_pl2(&self, watts: u32) -> io::Result<()> {
           let w = watts.clamp(self.bounds.pl2_min, self.bounds.pl2_max);
           self.sysfs.write_u32(RAPL_PL2_LIMIT, w.saturating_mul(1_000_000))
       }
       fn bounds(&self) -> &TdpBounds { &self.bounds }
   }

   fn uw_to_w(uw: u32) -> u32 { uw / 1_000_000 }
   ```
   - Integer saturation guards the µW multiply (a 4295 W PL value is the
     first overflow point; hardware tops out ~250 W).
   - EPERM/EACCES from a locked MSR flows through `io::Result` up to the
     profile-apply layer. Stage 2 decides what to do with it (we expect to
     log a one-time warning and leave read access usable).

6. No trait changes. Existing callers and tests remain valid.

## Tests (all hermetic, in the same file)

Extend the existing `#[cfg(test)] mod tests` block. Reuse the fake-sysfs
pattern from `EcTdp` but layered for RAPL text files.

1. **`setup_rapl(dir)`** helper: writes
   - `name` = `"package-0\n"`
   - `constraint_0_power_limit_uw` = `"15000000\n"` (15 W)
   - `constraint_0_max_power_uw`   = `"28000000\n"` (28 W)
   - `constraint_1_power_limit_uw` = `"28000000\n"` (28 W)
   - `constraint_1_max_power_uw`   = `"40000000\n"` (40 W)

2. **`rapl_get_pl1_reads_sysfs`** — expect `15`.
3. **`rapl_get_pl2_reads_sysfs`** — expect `28`.
4. **`rapl_set_pl1_writes_microwatts`**:
   - `set_pl1(20)` → file contains `"20000000"`.
   - `set_pl1(100)` → clamped to bounds `28`, file contains `"28000000"`.
   - `set_pl1(0)` → clamped to floor `1`, file contains `"1000000"`.
5. **`rapl_set_pl2_writes_microwatts`** — mirror of #4 with `40` / `1`.
6. **`rapl_bounds_from_sysfs`** — bounds come out as `pl1_max=28`,
   `pl2_max=40`, `pl1_min=pl2_min=1`.
7. **`rapl_probe_rejects_wrong_name`**:
   - Set up a dir where `name` is `"psys"` (or anything ≠ `"package-0"`).
   - `RaplTdp::with_path`-style probe helper returns `None`.
   - Because `probe()` hard-codes `RAPL_BASE`, add a private
     `probe_at(&Path)` method for tests so we can point it at tempdir. The
     public `probe()` becomes a one-liner: `Self::probe_at(Path::new(RAPL_BASE))`.
8. **`rapl_probe_requires_name_attr`** — empty dir → `probe_at(...)` returns
   `None`, no panic.
9. **`rapl_set_pl1_surfaces_permission_error`**:
   - Use `std::fs::Permissions` to make `constraint_0_power_limit_uw`
     read-only (mode `0o444`).
   - `set_pl1(10)` returns `Err(_)` with `kind() == ErrorKind::PermissionDenied`.
   - (Skipped automatically on non-Unix; we're Linux-only anyway.)

## Quality gates for this stage

- `just check` green (fmt + clippy + test). Clippy is `-D warnings`.
- No new dependencies. `tempfile` is already in dev-deps.
- No changes outside `tux-daemon/src/cpu/tdp.rs`.

## Out-of-scope reminders (deferred to Stage 2+)

- No factory/selection changes.
- No `DeviceDescriptor::tdp_source` enum yet — Stage 2.
- No D-Bus wiring beyond what already exists.
- No TUI changes.

## Follow-up candidates (tracked in `follow_up.toml`, not addressed here)

- Support `constraint_*_min_power_uw` when exposed.
- Typed error enum distinguishing `MissingSysfs`, `FirmwareLocked`,
  `ParseError` — current `io::Result` is enough for Stage 1 but Stage 2
  should decide if a bespoke enum is needed.
- Package sub-zones (`intel-rapl:0:0` dram) — not relevant for CPU TDP cap.
