# Stage 3 — TUI: gate Power form on capabilities + collapse absent GPU panel

## Goal

Stop showing Power controls that can't do anything on the hardware at
hand. Two user-visible symptoms on IBP14G9 today (after Stages 1 + 2):

1. The `TGP Offset` slider is always rendered, even though the daemon
   has no NB02 backend to apply it. Now that Stage 2 publishes honest
   `caps.gpu_control`, the TUI can gate this field.
2. Even when the iGPU panel is populated correctly (Stage 1), the empty
   dGPU panel still takes 50% of the horizontal space, showing
   "No dGPU detected" on a laptop that *has* no dGPU slot.

## Bug(s) in scope

### Bug — Power form fields are hardcoded, not capability-derived

`tux-tui/src/model.rs:818-828`:
```rust
form_tab: FormTabState::new(vec![FormField {
    label: "TGP Offset".into(),
    key: "tgp_offset".into(),
    field_type: FieldType::Number { value: 0, min: -15, max: 15, step: 1 },
    enabled: true,
}]),
```

`FormTabState::new` defaults `supported = true`. So the slider is
always shown, regardless of what the daemon reports in
`caps.gpu_control`.

Compare: `display_form()` at line 788-802 defaults `supported = false`
and lets `update.rs` flip it on only when the daemon confirms the
capability. `webcam` is the same. Power is the outlier.

### Bug — `render_gpu_info` uses a fixed 50/50 split

`tux-tui/src/views/power.rs:35-38`:
```rust
.constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
```

Two panels, always, regardless of what's actually populated. On an APU
laptop the left panel shows "No dGPU detected" — visual noise for a
socket the machine doesn't have.

## Design

### Model side

`tux-tui/src/model.rs`:

1. `PowerState::new()` builds an **empty form with `supported = false`**.
   Pattern mirrors `display_form()`.
2. The tgp_offset field definition moves into a small builder fn
   `fn tgp_offset_field() -> FormField` so the update.rs code path can
   push it without duplicating literals. Keep the `min: -15, max: 15`
   range as-is — the sign-mismatch follow-up in `follow_up.toml` owns
   any fix to that.

### Update side

`tux-tui/src/update.rs`, `DbusUpdate::Capabilities` arm (around
line 931-984, right after the `display_brightness` block):

```rust
// Power tab capability gating: TGP offset + any future Power-form fields.
model.power.form_tab.fields.clear();
if caps.gpu_control {
    model.power.form_tab.fields.push(model::tgp_offset_field());
}
// Future: if caps.tdp_control { push tdp fields }
model.power.form_tab.supported =
    caps.gpu_control || caps.tdp_control;
```

Rationale for building the whole list from scratch each `Capabilities`
update: every other form in `update.rs` already re-applies field
enable/disable on every capabilities frame, so rebuilding is idiomatic.
Capabilities updates are infrequent (daemon side); cost is negligible.

### View side

`tux-tui/src/views/power.rs` `render_gpu_info`:

Conditional layout. Decision signal: does the panel have anything
meaningful to show? A panel is "populated" when its `_name` is non-empty.
(Telemetry fields without a name would be anomalous — Stage 1 always
sets a name first.)

```rust
let has_d = !state.dgpu_name.is_empty();
let has_i = !state.igpu_name.is_empty();
match (has_d, has_i) {
    (true, true)  => 50/50 split (current behaviour)
    (true, false) => dGPU takes full width, iGPU not rendered
    (false, true) => iGPU takes full width, dGPU not rendered
    (false, false) => fall back to the current 50/50 with placeholders
}
```

`(false, false)` keeps existing placeholder behaviour so we don't break
the unit test that checks the empty-state render path, and so the box
still renders on machines that haven't yet received a daemon frame.

Extract the dGPU-paragraph and iGPU-paragraph construction into two
small helpers `build_dgpu_paragraph` / `build_igpu_paragraph` returning
`Paragraph<'_>` — the current render body is already split conceptually,
moving it into helpers makes the conditional layout readable. No
behaviour change within each panel.

## Files touched

- `tux-tui/src/model.rs` — `PowerState::new()` + new
  `tgp_offset_field()` helper. Make the helper `pub(crate)` so
  `update.rs` can call it.
- `tux-tui/src/update.rs` — Power capability gating in the
  `Capabilities` arm.
- `tux-tui/src/views/power.rs` — conditional layout; small helper
  extraction.

No daemon changes. No `tux-core` changes.

## Tests

### `tux-tui/src/views/power.rs`

- Update `power_state_renders_without_gpu_data`: new fresh
  `PowerState::new()` now has `supported = false` and an empty fields
  list. Assertions change accordingly.
- Add `power_state_supported_when_gpu_control_true` — doesn't exercise
  the render path directly; asserts that after a Capabilities update
  with `gpu_control = true`, the tgp_offset field exists and
  `supported == true`. (See update.rs test below — same concern, but
  placement goes wherever fits the existing file structure; if a test
  in views/power.rs for supported state is natural, put it there, else
  in update.rs.)

### `tux-tui/src/update.rs`

Two new tests in the existing `#[cfg(test)] mod tests`:

| Test | Setup | Asserts |
|---|---|---|
| `capabilities_enable_power_when_gpu_control` | feed a `Capabilities` TOML with `gpu_control = true, tdp_control = false, ...` | `model.power.form_tab.supported == true`; one field present with key `"tgp_offset"` |
| `capabilities_leave_power_disabled_when_no_gpu_or_tdp` | feed a `Capabilities` TOML with both flags false | `model.power.form_tab.supported == false`; fields empty |

Use the capabilities-update test fixtures already in the file (grep for
`Capabilities(` in the `#[cfg(test)]` block) as a starting point — if
they exist, extend/copy.

### `tux-tui/src/views/power.rs` render test (optional)

The `#[cfg(test)] mod tests` is tiny today. Add a headless render
smoke for the APU-only collapse case:
- `power_view_collapses_dgpu_panel_when_apu_only` — set
  `state.igpu_name = "amdgpu"`, render into a buffer, assert the
  "No dGPU detected" string is NOT in the buffer and the "amdgpu"
  string IS present.

Use the pattern from `render_all_tabs_headless` in `tux-tui/src/view.rs`
tests as a model for headless buffer rendering.

## Out of scope

- Dashboard package-power line (Stage 4).
- Docs (Stage 5).
- `tgp_offset` sign mismatch (`follow_up.toml`).
- The iGPU-only-desktop layout path is exercised implicitly by the
  APU collapse test above; no separate test needed.

## Phase exit criteria

- `cargo test --workspace`: all tests pass.
- `cargo clippy --workspace --tests -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.
- No new integration tests required — this is pure TUI-side gating.
- `worklog-3.md` written.
- Commit on existing branch; PR not opened.

## Branch

Continue on `feat/issue-19-amd-power-tab-fixes`.
