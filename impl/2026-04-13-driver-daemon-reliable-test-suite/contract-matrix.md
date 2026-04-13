# Contract Matrix (Uniwill First)

## Purpose

This matrix defines the driver-to-daemon integration contract covered by Stage 1.
It identifies source paths, units, expected ranges, and normalization behavior that
must remain stable or be explicitly reviewed when changed.

## Raw Driver Surfaces

| Contract Key | Source | Path or Method | Unit/Format | Expected Range | Notes |
| --- | --- | --- | --- | --- | --- |
| cpu_temp | sysfs | /sys/devices/platform/tuxedo_uw_fan/cpu_temp | celsius integer | 0..110 | Uniwill reports native celsius, not millicelsius. |
| fan1_pwm | sysfs | /sys/devices/platform/tuxedo_uw_fan/fan1_pwm | EC PWM | 0..200 | Uniwill PWM scale is 200. |
| fan2_pwm | sysfs | /sys/devices/platform/tuxedo_uw_fan/fan2_pwm | EC PWM | 0..200 | Optional on single-fan models. |
| fan1_enable | sysfs | /sys/devices/platform/tuxedo_uw_fan/fan1_enable | integer flag | 0 or 1 | Auto/manual source signal. |
| fan2_enable | sysfs | /sys/devices/platform/tuxedo_uw_fan/fan2_enable | integer flag | 0 or 1 | Auto/manual source signal. |
| charging_profile | sysfs | /sys/devices/platform/tuxedo_keyboard/charging_profile/charging_profile | string enum | high_capacity, balanced, stationary | Available only when backend is present. |
| charging_priority | sysfs | /sys/devices/platform/tuxedo_keyboard/charging_priority/charging_prio | string enum | charge_battery, performance | Available only when backend is present. |
| charging_profiles_available | sysfs | /sys/devices/platform/tuxedo_keyboard/charging_profile/charging_profiles_available | space-delimited string | backend-defined | Used for future enum sync checks. |
| charging_prios_available | sysfs | /sys/devices/platform/tuxedo_keyboard/charging_priority/charging_prios_available | space-delimited string | backend-defined | Used for future enum sync checks. |
| fn_lock | sysfs | /sys/devices/platform/tuxedo_keyboard/fn_lock | integer flag | 0 or 1 | Optional by model/driver support. |

## Normalized Daemon Surfaces

| Contract Key | D-Bus Method | Type | Rule |
| --- | --- | --- | --- |
| fan_data_n.rpm | Fan.GetFanData(u fan_index) | u32 | May be 0 when no tachometer is available. |
| fan_data_n.temp_celsius | Fan.GetFanData(u fan_index) | f32 | Derived from backend read_temp pathway. |
| fan_data_n.duty_percent | Fan.GetFanData(u fan_index) | u8 | Percentage based on backend PWM scaling. |
| fan_data_n.rpm_available | Fan.GetFanData(u fan_index) | bool | false means RPM is unavailable and duty is authoritative. |
| fan_health.status | Fan.GetFanHealth() | string enum | ok, degraded, failed. |
| fan_health.consecutive_failures | Fan.GetFanHealth() | u32 | Counter resets on successful reads. |
| charging.profile | Charging.GetChargingSettings() | optional string | Uniwill profile value. |
| charging.priority | Charging.GetChargingSettings() | optional string | Uniwill priority value. |
| capabilities.charging_profiles | Settings.GetCapabilities() | bool | true on EcProfilePriority devices. |
| capabilities.charging_thresholds | Settings.GetCapabilities() | bool | false on Uniwill profile/priority devices. |

## Normalization Rules to Lock

1. Uniwill PWM values must be interpreted on a 0..200 scale before computing duty percent.
2. Uniwill cpu_temp is already celsius and must not be divided by 1000.
3. Charging threshold methods may return 0 for Uniwill profile-based charging backends.
4. Fan RPM zero does not imply fan stopped when rpm_available is false.

## Drift Policy

1. Any contract shape change requires fixture refresh and changelog note.
2. Any contract unit/range change requires explicit migration notes in stage review.
3. Additive fields are allowed when defaults keep old fixtures valid.
