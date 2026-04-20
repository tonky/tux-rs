# Stage 1 — GPU info pipeline: classification + D-Bus contract

## Goal

End-to-end fix for the iGPU/dGPU panels: the Power-tab info block must
populate correctly on AMD APUs, hybrid laptops, and Intel-only systems.

This stage covers **three coupled bugs** identified during plan review.
All three live on the same code path, and shipping any one in isolation
would leave no user-visible change.

## Bugs in scope

### Bug A — `amdgpu` always typed `Discrete`

`tux-daemon/src/gpu/hwmon.rs:30-35`:
```rust
const GPU_DRIVERS: &[(&str, GpuType)] = &[
    ("nvidia", GpuType::Discrete),
    ("amdgpu", GpuType::Discrete),  // ← wrong on APU laptops
    ("i915",   GpuType::Integrated),
    ("xe",     GpuType::Integrated),
];
```

On the IBP14G9 (Ryzen 7 8845HS), the integrated Radeon also reports as
`amdgpu` and is typed `Discrete`, so it lands in the dGPU panel slot
(once Bug B/C are fixed) and the iGPU panel stays blank.

### Bug B — `GetGpuInfo` D-Bus method always errors

`tux-daemon/src/dbus/system.rs:83-86`:
```rust
fn get_gpu_info(&self) -> zbus::fdo::Result<String> {
    let gpus = hwmon::discover_gpus(Path::new("/sys/class/hwmon"));
    toml::to_string(&gpus).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
}
```

`toml::to_string(&Vec<_>)` returns `Err("unsupported rust type")` because
TOML requires a table at the root, not an array. Verified
2026-04-19 with `tmp/toml_probe/` standalone repro. So this method has
been returning a D-Bus error for the lifetime of the surface.

### Bug C — TUI consumer parses the wrong shape

`tux-tui/src/update.rs:1099-1119`:
```rust
DbusUpdate::GpuInfo(toml_str) => {
    if let Ok(table) = toml_str.parse::<toml::Table>() {
        if let Some(name) = table.get("dgpu_name").and_then(|v| v.as_str()) { ... }
        if let Some(name) = table.get("igpu_name").and_then(|v| v.as_str()) { ... }
        // dgpu_temp / dgpu_usage / dgpu_power / igpu_usage ...
    }
}
```

The TUI reads flat top-level keys, but the daemon was always going to
serialize a `Vec<GpuInfo>` (or, post-fix, `GpuInfoResponse { gpus: Vec<…> }`).
The two ends never agreed on a contract.

The error is silently swallowed at `tux-tui/src/dbus_task.rs:472`
(`if let Ok(s) = client.get_gpu_info().await { ... }`), so the user sees
the placeholder text from `views/power.rs:71-87` (`"No iGPU detected"`)
on every machine, not just APU-only ones.

## Design

### Contract

Use the already-defined `GpuInfoResponse` from
`tux-core/src/dbus_types.rs:52-66` as the single source of truth:
```rust
pub struct GpuInfoResponse { pub gpus: Vec<GpuData> }
pub struct GpuData {
    pub name: String,
    pub temperature:   Option<f32>,
    pub power_draw_w:  Option<f32>,
    pub usage_percent: Option<u8>,
    pub gpu_type: String,   // "discrete" | "integrated"
}
```

There is already a roundtrip test at line 271-281; we'll lean on it.

### Daemon side

1. `tux-daemon/src/gpu/hwmon.rs`:
   - Keep the static driver→type map for `nvidia`, `i915`, `xe`.
   - Make the `amdgpu` row "needs runtime classification".
   - In `read_gpu_from_hwmon`, after identifying `amdgpu`, read
     `<hwmon_dir>/device/boot_vga` (one byte: `'1'\n` or `'0'\n`).
     `1` → `Integrated`. `0` or missing or unreadable → `Discrete`
     (legacy behaviour).
   - Implementation note: `hwmon_dir.join("device/boot_vga")` —
     `device` is a symlink in real sysfs; `fs::read_to_string` follows
     it. Tests model it as a real subdir + file. No `symlink` calls
     needed.

