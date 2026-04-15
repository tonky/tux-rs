# TUI State Management Improvements

After investigating Statum (typestate library) as a potential replacement for the Elm-style TUI architecture, we concluded it's not a good fit — the paradigms are fundamentally incompatible. However, the investigation surfaced 4 concrete pain points worth fixing:

1. **BoundedIndex wrapper** — eliminate duplicated index wrapping/clamping across ProfilesState, FanCurveState, WebcamState
2. **Explicit form field keys** — remove fragile label-to-TOML-key normalization by giving all FormFields explicit keys
3. **Centralized text-edit state** — replace 7-branch tab checking with a single `editing_text_in: Option<Tab>` on Model
4. **Form dirty-data warnings** — warn user when daemon pushes new data while they have unsaved form edits
