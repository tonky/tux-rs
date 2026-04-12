# Review: Stages 1–4 (tuxedo-drivers migration)

Date: 2026-04-12

Two sub-agents reviewed the implementation for (a) conformance and (b) code quality.

---

## Conformance Gaps Found

### Stage 2 — Missing charging control migration
Spec required updating Clevo flexicharger and Uniwill charge profiles to use `tuxedo_keyboard` sysfs paths. `charging/clevo.rs` and `charging/uniwill.rs` still reference tux-kmod paths. Tracked in `follow_up.toml` → f002, f005.

### Stage 2 — Missing IBP16 Gen8 priority test
A regression test for InfinityBook Pro 16 Gen 8 (SKU `IBP16I08MK2`) through the full detection+backend path was absent. **Fixed**: added `exact_sku_match_ibp16_gen8` test to `dmi.rs`.

### Stage 3 — NB04 keyboard backlight not implemented
`tuxedo_nb04_kbd_backlight` LED subsystem integration was not done. Tracked in `follow_up.toml` → f003.

### Stage 4 — Device table SKU sweep incomplete
No reconciliation artifact documenting parity vs tuxedo-drivers DMI tables. Tracked in `follow_up.toml` → f004.

---

## Correctness/Quality Issues Fixed

| Severity | File | Issue | Fix Applied |
|---|---|---|---|
| MAJOR | `td_uniwill.rs` | `ec_to_pwm`: `ec as u16` wraps on negative input (e.g. hardware glitch returning -1 → 65535 → overflow) | Used `.clamp(0, EC_PWM_MAX)` before cast |
| MAJOR | `td_nb04.rs` | `set_auto` accepted out-of-range fan index without bounds check | Added `check_fan_index(fan_index)?` |
| MAJOR | `td_clevo.rs` | `write_pwm` only read `0..max_fans` when building packed i32, leaving third fan slot always 0 | Now reads all `CLEVO_MAX_FANS` slots; slot 2 error uses `.unwrap_or(0)` |
| MAJOR | `td_nb05.rs`, `td_tuxi.rs`, `td_nb04.rs` | `discover_hwmon`, `check_fan_index`, `fan_attr`, `PWM_ENABLE_*` triplicated | Extracted to `sysfs.rs` as public shared utilities |
| MODERATE | `td_tuxi.rs` | `read_fan_rpm` returned `Ok(0)` when hwmon absent, inconsistent with `read_temp` returning `Err(Unsupported)` | Now returns `Err(Unsupported)` |
| MODERATE | `device_table.rs` | `AURA14GEN4` and `AURA15GEN4` individual entries never matched on real hardware | Removed both; only `"AURA14GEN4 / AURA15GEN4"` remains |
| MINOR | `td_clevo.rs` | `W_CL_FANAUTO` vs Uniwill's `W_UW_FANAUTO` style difference unexplained | Added inline comment documenting `_IOW` vs `_IO` ioctl distinction by driver design |

## Items Deferred (not fixed)

- `MockTuxedoIo` cannot simulate read errors (returns only `Ok(val)`) → tracked in follow_up.toml, lower priority
- `Nb04Profile` could implement `fmt::Display` / `TryFrom<&str>` for standard Rust ergonomics → deferred
- `mod.rs` backend selection branching untested — to be addressed in Stage 5 integration work
