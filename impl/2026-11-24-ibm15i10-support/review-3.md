# Stage 3 review

Reviewers: **Opus (high)** + **Sonnet** (substitutes for AGENTS.md's pair).
The Opus run was paused on the user's request to save usage and resumed with
a short-report instruction. Both: no blockers; driver cross-check (ioctl codes,
errno decoding, PL index mapping, x5ar45xs table, double-PL4) found no errors.

## Reviewer B (Sonnet)

| # | Sev | Finding | Action |
|---|-----|---------|--------|
| 1 | should-fix | Live round-trip value choice could equal the original or exceed max. | **Fixed** `b5d72b6`: ±1 W, asserted ≠ orig and in bounds. |
| 2 | should-fix | Live test left PL1/PL2 modified if an assertion failed mid-way; PL1 > PL2 during clamp check. | **Fixed** `b5d72b6`: checks collected, restore always (PL2 first); clamp check on PL2. |
| 3 | should-fix | Untested probe branches (inconsistent bounds, missing PL4, unreadable PL1, io errors, write errors). | **Fixed** `05a8b6c`. |
| 4 | nit | `-v` overflows for `i32::MIN`. | **Fixed** `05a8b6c` (`saturating_neg` + test). |
| 5–8 | nit | Stale "Both" doc; weak clamp/factory asserts; `let _` in factory test. | **Fixed** `05a8b6c`. |
| 9 | nit | Client doc "daemon clamps". | No change: all three backends clamp. |

## Reviewer A (Opus, high)

| # | Sev | Finding | Action |
|---|-----|---------|--------|
| b1 | should-fix | `is_uniwill_hardware()` gate is load-bearing (W_UW_TDP on non-Uniwill → kernel NULL deref). | **Fixed** `b0dd77e` (comment). |
| b2 | should-fix | Writes force EC overboost (persistent); EC might revert values asynchronously. | **Verified live**: values hold and are *enforced* (PL 25 W → package 25.0 W after 8–20 s). Overboost documented in `hardware_support.md`. |
| b3 | should-fix | TDP not restored on resume. | Follow-up (medium); documented. |
| d1 | should-fix | TCC-compat `ODMPowerLimitsJSON` "{}"; positional `tdpValues` bug. | Follow-up (low, pre-existing). |
| d2 | nit | PL fields accept 1–9 W, silently clamped to 10. | **Fixed** `b0dd77e`: label shows range. |
| c1 | nit | Sim `cmd - W_UW_TDP0` underflow on unexpected cmd. | No change (test-only). |
| e | — | Optional: share the platform's tuxedo_io fd. | No change (second fd harmless). |
