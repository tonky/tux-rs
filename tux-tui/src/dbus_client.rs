//! D-Bus client for communicating with the tux-daemon.

use zbus::Connection;
use zbus::names::{BusName, InterfaceName};
use zbus::zvariant::OwnedValue;

/// Client wrapper around the D-Bus connection to tux-daemon.
pub struct DaemonClient {
    connection: Connection,
}

const BUS_NAME: &str = "com.tuxedocomputers.tccd";
const OBJECT_PATH: &str = "/com/tuxedocomputers/tccd";
const FAN_IFACE: &str = "com.tuxedocomputers.tccd.Fan";
const DEVICE_IFACE: &str = "com.tuxedocomputers.tccd.Device";
const SETTINGS_IFACE: &str = "com.tuxedocomputers.tccd.Settings";
const SYSTEM_IFACE: &str = "com.tuxedocomputers.tccd.System";
const PROFILE_IFACE: &str = "com.tuxedocomputers.tccd.Profile";
const CHARGING_IFACE: &str = "com.tuxedocomputers.tccd.Charging";

impl DaemonClient {
    /// Connect to the daemon on the system bus (or session bus for development).
    pub async fn connect(session_bus: bool) -> Result<Self, zbus::Error> {
        let connection = if session_bus {
            Connection::session().await?
        } else {
            Connection::system().await?
        };
        Ok(Self { connection })
    }

    // ── Fan Interface ───────────────────────────────────────────

    /// Read temperature in millidegrees Celsius for the given sensor.
    pub async fn get_temperature(&self, sensor: u32) -> Result<i32, zbus::Error> {
        self.call_method(FAN_IFACE, "GetTemperature", &(sensor,))
            .await
    }

    /// Read fan RPM for the given fan index.
    pub async fn get_fan_speed(&self, fan: u32) -> Result<u32, zbus::Error> {
        self.call_method(FAN_IFACE, "GetFanSpeed", &(fan,)).await
    }

    /// Get fan hardware info: (max_rpm, min_rpm, multi_fan, num_fans).
    pub async fn get_fan_info(&self) -> Result<(u32, u32, bool, u8), zbus::Error> {
        self.call_method(FAN_IFACE, "GetFanInfo", &()).await
    }

    /// Write PWM value (0–255) to a specific fan.
    pub async fn set_fan_speed(&self, fan_index: u32, pwm: u8) -> Result<(), zbus::Error> {
        self.call_method(FAN_IFACE, "SetFanSpeed", &(fan_index, pwm))
            .await
    }

    /// Restore hardware automatic fan control for a specific fan.
    pub async fn set_auto_mode(&self, fan_index: u32) -> Result<(), zbus::Error> {
        self.call_method(FAN_IFACE, "SetAutoMode", &(fan_index,))
            .await
    }

    /// Set fan mode: "auto", "manual", or "custom"/"custom-curve".
    pub async fn set_fan_mode(&self, mode: &str) -> Result<(), zbus::Error> {
        self.call_method(FAN_IFACE, "SetFanMode", &(mode,)).await
    }

    // ── Device Interface ────────────────────────────────────────

    /// Read a device property by name.
    pub async fn get_device_property(&self, name: &str) -> Result<OwnedValue, zbus::Error> {
        let proxy = zbus::fdo::PropertiesProxy::builder(&self.connection)
            .destination(BUS_NAME)?
            .path(OBJECT_PATH)?
            .build()
            .await?;
        let iface = InterfaceName::try_from(DEVICE_IFACE)?;
        proxy.get(iface, name).await.map_err(Into::into)
    }

    // ── System Interface ────────────────────────────────────────

    /// Get system info as TOML string.
    pub async fn get_system_info(&self) -> Result<String, zbus::Error> {
        self.call_method(SYSTEM_IFACE, "GetSystemInfo", &()).await
    }

    /// Get current power state: "ac" or "battery".
    pub async fn get_power_state(&self) -> Result<String, zbus::Error> {
        self.call_method(SYSTEM_IFACE, "GetPowerState", &()).await
    }

