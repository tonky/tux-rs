# Stage 5 — GPU telemetry (NVIDIA + Intel) without waking the dGPU

User request (2026-11-24): "nvidia and intel gpu stats: btop can read them, why can't we?"
Constraint from the user's goal (best battery life): never wake a runtime-suspended dGPU.

## Findings
- `GetGpuInfo` only scans hwmon; proprietary NVIDIA and i915 expose no GPU hwmon
  here → `gpus = []`. The TUI also fetches GPU info only once per connection.
- iGPU `0000:00:02.0` → `i915` (`card1`): `gt/gt0/rc6_residency_ms`,
  `gt/gt0/rps_act_freq_mhz`; RAPL `intel-rapl:0:1` = `uncore` (≈ iGPU power).
- dGPU `0000:01:00.0` → `nvidia`, `power/runtime_status = suspended` (RTD3).
  NVML/nvidia-smi queries wake it (btop keeps it awake). Model name is readable
  from `/proc/driver/nvidia/gpus/<bdf>/information` without waking it.
- Names: `/usr/share/hwdata/pci.ids` (Intel `7d51` → "Arrow Lake-P [Arc Pro 130T/140T]").

## Steps (one commit each)
1. core: `GpuData.{power_state, freq_mhz}` (additive, serde default).
2. gpu/intel: i915 sampler — busy % = 100 − ΔRC6/Δt, actual freq, uncore power (ΔE/Δt).
3. gpu/nvidia: runtime-PM guard; only when `active` query NVML (`nvml-wrapper`,
   dlopen at runtime) for temp/usage/power/clock; otherwise report `suspended`.
4. daemon: `GetGpuInfo` merges hwmon + Intel + NVIDIA via a persistent sampler.
5. tui: refresh GPU info every few seconds; Power tab shows state and clock.
6. live check: values on AC/battery; dGPU stays `suspended` while polled.

## Outcome (2026-11-24)
- iGPU (i915): name, busy %, clock, uncore power — verified live.
- dGPU (RTX 5060): `suspended` is never queried and stays asleep under polling
  (verified); while in use, NVML temp/usage/power/clock are shown (verified).
- Known, accepted limitation (user decision): if the dGPU is awake when the TUI
  is open, the TUI's 2 s polling can keep it awake until the TUI closes, because
  each NVML query restarts its ~20 s idle timer. Mitigated by only querying
  while a non-background process holds `/dev/nvidiaN` (NVIDIA daemons and
  holders seen while suspended are ignored); a pre-existing holder such as
  gnome-shell can still count as a user. The TUI is rarely open, so no
  further work (a "yield probe" was designed but not built).
