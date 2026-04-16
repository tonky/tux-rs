# Feature Description: CPU Active Cores and Frequency Control

Based on TCC capabilities (Issue #16), we need to allow changing the number of active CPU cores and their scaling frequencies.

## Investigation Findings

1. The data model in `tux-core/src/profile.rs` already supports `online_cores`, `scaling_min_frequency`, and `scaling_max_frequency` in the `CpuSettings` struct.
2. TCC controls these properties by writing to:
   - `/sys/devices/system/cpu/cpu*/online` for core activation. `cpu0` is always online, so writing `1` to `N` cores and `0` to the rest sets the active core count.
   - `/sys/devices/system/cpu/cpu*/cpufreq/scaling_min_freq` and `scaling_max_freq` for frequency constraints.
3. In `tux-rs`, the `CpuGovernor` in `tux-daemon/src/cpu/governor.rs` already controls sysfs settings (governor, EPP, turbo) across all CPUs.