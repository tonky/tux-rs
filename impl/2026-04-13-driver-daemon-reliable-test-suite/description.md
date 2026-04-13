# Driver-Daemon Reliable Test Suite (Uniwill First)

## Problem

We are seeing recurring driver-to-daemon integration regressions across hardware paths.
Current tests cover many units and some integration, but they do not yet provide a
contract-driven reliability safety net based on real driver-exposed values.

## Goal

Build a reliable integration test suite that is grounded in actual driver-exposed
data and can be run deterministically in CI, with a manual hardware capture loop for
truth refresh on Uniwill.

## Primary Scope

- Define and version driver-daemon data contracts for Uniwill-first paths.
- Capture real hardware data into canonical fixtures with provenance metadata.
- Add deterministic replay tests at backend and daemon D-Bus layers.
- Add fault-matrix and resilience tests (transient I/O errors, missing attrs,
	malformed values, unsupported readings, partial write failures).
- Add quality gates and workflow commands for repeatable local/CI validation.

## Bonus Scope

- Extra TUI features and UX polish become bonus stages only, after primary
	reliability stages are complete and verified.

## Out of Scope

- Hardware-in-the-loop CI as a mandatory gate.
- New hardware-control daemon subsystems (lightbar, mini-LED, USB power-share).

## Acceptance Criteria

- A deterministic contract suite guards driver-daemon behavior in PR checks.
- Uniwill hardware capture workflow exists and can refresh fixtures safely.
- Fault matrix tests verify retry/degrade/fail behavior across key integration paths.
- CI and local commands are documented and reproducible with flox-prefixed just tasks.
- Bonus TUI stages remain optional and do not block core reliability delivery.
