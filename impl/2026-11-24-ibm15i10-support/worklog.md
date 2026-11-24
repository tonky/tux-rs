# Worklog — IBM15I10 support

## 2026-11-24 — Investigation & planning

- Captured DMI/sysfs/module state on the device; read tuxedo-drivers 4.25.0
  sources (`/usr/src/tuxedo-drivers-4.25.0`) and the installed TCC 3.1.2 bundle
  for `X5AR45xS` / `IBM15I10` specifics. Findings in `description.md`.
- Baseline on `main` (cfde1f9): build OK; `cargo test --workspace` → 1 failure,
  `gpu::nb02::tests::new_returns_none_when_sysfs_missing` (probes real `/sys`,
  which exists on this NB02 machine).
- `tux-daemon --dump-startup-diagnostics`: SKU unknown → "Unknown Uniwill Device";
  all WMI probes `false` despite GUIDs being present (instance suffix `-N`).
- Drafted `plan.md` (4 stages); awaiting user agreement.

## 2026-11-24 — Stage 1 complete

- User approved all 4 stages; Stage 1 implemented on `feat/ibm15i10-support`
  (11 commits). Details in `worklog-1.md`, review in `review-1.md`.
- `just check` green (771 passed). Daemon now resolves IBM15I10 exactly.
- `stage-2.md` drafted (live validation; needs user to run the daemon with sudo).
