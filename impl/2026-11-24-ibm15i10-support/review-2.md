# Stage 2 review

Reviewers: **Opus (high)** + **Sonnet** (substitutes for the "Opus 4.6 high" /
"Gemini 3.1 Pro" pair named in AGENTS.md, which are unavailable here).
Range reviewed: `a1fa98a..2204775`. Both: no blockers; tests + clippy green.

## Reviewer A (Opus, high)

| # | Sev | Finding | Action |
|---|-----|---------|--------|
| a1 | should-fix | `hardware_support.md` still "validation in progress". | **Fixed** (wrap-up docs commit). |
| a2 | should-fix | No Stage 2 worklog; follow-ups uncommitted. | **Fixed** (`worklog-2.md`, wrap-up commit). |
| a3 | nit | Live test doesn't exercise no_turbo / keyboard-echo fixes. | **Fixed** `e5d28a5`. |
| b1 | should-fix | Fixture `ioctl_fans` inferred from `fan_data_0`, true for every fixture → legacy raw keys no longer required. | **Fixed** `f35b910`: explicit `meta.fan_transport`; mutation-checked. Charging profile-only stays inferred (both blanks + no normalized priority). |
| b2 | should-fix | nvidia-smi only queried at startup (boot race → read-only until restart). | **Fixed** `d7e226e`: lazy, 30 s rate-limited retry, cached once known. |
| b3 | should-fix | Profile cTGP > max (TCC import) rejected, previous offset stays. | **Fixed** `cf5fb62`: clamp to max with warn. |
| b4 | should-fix | TCC-compat advertises 0/1000 W limits; `Available` ignores nvidia-smi. | **Fixed** `dc7490e`: real limits via `power_limits_w`, -1/-1 + unavailable otherwise. |
| b5 | nit | Field max widened to out-of-range hardware value lets user pick rejectable values. | **Kept, deliberate**: shows the true hardware value; daemon rejects >max with a visible error; b3 removes the main source of such values. |
| b6 | nit | Missing `brightness` means 0 while empty colour/mode mean unchanged. | **Documented** `8276719`. |
| b7 | nit | nvidia-smi blocks main task (≈0.4 s, ≤5 s). | Follow-up (low). |
| c1 | nit | Doc comment of `execute_form_save` split. | **Fixed** `d86cf0d`. |
| c2 | nit | Typed `SetCtgpOffset` command instead of TOML round trip. | Follow-up (low). |
| c3 | nit | `supports_priority` default `true` is a footgun. | **Fixed** `a790098` (required). |
| c4 | nit | TUI `charging_profiles && charging_priority` redundant. | Kept (defensive against older daemons; harmless). |

## Reviewer B (Sonnet)

| # | Sev | Finding | Action |
|---|-----|---------|--------|
| 1 | should-fix | `charging_priority_capability_follows_backend` tautological (Flexicharger device). | **Fixed** `c2f3f18`: IBM15I10 descriptor. |
| 2 | should-fix | `.with_charging_priority(...)` wiring untested. | **Fixed** `c2f3f18`: e2e through `serve_on_bus`; mutation-checked. |
| 3 | should-fix | nvidia-smi one-shot at startup. | Same as A-b2, **fixed** `d7e226e`. |
| 4 | should-fix | Power data fetched once per connection; stale after save/profile switch. | **Partly fixed** `d86cf0d` (refetch after save); profile-switch refresh → follow-up (pre-existing for all form tabs). |
| 5 | nit | Widened field max. | See A-b5. |
| 6 | nit | Range update skipped while form dirty. | **Fixed** `d86cf0d` + test. |
| 7 | nit | Misplaced doc comment. | See A-c1. |
| 9 | nit | `[INFO]` uses raw echo. | No change (script has only a `warn` helper). |
