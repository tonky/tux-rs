# Stage 1 Review — Runit Service Integration

## Review Method

- Self-review of diff for `Justfile`, `README.md`, `dist/tux-daemon.runit/*`, and `tux-daemon/tests/init_system.rs`.
- Two independent `Explore` subagent review passes focused on correctness and portability risks.

## Findings

1. `deploy-runit` originally stopped service by name only, which could ignore non-default runit paths.
2. Runit startup had no explicit dbus readiness guard.
3. Tests did not verify runit script executable bits.

## Actions Taken

1. Changed stop command to `sv down "{{ENABLE_DIR}}"` in `Justfile`.
2. Added dbus socket readiness wait loop in `dist/tux-daemon.runit/run`.
3. Added unix executable-bit tests for runit scripts.
4. Added runit dbus guard assertion in cross-init dependency test.

## Residual Risks

1. Runit service directory conventions still vary by distro; recipe variables and docs reduce risk, but real Artix-runit validation remains required.
2. Container/runtime behavior is not covered in Stage 1; deferred to Stage 2 smoke harness.

## Verdict

Stage 1 is complete and validated. No open blockers for proceeding to Stage 2.
