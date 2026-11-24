# Stage 1 review

AGENTS.md asks for "Opus 4.6 high" + "Gemini 3.1 Pro" reviewers. Neither is
available in this environment; substituted **Opus (high effort)** and
**Sonnet** as the two parallel reviewers.

## Reviewer B (Sonnet)

Conformance: matches stage-1.md 1.1–1.5; descriptor values cross-checked
against tuxedo-drivers 4.25.0 (`uniwill_keyboard.h:987,1662`, `ite_8291.c:227`).
`cargo test` / `clippy -D warnings` clean. No blockers.

| # | Sev | Finding | Action |
|---|-----|---------|--------|
| 1 | should-fix | Now that WMI probes match, the Clevo GUID (also exposed by Uniwill firmware) can misdetect unknown-SKU NB02 / Tuxi / TCC-hinted machines as Clevo. | **Fixed** `bc6524d`: board vendor → TCC hint → NB04 → Uniwill → Tuxi → Clevo; 4 ordering tests. |
| 2 | should-fix | No `detect_platform`-level test for Uniwill beating Clevo with an unknown SKU. | **Fixed** in `bc6524d` (`nb02_with_both_guids_and_unknown_sku_is_uniwill`). |
| 3 | nit | Mixed negative WMI-name test hard to attribute. | **Fixed** in `bc6524d`: per-name loop with messages. |
| 4 | nit | Device comment "charging profile without priority" reads as contradicting `EcProfilePriority`. | **Fixed** `4c4c540`. |
| 5 | nit | Docs say 40 SKUs; spec said 40→41. | **No change**: counted `DEVICE_TABLE` — it was 39 before (docs were off by one), 40 now, so "40" is correct. README disclaimer is the maintainer's voice; left as is. |
| 6 | nit | `Nb02GpuPower::new()` now untested (trivial delegate). | No change needed. |

## Reviewer A (Opus, high)

Conformance: 1.1–1.4 match spec exactly; exit criteria verified on hardware
(`uniwill_guid_2=true`, `platform_guess=Uniwill`). No blockers.

| # | Sev | Finding | Action |
|---|-----|---------|--------|
| 1 | should-fix | `feature_support_matrix.md` Uniwill count 22, actual 29 (already wrong on main). | **Fixed** `6039eb5` (counted: NB05 4, Uniwill 29, Clevo 3, NB04 2, Tuxi 2). |
| 2 | nit | Record the 40-vs-41 spec deviation. | Recorded in `worklog-1.md`. |
| 3 | should-fix | IBM15I10 test doesn't pin `tdp_source` / `fan_rpm`. | **Fixed** `684cefc`: `fan_rpm == [true, true]`, `tdp_source != Rapl` (not `== None`, so Stage 3 can flip it to the EC backend). |
| 4 | nit | follow_up task 3 already done; `uwiwill` typo. | **Fixed** `6039eb5` / `53aa183`. |
| 5 | should-fix | WMI GUIDs come from ACPI `_WDG`, not the driver; comments claiming "unique" / "driver inherently present" are wrong. Consider board vendor before GUIDs. | **Fixed** `53aa183` (comments); ordering already fixed in `bc6524d`; new follow-up to capture WMI GUIDs on real Clevo/NB04 hardware. |
| 6 | nit | Add bare lower-case GUID case. | **Fixed** `53aa183`. Kept `split_at_checked` form (clear enough). |
| 7 | nit | Note colour match is narrower than vendor's (board name, any PID). | **Fixed** `6039eb5` (comment). |
| 8 | nit | Device comment said "Hardware-verified" before Stage 2. | Already reworded in `4c4c540`; TDP-off rationale is in the `684cefc` test. |
| 9 | nit | README "has also been added" wording. | Keep; update after Stage 2. |
