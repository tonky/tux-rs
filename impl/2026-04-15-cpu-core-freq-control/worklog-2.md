# Stage 2 Session Worklog

Date: 2026-04-16

## Implemented
- Added three CPU fields to TUI profile editor form in `tux-tui/src/model.rs`:
  - Online Cores (0=Auto)
  - Min Freq kHz (0=Unset)
  - Max Freq kHz (0=Unset)
- Added reverse mapping in profile form apply path with `0 -> None` behavior for optional CPU fields.
- Kept existing profile editor behavior and key handling unchanged.

## Tests
- Extended `profiles_apply_form_roundtrip` assertions for new CPU fields.
- Added `profiles_apply_form_cpu_optional_fields` test for:
  - explicit values mapping to `Some(...)`
  - zero values mapping to `None`
- Ran `just check` successfully.

## Review/Notes
- Ran two independent review passes.
- Confirmed remaining risks are daemon-side validation/policy items; tracked in `follow_up.toml`.
