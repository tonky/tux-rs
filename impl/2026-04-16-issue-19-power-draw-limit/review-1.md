# Stage 1 Review

Date: 2026-04-16

## Inputs
- Validation run: `just check` (fmt + clippy + full workspace tests)
- Independent review pass A (general-purpose sub-agent, Opus 4.6 — conformance focus)
- Independent review pass B (general-purpose sub-agent, Opus 4.6 — RAPL correctness focus)

Note: AGENTS.md calls for Opus 4.6 + Gemini 3.1 Pro reviewers. Gemini was
not available through the current harness; both passes used independent
Opus 4.6 sub-agents with disjoint review briefs to approximate diversity.

## Validation Result
- `just check`: PASS
- Workspace tests: 373 daemon lib tests (+2 vs Stage-1 baseline of 371 after
  replacing the skipped `rapl_set_pl1_surfaces_permission_error` with
  `rapl_probe_rejects_mismatched_constraint_name` and
  `rapl_probe_rejects_unparseable_bounds`).
- Zero clippy warnings under `-D warnings`.

## Findings Summary

### Addressed in Stage 1
1. **Medium (Reviewer B, M1)** — index→PL mapping (`constraint_0 = PL1`,
   `constraint_1 = PL2`) is convention, not a kernel API guarantee. Added
   `constraint_name_matches` helper in `probe_at` verifying
   `constraint_0_name == "long_term"` and `constraint_1_name == "short_term"`.
   On mismatch probe returns `None` with `debug!`, no silent mis-mapping.
2. **Medium (Reviewer B, M4)** — floor-division rationale moved from plan
   into a code comment on `read_bounds`.
3. **Low (Reviewer B, L6)** — added
   `rapl_probe_rejects_unparseable_bounds` test for the `read_u32` error
   path during bounds probing.
4. **Low (Reviewer A, L9)** — `rapl_probe_missing_base_returns_none` now
   uses a `tempfile::tempdir()` path that is dropped before probing,
   eliminating the residual hardcoded-path flake risk.
5. **Low (Reviewer A, L7)** — fixed stale `setup_rapl` docstring that
   claimed a return value.
6. **Low (Reviewer B, L5 observation)** — added a comment on `RAPL_BASE`
   noting MMIO-only zones are out of scope.

### Intentionally deferred (see `follow_up.toml`)
1. **Medium (Reviewer A, M1)** — hermetic permission-denied test. Root (the
   test harness and the live daemon) bypasses DAC `0o444`. A non-root-gated
   variant or a `NotFound`-based analog is a cheap future improvement; the
   `io::Result` return type already enforces error handling at every caller,
   and Stage 4 live-test will exercise firmware-lock for real.
2. **Medium (Reviewer B, M3)** — silent clamping + TUI `0 = Unset` interaction.
   The concern is that profile_apply must treat `None` as no-write, not pass
   `0` to `set_pl1`. Policy lives in Stage 2 (backend selection + profile
   wiring) where we can verify the contract holds.
3. **Low (Reviewer B, L6 observations)** — additional parse-error coverage
   on `get_pl{1,2}` and an "enabled == 0" zone test. These are Stage-4
   live-test territory.
4. **Low (Reviewer B, L9)** — `intel-rapl-mmio` server path. Laptops are
   the project scope; out of scope for this feature.
5. **Medium (Reviewer A, M2/M3)** — `probe_at(&Path)` vs symmetric
   `impl Into<PathBuf>`, and `#[cfg(test)]` asymmetry between `with_path` and
   `probe_at`. Cosmetic; no behavioural change. Can be tidied during Stage 2
   when `probe_at` becomes reachable from the factory.

## Notes
- No spec deviation beyond the acknowledged permission-denied test omission.
- No High-severity issues raised by either reviewer.
- Reviewer B independently confirmed the sysfs attribute names and
  `constraint_N` semantics are stable across 5.x and 6.x kernels.

## Outcome
Stage 1 is functionally complete, quality gates green, and all non-deferred
review items addressed. Remaining review items are logged in
`follow_up.toml` to be considered during Stage 2 or Stage 4.
