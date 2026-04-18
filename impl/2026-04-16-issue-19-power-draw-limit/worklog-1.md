# Stage 1 Worklog

## 2026-04-16

### Implementation
- Added `RaplTdp` struct + `TdpBackend` impl in
  `tux-daemon/src/cpu/tdp.rs` alongside the existing `EcTdp`.
- New constants for `/sys/class/powercap/intel-rapl:0` (`name`,
  `constraint_{0,1}_{name,power_limit_uw,max_power_uw}`), a 1 W floor, and
  a µW↔W conversion helper (`uw_to_w`).
- `probe()` / `probe_at(&Path)` split lets tests point at a hermetic
  tempdir; production path uses the real `RAPL_BASE` constant.
- Probe rejects domains whose `name` != `"package-0"` and whose
  `constraint_{0,1}_name` don't match `"long_term"` / `"short_term"`.
- `set_pl{1,2}` clamp to bounds then `saturating_mul(1_000_000)` to µW.
  `get_pl{1,2}` divide by 1_000_000. All via the existing `SysfsReader`
  helper (`read_u32` / `write_u32`).
- Module docs updated to describe both backends.

### Decisions
- Dropped the planned `rapl_set_pl1_surfaces_permission_error` test because
  root (CI harness and production daemon) bypasses DAC `0o444`. Inline NOTE
  explains the deferral; Stage 4 live-test is the right venue for the real
  firmware-lock path.
- Added `constraint_N_name` validation as a direct response to Reviewer B,
  M1: the index→PL mapping is a kernel convention, not API-guaranteed, and
  silently mis-mapping PL1/PL2 would be worse than refusing to probe.
- Kept silent clamping in `set_pl{1,2}` for consistency with existing
  `EcTdp::set_pl{1,2}`. Documented caveats in review and follow_up.

### Quality gates
- `just check`: green. 373 daemon lib tests (+2 vs baseline), zero clippy
  warnings under `-D warnings`, fmt clean.

### Reviews
- Two independent Opus 4.6 sub-agent reviews ran in parallel with disjoint
  briefs (conformance vs RAPL-correctness). Summary in `review-1.md`. No
  High-severity findings; five review items folded in before closing Stage 1,
  five deferred to `follow_up.toml`.

### Stage 1 exit state
- Single-file change in `tux-daemon/src/cpu/tdp.rs`.
- No changes to device table, D-Bus, TUI, or profile application.
- Existing `EcTdp` behaviour unaffected.
- Stage 1 implementation ready for Stage 2 to consume (`RaplTdp::probe()`).
