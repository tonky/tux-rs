# Plan: Dinit Support

## Stage 1 — Dinit service file, installation & docs

- Create `dist/tux-daemon.dinit` service file for Dinit
  - Process type, depends on dbus, restart on failure
- Add Justfile recipes: `deploy-dinit` (mirroring systemd equivalents)
- Add Dinit installation section to README.md (after the systemd daemon section)
- Document which file goes where (`/etc/dinit.d/tux-daemon`)

## Stage 2 — Make sd-notify optional via cargo feature

- Add `systemd` cargo feature (default) that gates the `sd-notify` dependency
- Use `#[cfg(feature = "systemd")]` around sd-notify calls in `main.rs`
- Non-systemd builds skip the dependency entirely
- All existing behavior preserved when feature is enabled (default)

## Stage 3 — Testing & CI considerations

- Verify `cargo build --no-default-features -p tux-daemon` compiles without sd-notify
- Verify default build still works identically
- Add a CI check for the no-default-features build
- Ensure clippy/fmt pass for both feature sets
