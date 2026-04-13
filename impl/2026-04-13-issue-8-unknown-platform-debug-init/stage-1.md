# Stage 1: Detection and Diagnostics Hardening

## Scope
- Detection behavior in tux-core DMI logic.
- Startup error handling and diagnostics in tux-daemon.
- Unit tests for new detection behavior.

## Planned changes
1. Update platform heuristic fallback:
- File: tux-core/src/dmi.rs
- Add NB02 board_vendor fallback to Platform::Uniwill before WMI-only checks.

2. Improve detection robustness for composite SKU values:
- File: tux-core/src/dmi.rs and/or tux-core/src/device_table.rs
- Consider tokenized SKU matching for strings like "SKU_A / SKU_B".

3. Add copy-paste startup diagnostics:
- File: tux-daemon/src/main.rs (and possibly tux-core/src/dmi.rs helper API)
- On detection/init failure, print a structured diagnostics block:
  - all DMI fields
  - platform probe booleans (WMI GUIDs/sysfs paths)
  - daemon version, build mode, and command hints

4. Tests:
- File: tux-core/src/dmi.rs tests
- Add regression test for NB02 + unknown SKU fallback.
- Add regression test for composite SKU behavior if implemented.

## Validation
- cargo test -p tux-core dmi
- cargo test -p tux-daemon (targeted startup/detection tests where applicable)
- cargo fmt --all -- --check

## Risks
- Misclassifying non-Uniwill NB02-like systems if heuristics are too broad.
- Overly verbose diagnostics in normal startup paths (must only emit on failures or explicit debug mode).
