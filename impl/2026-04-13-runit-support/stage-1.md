# Stage 1 — Runit Service Integration

## Objective

Ship first-class runit service support comparable to existing systemd and dinit support.

## Scope

- Add runit service scripts in `dist/`.
- Add local deploy flow via `Justfile`.
- Add README install/enable/start instructions.
- Add init-system tests for runit assets and cross-init consistency.

## Target Files

- dist/tux-daemon.runit/run
- dist/tux-daemon.runit/finish
- Justfile
- README.md
- tux-daemon/tests/init_system.rs
- impl/2026-04-13-runit-support/worklog-1.md
- impl/2026-04-13-runit-support/review-1.md

## Tasks

1. Create runit `run` script that execs `/usr/bin/tux-daemon` in foreground.
2. Add `finish` script with conservative semantics suitable for supervision loop diagnostics.
3. Add `deploy-runit` recipe with documented assumptions and safe stop/start behavior.
4. Add README runit section with both `just` and manual commands.
5. Extend `init_system.rs` to validate runit scripts exist, are non-empty, and reference daemon binary path.
6. Extend cross-init consistency checks to include runit command path alignment.

## Risks

- Runit service directory paths vary by distro (`/etc/sv` vs `/etc/runit/sv`).
- D-Bus readiness ordering is less declarative than systemd/dinit.

## Verification

- cargo fmt --all -- --check
- cargo clippy --workspace -- -D warnings
- dbus-run-session -- cargo test --workspace
- focused init-system test run for fast iteration

## Exit Criteria

- Runit artifacts are present and validated by tests.
- Local deploy instructions are clear and reproducible.
