# Disclaimer

1. This is 100% LLM-assisted project. I haven't looked much at the code. My last C experience was last century :)
I did quite a lot of planning, steering and testing, though(you can check the `impl` folder). I did fix some obvious hallucinations like `Arc<Mutex<Arc<dyn Settings>>>`.

2. This is only tested on my machine(Infinitybook Pro 16 Gen8). Drivers are ported only for it, no other hardware is expected to work. Not all features are implemented yet. Original TCC compatibility is partially implemented.

3. My goal was to see if this could be done via LLM power-coding as a weekend project. Now it looks like a good starting point to gather interest and community feedback. Maybe people with other Tuxedo hardware and C/Rust knowledge will find this useful and contribue as well.

# tux-rs

Unified Rust rewrite of TUXEDO laptop drivers and Tuxedo Control Center as a TUI app.
Kernel drivers are still in C, but they are machine-focuse,d stateless and much smaller.

See [docs/architecture.md](docs/architecture.md) for the architecture deep dive, and [docs/hardware_support.md](docs/hardware_support.md) for the supported hardware list.

![tux-rs demo](demo.gif)



## Crates

| Crate | Type | Description |
|-------|------|-------------|
| `tux-core` | lib | Hardware model system — device table, trait hierarchy, platform types |
| `tux-daemon` | bin | Root system daemon — fan control, D-Bus API, profile management |
| `tux-tui` | bin | Terminal UI — ratatui-based control interface |

## Kernel Modules

`tux-kmod/` contains 5 minimal C kernel shims for raw hardware access via sysfs.

## Installation

### 1. Kernel modules (DKMS)

```sh
just kmod-install       # copies to /usr/src, builds via DKMS, installs for current kernel
sudo modprobe tuxedo_uw_fan  # load your platform's module (varies by laptop)
```

### 2. Daemon

```sh
just deploy-daemon      # build release, install to /usr/bin, start systemd service
```

### 3. TUI

```sh
just install-tui        # install tux-tui binary
tux-tui                 # run (daemon must be running)
```

## Development

Requires Rust stable and [just](https://github.com/casey/just).

### Edit — Test — Run

```sh
just check              # fmt + clippy + test (492 tests)
just daemon-debug       # stop systemd service, run daemon with debug logging
just run-tui            # launch TUI against the running daemon
just live-test          # regression tests against a live daemon
```

### Individual commands

```sh
just build              # build all crates
just test               # run all tests
just clippy             # lint with warnings as errors
just fmt                # check formatting
```

### Kernel module development

```sh
just kmod-build                       # build all modules
just kmod-reload tuxedo-uw-fan        # rebuild + reload a single module
just kmod-install                     # full DKMS install
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