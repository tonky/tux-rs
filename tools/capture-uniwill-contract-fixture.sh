#!/usr/bin/env bash
set -euo pipefail

# Capture a Uniwill driver-daemon contract fixture.
# Usage:
#   ./tools/capture-uniwill-contract-fixture.sh [output-file]

now_utc="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
ts_file="$(date -u +%Y%m%dT%H%M%SZ)"
default_out="tmp/uniwill-contract-${ts_file}.toml"
out_file="${1:-$default_out}"

warning_log="$(dirname "$out_file")/uniwill-capture-${ts_file}.warnings.log"
: > "$warning_log"

warn() {
    local msg="[WARN] $*"
    echo "$msg" >&2
    echo "$msg" >> "$warning_log"
}

mkdir -p "$(dirname "$out_file")"

sysfs_base_fan="/sys/devices/platform/tuxedo_uw_fan"
sysfs_base_kbd="/sys/devices/platform/tuxedo_keyboard"

toml_escape() {
    local s="$1"
    s=${s//\\/\\\\}
    s=${s//$'\n'/\\n}
    s=${s//$'\r'/\\r}
    s=${s//$'\t'/\\t}
    s=${s//\"/\\\"}
    printf '%s' "$s"
}

read_attr() {
    local path="$1"
    if [[ -r "$path" ]]; then
        tr -d '\n' < "$path"
    else
        printf ''
    fi
}

probe_cmd() {
    command -v "$1" >/dev/null 2>&1
}

# Best-effort D-Bus method call that returns a plain string when possible.
# If no compatible tool is available or the call fails, returns an empty string.
call_dbus_string() {
    local iface="$1"
    local method="$2"
    local arg_sig="${3:-}"
    local arg_val="${4:-}"

    if probe_cmd gdbus; then
        local out
        if [[ -n "$arg_sig" ]]; then
            out="$(gdbus call --session \
                --dest com.tuxedocomputers.tccd \
                --object-path /com/tuxedocomputers/tccd \
                --method "${iface}.${method}" "$arg_val" 2>/dev/null \
                | sed -E "s/^\('?(.*)'?\)$/\1/" || true)"
        else
            out="$(gdbus call --session \
                --dest com.tuxedocomputers.tccd \
                --object-path /com/tuxedocomputers/tccd \
                --method "${iface}.${method}" 2>/dev/null \
                | sed -E "s/^\('?(.*)'?\)$/\1/" || true)"
        fi
        if [[ -z "$out" ]]; then
            warn "D-Bus call produced empty output: ${iface}.${method}"
        fi
        printf '%s' "$out"
        return 0
    fi

    if probe_cmd busctl; then
        local out
        if [[ -n "$arg_sig" ]]; then
            out="$(busctl --user call \
                com.tuxedocomputers.tccd \
                /com/tuxedocomputers/tccd \
                "$iface" "$method" "$arg_sig" "$arg_val" 2>/dev/null \
                | sed -E 's/^[a-z]+\s+"(.*)"$/\1/' || true)"
        else
            out="$(busctl --user call \
                com.tuxedocomputers.tccd \
                /com/tuxedocomputers/tccd \
                "$iface" "$method" 2>/dev/null \
                | sed -E 's/^[a-z]+\s+"(.*)"$/\1/' || true)"
        fi
        if [[ -z "$out" ]]; then
            warn "D-Bus call produced empty output: ${iface}.${method}"
        fi
        printf '%s' "$out"
        return 0
    fi

    warn "Neither gdbus nor busctl is available; skipping ${iface}.${method}"
    printf ''
}

kernel_release="$(uname -r)"
product_sku="${PRODUCT_SKU_OVERRIDE:-UNKNOWN_UNIWILL_SKU}"
daemon_version="$(call_dbus_string com.tuxedocomputers.tccd.Device DaemonVersion | tr -d '\n')"
if [[ -z "$daemon_version" ]]; then
    daemon_version="unknown"
fi

if [[ ! -d "$sysfs_base_fan" ]]; then
    warn "${sysfs_base_fan} does not exist. Capture likely not running on Uniwill fan backend."
fi

cpu_temp="$(read_attr "${sysfs_base_fan}/cpu_temp")"
fan1_pwm="$(read_attr "${sysfs_base_fan}/fan1_pwm")"
fan2_pwm="$(read_attr "${sysfs_base_fan}/fan2_pwm")"
fan1_enable="$(read_attr "${sysfs_base_fan}/fan1_enable")"
fan2_enable="$(read_attr "${sysfs_base_fan}/fan2_enable")"

charging_profile="$(read_attr "${sysfs_base_kbd}/charging_profile/charging_profile")"
charging_priority="$(read_attr "${sysfs_base_kbd}/charging_priority/charging_prio")"
charging_profiles_available="$(read_attr "${sysfs_base_kbd}/charging_profile/charging_profiles_available")"
charging_prios_available="$(read_attr "${sysfs_base_kbd}/charging_priority/charging_prios_available")"
fn_lock="$(read_attr "${sysfs_base_kbd}/fn_lock")"

fan_data_0="$(call_dbus_string com.tuxedocomputers.tccd.Fan GetFanData u 0 | tr -d '\r')"
fan_data_1="$(call_dbus_string com.tuxedocomputers.tccd.Fan GetFanData u 1 | tr -d '\r')"
fan_health="$(call_dbus_string com.tuxedocomputers.tccd.Fan GetFanHealth | tr -d '\r')"
charging_settings="$(call_dbus_string com.tuxedocomputers.tccd.Charging GetChargingSettings | tr -d '\r')"

# Best-effort derived duty percentages from raw PWM (Uniwill scale 0..200).
fan1_duty=0
fan2_duty=0
if [[ "$fan1_pwm" =~ ^[0-9]+$ ]]; then
    fan1_duty=$(( fan1_pwm * 100 / 200 ))
fi
if [[ "$fan2_pwm" =~ ^[0-9]+$ ]]; then
    fan2_duty=$(( fan2_pwm * 100 / 200 ))
fi

# Best-effort temp normalization for initial fixture draft.
fan_temp_norm=0
if [[ "$cpu_temp" =~ ^[0-9]+$ ]]; then
    fan_temp_norm="$cpu_temp"
fi

cat > "$out_file" <<EOF
schema_version = 1

[meta]
fixture_id = "uniwill-capture-${ts_file}"
platform = "Uniwill"
product_sku = "$(toml_escape "$product_sku")"
captured_at = "${now_utc}"
capture_tool_version = "0.1.0"
capture_source = "manual-hardware"
kernel_release = "$(toml_escape "$kernel_release")"
daemon_version = "$(toml_escape "$daemon_version")"
driver_stack = "tuxedo-drivers"

[raw.sysfs]
cpu_temp = "$(toml_escape "$cpu_temp")"
fan1_pwm = "$(toml_escape "$fan1_pwm")"
fan2_pwm = "$(toml_escape "$fan2_pwm")"
fan1_enable = "$(toml_escape "$fan1_enable")"
fan2_enable = "$(toml_escape "$fan2_enable")"
charging_profile = "$(toml_escape "$charging_profile")"
charging_priority = "$(toml_escape "$charging_priority")"
charging_profiles_available = "$(toml_escape "$charging_profiles_available")"
charging_prios_available = "$(toml_escape "$charging_prios_available")"
fn_lock = "$(toml_escape "$fn_lock")"

[raw.dbus]
fan_data_0 = "$(toml_escape "$fan_data_0")"
fan_data_1 = "$(toml_escape "$fan_data_1")"
fan_health = "$(toml_escape "$fan_health")"
charging_settings = "$(toml_escape "$charging_settings")"

[[normalized.fans]]
index = 0
temp_celsius = ${fan_temp_norm}.0
duty_percent = ${fan1_duty}
rpm = 0
rpm_available = false

[[normalized.fans]]
index = 1
temp_celsius = ${fan_temp_norm}.0
duty_percent = ${fan2_duty}
rpm = 0
rpm_available = false

[normalized.charging]
profile = "$(toml_escape "$charging_profile")"
priority = "$(toml_escape "$charging_priority")"

[normalized.health]
status = "ok"
consecutive_failures = 0
EOF

echo "Wrote fixture: $out_file"
echo "Review and adjust normalized sections before committing if needed."
echo "Run: just fixture-validate"

warning_count=0
if [[ -f "$warning_log" ]]; then
    warning_count="$(wc -l < "$warning_log" | tr -d ' ')"
fi

if [[ "$warning_count" -gt 0 ]]; then
    echo "Capture warnings: $warning_count"
    echo "Warnings log: $warning_log"
    echo "Use CAPTURE_STRICT=1 to fail the capture when warnings are present."
    if [[ "${CAPTURE_STRICT:-0}" == "1" ]]; then
        echo "[ERROR] Capture produced warnings with CAPTURE_STRICT=1" >&2
        exit 1
    fi
else
    rm -f "$warning_log"
fi
