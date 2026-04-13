# Stage 1 Review

Status: completed

## What was reviewed
- Detection logic updates in tux-core:
	- NB02 fallback behavior,
	- composite SKU lookup,
	- curated TCC-derived SKU platform hints.
- Startup diagnostics behavior in tux-daemon.

## Review passes
- Review pass A: flagged need for explicit typo-SKU test coverage.
- Review pass B: confirmed diagnostics completeness and highlighted descriptor-tuning follow-up.

## Actions taken
- Added missing regression test for TCC typo variant SKU:
	- `IIBP14A10MK1 / IBP15A10MK1` -> Uniwill fallback.
- Kept TCC integration intentionally curated to platform hints (not full descriptor import) to avoid unsafe capability assumptions.

## Validation
- `cargo test -p tux-core dmi -- --nocapture` passing.
- `cargo test -p tux-daemon --no-run` passing.
- `cargo fmt --all -- --check` passing.

## Remaining follow-up
- Evaluate explicit device-table descriptors for Gen9/Gen10 IBP AMD once verified hardware capability details are available.
