# Worklog — Stage 5: docs + follow-ups + branch wrap

## Session: 2026-04-19

### Documented

- **`docs/hardware_support.md`**:
  - Added two rows to the cross-platform capability matrix: GPU
    detection (hwmon + `boot_vga`) and package power draw (Intel RAPL
    with AMD `amd_energy` fallback). Noted as implemented on all five
    platforms since the code path is platform-agnostic.
  - Appended three AMD-specific bullet points to the TDP opt-in
    section clarifying what AMD laptops **do** get on this branch
    (iGPU detection, package-power via amd_energy, capability-gated
    Power form) — separate from the TDP control story that remains
    deferred.

- **`docs/feature_support_matrix.md`**:
  - Temperature sensors table: added "AMD APU iGPU detection" row
    pointing at `gpu/hwmon.rs` boot_vga classifier, plus two
    package-power rows (Intel RAPL and AMD amd_energy).
  - GPU power control table: added a row for runtime capability
    gating of the TGP Offset UI noting the TUI placeholder when
    neither `gpu_control` nor `tdp_control` is present.

- **`README.md`**:
  - Tightened the IBP14G9 sentence to state what now works on AMD
    (iGPU detection, package-power via amd_energy, capability-gated
    Power controls) and where AMD TDP control still sits (deferred,
    tracked in `follow_up.toml`).

### Follow-ups

Reviewed `impl/2026-04-19-issue-19-amd-power-tab-fixes/follow_up.toml`
— all four existing entries are still correct and cover the deferred
work from this branch:

1. AMD `ryzen_smu` / RyzenAdj backend — deferred by design (was never
   in scope for #19).
2. `tgp_offset` `i8`/`u8` sign mismatch — pre-existing, unchanged.
3. `amdgpu` classifier on pre-`boot_vga` kernels — new this branch,
   accepted limitation, documented.
4. Debug-log D-Bus GET errors in `dbus_task.rs` — surfaced by Stage 1,
   good-to-have, not blocking.

No new follow-ups from Stages 3 or 4. The Stage 3 worklog already
confirmed none; Stage 4 likewise — the amd_energy probe landed without
side effects.

### Verification

- `cargo test --workspace`: 173 pass, 0 fail, 1 ignored (live
  regression, pre-existing).
- `cargo clippy --workspace --tests -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.
- Docs render cleanly in the repo `cat` output; no broken markdown
  tables.

### Branch wrap-up

This is the last commit on `feat/issue-19-amd-power-tab-fixes`. Five
commits total:

- `981a324` fix(gpu): repair end-to-end GpuInfo pipeline + classify AMD APUs
- `749dd92` fix(caps): derive gpu_control from GPU backend presence
- `928a233` fix(tui/power): gate Power form on capabilities + collapse absent GPU panel
- `e0039de` feat(power): AMD amd_energy fallback for dashboard package-power
- `<this>` docs: AMD APU + capability gating + amd_energy notes

PR opening is left to the user. User will draft the GitHub-comment
reply to @Ciugam on issue #19 separately after the PR is up.

### Out of scope (confirmed not touched across branch)

- AMD `ryzen_smu` / RyzenAdj MSR backend.
- `tgp_offset` `i8`/`u8` sign mismatch.
- `amdgpu` classifier fallback heuristic for pre-`boot_vga` kernels.
- Per-core AMD energy counters.
- D-Bus error logging in `dbus_task.rs` (quality-of-life follow-up).
