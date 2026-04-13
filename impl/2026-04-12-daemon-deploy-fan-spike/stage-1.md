# Stage 1: Trace deploy/startup fan-control path

## Goal

Explain why `just deploy-daemon` can produce a 100% fan spike on Uniwill hardware and implement a targeted fix.

## Context

- `Justfile` `deploy-daemon` stops the running service, copies the new binary, then starts the service again.
- `tux-daemon/src/main.rs` restores fans to auto on shutdown and also performs a startup safety reset to auto before the fan engine starts.
- `tux-daemon/src/platform/td_uw_fan.rs` maps auto mode to global `fan_mode=0` for all fans.
- A same-day follow-up added an unconditional Uniwill AC performance-profile ioctl during initial profile apply and power-state/profile reassignment.

## Hypothesis

The deploy path temporarily hands control back to the EC, then the new unconditional Uniwill AC performance-profile write drives an aggressive firmware fan policy before the daemon's fan engine settles.

## Planned change

- remove the unconditional Uniwill AC performance-profile override from startup and auto-switch paths
- leave existing shutdown/startup fan safety behavior intact
- keep profile application limited to the profile's configured settings