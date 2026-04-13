# Disclaimer

1. This is 100% LLM-assisted project. I haven't looked much at the code.
I did quite a lot of planning, steering and testing, though(you can check the `impl` folder). I did fix some obvious hallucinations like `Arc<Mutex<Arc<dyn Settings>>>`.

2. This is only tested on my machine(Infinitybook Pro 16 Gen8). Support is currently limited to it, no other hardware is expected to work. Not all features are implemented yet. Original TCC compatibility is partially implemented.

3. My goal was to see if this could be done via LLM power-coding as a weekend project. Now it looks like a good starting point to gather interest and community feedback. Maybe people with other Tuxedo hardware knowledge will find this useful and contribue as well.

# tux-rs

Unified Rust implementation of TUXEDO laptop support and Tuxedo Control Center as a TUI app.
Hardware support is currently only for a single machine that i own, rest are untested.

See [docs/architecture.md](docs/architecture.md) for the architecture deep dive, and [docs/hardware_support.md](docs/hardware_support.md) for the supported hardware list.

![tux-rs demo](demo.gif)



## Crates

| Crate | Type | Description |
|-------|------|-------------|
| `tux-core` | lib | Hardware model system — device table, trait hierarchy, platform types |
| `tux-daemon` | bin | Root system daemon — fan control, D-Bus API, profile management |
| `tux-tui` | bin | Terminal UI — ratatui-based control interface |

## Development

