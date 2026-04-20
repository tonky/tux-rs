# Worklog — Stage 2: derive `gpu_control` capability from backend presence

## Session: 2026-04-19

### Implemented

Branch: `feat/issue-19-amd-power-tab-fixes` (Stage 1 already committed).

Single bug fixed on a single source-of-truth surface:

- **Bug — `gpu_control` hardcoded `false`.** `tux-daemon/src/dbus/settings.rs`:
  added `gpu_available: bool` as the trailing parameter of
  `SettingsInterface::new`; the `CapabilitiesResponse` now sets
  `gpu_control: gpu_available` instead of the hardcoded `false`. Updated
  all 9 in-file unit-test call sites to append `false` (preserves their
  current behaviour — none of them construct a GPU backend).
- **Production caller.** `tux-daemon/src/dbus/mod.rs:163`: passes
  `gpu_backend.is_some()`, mirroring the `tdp_backend.is_some()` pattern
  on the line above.

### Tests added / updated

- `tux-daemon/src/dbus/settings.rs` — extended
  `get_capabilities_reflects_device` with one extra assertion
  (`assert!(!caps.gpu_control)`); covers the new param's wiring without
  needing a dedicated test.
- `tux-daemon/tests/common/mod.rs` — added `MockGpuPowerBackend` (mutex
  over `u8`, infallible get/set; mirrors the in-tree `MockGpuPower` that
  lives in `tux-daemon/src/dbus/gpu_power.rs:39-60`, but kept in the
  integration test module so the daemon binary doesn't carry it).
  Added `gpu: Option<Arc<dyn GpuPowerBackend>>` to `TestDaemonBuilder`,
  added `with_gpu(backend)` builder method, and threaded the option
  through `start_with_options` into `DbusConfig.gpu_backend` (was
  previously hardcoded `None`).
- `tux-daemon/tests/integration.rs` — two new tests under a new
  `// ── GPU capability contract ──` section header, modelled directly on
  the existing TDP pair:
  - `capabilities_reflect_gpu_backend` — daemon built with
    `with_gpu(MockGpuPowerBackend::new(0))`, asserts
    `caps.gpu_control == true`.
  - `capabilities_no_gpu_when_no_backend` — default daemon, asserts
    `caps.gpu_control == false`.

### Verification

- `cargo test --workspace`: all tests pass (0 failed).
- `dbus-run-session -- cargo test -p tux-daemon --test integration`:
  16 tests pass (2 new + 14 prior, including Stage 1's
  `gpu_info_returns_parseable_response`).
- `cargo clippy --workspace --tests -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean (rustfmt expanded the
  `power_settings_with_governor` test's `SettingsInterface::new` call to
  multi-line form because the new arg pushed it past 100 cols; left
  as-is).

### Decisions & deviations

- **Mock placement.** The integration mock (`MockGpuPowerBackend`) is in
  `tests/common/mod.rs`, not `src/dbus/gpu_power.rs`. The in-source
  `MockGpuPower` there is `#[cfg(test)]`-gated to that module's tests
  only; integration tests live outside the crate so they can't reach it.
  Considered exposing it via a `test-utils` feature, judged premature —
  one duplicate, ~15 lines, no other callers in sight.
- **Param ordering.** New `gpu_available` appended at the end of
  `SettingsInterface::new` (already had
  `#[allow(clippy::too_many_arguments)]`). Considered grouping it next to
  `tdp_available` for readability, but appending preserves a clean diff
  for the production caller — the existing arg list stays untouched.
- **No review subagents this stage.** Same precedent as Stage 1: skipped
  per user preference until later in the branch.

### Follow-ups recorded

None new this stage. The Stage-1 follow-ups in `follow_up.toml` remain
relevant (AMD MSR backend, `tgp_offset` sign mismatch, `boot_vga` legacy
fallback, silent D-Bus drop debug log).

### Out of scope (confirmed not touched)

- TUI form-field gating against the new `caps.gpu_control` (Stage 3).
- dGPU panel collapse logic (Stage 3).
- Dashboard package-power line (Stage 4).
- Docs (Stage 5).
- AMD `ryzen_smu`/RyzenAdj backend (deferred; in `follow_up.toml`).
