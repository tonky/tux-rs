# Stage 3: Daemon Integration and Fault Matrix

## Objective

Validate end-to-end daemon behavior against contract fixtures and strengthen
reliability under failure conditions.

## Scope

- Add TestDaemon-driven integration cases for contract fixture replays.
- Add fault injection scenarios for transient and persistent failures.
- Verify retry, degrade, and fail behavior across monitored paths.

## Target Files

- tux-daemon/tests/common/mod.rs
- tux-daemon/tests/integration.rs
- tux-daemon/tests/e2e.rs
- tux-daemon/src/fan_engine.rs
- tux-daemon/src/platform/tuxedo_io.rs
- tux-daemon/src/charging/uniwill.rs

## Tasks

1. Add integration tests that compare daemon D-Bus outputs to fixture expectations.
2. Add error injection tests for read/write failures and malformed values.
3. Add failure-sequence tests for fan health and recovery behavior.
4. Add charging retry-path tests for transient I/O bursts.

## Risks

- Fault tests can become flaky if timing assumptions are too strict.
- Missing failure coverage can mask silent degradation paths.

## Verification

- Fault matrix tests are deterministic and pass repeatedly.
- Degrade/fail thresholds and reset behavior are explicitly asserted.

## Exit Criteria

- Daemon integration suite covers happy paths and failure paths for primary
	Uniwill contract surfaces.
- Reliability behavior is regression-protected by tests.
