# Worklog — Runit Support

## 2026-04-13 — Feature start and planning

- Investigated issue #7 request for runit compatibility.
- Confirmed existing multi-init pattern in repo (systemd + dinit + init-system tests).
- Assessed feasibility of containerized smoke tests using daemon `--mock` mode and dbus integration.
- Created staged implementation plan covering service integration, container smoke harness, and CI wiring.
- Awaiting user confirmation to start Stage 1.

## 2026-04-13 — Stage 1 implementation complete

- Added runit service assets in `dist/tux-daemon.runit/`:
	- `run` script with dbus socket readiness wait + foreground `exec /usr/bin/tux-daemon`
	- `finish` script with concise supervised-exit logging
- Added `deploy-runit` recipe in `Justfile` with overrideable runit paths:
	- `SERVICE_DIR` default `/etc/sv/tux-daemon`
	- `ENABLE_DIR` default `/var/service/tux-daemon`
	- Artix-compatible override flow documented inline
- Added README runit install section with:
	- `just deploy-runit` usage
	- Artix path override example
	- manual install commands and basic troubleshooting commands
- Extended `tux-daemon/tests/init_system.rs`:
	- runit file existence/content checks
	- executable bit checks (unix)
	- cross-init binary path consistency now includes runit
	- cross-init dbus dependency semantics now include runit startup guard check
- Validation results:
	- `flox activate -- cargo fmt --all -- --check` ✅
	- `flox activate -- cargo clippy --workspace -- -D warnings` ✅
	- `flox activate -- dbus-run-session -- cargo test --workspace` ✅

## 2026-04-13 — Stage 2 implementation complete

- Added containerized runit smoke harness under `containers/runit-smoke/`:
	- `Dockerfile` with multi-stage daemon build and runtime dependencies (`dbus`, `runit`)
	- runit service scripts (`service/run`, `service/finish`) for `--mock` daemon execution
	- `smoke.sh` orchestration script with deterministic assertions
- Added `Justfile` targets:
	- `runit-smoke` (single smoke run)
	- `runit-smoke-repeat` (two consecutive runs for race detection)
- Debugged and fixed two smoke harness issues:
	- installed D-Bus policy file (`dist/com.tuxedocomputers.tccd.conf`) in container to allow bus-name ownership
	- adjusted `sv status` parsing to accept path-based status format
- Validation results:
	- `sh -n` checks for smoke scripts ✅
	- `flox activate -- just runit-smoke-repeat` ✅ (2/2 successful container runs)

## 2026-04-13 — Stage 3 implementation complete

- Added isolated CI container smoke job in `.github/workflows/ci.yml`:
	- `runit-smoke` job builds image and runs smoke test twice
	- existing check job kept focused on fmt/clippy/no-systemd/tests
- Renamed CI step label to reflect broader non-systemd compatibility scope.
- Updated `README.md`:
	- added `just runit-smoke-repeat` in development commands
	- documented container smoke validation boundaries vs real hardware validation
- Stage 3 artifacts added:
	- `impl/2026-04-13-runit-support/worklog-3.md`
	- `impl/2026-04-13-runit-support/review-3.md`