    /// Get battery information as TOML string.
    pub async fn get_battery_info(&self) -> Result<String, zbus::Error> {
        self.call_method(SYSTEM_IFACE, "GetBatteryInfo", &()).await
    }

    /// Get average CPU frequency in MHz.
    pub async fn get_cpu_frequency(&self) -> Result<u32, zbus::Error> {
        self.call_method(SYSTEM_IFACE, "GetCpuFrequency", &()).await
    }

    /// Get the number of online CPU cores.
    pub async fn get_cpu_count(&self) -> Result<u32, zbus::Error> {
        self.call_method(SYSTEM_IFACE, "GetCpuCount", &()).await
    }

    /// Get CPU load (overall + per-core) as TOML string.
    pub async fn get_cpu_load(&self) -> Result<String, zbus::Error> {
        self.call_method(SYSTEM_IFACE, "GetCpuLoad", &()).await
    }

    /// Get per-core CPU frequencies as TOML string.
    pub async fn get_per_core_frequencies(&self) -> Result<String, zbus::Error> {
        self.call_method(SYSTEM_IFACE, "GetPerCoreFrequencies", &())
            .await
    }

    /// Get the active profile name for the current power state.
    pub async fn get_active_profile_name(&self) -> Result<String, zbus::Error> {
        self.call_method(SYSTEM_IFACE, "GetActiveProfileName", &())
            .await
    }

    // ── Settings Interface ──────────────────────────────────────

    /// Get capabilities as TOML string.
    pub async fn get_capabilities(&self) -> Result<String, zbus::Error> {
        self.call_method(SETTINGS_IFACE, "GetCapabilities", &())
            .await
    }

    // ── Profile Interface ───────────────────────────────────────

    /// Get profile assignments as TOML string.
    pub async fn get_profile_assignments(&self) -> Result<String, zbus::Error> {
        self.call_method(PROFILE_IFACE, "GetProfileAssignments", &())
            .await
    }

    /// List all profiles as TOML string.
    pub async fn list_profiles(&self) -> Result<String, zbus::Error> {
        self.call_method(PROFILE_IFACE, "ListProfiles", &()).await
    }

    /// Copy a profile, returning the new profile's ID.
    pub async fn copy_profile(&self, id: &str) -> Result<String, zbus::Error> {
        self.call_method(PROFILE_IFACE, "CopyProfile", &(id,)).await
    }

    /// Create a new custom profile from TOML string, returning the new ID.
    pub async fn create_profile(&self, toml_str: &str) -> Result<String, zbus::Error> {
        self.call_method(PROFILE_IFACE, "CreateProfile", &(toml_str,))
            .await
    }

    /// Delete a custom profile by ID.
    pub async fn delete_profile(&self, id: &str) -> Result<(), zbus::Error> {
        self.call_method(PROFILE_IFACE, "DeleteProfile", &(id,))
            .await
    }

    /// Update an existing custom profile from TOML string.
    pub async fn update_profile(&self, id: &str, toml_str: &str) -> Result<(), zbus::Error> {
        self.call_method(PROFILE_IFACE, "UpdateProfile", &(id, toml_str))
            .await
    }

    /// Set the active profile for a power state.
    pub async fn set_active_profile(&self, id: &str, state: &str) -> Result<(), zbus::Error> {
        self.call_method(PROFILE_IFACE, "SetActiveProfile", &(id, state))
            .await
    }

    // ── Fan Curve ───────────────────────────────────────────────

    /// Get the active fan curve config as a TOML string.
    pub async fn get_active_fan_curve(&self) -> Result<String, zbus::Error> {
        self.call_method(FAN_IFACE, "GetActiveFanCurve", &()).await
    }

    /// Set a new fan curve from TOML string.
    pub async fn set_fan_curve(&self, toml_str: &str) -> Result<(), zbus::Error> {
        self.call_method(FAN_IFACE, "SetFanCurve", &(toml_str,))
            .await
    }

    // ── Settings Interface (get/set) ────────────────────────────

