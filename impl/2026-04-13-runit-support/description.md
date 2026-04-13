# Runit Init System Support for tux-daemon

**Issue:** https://github.com/tonky/tux-rs/issues/7

## Problem

`tux-daemon` currently ships service integration for systemd and dinit, but users on
runit-based systems (for example Artix-runit) still need manual, ad-hoc setup.

## Goal

Add first-class runit compatibility with:

- a supported runit service definition in `dist/`
- install/deploy workflow in `Justfile`
- user-facing documentation in `README.md`
- automated service-file and behavior checks

## Non-Goals

- Replacing existing systemd/dinit support
- Solving every distro-specific runit layout in one pass
- Proving real hardware EC behavior in containers

## Scope

- Add runit service assets and docs
- Extend init-system tests to cover runit and cross-init consistency
- Add container smoke tests for runit supervision + dbus + daemon mock mode
- Add CI wiring for deterministic runit container smoke checks