Requires Rust stable and [just](https://github.com/casey/just).

### Edit — Test — Run

```sh
just check              # fmt + clippy + test
just daemon-debug       # stop systemd service, run daemon with debug logging
just tui                # launch TUI against the running daemon
just live-test          # regression tests against a live daemon
just runit-smoke-repeat # run runit/dbus smoke container twice
```

### Individual commands

```sh
just build              # build all crates
just test               # run all tests
just clippy             # lint with warnings as errors
just fmt                # check formatting
```

### Driver <-> Daemon Reliability Suite

These checks are hardware-independent and are meant to be run both locally and in CI.

```sh
flox activate -- just fixture-contract-test   # schema + deterministic replay contracts
flox activate -- just reliability-test         # fixture contracts + daemon integration/fault matrix
flox activate -- just ci                       # full CI-equivalent pipeline
```

### Fixture Refresh Workflow (Manual Hardware Capture)

1. Capture a candidate fixture from real hardware:

```sh
flox activate -- just fixture-capture-uniwill
```

To fail fast when capture warnings occur:

```sh
CAPTURE_STRICT=1 flox activate -- just fixture-capture-uniwill
```

2. Compare candidate vs canonical fixture:

```sh
latest=$(ls -1t tmp/uniwill-contract-*.toml | head -n 1)
git --no-pager diff --no-index \
  tux-daemon/tests/fixtures/driver_contract/uniwill/sample-ibp16g8-v1.toml \
  "$latest"
```

3. Review drift before promotion:
- Expected drift: kernel/daemon version fields, timestamp metadata, intentionally changed normalization rules.
- Unexpected drift: changed fan-duty mapping, health semantics, or charging profile/priority normalization.
- Any behavior drift must include a short rationale in the stage worklog/review notes.

4. Validate before opening PR:

```sh
flox activate -- just fixture-contract-test
flox activate -- just reliability-test
```

## Installation

**Prerequisite:** Install [tuxedo-drivers](https://github.com/tuxedocomputers/tuxedo-drivers) for your kernel (available via DKMS or your distro's package manager).

### 1. Daemon

#### Systemd (most distros)

```sh
just deploy-daemon      # build release, install to /usr/bin, start systemd service
```

#### Dinit (Artix, etc.)

```sh
just deploy-dinit       # build release (no systemd deps), install to /usr/bin, install + start dinit service
```

Or manually:
```sh
cargo build --release -p tux-daemon --no-default-features --features tcc-compat
sudo cp target/release/tux-daemon /usr/bin/tux-daemon
sudo cp dist/tux-daemon.dinit /etc/dinit.d/tux-daemon
sudo dinitctl enable tux-daemon
sudo dinitctl start tux-daemon
```

#### Runit (Artix-runit, Void, etc.)

```sh
just deploy-runit      # build release (no systemd deps), install to /usr/bin, install + start runit service
```

Default `just deploy-runit` paths are:
- service dir: `/etc/sv/tux-daemon`
- enabled symlink: `/var/service/tux-daemon`

The bundled run script waits briefly for `/run/dbus/system_bus_socket` before
starting `tux-daemon` to reduce early-boot startup races.

For Artix-runit layouts, override paths:

```sh
just deploy-runit SERVICE_DIR=/etc/runit/sv/tux-daemon ENABLE_DIR=/run/runit/service/tux-daemon
```

Or manually:
```sh
cargo build --release -p tux-daemon --no-default-features --features tcc-compat
sudo cp target/release/tux-daemon /usr/bin/tux-daemon
sudo mkdir -p /etc/sv/tux-daemon
sudo cp dist/tux-daemon.runit/run /etc/sv/tux-daemon/run
sudo cp dist/tux-daemon.runit/finish /etc/sv/tux-daemon/finish
sudo chmod +x /etc/sv/tux-daemon/run /etc/sv/tux-daemon/finish
sudo ln -sfn /etc/sv/tux-daemon /var/service/tux-daemon
sudo sv up /var/service/tux-daemon
```

Troubleshooting:
```sh
sudo sv status /var/service/tux-daemon
sudo sv down /var/service/tux-daemon
sudo sv up /var/service/tux-daemon
```

Validation scope note:
- `just runit-smoke` and `just runit-smoke-repeat` validate runit supervision,
  dbus service registration, and restart behavior in a container with `--mock`.
- They do not validate real EC/sysfs hardware behavior; keep real machine checks
  (especially Artix-runit) as final verification before release.

### 2. TUI

```sh
just install-tui        # install tux-tui binary
tux-tui                 # run (daemon must be running)
```

### NixOS (Flakes)

Add `tux-rs` to your flake inputs:

```nix
{
  inputs.tux-rs.url = "github:tonky/tux-rs";
}
```

Then enable the module in your NixOS configuration:

```nix
{ inputs, ... }: {
  imports = [ inputs.tux-rs.nixosModule ];
  services.tux-daemon.enable = true;
}
```

This will automatically:
- Install `tux-daemon` and `tux-tui`.
- Configure the systemd service and D-Bus policy.
- Build and load `tuxedo-drivers` for your configured kernel package.

You can also run the TUI directly without installing:
```sh
nix run github:tonky/tux-rs#tux-tui
```

### NixOS (classic Nix, no flakes)

If you're not using flakes, the repository exposes `default.nix`.

#### Simple usage

```nix
# /etc/nixos/configuration.nix
{ pkgs, ... }:
let
  tux-rs = import (builtins.fetchTarball {
    url = "https://github.com/tonky/tux-rs/archive/main.tar.gz";
  }) { inherit pkgs; };
in {
  imports = [ tux-rs.nixosModule ];
  services.tux-daemon.enable = true;
}
```

## License

GPL-3.0-or-later

## Ram and CPU usage

```
% systemctl status tux-daemon
● tux-daemon.service - TUXEDO Hardware Daemon
     Loaded: loaded (/etc/systemd/system/tux-daemon.service; disabled; preset: enabled)
     Active: active (running) since Mon 2026-04-06 23:39:38 CEST; 8min ago
 Invocation: 33674fdb700d462d874633b47e6fe79f
   Main PID: 2505426 (tux-daemon)
      Tasks: 15 (limit: 37921)
     Memory: 2.9M (peak: 5.2M)
        CPU: 2.316s

% systemctl status tccd
● tccd.service - TUXEDO Control Center Service
     Loaded: loaded (/etc/systemd/system/tccd.service; enabled; preset: enabled)
     Active: active (running) since Mon 2026-04-06 23:57:15 CEST; 8min ago
 Invocation: 5aec8f98800c4873a8a3250d0d4db673
   Main PID: 2515503 (MainThread)
      Tasks: 10 (limit: 37921)
     Memory: 76.7M (peak: 78.8M)
        CPU: 15.264s
```