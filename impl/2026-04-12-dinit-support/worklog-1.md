# Stage 1+2 Worklog — Dinit Support

## Session 2026-04-12

### Stage 1 — Service file, Justfile, README
- Created `dist/tux-daemon.dinit` — Dinit service file with process type, dbus dependency, restart policy
- Added `deploy-dinit` recipe to justfile (mirrors `deploy-daemon` for systemd)
- Added Dinit installation section to README.md under "2. Daemon"

### Stage 2 — Optional sd-notify
- Added `systemd` cargo feature (default on) to `tux-daemon/Cargo.toml`
- Made `sd-notify` dependency optional, gated by the `systemd` feature
- Added `#[cfg(feature = "systemd")]` to all 3 sd-notify call sites in `main.rs`:
  - Ready notification
  - Watchdog ping loop
  - Stopping notification
- Fixed deploy-dinit recipe: use `--no-default-features --features tcc-compat` to only drop systemd, not tcc-compat

### Verification
- `cargo check -p tux-daemon` (default features) — passes
- `cargo check -p tux-daemon --no-default-features --features tcc-compat` — passes
- `cargo clippy` — clean for both feature sets
- `cargo fmt --check` — passes
- Tests can't run on macOS (inotify linker error, pre-existing)

### Reviews
- Opus review: PASS on all stages 1+2. Suggested logfile directive (skipped — daemon uses tracing), waits-for vs depends-on (tracked in follow_up.toml).
- Sonnet review: PASS on all stages 1+2. Noted restart semantics difference (intentional — Dinit lacks on-failure equivalent), deploy-dinit enable gap (README manual section has it, recipe intentionally doesn't — mirrors deploy-daemon).

### Stage 3 — CI
- Added `cargo check` and `cargo clippy` for `--no-default-features --features tcc-compat` to `ci` recipe
- Both pass cleanly

### Docker verification (rust:1.94-slim on Linux)
- Default features: compiles + clippy clean
- No-systemd features: compiles + clippy clean
- Tests: pass (links on Linux, unlike macOS)
- Added no-systemd check to `.github/workflows/ci.yml`

### Test suite (`tux-daemon/tests/init_system.rs`)
- Added 10 tests covering both init systems:
  - Per-init: file existence, required fields, binary path, dbus dependency
  - Cross-init consistency: same binary path, same dbus dep across all services
  - Feature gate: compile-time check that sd-notify is gated by `systemd` feature
- Verified in Docker (rust:1.94-slim): 10/10 pass for both feature sets
- Added no-systemd build check to `.github/workflows/ci.yml`

### Decision log
- `restart = true` kept: Dinit has no `on-failure` equivalent, and daemon shouldn't exit 0 in normal operation
- No `logfile` directive: daemon logs via tracing, dinit captures stdout by default
- `depends-on` (hard dep) kept: mirrors systemd `Requires=`, tracked as follow-up for potential softening
