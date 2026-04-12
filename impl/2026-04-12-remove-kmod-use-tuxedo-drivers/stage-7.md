# Stage 7: Post-Migration Polish

## Context

All 6 stages of the kmod removal are complete (607 tests, 0 clippy warnings). This stage addresses 5 issues found during review: fan telemetry accuracy, error testing, and observability.

## Steps

### Phase A: Fan telemetry accuracy (P1 + P2)

1. Add `duty_percent: u8` field to `FanData` in `tux-core/src/dbus_types.rs` — the daemon already reads PWM via `read_pwm()`, this makes it available over D-Bus
2. Extend D-Bus `get_fan_speed()` in `tux-daemon/src/dbus/fan.rs:94` to also return `duty_percent` (or add a new `GetFanDuty` method), and a `rpm_available: bool` flag so TUI knows when RPM is real vs synthetic
3. Update `DashboardTelemetry` in `tux-tui/src/event.rs` and `tux-tui/src/dbus_task.rs` to carry duty data
4. Update `FanData` in `tux-tui/src/model.rs` to store `duty_percent` and `rpm_available`
5. Update `tux-tui/src/update.rs` — derive `speed_percent` from `duty_percent` (PWM-authoritative), not `rpm / max_rpm`
6. Update `tux-tui/src/views/dashboard.rs` — when `rpm_available == false`, show label as `"Fan N (~XX%)"` instead of `"Fan N (0 RPM)"`

### Phase B: Stale comment cleanup (P3)

7. Remove the reference to "legacy sysfs-based `UniwillFanBackend`" in `tux-daemon/src/platform/td_uniwill.rs:23`

### Phase C: MockTuxedoIo error injection (P4)

8. Add error injection to `MockTuxedoIo` in `tux-daemon/src/platform/tuxedo_io.rs`: add `fail_reads: AtomicBool`, `fail_writes: AtomicBool` fields with `set_fail_reads()` / `set_fail_writes()` setters (following the `MockFanBackend::set_fail_temp()` pattern)
9. Add tests in `td_clevo.rs` and `td_uniwill.rs` for:
   - `read_temp` failure when ioctl read fails
   - `write_pwm` failure when ioctl write fails
   - Clevo read-modify-write partial failure (read succeeds, write fails)
   - `set_auto` failure handling

### Phase D: Fan engine health status (P5)

10. Add a failure counter to `FanCurveEngine` in `tux-daemon/src/fan_engine.rs`: track consecutive temp-read failures, expose via `Arc<AtomicU32>`
11. Add a D-Bus method `GetFanHealth` on the fan interface returning `{ status: "ok"|"degraded"|"failed", consecutive_failures: u32 }` — `degraded` after 5+ consecutive failures, `failed` after 30+
12. Display health status in TUI dashboard when degraded (e.g., a warning line in the status block)
13. Add fan engine tests for failure counter behavior using `MockFanBackend::set_fail_temp()`

## Relevant files

- `tux-core/src/dbus_types.rs` — add `duty_percent` and `rpm_available` to wire types
- `tux-daemon/src/dbus/fan.rs` — extend D-Bus response with duty/RPM-availability
- `tux-daemon/src/fan_engine.rs` — failure counter
- `tux-daemon/src/platform/tuxedo_io.rs` — MockTuxedoIo error injection
- `tux-daemon/src/platform/td_uniwill.rs` — stale comment
- `tux-daemon/src/platform/td_clevo.rs` — error path tests
- `tux-tui/src/views/dashboard.rs` — label rendering
- `tux-tui/src/model.rs` — FanData struct
- `tux-tui/src/update.rs` — speed_percent calculation
- `tux-tui/src/event.rs` — DashboardTelemetry variant
- `tux-tui/src/dbus_task.rs` — polling loop

## Verification

1. Run `just test` — all existing 607+ tests pass, new tests added for each phase
2. Phase A: Unit test in `dbus/fan.rs` verifying `duty_percent` is returned; test in `update.rs` verifying speed_percent comes from duty not rpm
3. Phase C: At least 4 new tests in `td_clevo.rs` and `td_uniwill.rs` covering ioctl failure paths
4. Phase D: Test that failure counter increments on temp read failure and resets on success; test degraded threshold
5. `cargo clippy` — zero warnings

## Decisions

- NB04 fan engine handling is already correct (`backend = None` → engine not spawned) — no change needed
- D-Bus `get_fan_speed()` keeps its existing PWM→synthetic fallback for backward compatibility; `duty_percent` and `rpm_available` are additive fields
- Phase B is trivial and can be done inside any other phase

## Further Considerations

1. **Phase A wire format**: Adding fields to `FanData` is backward-compatible if using TOML/JSON serialization (older TUI ignores unknown fields). If the D-Bus method signature changes, the TUI must be updated in lockstep. Recommendation: add a new `GetFanData` method that returns the richer struct, keep `GetFanSpeed` unchanged.
2. **Phase D thresholds**: 5/30 consecutive failures for degraded/failed are initial values. Should these be configurable in `daemon.toml`? Recommendation: hardcode for now, make configurable later if needed.
