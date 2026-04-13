# Plan: Runit Support + Container Smoke Tests

## Summary

Deliver runit support in three stages:

1. Add runit service integration and local deploy/docs flow.
2. Add deterministic container smoke tests for runit supervision behavior.
3. Wire smoke tests into CI and document validation and drift expectations.

## Stage Breakdown

## Stage 1: Runit Service Integration

- Add `dist/tux-daemon.runit/run` script (foreground daemon process).
- Add `dist/tux-daemon.runit/finish` script for restart semantics/logging-friendly exit handling.
- Add `just deploy-runit` recipe with clear assumptions about runit paths.
- Add README section for runit install/enable/start/manual layout examples.
- Extend `tux-daemon/tests/init_system.rs` to validate runit service assets and cross-init binary path consistency.

## Stage 2: Container Smoke Test Harness

- Add a minimal container image for runit smoke tests (dbus + runit + built daemon binary).
- Add smoke script to verify:
  - runsv starts daemon
  - daemon registers on dbus in `--mock` mode
  - forced daemon crash triggers restart
- Keep tests deterministic and free of host hardware dependencies.

## Stage 3: CI Integration and Validation Docs

- Add CI job for runit smoke test (isolated from unit/integration test job).
- Add local `just` target(s) for smoke test execution.
- Document known limits (container smoke != hardware verification).
- Record final validation evidence in worklog + stage review.

## Dependencies and Order

- Stage 1 is required before stage 2.
- Stage 2 is required before stage 3.

## Execution Rules

- Ask for explicit confirmation before starting each stage.
- Keep feature worklog updated for every implementation session.
- Add regression tests for any ad-hoc bugfix discovered during implementation.
