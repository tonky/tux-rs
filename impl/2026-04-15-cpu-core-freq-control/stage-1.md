# Stage 1: Extend CpuGovernor for Core and Frequency Control

## Context
TCC controls CPU performance by modifying sysfs files. `tux-rs` already has `CpuGovernor` which provides basic controls. We need to add the ability to toggle CPU cores and set frequency limits.

## File References
- `tux-daemon/src/cpu/governor.rs`

## Code Changes
1. **`set_online_cores(count: u32)`**
   - Iterate over all logical CPUs.
   - For `cpu0`, it is always online.
   - For `cpu1` to `cpuN`, write `1` to `/sys/devices/system/cpu/cpuN/online` if `N < count`, else write `0`.
   - Read the total available cores via `/sys/devices/system/cpu/possible` or by iterating `cpuN` directories.

2. **`set_scaling_min_freq(freq: u32)`**
   - Write the frequency value to `/sys/devices/system/cpu/cpuN/cpufreq/scaling_min_freq` for all online CPUs.
   - We will use the existing `write_all` method, which skips offline CPUs since their `cpufreq` directories might be unavailable or read-only.

3. **`set_scaling_max_freq(freq: u32)`**
   - Write the frequency value to `/sys/devices/system/cpu/cpuN/cpufreq/scaling_max_freq` for all online CPUs.
   - We will use the existing `write_all` method for this as well.

4. **Testing**
   - Extend `setup_cpu_tree` in the tests to create the `online` file for fake CPUs.
   - Add unit tests for `set_online_cores`, `set_scaling_min_freq`, and `set_scaling_max_freq` using the fake CPU sysfs tree.

## Follow up
- Review the implemented stage.
- Run tests to ensure no regressions are introduced.