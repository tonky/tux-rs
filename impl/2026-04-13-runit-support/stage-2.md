# Stage 2 — Containerized Runit Smoke Tests

## Objective

Provide deterministic, hardware-independent smoke coverage for runit supervision behavior.

## Scope

- Build/run a dedicated smoke container with runit + dbus + tux-daemon.
- Verify daemon startup and restart behavior via runit supervision.
- Validate daemon availability on dbus in `--mock` mode.

## Target Files

- containers/runit-smoke/Dockerfile
- containers/runit-smoke/service/run
- containers/runit-smoke/service/finish
- containers/runit-smoke/smoke.sh
- Justfile
- impl/2026-04-13-runit-support/worklog-2.md
- impl/2026-04-13-runit-support/review-2.md

## Tasks

1. Create smoke image with runit and dbus runtime dependencies.
2. Copy daemon binary + service scripts into canonical runit service path in image.
3. Start runit supervision and assert daemon process is started.
4. Probe dbus service presence in mock mode.
5. Kill daemon process and assert runit restarts it.
6. Return non-zero on any failed assertion.

## Risks

- Running pid 1 + supervision in CI containers can be brittle if signal handling is incorrect.
- Package names can differ between base images.

## Verification

- local smoke run via dedicated `just` target
- repeated smoke run (at least 2 consecutive passes) to detect racey startup assumptions

## Exit Criteria

- Smoke harness runs green locally with deterministic pass/fail behavior.
- Restart semantics are asserted and captured in logs.
