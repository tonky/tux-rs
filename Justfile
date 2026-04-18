# tux-rs development recipes

OS := `uname -s`

list:
    just --list

# Internal helper to run a command in Linux environment (native or Docker)
_linux command:
    @if [ "{{OS}}" = "Darwin" ]; then \
        docker run --rm -v $(pwd):/work -v tux-rs-linux-target:/work/target -w /work rust:1.95.0-bookworm bash -c "apt-get update -qq && apt-get install -y -qq dbus && rustup component add rustfmt clippy >/dev/null 2>&1 && dbus-run-session -- {{command}}"; \
    else \
        if command -v dbus-run-session >/dev/null 2>&1; then \
            dbus-run-session -- {{command}}; \
        else \
            {{command}}; \
        fi \
    fi

# Build all workspace crates
build:
    cargo build --workspace

# Run all workspace tests (with Linux dispatch on macOS)
test:
    just _linux "cargo test --workspace --tests"

# Run clippy (with Linux dispatch on macOS)
clippy:
    just _linux "cargo clippy --workspace --tests -- -D warnings"

# Check formatting (with Linux dispatch on macOS)
fmt:
    just _linux "cargo fmt --all -- --check"

# Fix formatting
fmt-fix:
    cargo fmt --all

# Run all checks (fmt, clippy, test)
check: fmt clippy test

# Validate driver-daemon fixture schema (Stage 1 reliability suite)
fixture-validate:
    just _linux "cargo test -p tux-daemon --test fixture_schema"

# Validate fixture schema + deterministic replay contracts
fixture-contract-test:
    just _linux "cargo test -p tux-daemon --test fixture_schema --test contract_replay"

# Driver-daemon reliability suite (deterministic, CI-safe)
reliability-test:
    just _linux "cargo test -p tux-daemon --test fixture_schema --test contract_replay --test integration"

# Capture a Uniwill driver-daemon contract fixture into tmp/
fixture-capture-uniwill:
    ./tools/capture-uniwill-contract-fixture.sh

# Run the daemon
run-daemon:
    cargo run -p tux-daemon

# Run daemon in debug mode (stops systemd service first, runs release build with --debug)
daemon-debug:
    cargo build --release -p tux-daemon
    sudo systemctl stop tccd 2>/dev/null || true
    sudo systemctl stop tux-daemon || true
    sudo ./target/release/tux-daemon --debug

# Run the TUI (e.g. just tui --tab profiles, just tui --help)
tui *ARGS:
    cargo run -p tux-tui -- {{ARGS}}

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
    sudo systemctl stop tccd 2>/dev/null || true
    sudo systemctl stop tux-daemon 2>/dev/null || true
    sudo cp target/release/tux-daemon /usr/bin/tux-daemon
    sudo systemctl start tux-daemon

# Rebuild/reinstall daemon and print systemd status + recent journal logs
deploy-daemon-debug:
    cargo build --release -p tux-daemon
    sudo systemctl stop tccd 2>/dev/null || true
    sudo systemctl stop tux-daemon 2>/dev/null || true
    sudo cp target/release/tux-daemon /usr/bin/tux-daemon
    sudo systemctl start tux-daemon
    sudo systemctl --no-pager --full status tux-daemon || true
    sudo journalctl -u tux-daemon -n 120 --no-pager -o short-iso

# Persist Uniwill direct EC mode (workaround for WMI keyboard backlight failures)
enable-uniwill-ec-direct:
    echo 'options uniwill_wmi ec_direct_io=1' | sudo tee /etc/modprobe.d/99-tuxedo-uniwill-ec-direct.conf >/dev/null
    sudo systemctl stop tux-daemon 2>/dev/null || true
    sudo modprobe -r uniwill_wmi tuxedo_io tuxedo_uw_fan tuxedo_keyboard tuxedo_compatibility_check 2>/dev/null || true
    sudo modprobe uniwill_wmi
    sudo modprobe tuxedo_keyboard
    sudo modprobe tuxedo_io
    sudo modprobe tuxedo_uw_fan
    sudo systemctl start tux-daemon
    echo -n 'uniwill_wmi ec_direct_io=' && cat /sys/module/uniwill_wmi/parameters/ec_direct_io

# Rebuild, reinstall and restart the daemon (dinit)
deploy-dinit:
    cargo build --release -p tux-daemon --no-default-features --features tcc-compat
    sudo dinitctl stop tux-daemon 2>/dev/null || true
    sudo cp target/release/tux-daemon /usr/bin/tux-daemon
    sudo cp dist/tux-daemon.dinit /etc/dinit.d/tux-daemon
    sudo dinitctl start tux-daemon

# Rebuild/reinstall daemon and (re)enable runit service.
# Defaults target common runit layout on Void-like systems.
# Override paths for Artix, e.g.:
# just deploy-runit SERVICE_DIR=/etc/runit/sv/tux-daemon ENABLE_DIR=/run/runit/service/tux-daemon
deploy-runit SERVICE_DIR='/etc/sv/tux-daemon' ENABLE_DIR='/var/service/tux-daemon':
    cargo build --release -p tux-daemon --no-default-features --features tcc-compat
    sudo sv down "{{ENABLE_DIR}}" 2>/dev/null || true
    sudo cp target/release/tux-daemon /usr/bin/tux-daemon
    sudo mkdir -p "{{SERVICE_DIR}}"
    sudo cp dist/tux-daemon.runit/run "{{SERVICE_DIR}}/run"
    sudo cp dist/tux-daemon.runit/finish "{{SERVICE_DIR}}/finish"
    sudo chmod +x "{{SERVICE_DIR}}/run" "{{SERVICE_DIR}}/finish"
    sudo ln -sfn "{{SERVICE_DIR}}" "{{ENABLE_DIR}}"
    sudo sv up "{{ENABLE_DIR}}" 2>/dev/null || true

# Build and run runit smoke container (dbus + daemon --mock + restart assertion).
runit-smoke:
    docker build -f containers/runit-smoke/Dockerfile -t tux-rs-runit-smoke .
    docker run --rm --name tux-rs-runit-smoke tux-rs-runit-smoke

# Run the smoke container twice to catch startup races.
runit-smoke-repeat:
    docker build -f containers/runit-smoke/Dockerfile -t tux-rs-runit-smoke .
    docker run --rm --name tux-rs-runit-smoke-1 tux-rs-runit-smoke
    docker run --rm --name tux-rs-runit-smoke-2 tux-rs-runit-smoke

# Run focused regression tests (includes live checks; requires daemon for ignored live test)
live-test:
    cargo test -p tux-daemon keyboard_state_roundtrip
    cargo test -p tux-daemon set_keyboard_state_forwards_color_and_mode_to_hardware
    cargo test -p tux-daemon apply_scales_profile_keyboard_brightness_to_hardware
    cargo test -p tux-daemon set_online_cores_works
    cargo test -p tux-daemon set_scaling_min_max_freq_works
    cargo test -p tux-daemon apply_cpu_governor_and_tdp
    cargo test -p tux-tui --test live_regression -- --ignored --nocapture

# CI: run all checks under dbus-run-session (for environments without a session bus)
ci:
    cargo fmt --all -- --check
    cargo clippy --workspace --tests -- -D warnings
    cargo check -p tux-daemon --no-default-features --features tcc-compat
    cargo clippy -p tux-daemon --no-default-features --features tcc-compat -- -D warnings
    dbus-run-session -- cargo test -p tux-daemon --test fixture_schema --test contract_replay --test integration
    dbus-run-session -- cargo test --workspace --tests
