# Worklog

## 2026-04-13

- Created feature tracking folder and planning artifacts.
- Initial draft was TUI-forward.
- Scope corrected with user: reliable driver-daemon test suite is primary; TUI work moved to bonus stages.
- Renamed feature folder to reflect reliability focus.
- Rewrote description, plan, and stage definitions accordingly.
- Implemented Stage 1 core artifacts:
	- contract matrix,
	- fixture schema docs and sample fixture,
	- capture helper script,
	- schema validation integration test,
	- just targets for capture and validation.
- Performed two independent review passes and applied Stage 1 hardening fixes on script/test robustness.
- Validation completed: fmt, clippy, and full test suite all passing.
- Remaining Stage 1 closeout step: real hardware capture run and fixture review.
- Implemented Stage 2 deterministic replay baseline:
	- added `tux-daemon/tests/contract_replay.rs` with fixture consistency and D-Bus replay assertions,
	- aligned sample Uniwill fixture temperature values with deterministic mock behavior,
	- validated with targeted tests plus full `just clippy && just test` pass.
- Hardened Stage 2 replay tests:
	- added schema-version checks during fixture load,
	- switched to fixture-metadata-driven replay device resolution with fan-count guards,
	- iterated replay tests over all fixture files in the Uniwill contract directory,
	- improved panic diagnostics for D-Bus/TOML failure paths.
- Ran two independent review passes for Stage 2 and folded low-risk improvements into the implementation.
- Implemented Stage 3 daemon fault-matrix baseline in integration tests:
	- fan health transition/recovery assertions under injected temp-read failures,
	- graceful fan telemetry fallback assertions under sensor read failures,
	- charging settings retry assertions under transient I/O bursts.
- Ran two independent Stage 3 review passes and applied low-risk helper hardening.
- Validation completed with flox env: targeted integration pass plus full clippy/test pass.
- Implemented Stage 4 workflow hardening:
	- added reliability-focused just targets and explicit reliability gate in CI flows,
	- documented fixture refresh and drift review process in top-level and fixture docs,
	- hardened capture helper with warning accounting, warning logs, and strict failure mode.
- Executed full Stage 4 verification (`just fmt`, `just clippy`, `just reliability-test`,
	`just ci`) in flox environment and validated manual capture+compare flow.
- Continued real-hardware validation of fixture capture on Uniwill host.
- Found and fixed capture-helper mismatches discovered on hardware:
	- auto-detect D-Bus scope (system vs session) for `com.tuxedocomputers.tccd`,
	- read `Device.DaemonVersion` via D-Bus properties instead of method call,
	- support both `/sys/devices/platform/tuxedo_uw_fan` and `/sys/devices/platform/tuxedo-uw-fan`,
	- normalize fan temperature/duty from parsed D-Bus payloads with sysfs fallback.
- Re-validated with strict capture: `CAPTURE_STRICT=1 just fixture-capture-uniwill` now succeeds with zero warnings.
- Re-ran reliability fixture gate: `just fixture-contract-test` passing after helper hardening.
- Captured and promoted a real-hardware fixture for current host SKU (`IBP1XI08MK1`) into
	`tux-daemon/tests/fixtures/driver_contract/uniwill/ibp1xi08mk1-hardware-v1.toml`.
- Updated fixture schema constraints to validate normalized fan values against raw D-Bus payloads
	(first) and retain PWM-scaling checks as fallback when raw D-Bus fan payloads are absent.
- Ran full reliability and CI gates after promotion and schema update:
	- `just reliability-test` passing,
	- `just ci` passing.
- Live hardware runtime triage after `deploy-daemon-debug`:
	- diagnosed concurrent `tccd.service` + `tux-daemon.service` ownership conflict on `com.tuxedocomputers.tccd`,
	- observed fan-control EIO cascade under conflicting control and custom-curve mode,
	- mitigated by stopping `tccd.service`, restarting `tux-daemon`, and forcing `SetFanMode("auto")` for recovery.
- Hardened Just recipes to prevent recurrence by stopping `tccd` before debug/deploy daemon flows
	(`daemon-debug`, `deploy-daemon`, `deploy-daemon-debug`).
- Added fan-engine runtime hardening for manual-reapply backends (`tuxedo-uw-fan` path):
	- suspend CustomCurve and fall back to Auto after repeated control-loop read failures,
	- suspend CustomCurve and fall back to Auto after repeated PWM write failures.
- Added regression coverage in `fan_engine` tests for both read-failure and write-failure
	CustomCurve suspension paths.
- Live validation after deploy:
	- forcing `SetFanMode("custom")` reproduces transient EIO writes,
	- daemon now emits suspension warning and automatically falls back to Auto,
	- post-suspension error loop stops and TUI dashboard returns stable AC telemetry.
- Fixed stale AC/Battery reporting under missed inotify transitions:
	- rewrote power monitor to discover all AC-like sysfs sources (`type=Mains`, `AC*`, `ADP*`),
	- changed aggregate detection to `AC if any source online, Battery if all offline`,
	- added 2s periodic resync fallback so state updates even when inotify events are missed,
	- expanded unit tests with multi-source detection coverage.
- Live hardware validation after redeploy:
	- unplugged state now initializes and reports correctly as `Battery`,
	- D-Bus `System.GetPowerState` matches sysfs (`AC0 online=0`, `BAT0 Discharging`).
- Postponed bonus Stage 5 and Stage 6 TUI work by user decision.
- Fixed a daemon integration regression introduced by CPU hwmon fan-engine hardening:
	- integration `TestDaemon` was constructing `FanCurveEngine` with host hwmon temperature discovery enabled,
	- `fan_health_transitions_and_recovers_under_temp_failures` could therefore read real host CPU temps instead of the mock fan backend,
	- added a deterministic `new_with_manual_pwms_no_hwmon` constructor and switched the integration harness to use it.
- Re-validated after the harness fix:
	- targeted failing integration test now passes,
	- full `just check` passes again.

## Notes

- Core delivery is Stages 1-4 (reliability suite).
- Bonus delivery is Stages 5+ (TUI enhancements).
- New hardware-control subsystems are excluded from this feature.
- Next action is hardware validation on real Uniwill device(s) for Stage 1 closeout.
