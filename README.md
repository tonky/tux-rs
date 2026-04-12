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
```

### Individual commands

```sh
just build              # build all crates
just test               # run all tests
just clippy             # lint with warnings as errors
just fmt                # check formatting
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