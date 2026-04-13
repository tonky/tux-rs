# Uniwill Driver-Contract Fixtures

## Schema Version

Current schema version: 1

Each fixture is a TOML file with four top-level sections:

1. schema_version
2. meta
3. raw
4. normalized

## Required Structure

```toml
schema_version = 1

[meta]
fixture_id = "..."
platform = "Uniwill"
product_sku = "..."
captured_at = "RFC3339"
capture_tool_version = "..."
capture_source = "manual-hardware|manual-sample|ci-synthetic"
kernel_release = "..."
daemon_version = "..."
driver_stack = "tuxedo-drivers"

[raw.sysfs]
# key/value snapshot of raw driver-facing attributes

[raw.dbus]
# raw string payloads from daemon D-Bus methods

[[normalized.fans]]
index = 0
temp_celsius = 45.0
duty_percent = 57
rpm = 0
rpm_available = false

[normalized.charging]
profile = "balanced"
priority = "charge_battery"

[normalized.health]
status = "ok"
consecutive_failures = 0
```

## Versioning Policy

1. Minor additive fields are allowed if serde defaults keep old fixtures valid.
2. Removing or renaming fields requires a schema_version bump.
3. Behavior drift that changes normalized values requires fixture refresh with review notes.

## Capture Workflow

Use the capture helper:

```bash
./tools/capture-uniwill-contract-fixture.sh
```

This writes a fixture under tmp/ by default. Move reviewed fixtures into this directory.

## Validation

Fixture schema and value constraints are validated by:

```bash
cargo test -p tux-daemon --test fixture_schema
```
