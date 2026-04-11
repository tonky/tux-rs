- ~~daemon and tui should probably share code for data structures, like d-bus?~~ **DONE** (Phase 8.2: shared dbus_types.rs)

- ~~what would the most practical end to end tests look like, that excercise core, daemon, dbus and tui in one call? mock fs in a cgroup to emulate data from kernel and TUI extended with CLI commands to read/write everything user can see and change in UI? Also there should probably be a refactor to surface 'TUI <-> D-Bus' library.~~ **DONE** (Phase 8.1/8.2: integration tests, E2E tests, CLI dump mode)

- ~~some sort of basic E2E test for each device, covering most of functionality? So new devices could be added by defining a test for it first via capabilities/data and implementing without actual hardware.~~ **DONE** (Phase 8.2: device-driven E2E tests)

- ~~The daemon doesn't expose CPU frequency, core count, or active profile via D-Bus. These are aspirational spec items that would need daemon changes (Phase 7+ territory).~~ **DONE** (Phase 8.5: added get_cpu_frequency, get_cpu_count, get_active_profile_name to SystemInterface)

- ~~Evaluate feature parity: investigate full list of original(Angular) TCC UI  features and write them down in a file, comparing to our TUI implementation. Add what was deliberately omitted(e.g. webcam or tomte), and comment on any discrepancies in implementation.~~ **DONE** (Phase 8.4: 8.4-evaluation-results.md)

- ~~Overview and comparison of 'tuxedo-drivers' and our kernel drivers implementation, detailing which features and data reads/writes paths were changed. Provide 2 architectural diagrams for better visualisation, for old and new approaches.~~ **DONE** (Phase 8.4: kernel driver comparison + architectural diagrams)

- Probably should extract domain and other shared types(D-Bus etc) from 'tux-core' into separate crate or two. → **Reviewed in 8.3, not warranted** — types serve different roles across crates

- ~~Remove any mention of user-specific local paths, like '/home/user' from code and docs~~ **DONE** (Phase 8.3: fixed in 8.6-tcc-compat-shim.md)

- ~~Let's minimise 'magic raw value' usage, like 'pwm <= 255' and use constants with representative names.~~ **DONE** (Phase 8.3: added constants in fan_curve.rs, uniwill.rs; existing constants verified in HID drivers, nb05)

- ~~Revisit all suppressed clippy warnings in the repo~~ **DONE** (Phase 8.3: reviewed — all `#[allow(clippy::too_many_arguments)]` justified)

- ~~Review repo to improve any 'forward compatibility' or 'inconcistency' fixes or patches, e.g. serde(default) for 'CapabilitiesResponse'. This is a clean rewrite, so we should keep this clean and a good starting point.~~ **DONE** (Phase 8.3: `serde(default)` reviewed, intentional for forward compat; inconsistent D-Bus errors deferred to 8.9)

- ~~Integration tests shouldn't be ignored. They have either native D-Bus on dev machine, or supporting dbus session in CI, so they should always be running as part of tests.~~ **DONE** (Phase 8.2: removed `#[ignore]` from E2E tests; only DMI hardware test remains ignored)

- Let's distinguish between "unsupported" features, like webcam, for 'backend' and 'user facing' features. Daemon should support them, if kernel/ioctl/sysfs supports them, but TUI should display specific features, like 'video preview' or any other multimedia things that can't be managed in CLI as 'Unavailable in CLI". Alternative GUI implementations would be able to work with those, so daemon should be correctly exposing them.

- ~~let's improve TUI fan curves:~~
 1. ~~can 'reset' use whole 0 - 100C range with 5 default points? right now it's 4 points from 45C to 90C~~ **DONE** (changed FanConfig::default to 5 points 0-100°C; split reset/revert: 'r' resets to defaults, Esc reverts to original)
 2. ~~please make currently selected point more visible. going from green to purple dot is too subtle.~~ **DONE** (changed to Block marker with LightYellow color)

- ~~TCC:~~ 
  - ~~backlight brightness is not applied on profile activation~~ **DONE** (SharedKeyboard Arc<Mutex<>> shared between D-Bus interfaces and ProfileApplier; also fixed TCC compat and Settings interfaces receiving empty keyboard vecs)
  - ~~fan curve change doesn't affect fan - it's on 38%(same for TUI)~~ **DONE** (SetFanSpeed D-Bus call now auto-switches to Manual mode so engine doesn't override; root cause was active config in Manual mode where engine does nothing)


  - Looks like changing charging threshold settings in TUI aren't working for this laptop. I think it doesn't have explicit ones, they are implicit in charing profile. TUI error: Error: charging: org.freedesktop.DBus.Error.UnknownMethod: Unknown method 'SetChargingSettings

  - "the one flaky test is a known timing issue in config_change_bypasses_hysteresis" - let's fix the flakiness

  - TUI Charging settings aren't persisted after restart: i save them as stationary/performance and after restart they are at 'high capacity' / charge again.

  - keyboard lightning levels from TUI still doesn't work. they are saved, but laptop keyboard isn't illuminated. works with legacy dirvers/TCC

  - legacy TCC is crashing GUI again, leaving zombie processes. looks like "CPU Power" isn't displaying, and here are some console errors:

  - let's extend our '--debug' flag on daemon to emit more info, useful to pinpoint issues. and remember that it can be used in debugging session.

  - ~~TUI - display controls should also work on this laptop. at least changing the brightness. works with vendor drivers and legacy TCC~~ **DONE** (DisplayBacklight backend scanning /sys/class/backlight/; D-Bus GetDisplaySettings/SetDisplaySettings; TUI brightness slider; ProfileApplier applies display brightness on profile switch)

   - ~~looks like vendor kernel modules are loaded on startup and give error when trying to start our 'tuxedo-uw-fan'. let's make a single 'just' command, that unloads all old modules and builds/loads our rewrited ones, specific to user running machine. make sure all required supporting modules(keyboard brightness etc) are also loaded.~~ **DONE** — Merged vendor `tuxedo_keyboard` + `uniwill_wmi` into unified `tuxedo-uniwill` module. Added `just kmod-swap` command. Old `tuxedo-uw-fan` renamed to `tuxedo-uniwill` with full vendor feature parity (fan, LED, fn_lock, input, battery, charging, lightbar, mini-LED).

   - we have battery cycle count? nice, let's integrate it into TUI info screen