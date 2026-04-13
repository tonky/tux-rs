# Stage 2: Deterministic Contract Replay Tests

## Objective

Turn captured driver data into deterministic backend and wire-contract tests that run
reliably without hardware.

## Scope

- Replay fixture values through mock sysfs/ioctl pathways.
- Verify daemon/backend normalization outputs against expected snapshots.
- Add wire-format roundtrip tests for integration payload types.

## Target Files

- tux-daemon/tests/contract_replay.rs
- tux-daemon/tests/fixture_schema.rs
- tux-daemon/tests/common/mod.rs
- tux-daemon/tests/fixtures/driver_contract/uniwill/sample-ibp16g8-v1.toml
- impl/2026-04-13-driver-daemon-reliable-test-suite/review-2.md
- impl/2026-04-13-driver-daemon-reliable-test-suite/worklog-2.md

## Tasks

1. Build fixture replay helpers for mocked platform trees and ioctl behavior.
2. Add backend tests for fan, charging, and related normalization paths.
3. Add shared-type roundtrip tests covering contract payload structs.
4. Add regression tests for edge values and missing optional attributes.

## Risks

- Overfitting tests to a single fixture can miss value-shape diversity.
- Mock behavior can diverge from real driver semantics.

## Verification

- New deterministic replay tests pass locally and in CI.
- Known edge cases are covered by explicit regression tests.

## Exit Criteria

- Backend and wire-contract replay coverage is in place for Uniwill-first paths.
- Replay tests are stable and hardware-independent.
