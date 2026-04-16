# Stage 1 Review

Date: 2026-04-16

## Inputs
- Validation run: `just check` (fmt + clippy + all tests)
- Independent review pass A (Explore subagent)
- Independent review pass B (Explore subagent)

## Validation Result
- `just check`: PASS
- Workspace tests: PASS

## Findings Summary
1. High: CPU profile values are signed (`i32`) but cast directly to `u32` in profile application.
2. High: Missing explicit cross-field validation for `scaling_min_frequency` and `scaling_max_frequency` combinations.
3. Medium: `online_cores=None` behavior should be explicitly documented and tested as a contract.

## Notes
- One review claim that `set_online_cores(256)` would offline all but `cpu0` was rejected after code inspection: the current implementation writes `online=1` for indices `< count`, so an oversized count enables all discovered CPUs.
- Oversized sentinel values are still considered a maintainability concern, so behavior policy remains tracked in follow-up.

## Outcome
- Stage 1 implementation is functionally in place and passing checks.
- Follow-up tasks were added for input validation and policy hardening.
- `just live-test` was extended to include CPU core/frequency regression coverage.
