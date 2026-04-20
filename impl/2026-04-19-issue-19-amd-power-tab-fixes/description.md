# Feature Description: AMD APU detection + hide unsupported Power-tab elements

Follow-up to [issue #19](https://github.com/tonky/tux-rs/issues/19) /
[PR #20](https://github.com/tonky/tux-rs/pull/20). Triggered by the
[2026-04-19 comment from @Ciugam](https://github.com/tonky/tux-rs/issues/19#issuecomment-4275594280)
reporting on an `IBP14G9` AMD machine (Ryzen 7 8845HS, AMD APU only, no NVIDIA dGPU):

- The Power tab's "iGPU" panel reports "No iGPU detected".
- The Power tab still shows a "TGP Offset" slider that does nothing on this hardware.

Vendor reality (cross-checked against vendored sources):

- `vendor/tuxedo-control-center/src/service-app/classes/TuxedoControlCenterDaemon.ts:542`
  lists `IBP14A09MK1 / IBP15A09MK1` (the IBP14G9 AMD SKU string) as an
  **unsupported** cTGP device.
- `vendor/tuxedo-drivers/src/tuxedo_io/tuxedo_io.c:166-262` has no AMD Gen9
  entry in `uw_id_tdp()` — falls through to `tdp_min_defs = NULL`. Vendor
  does not sanction TDP control on this platform either.
- AMD APU power capping (RyzenAdj / `ryzen_smu` / MSR) is **explicitly out of
  scope** for this work; tracked separately as a future feature.

## Scope (in)

1. **iGPU detection / GPU classification.** `tux-daemon/src/gpu/hwmon.rs` maps
   the `amdgpu` driver to `GpuType::Discrete` unconditionally. On an AMD APU
   (no separate dGPU), the integrated Radeon also exposes itself as `amdgpu`
   and gets classified as discrete, so the daemon publishes `dgpu_name` /
   `dgpu_temp` / `dgpu_power` for what the user perceives as their iGPU, and
   `igpu_name` stays empty. Need a real classifier that distinguishes
   integrated vs discrete `amdgpu` devices.

2. **Capability gate for `gpu_control`.** `tux-daemon/src/dbus/settings.rs:139`
   hard-codes `gpu_control: false`. It should reflect whether an NB02 cTGP
   backend was actually constructed (mirroring the `tdp_control` derivation
   added in PR #20). The flag is in the public `CapabilitiesResponse`
   contract (`tux-core/src/dbus_types.rs:101`) but is not currently wired.

3. **TUI: hide / disable unsupported Power-tab elements based on
   capabilities.**
   - The "TGP Offset" form field should not be rendered (or should be
     disabled with an explanatory message) when `gpu_control = false`.
     Today `power.form_tab.supported` is hardcoded to `true` in
     `tux-tui/src/model.rs:818` and never set from capabilities.
   - The dGPU info panel should collapse to "No dGPU detected" only when
     there is genuinely no discrete GPU (will fall out naturally once #1 is
     correct).
   - When `gpu_control = false` AND `tdp_control = false` AND there are no
     fields to render, the whole Power-settings form should follow the same
     "Power controls not available on this device" path that already exists
     at `tux-tui/src/views/power.rs:14-20`.

4. **Dashboard package-power-draw line.** PR #20 added a status-line value
   sourced from `/sys/class/powercap/intel-rapl:0/energy_uj`
   (`tux-daemon/src/dbus/system.rs:192`). On AMD this path may not exist
   (or returns a counter that doesn't represent package power), so the line
   silently shows 0 W. Either:
   - probe `intel-rapl:0` and only publish `package_power_w` when present
     (preferred — keeps the surface honest), and hide / omit the field on
     the TUI side when missing; or
   - add an `amd_energy` hwmon fallback. Decision deferred to plan stage —
     bias toward "hide, don't fake".

## Scope (explicitly out)

- AMD CPU TDP control via RyzenAdj / `ryzen_smu` / MSR. Tracked as a separate
  follow-up; not implemented in this branch.
- AMD/Intel iGPU power-draw control. Same.
- Adding new `IBP14A09MK1` / `IBP15A09MK1` device-table rows beyond what
  `platform_hint_from_tcc_sku_map` already provides. The fixes here should
  work generically based on runtime probing, not new SKU rows.
- The latent `i8`-vs-`u8` sign mismatch on `tgp_offset` (TUI exposes
  `-15..=15`, sysfs takes `u8 0..=255`) — separate bug, file as follow-up.

## Acceptance signals

- On an AMD APU machine with no dGPU: Power tab shows the AMD iGPU in the
  iGPU panel; dGPU panel shows "No dGPU detected"; no TGP Offset slider; if
  there is also no TDP backend, the whole settings form is replaced by the
  "not available" notice.
- On an Intel + NVIDIA dGPU machine (e.g. NB02 Stellaris): unchanged
  behaviour — TGP Offset still rendered, dGPU info still rendered, status
  line still shows package power.
- On the dev machine (IBP1XI08MK1, Intel, no dGPU): TGP Offset disappears,
  TDP fields remain, status line still shows package power.
- Capability tests in `tux-daemon/tests/integration.rs` extended for
  `gpu_control` (currently only `tdp_control` is asserted there).
