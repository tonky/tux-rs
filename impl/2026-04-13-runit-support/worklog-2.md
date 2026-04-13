# Stage 2 Worklog — Containerized Runit Smoke Tests

## Scope Completed

- Added `containers/runit-smoke/Dockerfile`:
  - builder stage compiles `tux-daemon` with `--no-default-features --features tcc-compat`
  - runtime stage installs `dbus` + `runit`
  - installs daemon binary and smoke scripts
- Added container-local runit service scripts:
  - `containers/runit-smoke/service/run`
  - `containers/runit-smoke/service/finish`
- Added smoke orchestrator script:
  - `containers/runit-smoke/smoke.sh`
  - checks startup, dbus ownership, crash/restart behavior
- Added local execution targets in `Justfile`:
  - `runit-smoke`
  - `runit-smoke-repeat`

## Issues Found During Validation

1. Initial smoke failure: service did not stay up.
2. Root cause A: missing D-Bus policy for `com.tuxedocomputers.tccd` in runtime image.
3. Root cause B: `sv status` output pattern included full service path, while parser expected service name only.

## Fixes Applied

1. Copied `dist/com.tuxedocomputers.tccd.conf` into `/etc/dbus-1/system.d/` in image.
2. Broadened status matching from `^run: tux-daemon:` to `^run:` with PID extraction.
3. Added daemon log capture (`/tmp/tux-daemon-smoke.log`) and debug dump output for failures.

## Verification

- `sh -n containers/runit-smoke/service/run containers/runit-smoke/service/finish containers/runit-smoke/smoke.sh` passed.
- `flox activate -- just runit-smoke-repeat` passed:
  - Run 1: startup + dbus + restart assertions passed
  - Run 2: startup + dbus + restart assertions passed
