# Worklog — Stage 2 (live validation)

## 2026-11-24

### Setup
- User ran `just daemon-debug-log` (new recipe: stops `tccd`, runs the release
  daemon as root, tees plain-text logs to `tmp/`). TCC's installed D-Bus policy
  lets the unprivileged agent drive TUI/live tests. Four daemon restarts total,
  one per batch of daemon-side fixes.

### Bugs found live (each fixed with a regression test)
| Commit | Bug | Impact on IBM15I10 |
|--------|-----|--------------------|
| `b27614f` | `GetChargingSettings` failed with ENOENT when `charging_priority/` is absent | Charging tab unusable |
| `02a2966` | `SetKeyboardState` with the daemon's own empty colour/mode failed on ITE, reset colour to white | Live test failed; read-modify-write clients broken |
| `547031b` | `SetPowerSettings` without `no_turbo` re-enabled turbo | Every Power tab save flipped turbo |
| `af29fdc` | TUI called `GetGpuInfo` on Settings (served on System) | Power tab never got GPU data |
| `695c982` | Power tab TGP offset read/wrote nothing, range −15..15 | cTGP not controllable from TUI (TCC had set 65 W) |
| `da065ec` | Priority selector shown on profile-only EC | Control that silently did nothing |

### Features added along the way
- `b2fbb67`/`39b46a3`/`dc7490e`: cTGP bounded by `nvidia-smi` limits
  (`max − default` = 65 W for the RTX 5060), `GpuPower.GetCtgpOffsetMax`,
  real limits in TCC compat. User chose "mirror TCC" over a fixed 0–255 range.
- `d7e226e`: lazy, rate-limited retry of the limits query (boot race).
- `cf5fb62`: profile cTGP clamped to the GPU's max.

### Tooling / tests
- Live test is device-aware (`LiveExpectations`: IBP Gen8, IBM15I10) and now
  covers cTGP bounds, priority capability, keyboard echo and partial power
  updates. Hermetic guard ties expectations to device-table names.
- Fixture capture: SKU from DMI, `fan_transport = "tuxedo_io"`, profile-only
  charging. Schema/replay accept both variants; legacy strictness preserved
  (mutation-checked). New `ibm15i10-hardware-v1.toml`.

### Verification
- `just check` → 799 passed, 0 failed (`tmp/stage2-check-2.log`).
- `just live-test` on hardware → PASSED (`tmp/live-test-5.log`).
- TUI via tmux: Power tab shows/edits cTGP (65→60→65 W verified in sysfs);
  Charging tab hides priority; Dashboard shows 2 fans, 16 cores, package power.
- Hardware state after tests unchanged (charging `balanced`, cTGP 0 W, turbo on).

### Decisions
- Out-of-range hardware cTGP values are displayed truthfully (field bound widens);
  the daemon rejects writes above max (review-2 A-b5).
- Generic issues seen here but not IBM15I10-specific are follow-ups: `schedutil`
  governor fallback, empty GPU telemetry (no hwmon), per-profile cTGP in the
  editor, form-tab refresh on profile switch, brightness rounding.
