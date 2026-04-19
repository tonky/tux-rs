# Worklog: AMD APU detection + hide unsupported Power-tab elements

## 2026-04-19 — kickoff

- User flagged the IBP14G9 (AMD, Ryzen 7 8845HS) reporter comment on
  issue #19: iGPU not detected, TGP Offset slider visible despite no NB02
  hardware.
- Confirmed via vendored sources:
  - TCC marks `IBP14A09MK1 / IBP15A09MK1` as `UnsupportedConfigurableTGPDevice`
    (`vendor/tuxedo-control-center/...TuxedoControlCenterDaemon.ts:542`).
  - `tuxedo_io.c:166-262` (`uw_id_tdp()`) has no AMD Gen9 entry —
    vendor does not sanction TDP control on this platform.
- Decided AMD RyzenAdj-equivalent backend is **out of scope** for this
  branch; tracked separately.
- Drafted `description.md` and `plan.md`.
- User answered the four open questions:
  1. GPU classifier — agent's call (chose `boot_vga`).
  2. Add `amd_energy` fallback; omit field if neither source present.
  3. Collapse dGPU panel when no dGPU.
  4. Stage 5 closes the branch; user drafts GH reply later.
- Plan locked. Stage-1 drafted, confirmed, implemented; branch
  `feat/issue-19-amd-power-tab-fixes` created. Per-stage details in
  `worklog-1.md`.
- Stage-2 drafted, confirmed, implemented (commit follows). Per-stage
  details in `worklog-2.md`.
- Stage-3 drafted, implemented autonomously (user authorized batch).
  TUI capability gating + conditional GPU panel layout. Per-stage
  details in `worklog-3.md`.
- Stage-4 drafted, implemented autonomously. Daemon-side `amd_energy`
  hwmon fallback for the dashboard package-power line; wire shape
  kept as `f64` since the TUI already filters `0.0 → "—"` honestly.
  Per-stage details in `worklog-4.md`.
- **Discovery during stage-1 prep**: pre-existing two-bug pipeline failure
  found that explains the user-visible "iGPU not detected" symptom even
  beyond the classification issue.
  - Daemon: `tux-daemon/src/dbus/system.rs:85` does
    `toml::to_string(&Vec<GpuInfo>)`, which the toml crate rejects with
    "unsupported rust type" because TOML requires a table at the root.
    Verified with `tmp/toml_probe/` standalone repro. So `GetGpuInfo`
    has been returning an error to every caller; `tux-tui/src/dbus_task.rs:472`
    silently drops the error.
  - TUI: `tux-tui/src/update.rs:1099-1119` parses flat `dgpu_name` /
    `igpu_name` keys at the top level — a shape the daemon was never
    going to send (the fully-typed `GpuInfoResponse` at
    `tux-core/src/dbus_types.rs:54` defines a `gpus: Vec<GpuData>` form).
  - Folded both into Stage 1 alongside the classification fix; shipping
    just one of the three would not be user-visible.
