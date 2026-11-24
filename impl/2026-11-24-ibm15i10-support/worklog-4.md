# Worklog — Stage 4 (EPP quirk, AC only)

## 2026-11-24
- User goal: "best possible battery life" → TCC's IBM15I10 EPP quirk applied on
  AC only (TCC applies it always).
- `c762c02` quirk + `apply_on(profile, PowerState)`; `1164370` review nits +
  unplug/replug test; `8acb180` battery fallback `balance_power` when a battery
  profile has no EPP (review blocker).
- Live (daemon restarted by user, charger plugged → unplugged → replugged):
  AC start Office → EPP `performance` (quirk); battery → Quiet, profile EPP
  `balance_power`, no override; AC again → `performance`.
- `just check` → 824 passed, 0 failed.
- Earlier measurement motivating the quirk: `balance_performance` ≈ 28 W vs
  `performance` ≈ 50 W sustained (16-thread load, PL 90 W).
