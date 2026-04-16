# Worklog

## 2026-04-15
- Investigated Tuxedo Control Center (TCC) source code to understand how it configures active cores and frequency scaling.
- Found out TCC writes to `/sys/devices/system/cpu/cpu*/online` and `/sys/devices/system/cpu/cpu*/cpufreq/scaling_{min,max}_freq`.
- Inspected the current `tux-rs` implementation. `tux-core` already has the required `CpuSettings` fields. `tux-daemon` has `CpuGovernor` which controls related aspects but is missing these exact sysfs paths.
- Proposed a high-level plan and created feature directory (`impl/2026-04-15-cpu-core-freq-control`) to structure the effort as per `@AGENTS.md`.

## 2026-04-16
- Verified stage implementation quality gates with `just check` (fmt, clippy, tests): all passing.
- Ran two independent code reviews for stage 1 and triaged findings.
- Updated `just live-test` to include CPU core/frequency regressions:
	- `set_online_cores_works`
	- `set_scaling_min_max_freq_works`
	- `apply_cpu_governor_and_tdp`
- Validated the updated recipe with `just --dry-run live-test`.
- Executed all newly added targeted daemon tests; all passed.

- Stage 2 started and completed: exposed CPU core/frequency profile fields in TUI profile editor.
- Added TUI profile editor fields:
	- `Online Cores (0=Auto)`
	- `Min Freq kHz (0=Unset)`
	- `Max Freq kHz (0=Unset)`
- Wired form serialization in `ProfilesState::apply_form_to_profile` with explicit `0 -> None` mapping for optional CPU fields.
- Extended TUI model tests for round-trip and optional-field mapping behavior.
- Re-ran quality gates (`just check`): passing.
- Ran two additional independent review passes for stage 2 and updated `follow_up.toml` with remaining plan/description-derived hardening items.