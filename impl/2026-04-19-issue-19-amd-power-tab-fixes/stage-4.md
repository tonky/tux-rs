# Stage 4 — Dashboard package-power: AMD `amd_energy` fallback probe

## Goal

Make the dashboard "Pkg" power line work on AMD laptops (IBP14G9 today,
every Ryzen Tuxedo tomorrow) by probing the `amd_energy` hwmon counter
when `intel-rapl:0` is absent. Today the Intel-only path silently
returns `0.0` on AMD, so the dashboard renders "—" forever — accurate
but uninformative.

## Bug(s) in scope

### Bug — Energy sampler is hardcoded to Intel RAPL

`tux-daemon/src/dbus/system.rs:201`:
```rust
const RAPL_ENERGY_PATH: &str = "/sys/class/powercap/intel-rapl:0/energy_uj";
```

`EnergySampler::new(RAPL_ENERGY_PATH)` is the only constructor and
takes a single static path. AMD systems expose package energy via the
`amd_energy` hwmon driver instead (microjoules, same units as RAPL),
which the sampler has no way to find.

Vendor confirmation: kernel docs/`amd_energy.c` exposes
`/sys/class/hwmon/hwmonN/` with `name == "amd_energy"` and
`energy*_input` counters in microjoules. The Linux RAPL Powercap
interface (`intel-rapl:0`) is gated to Intel CPUs and does not appear
on Ryzen.

### Non-bug — TUI surface already handles absence honestly

`tux-tui/src/dbus_task.rs:160-165`:
```rust
let power_draw_w = client
    .get_package_power_w()
    .await
    .ok()
    .filter(|&w| w > 0.0)
    .map(|w| w as f32);
```

`tux-tui/src/views/dashboard.rs:200-203`:
```rust
let power_draw_str = state
    .power_draw_w
    .map(|w| format!("{w:.1} W"))
    .unwrap_or_else(|| "—".to_string());
```

The TUI filters `> 0.0` and renders "—" when absent. So the daemon can
keep returning `f64` (with `0.0` = absent) and we don't need the
`Option<f64>` wire shape that `plan.md` floated. **Wire shape stays
unchanged.** This is a pure additive fix on the daemon side.

## Design

### Energy source resolution

Replace the single hardcoded path with a small probe that returns the
first matching source on construction:

```rust
enum EnergySource {
    /// Direct file read (microjoules, monotonically increasing counter).
    File(PathBuf),
    /// No source available on this system.
    None,
}

fn discover_energy_source(
    powercap_root: &Path, // /sys/class/powercap
    hwmon_root: &Path,    // /sys/class/hwmon
) -> EnergySource {
    // 1. Intel RAPL package counter (preferred — matches existing behaviour).
    let rapl = powercap_root.join("intel-rapl:0/energy_uj");
    if rapl.exists() {
        return EnergySource::File(rapl);
    }
    // 2. AMD amd_energy hwmon counter.
    if let Ok(entries) = std::fs::read_dir(hwmon_root) {
        for entry in entries.flatten() {
            let name_path = entry.path().join("name");
            let Ok(name) = std::fs::read_to_string(&name_path) else { continue };
            if name.trim() != "amd_energy" { continue; }
            // Pick the first energy*_input file (typically energy1_input —
            // package socket; subsequent entries are per-core).
            let Ok(files) = std::fs::read_dir(entry.path()) else { continue };
            let mut candidates: Vec<PathBuf> = files
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("energy") && n.ends_with("_input"))
                        .unwrap_or(false)
                })
                .collect();
            candidates.sort();
            if let Some(first) = candidates.into_iter().next() {
                return EnergySource::File(first);
            }
        }
    }
    EnergySource::None
}
```

### `EnergySampler` changes

- Replace `path: &'static str` with `path: Option<PathBuf>` (or
  `Option<PathBuf>` field — `EnergySource` collapses to that anyway).
- Constructor takes the discovered source: `EnergySampler::new(source)`.
- `sample()` returns `None` when path is `None`.
- The wraparound logic stays as-is. AMD `amd_energy` counters are 64-bit,
  so wraparound never fires there in practice — the existing
  `u32::MAX` wraparound is harmless on a 64-bit counter (the branch is
  only taken on `uj < prev_uj`, which can't happen on a monotonic 64-bit
  counter unless the kernel resets it). Leave the branch in place.
