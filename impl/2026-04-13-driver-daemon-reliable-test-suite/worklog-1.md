# Worklog 1

## Stage

Stage 1: Contract Mapping and Hardware Fixture Capture

## Status

In Progress

## Session Entries

### 2026-04-13

- Stage plan created.
- Stage objective refocused to driver-daemon contract capture and fixture schema.
- Awaiting explicit user confirmation before implementation.

### 2026-04-13 (implementation)

- Added contract matrix document for Uniwill raw and normalized surfaces.
- Added fixture schema documentation and a baseline sample fixture.
- Added capture helper script at tools/capture-uniwill-contract-fixture.sh.
- Added fixture schema integration test at tux-daemon/tests/fixture_schema.rs.
- Added just targets: fixture-validate and fixture-capture-uniwill.
- Ran two independent review passes over Stage 1 artifacts and applied hardening fixes:
	- robust TOML escaping in capture helper,
	- warnings for missing D-Bus tooling and probable non-Uniwill capture context,
	- stronger schema constraints for required non-empty values,
	- raw/normalized consistency checks for duty scaling,
	- stricter D-Bus payload parsing requirements.
- Validation results:
	- just fixture-validate: pass.
	- just fmt: pass (after fmt-fix on new test file).
	- just clippy: pass.
	- just test: pass.
- Environment note: flox activate -- <cmd> could not be used in this repo path because no flox environment is initialized yet.
- Remaining Stage 1 item: execute real manual hardware capture on target Uniwill host and review generated fixture before promoting it to canonical hardware-captured baseline.
- Live capture attempt executed via just fixture-capture-uniwill in this session:
	- script ran successfully,
	- current host context is not active Uniwill driver path and no daemon payloads were available,
	- output was written to tmp/ for inspection only (not promoted as canonical fixture).
