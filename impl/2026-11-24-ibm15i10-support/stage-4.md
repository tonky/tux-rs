# Stage 4 — EPP performance quirk (AC only)

## Context
- TCC 3.1.2 `CpuWorker`: `eppPerformanceQuirkDevices = [GEMINI17I04, IBPG10INTEL,
  IBM15I10]` → EPP forced to `performance` "to allow full frequency range",
  on every profile, AC and battery.
- Measured on IBM15I10 (16-thread busy loop, EC PL1/PL2 90 W):
  `balance_performance` ≈ 28 W sustained; `performance` 86 W burst / 50 W sustained.
- **User decision (2026-11-24): "best possible battery life"** → apply the
  quirk on AC only; battery profiles keep their own EPP (built-ins: Quiet
  `balance_power`, Max Energy Save `power`).

## Implementation (`c762c02`)
- `tux-daemon/src/profile_apply.rs`: `AC_EPP_PERFORMANCE_SKUS = ["IBM15I10"]`
  (only hardware-verified SKUs), `ProfileApplier::with_device_quirks(sku)`,
  `apply_on(profile, PowerState)`; `apply()` unchanged (no quirks).
- `tux-daemon/src/main.rs`: initial apply and `apply_profile_by_id` pass the
  power source.
- Tests: AC → `performance`; battery → profile EPP; other SKU → profile EPP;
  plain `apply` ignores the quirk.

## Live verification
- Startup log: `EPP quirk active for IBM15I10`; on AC sysfs EPP = `performance`.
- Unplug AC → battery profile applied, EPP = `balance_power`; replug → `performance`.
