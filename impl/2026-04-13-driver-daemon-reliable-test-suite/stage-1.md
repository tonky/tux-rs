# Stage 1: Contract Mapping and Hardware Fixture Capture

## Objective

Define the driver-daemon contract surfaces and create a reproducible Uniwill
hardware capture pipeline that produces canonical fixtures.

## Scope

- Enumerate concrete contract surfaces used by daemon (sysfs/ioctl-derived values,
  units, ranges, and normalization rules).
- Define fixture schema and metadata (kernel, driver revision, SKU, timestamp,
  capture source).
- Add manual capture workflow for target Uniwill hardware.
- Add fixture schema validation tests.

## Target Files

- tux-core/src/dbus_types.rs
- tux-core/src/mock/sysfs.rs
- tux-daemon/tests/common/mod.rs
- tux-daemon/tests/e2e.rs
- impl/2026-04-13-driver-daemon-reliable-test-suite/*
- Justfile

## Tasks

1. Create a contract matrix document with field names, source paths, expected units,
   ranges, and fallback semantics.
2. Define fixture file structure and versioning policy.
3. Add capture scripts/helpers that collect both raw values and normalized outputs.
4. Add fixture schema validation tests to prevent malformed fixtures entering repo.
5. Document fixture refresh policy and review process.

## Risks

- Captured fixtures can become stale across kernel/driver updates.
- Missing metadata can make fixture deltas hard to interpret.

## Verification

- Schema validation tests pass on all committed fixtures.
- Manual capture run completes on Uniwill hardware.
- Fixture provenance metadata is complete for each artifact.

## Exit Criteria

- Contract matrix and fixture schema are complete and reviewed.
- At least one canonical Uniwill fixture set is captured and validated.
