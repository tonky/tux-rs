# Stage 1 Worklog — Runit Service Integration

## Scope Completed

- Added runit service scripts:
  - `dist/tux-daemon.runit/run`
  - `dist/tux-daemon.runit/finish`
- Added `deploy-runit` in `Justfile` with layout override variables.
- Added runit install/docs section in `README.md`.
- Extended init-system tests to include runit content, permission, and cross-init checks.

## Notable Decisions

- Kept daemon startup path consistent across init systems (`/usr/bin/tux-daemon`).
- Added a short dbus socket wait loop in runit `run` script to reduce boot ordering races.
- Used parameterized runit paths in `Justfile` to avoid baking in a single distro layout.

## Review Feedback Applied

- Aligned `sv down` to use the configured `ENABLE_DIR` path for non-default runit layouts.
- Added runit dbus startup guard assertion in test coverage.
- Added executable-bit checks for runit scripts in unix test environments.

## Verification

- `flox activate -- cargo fmt --all -- --check` passed.
- `flox activate -- cargo clippy --workspace -- -D warnings` passed.
- `flox activate -- dbus-run-session -- cargo test --workspace` passed.
  - `tests/init_system.rs`: 14 passed, 0 failed.
