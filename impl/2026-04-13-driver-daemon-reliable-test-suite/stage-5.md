# Stage 5 (Bonus): TUI Actionable Profile Coverage

## Objective

As a bonus stage after core reliability delivery, expose all actionable Uniwill
profile options in TUI profile workflows.

## Scope

- Extend TUI profile editor build/apply for actionable fields.
- Preserve compatibility and capability gating behavior.
- Add focused tests for profile editor roundtrip and save flows.

## Target Files

- tux-tui/src/model.rs
- tux-tui/src/update.rs
- tux-tui/src/views/profiles.rs
- tux-tui/tests/live_regression.rs
- tux-core/src/profile.rs

## Tasks

1. Add missing actionable Uniwill fields to profile editor form construction.
2. Add apply logic to persist all newly exposed fields.
3. Verify copy/save flows preserve these fields.
4. Add unit and regression tests for profile form roundtrip.

## Risks

- Larger forms can reduce usability if not grouped well.
- Mis-mapped fields can overwrite existing profile values.

## Verification

- Targeted TUI unit tests for profile form build/apply paths.
- TUI live regression checks for profile save/load behavior.

## Exit Criteria

- Actionable Uniwill profile options are editable in TUI and persist correctly.
- Bonus stage does not weaken core reliability test guarantees.
