# Stage 2: TUI Profile Editor Support for CPU Core/Frequency Fields

## Context
Stage 1 and daemon profile application already support these CPU fields:
- `online_cores`
- `scaling_min_frequency`
- `scaling_max_frequency`

But the TUI profile editor does not expose them, so users cannot set them interactively.

## File References
- `tux-tui/src/model.rs`

## Code Changes
1. Extend `ProfilesState::build_editor_form` with three CPU form fields:
   - `Online Cores (0=Auto)`
   - `Min Freq kHz (0=Unset)`
   - `Max Freq kHz (0=Unset)`
2. Extend `ProfilesState::apply_form_to_profile` to map those fields back into `CpuSettings`:
   - `0` => `None`
   - `>0` => `Some(value)`
3. Keep existing behavior and field ordering otherwise unchanged.

## Testing
1. Extend model tests to verify CPU core/frequency values round-trip through the editor form.
2. Add a dedicated test to verify `0` values map to `None` for optional CPU fields.

## Follow up
- If stage passes, keep daemon-side validation hardening tasks tracked in `follow_up.toml`.
