# Worklog: Daemon Deploy Fan Spike

## 2026-04-12
- Traced `just deploy-daemon` to a full stop/copy/start cycle rather than a hot reload.
- Confirmed daemon shutdown and startup both restore fans to backend auto mode before the fan engine resumes control.
- Confirmed Uniwill `tuxedo_uw_fan` auto mode is global `fan_mode=0`, which hands control back to firmware/EC.
- Identified a same-day follow-up change in `tux-daemon/src/main.rs` that unconditionally forces Uniwill AC performance profile (`W_UW_PERF_PROF=2`) during initial profile application and later auto-switch/profile reassignment.
- Removed that unconditional EC override as the likely cause of the deploy-time 100% fan spike on Uniwill systems.
- Validation completed:
	- `cargo test -p tux-daemon auto_switch_`
	- `cargo test -p tux-daemon`
	- `cargo fmt --all --check`
	- `cargo clippy -p tux-daemon -- -D warnings`
- Ran two independent read-only review passes. Both agreed the change is coherent; the only remaining high-priority follow-up is real-hardware verification on Uniwill firmware.
- Executed `just deploy-daemon` on the target machine.
- Observed successful stop/start and clean startup sequence in journald:
	- startup safety reset restored both fans to auto
	- initial profile applied on AC
	- fan curve engine started
	- no `forced Uniwill performance profile on AC` log line appeared after the fix
- Noted a separate pre-existing runtime warning from the old daemon instance before shutdown: repeated `tuxedo-uw-fan/fan_mode: Input/output error (os error 5)` while writing PWM. This is likely independent of the removed AC override and should be investigated separately if fan control still misbehaves.

## 2026-04-12 (continued) — root cause 2: tuxedo_uw_fan sysfs EC reset
- Live sysfs observation after first fix: `fan_mode` was flipping from 1→0 every 1-2 seconds — the EC's own thermal loop was resetting it.
- The `tuxedo_uw_fan` sysfs backend was fighting the EC continuously, keeping fans pegged at EC default max.
- Removed `TdUwFanBackend` and `td_uw_fan.rs` entirely — the sysfs interface is fundamentally unreliable by design.
- Flipped Uniwill backend preference in `platform/mod.rs` to always prefer `tuxedo_io` ioctl, matching TCC's approach.

## 2026-04-12 (continued) — root cause 3: missing W_UW_MODE_ENABLE
- Cross-checked against TCC vendor source (`FanControlTuxedoIO.ts`, `tuxedo_io_api.hh`).
- TCC calls `W_UW_MODE_ENABLE(1)` before every fan speed write and `W_UW_MODE_ENABLE(0)` on exit.
- Our `td_uniwill.rs` was missing this entirely — without it the EC's firmware thermal loop ignores `W_UW_FANSPEED` and reverts within ~1 second.
- Added `W_UW_MODE_ENABLE = 0x4008_F013` constant to `tuxedo_io.rs`.
- Updated `write_pwm` to call `write_i32(W_UW_MODE_ENABLE, 1)` before every speed write.
- Updated `set_auto` to call `write_i32(W_UW_MODE_ENABLE, 0)` after `W_UW_FANAUTO`.
- Updated tests to assert new ioctl ordering.
- Deployed and confirmed: steady fan control, no warnings in 30s observation window.