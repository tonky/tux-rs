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

# --- Kernel module recipes ---

kmod_version := "0.1.0"
kmod_src := "/usr/src/tux-kmod-" + kmod_version

# Build all kernel modules
kmod-build:
    make -C tux-kmod

# Build a single module (e.g. just kmod-build-one tuxedo-uniwill)
kmod-build-one mod:
    make -C /lib/modules/$(uname -r)/build M={{justfile_directory()}}/tux-kmod/{{mod}} modules

# Clean kernel module build artifacts
kmod-clean:
    make -C tux-kmod clean

# Copy sources to /usr/src for DKMS, then add + build + install
kmod-install:
    sudo rm -rf {{kmod_src}}
    sudo cp -r tux-kmod {{kmod_src}}
    sudo dkms remove tux-kmod/{{kmod_version}} --all 2>/dev/null || true
    sudo dkms add tux-kmod/{{kmod_version}}
    sudo dkms build tux-kmod/{{kmod_version}}
    sudo dkms install tux-kmod/{{kmod_version}}

# DKMS remove (requires sudo)
kmod-remove:
    sudo dkms remove tux-kmod/{{kmod_version}} --all
    sudo rm -rf {{kmod_src}}

# Load module via insmod from build dir (e.g. just kmod-load tuxedo-uniwill)
kmod-load mod:
    sudo insmod tux-kmod/{{mod}}/$(echo {{mod}} | tr '-' '_').ko

# Unload a module (e.g. just kmod-unload tuxedo-uniwill)
kmod-unload mod:
    sudo rmmod $(echo {{mod}} | tr '-' '_')

# Rebuild, reload a single module (e.g. just kmod-reload tuxedo-uniwill)
kmod-reload mod: (kmod-build-one mod)
    -sudo rmmod $(echo {{mod}} | tr '-' '_') 2>/dev/null
    sudo insmod tux-kmod/{{mod}}/$(echo {{mod}} | tr '-' '_').ko
    @echo "Loaded $(echo {{mod}} | tr '-' '_'), checking dmesg..."
    sudo dmesg | tail -5

# Unload vendor modules and load our tuxedo-uniwill (replaces tuxedo_keyboard + uniwill_wmi)
kmod-swap:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Stopping vendor daemon if running..."
    sudo systemctl stop tccd.service 2>/dev/null && echo "  stopped tccd.service" || true
    # Kill any remaining tccd-daemon process holding /dev/tuxedo_io
    if [ -e /dev/tuxedo_io ]; then
        sudo fuser -k /dev/tuxedo_io 2>/dev/null && echo "  killed /dev/tuxedo_io users" || true
    fi
    sleep 0.5
    echo "Unloading modules..."
    # Order matters: tuxedo_io depends on tuxedo_keyboard, which depends on tuxedo_compatibility_check
    # Also unload our own module if loaded (for rebuild-reload cycle)
    for m in tuxedo_uniwill tuxedo_uw_fan tuxedo_io uniwill_wmi clevo_wmi tuxedo_keyboard tuxedo_compatibility_check; do
        if lsmod | grep "^${m} " >/dev/null; then
            sudo rmmod "$m" && echo "  removed $m" || echo "  WARN: $m busy (refcount>0)"
        fi
    done
    echo "Building tuxedo-uniwill..."
    make -C /lib/modules/$(uname -r)/build M={{justfile_directory()}}/tux-kmod/tuxedo-uniwill modules
    echo "Loading tuxedo_uniwill..."
    sudo insmod tux-kmod/tuxedo-uniwill/tuxedo_uniwill.ko
    echo "Done. Checking dmesg..."
    sudo dmesg | tail -10
