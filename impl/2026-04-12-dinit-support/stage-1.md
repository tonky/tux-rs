# Stage 1 — Dinit service file, installation & docs

## Tasks

### 1. Create `dist/tux-daemon.dinit`

Dinit service file equivalent to the existing `dist/tux-daemon.service`.

Key properties:
- `type = process` — daemon runs in foreground (like systemd Type=simple)
- `depends-on = dbus` — mirrors systemd's `Requires=dbus.service`
- `restart = true` — restart on failure
- `restart-delay = 2` — matches systemd's `RestartSec=2s`
- `smooth-recovery = true` — re-attach to restarted process without failing dependents

Reference: existing systemd file at `dist/tux-daemon.service`

### 2. Add Justfile recipe `deploy-dinit`

Mirror the existing `deploy-daemon` recipe (lines 61-65 of justfile):
```
deploy-dinit:
    cargo build --release -p tux-daemon --no-default-features
    sudo dinitctl stop tux-daemon 2>/dev/null || true
    sudo cp target/release/tux-daemon /usr/bin/tux-daemon
    sudo cp dist/tux-daemon.dinit /etc/dinit.d/tux-daemon
    sudo dinitctl start tux-daemon
```

### 3. Update README.md

Add a "Dinit (Artix, etc.)" section after the systemd daemon install (line 76), documenting:
- Manual service file install path
- `just deploy-dinit` recipe
- Build note about `--no-default-features` to skip sd-notify (once stage 2 is done)
