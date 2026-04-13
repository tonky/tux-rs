# Stage 2 Review — Containerized Runit Smoke Tests

## Review Summary

Stage 2 objectives were met: a deterministic, hardware-independent runit smoke harness now validates daemon startup, dbus registration, and supervised restart behavior.

## Findings and Resolutions

1. **D-Bus AccessDenied on service registration**
   - Cause: runtime image lacked `com.tuxedocomputers.tccd` system-bus policy.
   - Resolution: install policy in `/etc/dbus-1/system.d/` from `dist/`.

2. **False negative from status parser**
   - Cause: `sv status` emits path-based service identifiers; parser expected name-based output.
   - Resolution: use generic `^run:` state match + PID extraction.

3. **Insufficient failure diagnostics**
   - Cause: daemon logs were not surfaced on failures.
   - Resolution: run script now writes to `/tmp/tux-daemon-smoke.log`; smoke debug dump prints it.

## Validation Evidence

- `flox activate -- just runit-smoke-repeat` succeeded with two consecutive passes.
- Each pass asserted:
  1. runit started daemon process
  2. daemon owned D-Bus name `com.tuxedocomputers.tccd`
  3. daemon restart occurred after `SIGTERM`

## Residual Risks

1. Stage 2 validates service supervision/integration semantics only, not real hardware behavior.
2. Docker-based smoke coverage is good for CI parity, but real Artix-runit machine validation remains required for final issue confidence.

## Verdict

Stage 2 complete and ready for Stage 3 (CI integration + validation docs).
