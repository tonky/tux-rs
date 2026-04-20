# Stage 2 — Derive `gpu_control` capability from GPU backend presence

## Goal

Stop lying about GPU power control on hardware that has no NB02 backend.

Today `tux-daemon/src/dbus/settings.rs:139` hardcodes
`gpu_control: false` regardless of what the daemon actually wired up.
The `GpuPowerInterface` is registered conditionally on
`gpu_backend.is_some()` (`tux-daemon/src/dbus/mod.rs:229-233`), but the
capability flag the TUI consumes never reflects that. So the TUI today
*can't* gate the cTGP-offset slider — even on a working NB02 system the
flag is always false.

This stage establishes a single source of truth for `gpu_control`:
the daemon-built `Option<Arc<dyn GpuPowerBackend>>`. TUI gating against
it is Stage 3.

## Bugs in scope

### Single bug — `gpu_control` is hardcoded `false`

`tux-daemon/src/dbus/settings.rs:139`:
```rust
let caps = CapabilitiesResponse {
    ...
    tdp_control: tdp_available,
    power_profiles: true,
    gpu_control: false,             // ← hardcoded
    display_brightness: ...,
};
```

Two consequences, both mirror the pre-Stage-1 TDP situation:
1. NB02 systems can't be detected client-side, so the TUI has nothing
   honest to gate on.
2. AMD/Intel-only systems can't be reliably distinguished from
   "backend not yet probed", which would otherwise let us hide the
   slider on hardware where it is genuinely unsupported.

The fix mirrors the existing `tdp_control: tdp_available` pattern two
lines above (added in PR #20, Stage 4 of the RAPL work): pass a
`gpu_available: bool` through `SettingsInterface::new`, derived in
`dbus/mod.rs` from `gpu_backend.is_some()`.

## Design

### Daemon side

1. `tux-daemon/src/dbus/settings.rs`:
   - Add `gpu_available: bool` as the **last** parameter of
     `SettingsInterface::new` (preserves the existing
     `#[allow(clippy::too_many_arguments)]` annotation; one more
     argument doesn't change the lint posture).
   - In the body: `gpu_control: gpu_available`.
   - All 8 in-file unit-test call sites get `false` appended (current
     behaviour — none of them construct a GPU backend).

2. `tux-daemon/src/dbus/mod.rs:153`:
   - Append `gpu_backend.is_some()` to the `SettingsInterface::new`
     call. `gpu_backend` is in scope at that point (cloned at line
     128 for the TCC compat path; the `Option<Arc<…>>` itself is
     consumed by `serve_at` at line 230, but `is_some()` is read
     before that).

### Test-helper side

3. `tux-daemon/tests/common/mod.rs`:
   - Add `gpu: Option<Arc<dyn GpuPowerBackend>>` field to
     `TestDaemonBuilder` (default `None`).
   - Add `with_gpu(backend)` builder method, mirroring `with_tdp`.
   - Extend `start_with_options` signature to accept
     `gpu_backend: Option<Arc<dyn GpuPowerBackend>>`, default `None`
     in `TestDaemon::start()`, threaded through to
     `DbusConfig.gpu_backend` (currently hardcoded `None` at line
     244 — replace with the parameter).
   - Add `use tux_daemon::gpu::GpuPowerBackend;` to the imports.

4. **Mock backend for the +ve test.**
   `tux-daemon/src/dbus/gpu_power.rs:39-60` already defines a
   `MockGpuPower` for unit tests, but it's `#[cfg(test)]`-only inside
   that module. Define a sibling `MockGpuPowerBackend` in
   `tests/common/mod.rs` (the integration-test `common` module),
   identical contract: `Arc<Mutex<u8>>` storage, infallible
   get/set. Keep it local to integration tests so the daemon binary
   doesn't carry a mock.

### TUI side

**No changes** — Stage 3 wires the TUI to `caps.gpu_control`. This
stage only fixes the daemon's source of truth.

## Files touched

- `tux-daemon/src/dbus/settings.rs` — signature + body + 8 test call
  sites.
- `tux-daemon/src/dbus/mod.rs` — pass `gpu_backend.is_some()`.
- `tux-daemon/tests/common/mod.rs` — builder field + method + plumb
  through `start_with_options` + DbusConfig + integration mock.
- `tux-daemon/tests/integration.rs` — two new tests.

No `tux-core` changes. No TUI changes.

## Tests

### `tux-daemon/src/dbus/settings.rs` unit test addition

- Extend `get_capabilities_reflects_device` (line 384) to assert
  `assert!(!caps.gpu_control)` when called with `gpu_available =
  false`. (One assertion line; covers the new param's wiring without
  needing a new test.)

### `tux-daemon/tests/integration.rs` regressions

Two tests modelled directly on the existing TDP pair at lines
494 (`capabilities_reflect_tdp_backend`) and 603
(`capabilities_no_tdp_when_no_backend`):

| Test | Setup | Asserts |
|---|---|---|
| `capabilities_reflect_gpu_backend` | `TestDaemonBuilder::new(...).with_gpu(Arc::new(MockGpuPowerBackend::new(0))).build()` | `caps.gpu_control == true` |
| `capabilities_no_gpu_when_no_backend` | `TestDaemon::start(...)` (no builder) | `caps.gpu_control == false` |

Place both in a `// ── GPU capability contract ──` section header,
right after the existing GpuInfo block (after line 622).

The negative test will technically be redundant with any existing
`get_capabilities` smoke that runs through the default test daemon —
but keep it explicit because the parallel `capabilities_no_tdp_…`
exists for the same reason: it documents the contract directly at the
boundary, so future drift is caught at the right layer.

## Justfile

No new commands. Existing `just test`, `just clippy`, `just fmt` cover
this stage.

## Out of scope

- Any TUI change (form-field gating, dGPU panel collapse — all
  Stage 3).
- AMD `ryzen_smu`/RyzenAdj backend (deferred per `follow_up.toml`).
- The `tgp_offset` `i8`/`u8` sign mismatch (deferred per
  `follow_up.toml`).
- Dashboard package power (Stage 4).
- Docs (Stage 5).

## Phase exit criteria (per AGENTS.md)

- `cargo test --workspace`: all tests pass.
- `dbus-run-session -- cargo test -p tux-daemon --test integration`:
  both new tests pass.
- `cargo clippy --workspace --tests -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.
- Review subagents: skipped this stage too unless user asks otherwise
  (precedent from Stage 1).
- `worklog-2.md` written summarising decisions and any deviations.
- `follow_up.toml` updated only if new follow-ups surface.
- Commit on the existing `feat/issue-19-amd-power-tab-fixes` branch.
  PR not opened until Stage 5.

## Branch

Continue on `feat/issue-19-amd-power-tab-fixes` (already exists,
Stage 1 committed at 981a324).
