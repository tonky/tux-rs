# Dinit Support for tux-daemon

**Issue:** https://github.com/tonky/tux-rs/issues/2

## Problem

tux-daemon currently only supports systemd as its init system. Users on Artix Linux (and other non-systemd distributions) using Dinit cannot enable or manage the daemon as a service.

## Goal

Add Dinit as a supported init system alongside systemd, so users on Dinit-based systems (e.g. Artix-dinit) can install, enable, and run tux-daemon as a supervised service.

## Current State

- Systemd service file: `dist/tux-daemon.service`
- `sd-notify` crate used in `tux-daemon/src/main.rs` for Ready/Stopping/Watchdog notifications
- All `sd_notify` calls already use `let _ =` (silently fail when not under systemd)
- Justfile has systemd-specific recipes (`deploy-daemon`, `daemon-debug`)
- D-Bus policy file: `dist/com.tuxedocomputers.tccd.conf`

## Scope

- Dinit service file
- Make `sd-notify` an optional cargo feature (default on) so non-systemd builds are cleaner
- Justfile recipes for Dinit install/deploy
- Conditional compilation for sd-notify calls
