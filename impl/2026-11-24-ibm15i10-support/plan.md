# High-Level Plan — IBM15I10 support

Branch: `feat/ibm15i10-support`. Small, focused commits per fix/feature.
Each stage is gated on user confirmation (AGENTS.md).

## Stage 1 — Detection + device descriptor (no root needed)

1. **fix(dmi)**: WMI GUID probe matches instance-suffixed device dirs
   (`<GUID>` or `<GUID>-<n>`, case-insensitive). Add `wmi_guid_exists` tests
   against a tempdir tree + regression test for the suffixed form.
2. **test(gpu)**: make `nb02::new_returns_none_when_sysfs_missing` hermetic
   (probe a tempdir path instead of real `/sys`).
3. **feat(device)**: add `IBM15I10` row:
   - `Platform::Uniwill`, `PlatformRegisters::Uniwill`
   - fans: 2, `Direct`, `pwm_scale: 200`
   - keyboard: `IteHid(Ite8291)` (USB `048D:600B`)
   - sensors: cpu + gpu temp, `fan_rpm: [true, true]`
   - charging: `EcProfilePriority` (backend already tolerates missing priority)
   - gpu_power: `Nb02Nvidia`
   - tdp: `None` / `TdpSource::None` for now (Stage 3 flips it)
   - DMI detection test: exact SKU match for `IBM15I10` with `NB02` / `X5AR45xS`.
4. **fix(hid)**: identity colour scaling for `IBM15I10` + PID `0x600b`
   (matches `ite_8291.c`), with a unit test.
5. **docs**: `docs/hardware_support.md` + README mention.

Exit: `just check` green; `tux-daemon --dump-startup-diagnostics` shows
`uniwill_guid_2=true` and the daemon resolves `IBM15I10` exactly.

## Stage 2 — Live validation on the device (needs sudo)

1. Stop `tccd` (it fights over fans/keyboard/EPP), run `just daemon-debug`,
   check each capability via TUI + `busctl`: fan curve + auto, CPU/GPU temps,
   keyboard brightness/colour, charging profile, cTGP offset.
2. Capture a hardware fixture via `just fixture-capture-uniwill` →
   `tux-daemon/tests/fixtures/driver_contract/uniwill/ibm15i10-hardware-v1.toml`
   and wire it into `fixture_schema` / `contract_replay` tests.
3. Fix anything that misbehaves (each as its own `fix:` commit + regression test).

## Stage 3 — Uniwill EC TDP backend via `tuxedo_io`

Why not RAPL: firmware publishes `constraint_0_max_power_uw = 28 W` while the EC
programs 90 W, so `RaplTdp` would cap PL1 at 28 W. The vendor path is EC via
`tuxedo_io`.

1. Add ioctl constants `R_UW_TDP{0,1,2}`, `R_UW_TDP{0,1,2}_{MIN,MAX}`,
   `W_UW_TDP{0,1,2}`.
2. New `UniwillEcTdp` implementing `TdpBackend` over the `TuxedoIo` trait;
   bounds read from the driver (`*_MIN/*_MAX`), falling back to `descriptor.tdp`.
   Clamp + validate like the other backends. Mock-based unit tests.
3. New `TdpSource::UniwillEc`, wired in `build_backend`; opt in `IBM15I10`
   with bounds PL1 10–90, PL2 10–90, PL4 10–230.
4. Document the side effect: the driver forces EC "overboost" profile on TDP
   write for this board (`uniwill_custom_profile_mode_needed`).
5. Live-verify on the device (sudo): set PL1/PL2, read back via
   `R_UW_TDP*` and RAPL `constraint_*_power_limit_uw`.

PL4 stays read-only/out of scope unless the trait grows PL4 (follow-up).

## Stage 4 (optional) — EPP performance quirk

TCC forces `EPP=performance` on IBM15I10 to unlock the full frequency range.
Add a narrow SKU-keyed CPU quirk in the daemon (no new descriptor field), with a
unit test, and verify the max frequency behaviour on-device.

## Per-stage wrap-up (AGENTS.md)

`just check` → 2 parallel review sub-agents → `just check` → `worklog-N.md`
summary → walk `follow_up.toml`.
