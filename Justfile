# tux-rs development recipes

list:
    just --list

# Build all workspace crates
build:
    cargo build --workspace

# Run all tests
test:
    cargo test --workspace

# Run clippy with warnings as errors
clippy:
    cargo clippy --workspace -- -D warnings

# Check formatting
fmt:
    cargo fmt --all -- --check

# Fix formatting
fmt-fix:
    cargo fmt --all

# Run the daemon
run-daemon:
    cargo run -p tux-daemon

# Run daemon in debug mode (stops systemd service first, runs release build with --debug)
daemon-debug:
    cargo build --release -p tux-daemon
    sudo systemctl stop tux-daemon || true
    sudo ./target/release/tux-daemon --debug

# Run the TUI
tui:
    cargo run -p tux-tui

# Record TUI demo interactively (daemon must be running, navigate freely, Ctrl-D to stop)
demo-record:
    asciinema rec demo.cast --cols 120 --rows 35 -c "just run-tui"

# Convert recording to GIF (requires agg: cargo install agg)
demo-gif:
    agg --theme nord demo.cast demo.gif

# Scripted demo recording via VHS (requires vhs; daemon must be running)
demo-vhs:
    flox activate -- vhs demo.tape

# Install daemon binary
install-daemon:
    cargo install --path tux-daemon

# Install TUI binary
install-tui:
    cargo install --path tux-tui

# Rebuild, reinstall and restart the daemon (systemd)
deploy-daemon:
    cargo build --release -p tux-daemon
    sudo systemctl stop tux-daemon 2>/dev/null || true
    sudo cp target/release/tux-daemon /usr/bin/tux-daemon
    sudo systemctl start tux-daemon

# Rebuild, reinstall and restart the daemon (dinit)
deploy-dinit:
    cargo build --release -p tux-daemon --no-default-features --features tcc-compat
    sudo dinitctl stop tux-daemon 2>/dev/null || true
    sudo cp target/release/tux-daemon /usr/bin/tux-daemon
    sudo cp dist/tux-daemon.dinit /etc/dinit.d/tux-daemon
    sudo dinitctl start tux-daemon

# Run live regression test against a running daemon (requires tux-daemon on system or session bus)
live-test:
    cargo test -p tux-tui --test live_regression -- --ignored --nocapture

# Run all checks (fmt, clippy, test)
check: fmt clippy test

# CI: run all checks under dbus-run-session (for environments without a session bus)
ci:
    cargo fmt --all -- --check
    cargo clippy --workspace -- -D warnings
    cargo check -p tux-daemon --no-default-features --features tcc-compat
    cargo clippy -p tux-daemon --no-default-features --features tcc-compat -- -D warnings
    dbus-run-session -- cargo test --workspace
