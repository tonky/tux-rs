# Worklog — Stage 4: Dashboard package-power AMD `amd_energy` fallback

## Session: 2026-04-19

### Implemented

Branch: `feat/issue-19-amd-power-tab-fixes` (Stages 1–3 already
committed at 981a324, 749dd92, 928a233).

Single daemon-side change. AMD APUs (no `intel-rapl:0`) now feed the
dashboard "Pkg" line from the `amd_energy` hwmon counter instead of
reporting silent zero forever.

- **`tux-daemon/src/dbus/system.rs`**:
  - New private enum `EnergySource { File(PathBuf), None }`.
  - New private fn `discover_energy_source(powercap_root, hwmon_root)`:
    probes Intel RAPL first (preserves existing behaviour on Intel
    laptops), falls back to scanning `/sys/class/hwmon/*/name ==
    "amd_energy"` and picking the first sorted `energy*_input` file
    (typically `energy1_input` — the socket counter).
  - `EnergySampler` now holds `Option<PathBuf>` and `sample()` short-
    circuits to `None` when the source is `None`.
  - `EnergySampler::new` takes `EnergySource` instead of a `&'static
    str`. `SystemInterface::new` runs the discovery once at daemon
    startup.
  - Deleted dead `RAPL_ENERGY_PATH` constant — discovery owns the path
    now.
  - The 32-bit wraparound branch is preserved (harmless on 64-bit
    `amd_energy` counters since they don't wrap in practice).

### Wire shape

Unchanged. `get_package_power_w(&self) -> f64` still returns `0.0` to
mean "absent / no delta yet". The TUI side already filters this
honestly:

- `tux-tui/src/dbus_task.rs:160-165` — `.filter(|&w| w > 0.0)`.
- `tux-tui/src/views/dashboard.rs:200-203` — renders "—" on `None`.

`stage-4.md` decided against the `Option<f64>` wire-shape change that
`plan.md` initially floated, on the grounds that there is no
user-visible difference and it would touch the contract for nothing.

### Tests added

All 7 in `tux-daemon/src/dbus/system.rs` `mod tests`:

| Test | Covers |
|---|---|
| `discover_energy_source_prefers_intel_rapl` | Intel-priority on hybrid sysfs trees |
| `discover_energy_source_falls_back_to_amd_energy` | AMD-only laptop |
| `discover_energy_source_picks_first_energy_input` | sort-stable selection of `energy1_input` over `energy2/3_input` |
| `discover_energy_source_skips_unrelated_hwmon` | `k10temp` and other hwmons are ignored |
| `discover_energy_source_none_when_neither_present` | empty sysfs → `None` |
| `energy_sampler_returns_none_when_source_none` | `None` source → `sample() = None` (twice — guards against accidental side-effects on the disabled path) |
| `energy_sampler_computes_watts_from_two_samples` | end-to-end: write 1 µJ → sleep 50ms → write 2 µJ → assert positive watts |

The "computes watts" test uses a real `std::thread::sleep(50ms)` since
the production code calls `Instant::now()` directly. Tolerance is
generous (`> 0.0`) — the exact value is fragile and not what we're
verifying. Other daemon tests already use `sleep` for similar timing
needs.

### Verification

- `cargo test -p tux-daemon --lib system::`: 27/27 pass (7 new + 20
  pre-existing).
- `cargo test --workspace`: 173 tests pass, 0 failed, 1 ignored
  (pre-existing `ibp_gen8_live_regression`).
- `cargo clippy --workspace --tests -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.

### Decisions & deviations

- **Wire shape kept as `f64`.** Plan floated `Option<f64>`. Rejected
  during stage-4 exploration: TUI already filters `> 0.0` and renders
  "—", so the contract change adds churn for no user-visible benefit.
  Stage-4 spec documents this decision.
- **`discover_energy_source` is private.** No external caller. Tests
  live in the same module, so visibility stays at default (private).
- **`energy*_input` candidates sorted alphabetically.** This puts
  `energy1_input` first, which matches the kernel docs convention
  (socket-level before per-core). If a future AMD platform ships with
  a different layout we can revisit, but no current Tuxedo machine has
  that geometry.
- **Wraparound branch preserved.** It is dead code on 64-bit
  `amd_energy` counters but still correct on Intel RAPL. Removing it
  would be a separate cleanup; not in scope.
- **`PathBuf` over `&'static str`.** Discovery returns paths assembled
  at runtime, so static lifetimes don't apply. `EnergySampler` owns
  the `PathBuf`; cheap clone-free read each `sample()`.

### Follow-ups recorded

None new. Existing follow-ups in `follow_up.toml` are unchanged:
- AMD `ryzen_smu` / RyzenAdj backend (deferred).
- `tgp_offset` `i8`/`u8` sign mismatch.
- amdgpu classifier on pre-`boot_vga` kernels.
- D-Bus error logging in `dbus_task.rs`.

### Out of scope (confirmed not touched)

- TUI changes (already handles absence honestly).
- AMD TDP / `ryzen_smu` backend (deferred).
- Per-core AMD energy counters (`energy2_input`+) — dashboard shows
  one number.
- Renaming/hiding the public `get_package_power_w` method — wire stable.
