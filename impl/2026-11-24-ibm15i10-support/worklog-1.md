# Worklog — Stage 1 (detection + descriptor)

## 2026-11-24

### Done
- `3fc549a` fix(dmi): WMI probe matches `<GUID>` and `<GUID>-<n>` (case-insensitive).
  Root cause of all WMI probes reading `false` on Linux 7.3.
- `65bada3` test(gpu): `Nb02GpuPower::probe(path)` + tempdir tests; the old test
  probed real `/sys` and failed on this NB02 + NVIDIA machine.
- `76be014` feat(device): `IBM15I10` row (Uniwill, 2 fans, ITE 8291, cpu+gpu temp,
  EcProfilePriority, Nb02Nvidia, TDP off).
- `69feff6` fix(hid): identity colour scaling for `("IBM15I10", 0x600b)`.
- `495b6fe` docs: hardware_support / feature matrix / README.
- Review follow-ups: `bc6524d`, `4c4c540`, `53aa183`, `684cefc`, `6039eb5` (see `review-1.md`).

### Decisions
- **Detection order** (new): NB05 vendor → NB02 vendor → TCC SKU hint → NB04 WMI →
  Uniwill WMI → Tuxi sysfs → Clevo WMI. The Clevo event GUID is also exposed by
  Uniwill firmware, so it is the weakest signal now that WMI probing works.
- **SKU count stays 40** (spec said 40→41): main actually had 39 named SKUs; the
  docs were off by one. Verified by counting `DEVICE_TABLE`.
- **TDP left off** (`TdpSource::None`): RAPL publishes PL1 max 28 W while the EC
  programs 90 W, so RAPL would clamp. Proper EC backend is Stage 3.
- **Reviewers**: "Opus 4.6 high" / "Gemini 3.1 Pro" unavailable → Opus (high) + Sonnet.

### Verification
- `just check` → exit 0, 771 tests passed, 0 failed (log: `tmp/stage1-check.log`).
- `tux-daemon --dump-startup-diagnostics`: `dmi.product_sku=IBM15I10`,
  `detect.platform_guess=Uniwill`, `probe.wmi.uniwill_guid_2=true`.
- `tux-daemon` (non-root) logs `detected: TUXEDO InfinityBook Max 15 Gen10 Intel
  (platform=Uniwill, exact=true)` before failing on root-only resources (expected).

### Follow-ups touched
- Clevo-GUID note → done. Added: capture WMI GUID sets on real Clevo/NB04 hardware.
