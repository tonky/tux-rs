# Worklog — Stage 3 (Uniwill EC TDP)

## 2026-11-24

### Done
- `0307030` tuxedo_io TDP ioctl codes (+ `_IOC` derivation test).
- `ac1fa3d` `UniwillEcTdp`: driver bounds, errno-as-value decoding, clamp +
  read-back verification; simulator mirroring tuxedo_io's X5AR45xS behaviour.
- `feb6209` `TdpSource::UniwillEc` + `build_backend_with` seam; `bcdf125`
  IBM15I10 opt-in (`tdp: None`, bounds from the driver).
- `c8e171f`/`84c7b07`/`b5d72b6` client PL methods + live round-trip that always
  restores.
- Review fixes: `05a8b6c`, `b0dd77e`; docs + follow-ups in the wrap-up commit.

### Live verification (daemon restarted by user)
- Startup: `Uniwill EC TDP backend available (PL1: 10-90W, PL2: 10-90W, PL4
  read-only: 10-230W)`.
- `just live-test` PASSED: 90→89 W read-back, PL2 above max clamped to 90 W,
  restored 90/90 W.
- RAPL MSR/MMIO `constraint_0_power_limit_uw` stay at 90 W after `SetPl1 60`:
  the EC does not reprogram RAPL. **Enforcement measured instead** (16-thread
  busy loop, `GetPackagePowerW`): PL1=PL2=25 W → 32.6 W @4 s, then 25.0 W;
  90 W → ~28 W sustained with EPP `balance_performance`.
- Same load at 90 W with EPP=`performance`: 85.8 W burst, 50.0 W sustained —
  confirms TCC's IBM15I10 EPP quirk matters (Stage 4). Values restored after.

### Decisions
- Driver bounds are authoritative; `descriptor.tdp` unused (plan deviation,
  recorded in stage-3.md).
- PL4 read-only (trait is PL1/PL2).
- TDP only written when a profile sets it (unlike TCC, which writes maxima on
  every activation) — keeps the EC out of overboost unless asked.

### Verification
- `just check` green (see wrap-up); live test green on hardware.
