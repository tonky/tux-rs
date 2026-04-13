# Worklog 4

## Stage

Stage 4: Workflow Hardening and Release Validation

## Status

Completed

## Session Entries

### 2026-04-13

- Added Stage 4 workflow commands in `Justfile`:
	- `fixture-contract-test` for schema + deterministic replay checks,
	- `reliability-test` for deterministic reliability suite under `dbus-run-session`.
- Updated `just ci` to include an explicit deterministic reliability gate before full
	workspace tests.
- Updated CI workflow (`.github/workflows/ci.yml`) with a dedicated reliability-suite
	test step.
- Added reliability-suite and fixture refresh documentation to `README.md`.
- Expanded fixture governance docs in
	`tux-daemon/tests/fixtures/driver_contract/uniwill/README.md` with capture/compare
	checklist and drift approval rules.
- Hardened capture helper (`tools/capture-uniwill-contract-fixture.sh`) with:
	- warning counting and warning log output,
	- optional strict mode (`CAPTURE_STRICT=1`) that fails capture when warnings exist.
- Verification executed:
	- `flox activate -- just fmt-fix`
	- `flox activate -- just fmt`
	- `flox activate -- just clippy`
	- `flox activate -- just reliability-test`
	- `flox activate -- just ci`
	- manual capture + compare against canonical fixture.
- Manual capture/compare result on this host:
	- capture completed but reported missing live Uniwill surfaces,
	- diff showed expected non-promotable drift (empty raw values),
	- strict capture mode correctly failed with non-zero exit on warnings.
- Ran two independent review passes and applied low-risk hardening fixes.
