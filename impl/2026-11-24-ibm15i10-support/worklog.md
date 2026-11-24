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

## 2026-11-24 — Stage 2 complete

- Live validation on IBM15I10 with the user running the daemon via sudo.
  Six real bugs found and fixed (charging without priority, keyboard empty
  mode, turbo reset, GetGpuInfo interface, Power tab cTGP wiring, phantom
  priority field) plus cTGP bounds mirroring TCC. 26 commits; details in
  `worklog-2.md`, review in `review-2.md`.
- `just check` green (799), `just live-test` PASSED on hardware.
- Next: Stage 3 (Uniwill EC TDP via tuxedo_io) — awaiting user confirmation.

## 2026-11-24 — Stage 3 complete

- Uniwill EC TDP via tuxedo_io on IBM15I10 (PL1/PL2 10–90 W), verified live
  including measured enforcement under load. Review in `review-3.md`, details
  in `worklog-3.md`.
- Finding for Stage 4: EPP `performance` roughly doubles sustained package
  power (28 → 50 W) on this machine — matches TCC's quirk.

## 2026-11-24 — Stage 4 complete

- EPP `performance` on AC only for IBM15I10 (battery keeps profile EPP;
  fallback `balance_power`). Verified live across unplug/replug. Review in
  `review-4.md`. All four stages done.

## 2026-11-24 — Ad hoc: suspend fix + battery profile

- `3cc49d4` fix(sleep): on resume, re-apply the active profile's EC-held power
  limits (TDP, cTGP) for the current power source (follow-up closed).
- User config (via D-Bus, not in the repo): created **Max Battery**
  (`max-energy-save-copy`: EPP power, no turbo, PL1 15 W / PL2 20 W, keyboard
  20 %) → battery, and **Office (full power)** (`office-copy`: Office + PL1/PL2
  90 W) → AC. The AC profile needs explicit 90 W because unset TDP means
  "leave as is", so battery limits would otherwise persist on AC.

## 2026-11-24 — Stage 5: GPU telemetry + governor fix

- Intel iGPU via i915 sysfs/RAPL uncore; NVIDIA via NVML, never querying a
  suspended dGPU; TUI polls every 2 s. Live-verified both GPUs.
- Found live: NVML polling keeps an otherwise idle dGPU awake (restarts its
  ~20 s idle timer); guarded by querying only with real users, ignoring
  nvidia-powerd / nvidia-persistenced (comm truncated to 15 bytes!) and
  holders seen while suspended. Residual case accepted by the user.
- `schedutil` → `powersave` mapping on pstate drivers (no more startup WARN).
- Profile keyboard control setting; turned off for this user (persisted).