- `SystemInterface::new` calls
  `discover_energy_source(Path::new("/sys/class/powercap"), Path::new("/sys/class/hwmon"))`
  and feeds the result into `EnergySampler::new`.

### Wire shape (unchanged)

`get_package_power_w(&self) -> f64` keeps its signature.
- intel-rapl present → first call returns 0.0 (no delta), subsequent
  calls return real watts.
- amd_energy present → same.
- neither → always 0.0.

TUI filters 0.0 to "—", which is the correct user-visible behaviour for
all three cases. (The first-call 0.0 is a known quirk that the existing
1-Hz sampling cadence in `dbus_task` masks within one second of boot —
not new in this stage, not regressing.)

## Files touched

- `tux-daemon/src/dbus/system.rs`:
  - Add `discover_energy_source` (private fn).
  - Add `EnergySource` enum (private).
  - Generalize `EnergySampler` to hold `Option<PathBuf>`.
  - Change `EnergySampler::new` signature to take the discovered source.
  - Update `SystemInterface::new` to call the discovery once.

No `tux-core` change. No TUI change. No D-Bus contract change.

## Tests

All in the existing `#[cfg(test)] mod tests` of `system.rs`.

| Test | Setup | Asserts |
|---|---|---|
| `discover_energy_source_prefers_intel_rapl` | tempdir layout: `powercap/intel-rapl:0/energy_uj = "1000"` AND `hwmon/hwmon0/name = "amd_energy"`, `hwmon/hwmon0/energy1_input = "5000"` | result is `EnergySource::File(<intel path>)` |
| `discover_energy_source_falls_back_to_amd_energy` | only `hwmon/hwmon0/name = "amd_energy"`, `hwmon/hwmon0/energy1_input = "5000"` | result is `EnergySource::File(<amd_energy path>)` |
| `discover_energy_source_picks_first_energy_input` | `hwmon/hwmon0/name = "amd_energy"`, multiple files (`energy1_input`, `energy2_input`, `energy3_input`) | path ends with `energy1_input` (sort-stable) |
| `discover_energy_source_skips_unrelated_hwmon` | `hwmon/hwmon0/name = "k10temp"` (no amd_energy anywhere), no powercap | result is `EnergySource::None` |
| `discover_energy_source_none_when_neither_present` | empty tempdirs | result is `EnergySource::None` |
| `energy_sampler_returns_none_when_source_none` | `EnergySampler::new(EnergySource::None)` | `sample() == None` (twice — no first-call magic) |
| `energy_sampler_computes_watts_from_two_samples` | tempfile with two writes (`1_000_000` then `2_000_000` µJ over a known dt mocked via `std::thread::sleep(Duration::from_millis(50))`) | `sample()` second call returns positive `f64`; tolerance check |

Note on the "computes watts" test: the existing code uses
`Instant::now()` directly, so we either (a) accept a small wall-clock
sleep in the test, or (b) refactor to take `Instant` injection. Going
with (a) — `50ms` is acceptable in the daemon test suite (other tests
already do similar). Tolerance is generous (assert `> 0.0`); the exact
value is fragile and not what we're testing.

## Out of scope

- AMD `ryzen_smu` / RyzenAdj backend (still deferred — `follow_up.toml`).
- Any TUI change (the absence path already works).
- `Option<f64>` wire-shape change (rejected — adds churn without
  user-visible benefit since the TUI already filters 0.0).
- Per-core AMD energy counters (`energy2_input` and beyond). The
  dashboard only shows a single package-power number.
- Renaming `RAPL_ENERGY_PATH` (now dead). Delete instead — no
  back-compat reason to keep it.

## Phase exit criteria

- `cargo test --workspace`: all tests pass.
- `cargo clippy --workspace --tests -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.
- New tests cover all three source-presence combinations.
- `worklog-4.md` written.
- Commit on existing branch; PR not opened until Stage 5.

## Branch

Continue on `feat/issue-19-amd-power-tab-fixes`.
