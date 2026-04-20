# Worklog — Stage 3: TUI Power gating + conditional GPU panel layout

## Session: 2026-04-19

### Implemented

Branch: `feat/issue-19-amd-power-tab-fixes` (Stages 1 + 2 already
committed at 981a324 and 749dd92).

Two user-visible symptoms on IBP14G9 addressed:

- **Power form gating.** `tux-tui/src/model.rs`: `PowerState::new()` now
  starts with `form_tab.supported = false` and `tgp_offset.enabled =
  false`. `tux-tui/src/update.rs` `Capabilities` arm sets `supported =
  caps.gpu_control || caps.tdp_control` and flips `tgp_offset.enabled`
  based on `caps.gpu_control`. On systems with no NB02 backend (every
  AMD laptop today), the slider is disabled and the tab shows the
  standard "Power controls not available" placeholder.
- **dGPU panel collapse.** `tux-tui/src/views/power.rs`: `render_gpu_info`
  replaced fixed 50/50 horizontal split with a match on
  `(has_dgpu, has_igpu)`. APU-only → iGPU takes full width, dGPU not
  rendered; dGPU-only → iGPU not rendered; both or neither → old 50/50
  behaviour (the neither-case keeps the pre-frame placeholders for
  machines that haven't yet got a daemon response).
- **Helper extraction.** Moved each panel's paragraph construction into
  `build_dgpu_paragraph` / `build_igpu_paragraph` so the conditional
  layout stays readable.

### Tests added / updated

- `tux-tui/src/views/power.rs`:
  - Updated `power_state_renders_without_gpu_data` to reflect the new
    default: `supported = false`, tgp_offset field exists but is
    disabled.
  - New `power_view_collapses_dgpu_panel_when_apu_only` — headless
    TestBackend render with only iGPU name populated; asserts
    "No dGPU detected" is absent and the iGPU name is present.
  - New `power_view_collapses_igpu_panel_when_dgpu_only` — mirror
    regression in case someone edits the match arms.
- `tux-tui/src/update.rs`:
  - New `capabilities_enable_power_when_gpu_control` — `gpu_control =
    true` flips `supported` and `tgp_offset.enabled`.
  - New `capabilities_leave_power_disabled_when_no_gpu_or_tdp` — flips
    on then off; asserts state clears back to disabled (guards against
    a stale-enabled bug if someone rewrites the gating logic).
  - Updated pre-existing `power_form_save_returns_command` and
    `daemon_updates_skipped_when_form_dirty`: they rely on the Power
    form being dirty-able, which needs `tgp_offset.enabled = true`.
    Added a `Capabilities("gpu_control = true")` update at the start of
    each to put the model in the "Power is supported" state. Without
    this, `Form::adjust` (`tux-tui/src/model.rs:1122-1126`) no-ops on
    disabled fields — correct production behaviour, but breaks the
    tests' assumption that Right-arrow makes the form dirty.

### Verification

- `cargo test --workspace`: all tests pass (0 failed).
- `cargo clippy --workspace --tests -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean (rustfmt collapsed one
  `terminal.draw(...)` call to single-line; left as-is).

### Decisions & deviations

- **Hybrid gating instead of dynamic rebuild.** The stage-3 spec
  initially proposed clearing and re-pushing fields in the
  `Capabilities` arm. Switched to the idiomatic hybrid used by every
  other form in this codebase: the field always exists in the model;
  `supported` and per-field `enabled` are toggled from the capabilities
  frame. Rationale: the dynamic approach broke two pre-existing tests
  that adjust the Power form directly (bypassing capability updates),
  and the hybrid matches what `charging` / `display` / `webcam` already
  do. No user-visible difference — the disabled field is not rendered
  as interactive, and the tab-level placeholder shows when
  `supported = false`.
- **Neither-GPU layout kept at 50/50.** If both `dgpu_name` and
  `igpu_name` are empty (happens briefly before the first GpuInfo
  frame lands), we render both placeholder panels at 50/50. Once a
  real frame arrives the collapse kicks in. Alternative: render
  nothing in that window. Rejected — visual flash is worse than the
  placeholder.
- **Headless TestBackend render tests.** Used ratatui's `TestBackend`
  to verify the collapse behaviour end-to-end (vs. just asserting the
  layout split). The string-scan pattern ("contains 'No dGPU
  detected'") is cheap and reads well.

### Follow-ups recorded

None new. The `tgp_offset` `i8`/`u8` sign-mismatch follow-up is
unchanged in `follow_up.toml`.

### Out of scope (confirmed not touched)

- Dashboard package-power line (Stage 4).
- Docs (Stage 5).
- AMD `ryzen_smu`/RyzenAdj backend (deferred).
- `tgp_offset` sign mismatch (deferred).
