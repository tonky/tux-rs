# Worklog 2

## Stage

Stage 2: Deterministic Contract Replay Tests

## Status

Completed

## Session Entries

### 2026-04-13

- Re-read `tux-daemon/tests/fixture_schema.rs` before implementation because of prior
	in-progress edits.
- Added deterministic replay test suite:
	- `tux-daemon/tests/contract_replay.rs` with fixture parsing and two tests:
		- raw D-Bus payload vs normalized fixture consistency checks,
		- replayed fixture values vs live daemon D-Bus outputs.
- Updated fixture values in
	`tux-daemon/tests/fixtures/driver_contract/uniwill/sample-ibp16g8-v1.toml` to
	align with deterministic mock semantics (single shared temperature source for both
	fan indices).
- Validation runs:
	- `just fmt-fix && just fmt`
	- `cargo test -p tux-daemon --test fixture_schema --test contract_replay`
	- `just clippy && just test`
- Result summary:
	- workspace clippy passed,
	- full workspace test suite passed,
	- no failing tests in new replay coverage.
- Stage 2 hardening pass (post-review):
	- replay tests now iterate over all fixture TOML files under
		`tests/fixtures/driver_contract/uniwill/` instead of one hardcoded fixture,
	- added fixture `schema_version` enforcement in replay loader,
	- switched replay device selection to fixture metadata SKU with guard rails for
		required fan count,
	- replaced bare unwraps on D-Bus and parse paths with fixture-scoped panic context.
- Removed replay-target dead code warning noise by allowing `TestDaemon::start` in shared
	test helper when that method is unused by a specific test binary.
- Re-ran validation after hardening:
	- `cargo test -p tux-daemon --test fixture_schema --test contract_replay` (pass),
	- `just clippy && just test` (pass).
- Ran two independent subagent reviews for Stage 2 quality/risk checks.
