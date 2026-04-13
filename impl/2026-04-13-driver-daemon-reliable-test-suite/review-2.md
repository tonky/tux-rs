# Review 2

## Stage

Stage 2: Deterministic Contract Replay Tests

## Status

Completed

## Checklist

- [x] Stage goals implemented
- [x] Tests added and passing
- [x] Clippy and fmt clean
- [x] Replay coverage gaps documented

## Findings

- Added deterministic replay coverage in
	`tux-daemon/tests/contract_replay.rs` for fixture-normalized consistency and live
	D-Bus contract assertions.
- Fixture sample normalization corrected for fan index 1 temperature to match current
	mock backend behavior during replay.
- Full validation passed (`just clippy && just test`) with no failures.
- Temporary per-target dead code warning on `TestDaemon::start` was suppressed via
	local attribute in test helper to keep replay target builds clean.
- Hardening applied after independent reviews:
	- replay loader validates `schema_version`,
	- replay tests now iterate across all contract fixture TOML files,
	- replay device selection is fixture-metadata driven with required-fan-count guard,
	- D-Bus and TOML parsing failures now include fixture-specific context.
- Independent review consensus:
	- no critical behavioral regressions in the implemented Stage 2 baseline,
	- remaining risk is coverage breadth (single happy-path fixture only), not correctness
	  of current assertions.


## Follow-up

- Add at least one additional fixture variant for malformed/missing optional fields to
	widen replay edge-case coverage in Stage 3.
- Add boundary-value fixture variants (temp/duty/rpm extremes) and ensure replay tests
	automatically include them.
- Add explicit health transition/fault-matrix coverage in Stage 3 (`ok -> degraded -> failed`).