2. `tux-daemon/src/dbus/system.rs:83-86`: build a
   `GpuInfoResponse { gpus: vec_of_gpu_data }`, where `GpuData` is
   constructed from each `GpuInfo` (`gpu_type` becomes `"discrete"` or
   `"integrated"` to match `tux-core`'s contract). Serialize that.

3. `tux-daemon/Cargo.toml` likely already depends on `tux-core` —
   confirm during implementation.

### TUI side

1. `tux-tui/src/update.rs` `DbusUpdate::GpuInfo` arm:
   - Replace the flat-key parser with `toml::from_str::<GpuInfoResponse>`.
   - Reset model fields first (so devices that disappeared between polls
     don't leave stale state).
   - For each `GpuData`:
     - `gpu_type == "discrete"` → first one wins for `dgpu_*`,
       extras logged as `EventSource::Daemon`/`"extra dGPU ignored"`.
     - `gpu_type == "integrated"` → same for `igpu_*`.
     - Anything else → log + skip.
   - On parse error (shouldn't happen post-fix), log and leave model
     unchanged.

2. `tux-tui/src/dbus_task.rs:472`: no logic change, but verify the
   error path; if it's silently swallowed, decide whether to log a
   one-shot debug event (low value but cheap). Skip for stage-1; revisit
   only if testing shows the silent-drop bites us.

## Files touched

- `tux-daemon/src/gpu/hwmon.rs` — classification + tests.
- `tux-daemon/src/dbus/system.rs` — serialization contract.
- `tux-tui/src/update.rs` — deserialization + pivot.
- `tux-daemon/tests/integration.rs` — new regression for `GetGpuInfo`
  returning a parseable `GpuInfoResponse`.
- (Possibly) `tux-daemon/Cargo.toml` if `tux-core` types aren't already
  in scope from `dbus/system.rs`.

No changes to `tux-core/src/dbus_types.rs` — the type already exists and
is roundtrip-tested.

## Tests

### `tux-daemon/src/gpu/hwmon.rs` unit tests

Replace / extend the existing `discover_amdgpu` test. New layout:

| Test | Setup | Expected |
|---|---|---|
| `discover_amdgpu_apu_classified_integrated` | `amdgpu` hwmon with `device/boot_vga = "1"` | 1 entry, `gpu_type = Integrated` |
| `discover_amdgpu_dgpu_classified_discrete` | `amdgpu` hwmon with `device/boot_vga = "0"` | 1 entry, `gpu_type = Discrete` |
| `discover_amdgpu_missing_boot_vga_falls_back_to_discrete` | `amdgpu` hwmon, no `device/` subdir | 1 entry, `gpu_type = Discrete` |
| `discover_hybrid_amd_apu_plus_nvidia_dgpu` | `amdgpu`+`boot_vga=1` AND `nvidia` hwmon | 2 entries, types `Integrated`+`Discrete` |
| `discover_hybrid_intel_igpu_plus_nvidia_dgpu` | `i915`+`nvidia` (no `amdgpu`) | unchanged behaviour, still passes |

The existing `discover_intel_igpu`, `discover_nvidia_gpu`,
`no_gpu_present_returns_empty`, `missing_sensors_returns_none`,
`multiple_gpus_discovered`, `missing_hwmon_dir_returns_empty`,
`invalid_temp_value_returns_none` — all keep passing as-is, since none
exercise `amdgpu`.

Helper `setup_hwmon` likely needs a sibling helper:
```rust
fn setup_hwmon_with_boot_vga(dir: &Path, name: &str, boot_vga: &str) -> PathBuf {
    let hwmon = dir.join(format!("hwmon{n}"));
    fs::create_dir_all(hwmon.join("device")).unwrap();
    fs::write(hwmon.join("name"), format!("{name}\n")).unwrap();
    fs::write(hwmon.join("device/boot_vga"), format!("{boot_vga}\n")).unwrap();
    dir.to_path_buf()
}
```

### `tux-daemon/tests/integration.rs` regression for Bug B

Add a `gpu_info_returns_parseable_response` test, modelled on the
`tdp_control` tests at lines 494 / 567:
- Spin up the daemon.
- Call `Settings.GetGpuInfo` over D-Bus.
- Assert: returns `Ok(toml_str)`, `toml_str` parses as
  `GpuInfoResponse`. Fields can be empty on the test host — the contract
  is what matters.

### `tux-tui/src/update.rs` test rework

Existing test around line 2283:
```rust
DbusUpdate::GpuInfo(
    "dgpu_name = \"RTX 4060\"\ndgpu_temp = 45.0\ndgpu_usage = 3\ndgpu_power = 15.0\nigpu_name = \"Iris Xe\"\nigpu_usage = 12".to_string(),
)
```

Rewrite its TOML payload to the `GpuInfoResponse` shape:
```toml
[[gpus]]
name = "RTX 4060"
temperature = 45.0
power_draw_w = 15.0
usage_percent = 3
gpu_type = "discrete"

[[gpus]]
name = "Iris Xe"
usage_percent = 12
gpu_type = "integrated"
```

Assertions stay the same (`model.power.dgpu_name == "RTX 4060"`, etc.).

Add a second test:
- `gpu_info_apu_only_populates_igpu_only` — single `[[gpus]]` entry with
  `gpu_type = "integrated"`, `name = "amdgpu"`. Assert
  `model.power.igpu_name == "amdgpu"` and `model.power.dgpu_name`
  remains empty.

## Justfile

Existing commands cover this stage (`just test`, `just clippy`,
`just fmt`). No new commands needed unless something repetitive
emerges during implementation — add then.

## Out of scope

- Any TUI rendering change (the dGPU-panel collapse is Stage 3).
- The capability-derived form supported flag (Stage 2/3).
- Dashboard package power (Stage 4).
- Documentation / follow_up entries (Stage 5).
- The `tgp_offset` `i8`/`u8` sign mismatch (Stage 5 follow-up).

## Phase exit criteria (per AGENTS.md)

- All new + existing tests pass: `cargo test -p tux-core -p tux-daemon
  -p tux-tui`.
- `cargo clippy --workspace --tests -- -D warnings` clean.
- `cargo fmt --all -- --check` clean.
- Two parallel review subagents (Opus 4.6 high + Gemini 3.1 Pro) confirm
  conformance and look for refactor opportunities.
- `worklog-1.md` written summarising decisions and any deviations.
- `follow_up.toml` updated for the `boot_vga`-missing fallback caveat.
- Branch + commit ready, but PR not opened until later stages land.

## Branch

Will create `feat/issue-19-amd-power-tab-fixes` from `main` at the
start of implementation, after this stage plan is approved.
