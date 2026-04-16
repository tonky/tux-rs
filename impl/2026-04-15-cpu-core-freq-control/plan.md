# High-Level Plan

1. **Extend `CpuGovernor` in `tux-daemon/src/cpu/governor.rs`:**
   - Add `set_online_cores(count: u32)` to write `1` to the first `count` cores and `0` to the remaining cores in `/sys/devices/system/cpu/cpu*/online`.
   - Add `set_scaling_min_freq(freq: u32)` and `set_scaling_max_freq(freq: u32)` to iterate through online CPUs and update their `/sys/devices/system/cpu/cpu*/cpufreq/scaling_{min,max}_freq`.
   - Ensure these sysfs operations do not break tests and follow the existing `write_all` pattern safely.
2. **Update Profile Application:**
   - Update `tux-daemon/src/profile_apply.rs` to read the properties (`online_cores`, `scaling_min_frequency`, `scaling_max_frequency`) from `profile.cpu` and apply them using the new methods in `CpuGovernor`.
   - If values are `None` (not specified in the profile), ensure they are restored to appropriate defaults or left untouched depending on defined behavior.
3. **Expose Controls in TUI Profile Editor:**
   - Update `tux-tui/src/model.rs` profile editor form to include CPU `online_cores`, `scaling_min_frequency`, and `scaling_max_frequency`.
   - Ensure option semantics are preserved in UI serialization (`None` vs explicit numeric values).
   - Add/extend TUI model tests for round-trip behavior.
4. **Follow up and Reviews:**
   - Run tests (`just check`).
   - Run two sub-agents to review the changes.
   - Stage-by-stage implementation and tracking.