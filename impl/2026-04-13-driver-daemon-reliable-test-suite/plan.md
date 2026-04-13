# Plan

## Summary

Implement a reliable driver-daemon integration test suite first, then treat TUI work
as bonus stages:

1. Contract boundaries and real-data fixture capture.
2. Deterministic backend and wire-contract replay tests.
3. Daemon-level integration and fault matrix coverage.
4. Validation, CI workflow, and release-readiness checks.
5. Bonus Stage A: actionable Uniwill profile options in TUI.
6. Bonus Stage B: additional Uniwill runtime TUI controls.

## Stage Breakdown

## Stage 1: Contract Mapping and Fixture Capture

- Define contract surfaces for Uniwill paths used by daemon.
- Establish fixture schema and provenance metadata.
- Add manual hardware capture workflow and validation.

## Stage 2: Deterministic Contract Replay Tests

- Replay captured fixtures into mock sysfs/ioctl backends.
- Assert normalization behavior and wire-type roundtrips.
- Cover edge parsing and path discovery behavior.

## Stage 3: Daemon Integration + Fault Matrix

- Add TestDaemon-level integration checks for contract replay outputs.
- Add resilience tests for transient I/O failures and malformed data.
- Verify degradation and retry behavior over failure sequences.

## Stage 4: Validation and Workflow Hardening

- Add or refine just targets for contract suites and capture checks.
- Integrate deterministic suites into CI-safe flows.
- Document manual hardware refresh workflow and drift handling.

## Stage 5 (Bonus): TUI Actionable Profile Coverage

- Add actionable Uniwill profile options in TUI profile editor.
- Keep compatibility-safe behavior and capability gating.

## Stage 6 (Bonus): TUI Runtime Controls

- Add fn-lock, keyboard detail, and multi-zone keyboard UX flows.
- Add daemon API extensions only if required for these bonus features.

## Dependencies and Order

- Stages 1 through 4 are primary and mandatory.
- Stages 5 and 6 are optional bonus stages and should not block core delivery.

## Execution Rules

- Ask for explicit confirmation before starting each stage.
- Keep worklogs current at feature and per-stage levels.
- Add regression tests for any bug fixed during implementation.

