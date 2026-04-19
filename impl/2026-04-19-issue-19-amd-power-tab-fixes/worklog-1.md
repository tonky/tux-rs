# Worklog — Stage 1: GPU info pipeline

## Session: 2026-04-19

### Implemented

Branch: `feat/issue-19-amd-power-tab-fixes` (forked from `main`).

Three bugs fixed on one coupled pipeline:

- **Bug A — classification.** `tux-daemon/src/gpu/hwmon.rs`: extracted the
  driver→type mapping into `classify_gpu()`. `nvidia`/`i915`/`xe` keep
  their static type; `amdgpu` resolves at runtime via
  `<hwmon>/device/boot_vga` (`'1'` → `Integrated`, else `Discrete`).
  Added `GpuType::as_wire_str()` and `From<GpuInfo> for GpuData` so the
  daemon-side type converts cleanly into the `tux-core` wire type.
- **Bug B — serialization.** `tux-daemon/src/dbus/system.rs`:
  `get_gpu_info` now builds a `GpuInfoResponse { gpus: Vec<GpuData> }`
  (table at the root, required by TOML) instead of serializing
  `Vec<GpuInfo>` directly. Added a short doc-block explaining *why*
  (pre-existing bug: every call returned "unsupported rust type").
- **Bug C — TUI consumer.** `tux-tui/src/update.rs`: the
  `DbusUpdate::GpuInfo` arm now deserializes `GpuInfoResponse`, resets
  model fields first so stale state doesn't persist across polls, and
  pivots entries into `dgpu_*` / `igpu_*` based on `gpu_type`
  (case-insensitive "integrated" check; anything else treated as
  discrete). Extras log a debug event and are dropped. Parse failures
  log a debug event.

### Tests added

- `tux-daemon/src/gpu/hwmon.rs` — 6 new unit tests:
  - `discover_intel_igpu_xe`
  - `discover_amdgpu_apu_classified_integrated`
  - `discover_amdgpu_dgpu_classified_discrete`
  - `discover_amdgpu_missing_boot_vga_falls_back_to_discrete`
  - `discover_hybrid_amd_apu_plus_nvidia_dgpu`
  - `discover_hybrid_intel_igpu_plus_nvidia_dgpu`
  - `gpu_data_conversion_carries_wire_strings`
  (the original `discover_amdgpu` test was replaced by the two boot_vga
  variants.)
- `tux-tui/src/update.rs` — rewrote `gpu_info_updates_power_state`
  against the new TOML shape; added
  `gpu_info_apu_only_populates_igpu_only` (IBP14G9 regression) and
  `gpu_info_clears_stale_state_between_polls`.
- `tux-daemon/tests/integration.rs` — new
  `gpu_info_returns_parseable_response` regression for Bug B (asserts
  `GetGpuInfo` returns a parseable `GpuInfoResponse` and every entry
  has a known `gpu_type` wire value).

### Verification

- `cargo test --workspace`: all 754 tests pass (0 failed, 2 ignored —
  `ibp_gen8_live_regression` and one pre-existing ignore).
- `cargo clippy --workspace --tests -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.
- `dbus-run-session -- cargo test -p tux-daemon --test integration`:
  all 14 tests pass, including the new `gpu_info_returns_parseable_response`.

### Decisions & deviations

- **Wire format**: chose lowercase `"discrete"` / `"integrated"` strings
  for `GpuData.gpu_type`. The existing `gpu_info_roundtrip` test in
  `tux-core/src/dbus_types.rs:269` uses PascalCase `"Discrete"` as a
  fixture — that test only checks roundtrip equivalence, so any string
  is valid. Lowercase keeps the wire contract consistent with
  lowercase-convention D-Bus surfaces elsewhere in the project. The TUI
  consumer uses `eq_ignore_ascii_case` so PascalCase would also parse,
  giving us tolerance if the format ever drifts.
- **From impl placement**: put `impl From<GpuInfo> for GpuData` in
  `tux-daemon/src/gpu/hwmon.rs` (the daemon-side type lives there). A
  `tux-core`-side impl would require a reverse dependency.
- **No review agents run**: per AGENTS.md we would normally launch two
  parallel review subagents after the phase. User opted to skip this
  round to save tokens/time; noted.

### Follow-ups recorded

See `follow_up.toml`:
- `boot_vga_missing_apu` — on pre-modern kernels without `boot_vga`,
  single-`amdgpu` APU laptops still fall back to `Discrete`. Low priority.
- `ctgp_offset_sign_mismatch` — Power form exposes `-15..=15` but the
  sysfs write path takes `u8 0..=255`. Pre-existing.
- `amd_energy_fallback` — already planned as part of Stage 4.

### Out of scope (confirmed not touched)

- Capability flags (Stage 2).
- TUI rendering / form gating (Stage 3).
- Dashboard package-power line (Stage 4).
- Docs (Stage 5).
