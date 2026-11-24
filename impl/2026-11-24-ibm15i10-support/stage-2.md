# Stage 2 — Live validation on IBM15I10

Goal: prove every capability in the IBM15I10 descriptor works against the real
tuxedo-drivers 4.25.0 stack, and leave behind repeatable tooling.

## Preconditions / sudo

`tccd` (TCC 3.1.2) is running and owns `com.tuxedocomputers.tccd`; it also
rewrites EPP/fan state every few seconds, so it must be stopped while
tux-daemon runs. The installed policy
`/usr/share/dbus-1/system.d/com.tuxedocomputers.tccd-dbus.conf` already allows
root to own the name and any user to call it, so only the daemon needs root;
the agent drives TUI/live tests unprivileged.

User runs in a separate terminal (foreground, Ctrl-C to stop):
```sh
cd ~/programming/tux-rs && just daemon-debug-log
```
New recipe `daemon-debug-log` = `daemon-debug` + `tee tmp/daemon-debug.log`, so
the agent can read the startup log without root. Afterwards:
`sudo systemctl start tccd` restores TCC.

## 2.1 test(live): make live regression device-aware

Context: `tux-tui/tests/live_regression.rs:77` hard-codes IBP Gen8 (name,
`tdp_control == true`, `gpu_control == false`). Running `just live-test` on
IBM15I10 would fail on identity/capabilities before exercising anything.

Change: a small `LiveExpectations` table selected from `DeviceName`:

| device | fan_count | tdp_control | gpu_control |
|--------|-----------|-------------|-------------|
| InfinityBook Pro … Gen8 | 2 | true | false |
| InfinityBook Max 15 Gen10 Intel | 2 | false (until Stage 3) | true |

Unknown device → panic with a clear message (keeps the test honest).
Test name becomes `uniwill_live_regression`; Justfile `live-test` keeps working.

## 2.2 chore(just): `daemon-debug-log` recipe

Tee daemon output into `tmp/daemon-debug.log` (repo-local, gitignored).

## 2.3 Live run (user starts daemon with sudo)

Check, from the log + D-Bus + TUI:
- detection line: `TUXEDO InfinityBook Max 15 Gen10 Intel (platform=Uniwill, exact=true)`;
- fan backend: `tuxedo_io` ioctl, 2 fans; temps sane; custom curve → fans
  respond; auto restore works (note: board uses the legacy, non-"universal"
  EC fan path in the driver);
- keyboard: ITE 8291 via hidraw discovered; brightness/colour change visible;
  no fight with the in-kernel `ite_8291` LED class device;
- charging: profile read/write (`high_capacity|balanced|stationary`), priority
  absent → handled gracefully;
- GPU: cTGP offset read/write via `tuxedo_nvidia_power_ctrl`;
- `just live-test` passes.

## 2.4 Fixture capture

`just fixture-capture-uniwill` (needs the daemon running) →
`tux-daemon/tests/fixtures/driver_contract/uniwill/ibm15i10-hardware-v1.toml`,
then wire into `fixture_schema` / `contract_replay` like
`ibp1xi08mk1-hardware-v1.toml`.

## 2.5 Fixes

Anything that misbehaves → its own `fix:` commit with a regression test.

## Exit criteria

- `just check` green; `just live-test` green on IBM15I10.
- New fixture validated by `just fixture-contract-test`.
- `docs/hardware_support.md` IBM15I10 note updated from "in progress" to the
  validated capability list.

## Outcome (2026-11-24)

All exit criteria met: `just check` green, `just live-test` green on IBM15I10,
fixture validated by `just fixture-contract-test`, hardware docs updated.
Scope grew beyond 2.1–2.5 with six live-found fixes and the cTGP-bounds work
(user decision: mirror TCC) — see `worklog-2.md`.
