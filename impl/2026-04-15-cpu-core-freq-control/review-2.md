# Stage 2 Review

Date: 2026-04-16

## Scope
Expose CPU profile fields in TUI profile editor:
- online_cores
- scaling_min_frequency
- scaling_max_frequency

## Files Reviewed
- tux-tui/src/model.rs
- impl/2026-04-15-cpu-core-freq-control/description.md
- impl/2026-04-15-cpu-core-freq-control/plan.md
- impl/2026-04-15-cpu-core-freq-control/stage-2.md

## Validation
- `just check`: PASS (fmt, clippy, workspace tests)

## Independent Reviews
- Review A: TUI implementation focused review
- Review B: description/plan alignment + follow-up review

## Findings
1. No blocking issues in stage-2 TUI form wiring itself.
2. Remaining high-priority hardening is daemon-side:
   - signed input validation before i32 -> u32 casts
   - min/max frequency cross-field validation and ordering
3. Remaining medium-priority policy/spec items:
   - explicit `online_cores=None` contract
   - align/document "online-only" wording vs current write-all cpufreq behavior

## Outcome
- Stage 2 implementation accepted.
- Follow-up tasks remain tracked in `follow_up.toml`.