    /// Get global settings as TOML.
    pub async fn get_global_settings(&self) -> Result<String, zbus::Error> {
        self.call_method(SETTINGS_IFACE, "GetGlobalSettings", &())
            .await
    }

    /// Set global settings from TOML.
    pub async fn set_global_settings(&self, toml_str: &str) -> Result<(), zbus::Error> {
        self.call_method(SETTINGS_IFACE, "SetGlobalSettings", &(toml_str,))
            .await
    }

    /// Get keyboard state as TOML.
    pub async fn get_keyboard_state(&self) -> Result<String, zbus::Error> {
        self.call_method(SETTINGS_IFACE, "GetKeyboardState", &())
            .await
    }

    /// Set keyboard state from TOML.
    pub async fn set_keyboard_state(&self, toml_str: &str) -> Result<(), zbus::Error> {
        self.call_method(SETTINGS_IFACE, "SetKeyboardState", &(toml_str,))
            .await
    }

    /// Get charging settings as TOML.
    pub async fn get_charging_settings(&self) -> Result<String, zbus::Error> {
        self.call_method(CHARGING_IFACE, "GetChargingSettings", &())
            .await
    }

    /// Set charging settings from TOML.
    pub async fn set_charging_settings(&self, toml_str: &str) -> Result<(), zbus::Error> {
        self.call_method(CHARGING_IFACE, "SetChargingSettings", &(toml_str,))
            .await
    }

    /// Get GPU info as TOML.
    pub async fn get_gpu_info(&self) -> Result<String, zbus::Error> {
        self.call_method(SETTINGS_IFACE, "GetGpuInfo", &()).await
    }

    /// Get power settings as TOML.
    pub async fn get_power_settings(&self) -> Result<String, zbus::Error> {
        self.call_method(SETTINGS_IFACE, "GetPowerSettings", &())
            .await
    }

    /// Set power settings from TOML.
    pub async fn set_power_settings(&self, toml_str: &str) -> Result<(), zbus::Error> {
        self.call_method(SETTINGS_IFACE, "SetPowerSettings", &(toml_str,))
            .await
    }

    /// Get display settings as TOML.
    pub async fn get_display_settings(&self) -> Result<String, zbus::Error> {
        self.call_method(SETTINGS_IFACE, "GetDisplaySettings", &())
            .await
    }

    /// Set display settings from TOML.
    pub async fn set_display_settings(&self, toml_str: &str) -> Result<(), zbus::Error> {
        self.call_method(SETTINGS_IFACE, "SetDisplaySettings", &(toml_str,))
            .await
    }

    /// List webcam devices.
    #[allow(dead_code)]
    pub async fn list_webcam_devices(&self) -> Result<String, zbus::Error> {
        self.call_method(SETTINGS_IFACE, "ListWebcamDevices", &())
            .await
    }

    /// Get webcam controls for a device as TOML.
    #[allow(dead_code)]
    pub async fn get_webcam_controls(&self, device: &str) -> Result<String, zbus::Error> {
        self.call_method(SETTINGS_IFACE, "GetWebcamControls", &(device,))
            .await
    }

    /// Set webcam controls for a device from TOML.
    pub async fn set_webcam_controls(
        &self,
        device: &str,
        toml_str: &str,
    ) -> Result<(), zbus::Error> {
        self.call_method(SETTINGS_IFACE, "SetWebcamControls", &(device, toml_str))
            .await
    }

    // ── Helpers ─────────────────────────────────────────────────

    async fn call_method<B, R>(&self, iface: &str, method: &str, body: &B) -> Result<R, zbus::Error>
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
        R: serde::de::DeserializeOwned + zbus::zvariant::Type,
    {
        let dest: BusName<'_> = BUS_NAME.try_into()?;
        let iface_name: InterfaceName<'_> = iface.try_into()?;
        let reply = self
            .connection
            .call_method(Some(dest), OBJECT_PATH, Some(iface_name), method, body)
            .await?;
        reply.body().deserialize()
    }
}
