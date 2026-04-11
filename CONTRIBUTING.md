# Contributing to tux-rs

First off, thanks for taking the time to contribute!

The following is a set of guidelines for contributing to tux-rs and its packages.

## Getting Started

1. Ensure you have Rust and Cargo installed via `rustup`.
2. Install `just` if you haven't already (`cargo install just` or via package manager).
3. If making UI changes, read the [ratatui architecture overview](https://ratatui.rs) and look at the TEA format under `tux-tui/src`.

## Development Workflows

The `Justfile` defines many commands for standard development workflows.

### Building & Checking

We expect all code to pass standard format checks, clippy lints, and unit tests.

```sh
just build              # Build all workspace crates
just test               # Run all workspace unit tests
just clippy             # Run clippy with warnings explicitly denied
just fmt                # Check code formatting (use `just fmt-fix` to resolve)
just check              # Combine fmt, clippy, and test
```

### Daemon Testing

To run the daemon with your local changes during development:

```sh
just daemon-debug
```
This stops the current systemd daemon (if available) and runs your built executable manually with full tracing debug logs routed to the terminal.

### Adding Hardware

If you are adding support for a new laptop or keyboard style:
Please refer to `docs/architecture.md` and `docs/hardware_support.md` to see the structure of our `DeviceDescriptor` table. You can map out your new hardware capabilities inside `tux-core/src/platforms` under the correct OEM struct (Clevo, Uniwill, etc). 

Be sure to include DMI values (`sudo dmidecode` on your device) to ensure accurate fallback detection.
