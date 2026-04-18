//! D-Bus Settings interface: `com.tuxedocomputers.tccd.Settings`.

use std::sync::Arc;

use zbus::interface;

use tux_core::dbus_types::CapabilitiesResponse;
use tux_core::device::{ChargingCapability, KeyboardType};
use tux_core::dmi::DetectedDevice;

use crate::cpu::governor::CpuGovernor;
use crate::display::SharedDisplay;
use crate::hid::{Rgb, SharedKeyboard};

/// Parse a "#RRGGBB" hex color string into an Rgb value.
fn parse_hex_color(s: &str) -> Rgb {
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() >= 6 {
        let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(255);
        let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(255);
        Rgb::new(r, g, b)
    } else {
        Rgb::WHITE
    }
}

/// Global daemon settings exposed via D-Bus.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GlobalSettings {
    pub temperature_unit: String,
    pub fan_control_enabled: bool,
    pub cpu_settings_enabled: bool,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            temperature_unit: "celsius".to_string(),
            fan_control_enabled: true,
            cpu_settings_enabled: true,
        }
    }
}

/// Keyboard state stored and returned via Settings D-Bus interface.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct KeyboardState {
    #[serde(default)]
    pub brightness: i64,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub mode: String,
}

/// Power settings stored and returned via Settings D-Bus interface.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct PowerSettings {
    #[serde(default)]
    pub governor: String,
    #[serde(default)]
    pub epp: String,
    #[serde(default)]
    pub no_turbo: bool,
}

/// D-Bus object implementing the Settings interface.
pub struct SettingsInterface {
    settings: std::sync::RwLock<GlobalSettings>,
    capabilities: CapabilitiesResponse,
    keyboard_state: std::sync::RwLock<KeyboardState>,
    keyboards: Vec<SharedKeyboard>,
    cpu_governor: Option<Arc<CpuGovernor>>,
    display: Option<SharedDisplay>,
}

impl SettingsInterface {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: &DetectedDevice,
        has_fan: bool,
        fan_count: u8,
        keyboards: Vec<SharedKeyboard>,
        cpu_governor: Option<Arc<CpuGovernor>>,
        display: Option<SharedDisplay>,
        charging_available: bool,
        tdp_available: bool,
    ) -> Self {
        let kb = device.descriptor.keyboard;
        // Collect available modes from discovered keyboard hardware.
        let keyboard_modes: Vec<String> = if keyboards.is_empty() {
            // No hardware keyboards — derive default from device type.
            match kb {
                KeyboardType::None => vec![],
                _ => vec!["static".into()],
            }
        } else {
            // Union of modes from all discovered keyboards, deduplicated.
            let mut modes = Vec::new();
            for k in &keyboards {
                if let Ok(guard) = k.lock() {
                    for m in guard.available_modes() {
                        if !modes.contains(&m) {
                            modes.push(m);
                        }
                    }
                }
            }
            if modes.is_empty() {
                vec!["static".into()]
            } else {
                modes
            }
        };
        let caps = CapabilitiesResponse {
            fan_control: has_fan,
            fan_count,
            keyboard_backlight: !keyboards.is_empty(),
            keyboard_type: match kb {
                KeyboardType::None => "none",
                KeyboardType::White | KeyboardType::WhiteLevels(_) => "white",
                KeyboardType::Rgb1Zone
                | KeyboardType::Rgb3Zone
                | KeyboardType::RgbPerKey
                | KeyboardType::IteHid(_) => "rgb",
            }
            .to_string(),
            keyboard_modes,
            charging_thresholds: charging_available
                && matches!(device.descriptor.charging, ChargingCapability::Flexicharger),
            charging_profiles: charging_available
                && matches!(
                    device.descriptor.charging,
                    ChargingCapability::EcProfilePriority
                ),
            tdp_control: tdp_available,
            power_profiles: true,
            gpu_control: false,
            display_brightness: display.as_ref().is_some_and(|d| d.is_available()),
        };
        Self {
            settings: std::sync::RwLock::new(GlobalSettings::default()),
            capabilities: caps,
            keyboard_state: std::sync::RwLock::new(KeyboardState::default()),
            keyboards,
            cpu_governor,
            display,
        }
    }
}

#[interface(name = "com.tuxedocomputers.tccd.Settings")]
impl SettingsInterface {
    /// Get global settings as TOML.
    fn get_global_settings(&self) -> zbus::fdo::Result<String> {
        let settings = self
            .settings
            .read()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        toml::to_string(&*settings).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Update global settings from a TOML string.
    fn set_global_settings(&self, toml_str: &str) -> zbus::fdo::Result<()> {
        let new_settings: GlobalSettings = toml::from_str(toml_str)
            .map_err(|e| zbus::fdo::Error::InvalidArgs(format!("invalid settings TOML: {e}")))?;
        let mut settings = self
            .settings
            .write()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        *settings = new_settings;
        Ok(())
    }

    /// Get hardware capabilities as TOML.
    fn get_capabilities(&self) -> zbus::fdo::Result<String> {
        toml::to_string(&self.capabilities).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Get keyboard state as TOML (brightness, color, mode).
    fn get_keyboard_state(&self) -> zbus::fdo::Result<String> {
        let state = self
            .keyboard_state
            .read()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        toml::to_string(&*state).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Set keyboard state from TOML. Stores the state and forwards brightness/mode to hardware if keyboards are available.
    fn set_keyboard_state(&self, toml_str: &str) -> zbus::fdo::Result<()> {
        let new_state: KeyboardState = toml::from_str(toml_str)
            .map_err(|e| zbus::fdo::Error::InvalidArgs(format!("invalid keyboard TOML: {e}")))?;

        // Forward brightness, color and mode to hardware keyboards.
        if !self.keyboards.is_empty() {
            let hw_brightness = ((new_state.brightness.clamp(0, 100) as f32 / 100.0) * 255.0) as u8;
            let color = parse_hex_color(&new_state.color);
            let mode = new_state.mode.to_lowercase();
            for kb in &self.keyboards {
                if let Ok(mut guard) = kb.lock() {
                    let dev = guard.device_type().to_string();
                    guard.set_color(0, color).map_err(|e| {
                        zbus::fdo::Error::Failed(format!("keyboard {dev}: set_color failed: {e}"))
                    })?;
                    guard.set_mode(&mode).map_err(|e| {
                        zbus::fdo::Error::Failed(format!("keyboard {dev}: set_mode failed: {e}"))
                    })?;
                    if hw_brightness == 0 {
                        guard.turn_off().map_err(|e| {
                            zbus::fdo::Error::Failed(format!(
                                "keyboard {dev}: turn_off failed: {e}"
                            ))
                        })?;
                    } else {
                        // Re-enable LEDs on stateful backends (ITE), then re-apply target
                        // brightness so sysfs backends don't stay at max after turn_on().
                        guard.set_brightness(hw_brightness).map_err(|e| {
                            zbus::fdo::Error::Failed(format!(
                                "keyboard {dev}: set_brightness failed: {e}"
                            ))
                        })?;
                        guard.turn_on().map_err(|e| {
                            zbus::fdo::Error::Failed(format!("keyboard {dev}: turn_on failed: {e}"))
                        })?;
                        guard.set_brightness(hw_brightness).map_err(|e| {
                            zbus::fdo::Error::Failed(format!(
                                "keyboard {dev}: set_brightness (reapply) failed: {e}"
                            ))
                        })?;
                    }
                    guard.flush().map_err(|e| {
                        zbus::fdo::Error::Failed(format!("keyboard {dev}: flush failed: {e}"))
                    })?;
                }
            }
        }

        let mut state = self
            .keyboard_state
            .write()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        *state = new_state;
        Ok(())
    }

    /// Get power settings as TOML (governor, EPP, turbo).
    fn get_power_settings(&self) -> zbus::fdo::Result<String> {
        let settings = match &self.cpu_governor {
            Some(gov) => {
                let governor = gov.get_governor().unwrap_or_default();
                let epp = gov.get_epp().unwrap_or(None).unwrap_or_default();
                let no_turbo = gov.get_no_turbo().unwrap_or(false);
                PowerSettings {
                    governor,
                    epp,
                    no_turbo,
                }
            }
            None => PowerSettings::default(),
        };
        toml::to_string(&settings).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Set power settings from TOML.
    ///
    /// Best-effort: each attribute (governor, EPP, no_turbo) is set
    /// independently. If all writes fail the first error is returned;
    /// partial success (e.g. governor set but no_turbo blocked by thermald)
    /// is reported as OK.
    fn set_power_settings(&self, toml_str: &str) -> zbus::fdo::Result<()> {
        let settings: PowerSettings = toml::from_str(toml_str)
            .map_err(|e| zbus::fdo::Error::InvalidArgs(format!("invalid power TOML: {e}")))?;
        if let Some(gov) = &self.cpu_governor {
            let mut first_err = None;
            let mut any_ok = false;

            if !settings.governor.is_empty() {
                match gov.set_governor(&settings.governor) {
                    Ok(()) => any_ok = true,
                    Err(e) if first_err.is_none() => first_err = Some(e),
                    Err(_) => {}
                }
            }
            if !settings.epp.is_empty() {
                match gov.set_epp(&settings.epp) {
                    Ok(()) => any_ok = true,
                    Err(e) if first_err.is_none() => first_err = Some(e),
                    Err(_) => {}
                }
            }
            match gov.set_no_turbo(settings.no_turbo) {
                Ok(()) => any_ok = true,
                Err(e) if first_err.is_none() => first_err = Some(e),
                Err(_) => {}
            }

            if !any_ok && let Some(e) = first_err {
                return Err(zbus::fdo::Error::Failed(e.to_string()));
            }
        }
        Ok(())
    }

    /// Get display brightness state as TOML.
    fn get_display_settings(&self) -> zbus::fdo::Result<String> {
        let display = self
            .display
            .as_ref()
            .ok_or_else(|| zbus::fdo::Error::Failed("no display backlight available".into()))?;
        let ctrl = display
            .primary()
            .ok_or_else(|| zbus::fdo::Error::Failed("no backlight controller found".into()))?;
        let state = tux_core::dbus_types::DisplayState {
            brightness: ctrl.brightness_percent().unwrap_or(0),
            max_brightness: ctrl.max_brightness(),
            driver: ctrl.driver.clone(),
        };
        toml::to_string(&state).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Set display brightness from TOML. Expects a `brightness` field (0–100 percent).
    fn set_display_settings(&self, toml_str: &str) -> zbus::fdo::Result<()> {
        #[derive(serde::Deserialize)]
        struct SetBrightness {
            brightness: u32,
        }
        let req: SetBrightness = toml::from_str(toml_str)
            .map_err(|e| zbus::fdo::Error::InvalidArgs(format!("invalid display TOML: {e}")))?;
        let display = self
            .display
            .as_ref()
            .ok_or_else(|| zbus::fdo::Error::Failed("no display backlight available".into()))?;
        display
            .set_brightness_percent(req.brightness)
            .map_err(|e| zbus::fdo::Error::Failed(format!("failed to set brightness: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tux_core::device::*;
    use tux_core::platform::Platform;
    use tux_core::registers::PlatformRegisters;

    fn make_test_device() -> DetectedDevice {
        let desc = Box::leak(Box::new(DeviceDescriptor {
            name: "Test Device",
            product_sku: "TEST123",
            platform: Platform::Uniwill,
            fans: FanCapability {
                count: 2,
                control: FanControlType::Direct,
                pwm_scale: 200,
            },
            keyboard: KeyboardType::White,
            sensors: SensorSet {
                cpu_temp: true,
                gpu_temp: false,
                fan_rpm: &[true, true],
            },
            charging: ChargingCapability::Flexicharger,
            tdp: None,
            tdp_source: tux_core::device::TdpSource::None,
            gpu_power: GpuPowerCapability::None,
            registers: PlatformRegisters::Uniwill,
        }));
        DetectedDevice {
            descriptor: desc,
            dmi: tux_core::dmi::DmiInfo {
                board_vendor: String::new(),
                board_name: String::new(),
                product_sku: "TEST123".to_string(),
                sys_vendor: String::new(),
                product_name: String::new(),
                product_version: String::new(),
            },
            exact_match: true,
        }
    }

    #[test]
    fn get_capabilities_reflects_device() {
        use crate::hid::{KeyboardLed, Rgb};
        use std::sync::{Arc, Mutex};
        struct DummyKb;
        impl KeyboardLed for DummyKb {
            fn set_brightness(&mut self, _: u8) -> std::io::Result<()> {
                Ok(())
            }
            fn set_color(&mut self, _: u8, _: Rgb) -> std::io::Result<()> {
                Ok(())
            }
            fn set_mode(&mut self, _: &str) -> std::io::Result<()> {
                Ok(())
            }
            fn zone_count(&self) -> u8 {
                1
            }
            fn turn_off(&mut self) -> std::io::Result<()> {
                Ok(())
            }
            fn turn_on(&mut self) -> std::io::Result<()> {
                Ok(())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
            fn device_type(&self) -> &str {
                "dummy"
            }
            fn available_modes(&self) -> Vec<String> {
                vec!["static".into()]
            }
        }
        let kb: SharedKeyboard = Arc::new(Mutex::new(Box::new(DummyKb)));
        let device = make_test_device();
        let iface = SettingsInterface::new(&device, true, 2, vec![kb], None, None, true, false);

        let caps_toml = iface.get_capabilities().unwrap();
        let caps: CapabilitiesResponse = toml::from_str(&caps_toml).unwrap();

        assert!(caps.fan_control);
        assert_eq!(caps.fan_count, 2);
        assert!(caps.keyboard_backlight);
        assert_eq!(caps.keyboard_type, "white");
        assert_eq!(caps.keyboard_modes, vec!["static"]);
        assert!(caps.charging_thresholds);
        assert!(!caps.tdp_control);
    }

    #[test]
    fn get_capabilities_keyboard_unavailable_when_no_backends() {
        let device = make_test_device();
        let iface = SettingsInterface::new(&device, true, 2, vec![], None, None, true, false);
        let caps_toml = iface.get_capabilities().unwrap();
        let caps: CapabilitiesResponse = toml::from_str(&caps_toml).unwrap();
        assert!(
            !caps.keyboard_backlight,
            "keyboard_backlight must be false when no backends discovered"
        );
    }

    #[test]
    fn get_global_settings_defaults() {
        let device = make_test_device();
        let iface = SettingsInterface::new(&device, true, 2, vec![], None, None, true, false);

        let settings_toml = iface.get_global_settings().unwrap();
        let settings: GlobalSettings = toml::from_str(&settings_toml).unwrap();

        assert_eq!(settings.temperature_unit, "celsius");
        assert!(settings.fan_control_enabled);
    }

    #[test]
    fn keyboard_state_roundtrip() {
        let device = make_test_device();
        let iface = SettingsInterface::new(&device, true, 2, vec![], None, None, true, false);

        let input = "brightness = 75\ncolor = \"#ff0000\"\nmode = \"breathing\"";
        iface.set_keyboard_state(input).unwrap();

        let state_toml = iface.get_keyboard_state().unwrap();
        let state: KeyboardState = toml::from_str(&state_toml).unwrap();
        assert_eq!(state.brightness, 75);
        assert_eq!(state.color, "#ff0000");
        assert_eq!(state.mode, "breathing");
    }

    #[test]
    fn set_keyboard_state_forwards_color_and_mode_to_hardware() {
        use crate::hid::{KeyboardLed, Rgb};
        use std::sync::{Arc, Mutex};

        /// Recorded calls from the mock keyboard.
        #[derive(Default)]
        struct Calls {
            brightness: Option<u8>,
            color: Option<(u8, Rgb)>,
            mode: Option<String>,
            flushed: bool,
            turn_on_count: usize,
            turn_off_count: usize,
        }

        struct MockKb(Arc<Mutex<Calls>>);
        impl KeyboardLed for MockKb {
            fn set_brightness(&mut self, b: u8) -> std::io::Result<()> {
                self.0.lock().unwrap().brightness = Some(b);
                Ok(())
            }
            fn set_color(&mut self, zone: u8, color: Rgb) -> std::io::Result<()> {
                self.0.lock().unwrap().color = Some((zone, color));
                Ok(())
            }
            fn set_mode(&mut self, mode: &str) -> std::io::Result<()> {
                self.0.lock().unwrap().mode = Some(mode.to_string());
                Ok(())
            }
            fn zone_count(&self) -> u8 {
                1
            }
            fn turn_off(&mut self) -> std::io::Result<()> {
                self.0.lock().unwrap().turn_off_count += 1;
                Ok(())
            }
            fn turn_on(&mut self) -> std::io::Result<()> {
                self.0.lock().unwrap().turn_on_count += 1;
                Ok(())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                self.0.lock().unwrap().flushed = true;
                Ok(())
            }
            fn device_type(&self) -> &str {
                "mock"
            }
            fn available_modes(&self) -> Vec<String> {
                vec!["static".into()]
            }
        }

        let calls = Arc::new(Mutex::new(Calls::default()));
        let kb: SharedKeyboard = Arc::new(Mutex::new(Box::new(MockKb(calls.clone()))));
        let device = make_test_device();
        let iface = SettingsInterface::new(&device, true, 2, vec![kb], None, None, true, false);

        // Verify keyboard_modes comes from hardware.
        let caps_toml = iface.get_capabilities().unwrap();
        let caps: CapabilitiesResponse = toml::from_str(&caps_toml).unwrap();
        assert_eq!(caps.keyboard_modes, vec!["static"]);

        let input = "brightness = 50\ncolor = \"#ff8000\"\nmode = \"Static\"";
        iface.set_keyboard_state(input).unwrap();

        let c = calls.lock().unwrap();
        // Brightness: 50% of 255 = 127
        assert_eq!(c.brightness, Some(127));
        // Color: #ff8000 → RGB(255, 128, 0)
        let (zone, color) = c.color.unwrap();
        assert_eq!(zone, 0);
        assert_eq!(color, Rgb::new(255, 128, 0));
        // Mode should be lowercased
        assert_eq!(c.mode.as_deref(), Some("static"));
        assert_eq!(c.turn_on_count, 1);
        assert_eq!(c.turn_off_count, 0);
        assert!(c.flushed);
    }

    #[test]
    fn set_keyboard_state_zero_turns_off_hardware() {
        use crate::hid::{KeyboardLed, Rgb};
        use std::sync::{Arc, Mutex};

        #[derive(Default)]
        struct Calls {
            turn_on_count: usize,
            turn_off_count: usize,
            flushed: bool,
        }

        struct MockKb(Arc<Mutex<Calls>>);
        impl KeyboardLed for MockKb {
            fn set_brightness(&mut self, _b: u8) -> std::io::Result<()> {
                Ok(())
            }
            fn set_color(&mut self, _zone: u8, _color: Rgb) -> std::io::Result<()> {
                Ok(())
            }
            fn set_mode(&mut self, _mode: &str) -> std::io::Result<()> {
                Ok(())
            }
            fn zone_count(&self) -> u8 {
                1
            }
            fn turn_off(&mut self) -> std::io::Result<()> {
                self.0.lock().unwrap().turn_off_count += 1;
                Ok(())
            }
            fn turn_on(&mut self) -> std::io::Result<()> {
                self.0.lock().unwrap().turn_on_count += 1;
                Ok(())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                self.0.lock().unwrap().flushed = true;
                Ok(())
            }
            fn device_type(&self) -> &str {
                "mock"
            }
            fn available_modes(&self) -> Vec<String> {
                vec!["static".into()]
            }
        }

        let calls = Arc::new(Mutex::new(Calls::default()));
        let kb: SharedKeyboard = Arc::new(Mutex::new(Box::new(MockKb(calls.clone()))));
        let device = make_test_device();
        let iface = SettingsInterface::new(&device, true, 2, vec![kb], None, None, true, false);

        let input = "brightness = 0\ncolor = \"#ffffff\"\nmode = \"static\"";
        iface.set_keyboard_state(input).unwrap();

        let c = calls.lock().unwrap();
        assert_eq!(c.turn_off_count, 1);
        assert_eq!(c.turn_on_count, 0);
        assert!(c.flushed);
    }

    #[test]
    fn set_keyboard_state_propagates_hardware_errors() {
        use crate::hid::{KeyboardLed, Rgb};
        use std::sync::{Arc, Mutex};

        struct FailingKb;
        impl KeyboardLed for FailingKb {
            fn set_brightness(&mut self, _b: u8) -> std::io::Result<()> {
                Err(std::io::Error::other("ec write failed"))
            }
            fn set_color(&mut self, _zone: u8, _color: Rgb) -> std::io::Result<()> {
                Ok(())
            }
            fn set_mode(&mut self, _mode: &str) -> std::io::Result<()> {
                Ok(())
            }
            fn zone_count(&self) -> u8 {
                1
            }
            fn turn_off(&mut self) -> std::io::Result<()> {
                Ok(())
            }
            fn turn_on(&mut self) -> std::io::Result<()> {
                Ok(())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
            fn device_type(&self) -> &str {
                "failing"
            }
            fn available_modes(&self) -> Vec<String> {
                vec!["static".into()]
            }
        }

        let kb: SharedKeyboard = Arc::new(Mutex::new(Box::new(FailingKb)));
        let device = make_test_device();
        let iface = SettingsInterface::new(&device, true, 2, vec![kb], None, None, true, false);

        let input = "brightness = 50\ncolor = \"#ffffff\"\nmode = \"static\"";
        let err = iface
            .set_keyboard_state(input)
            .expect_err("must propagate hw failure");
        let msg = err.to_string();
        assert!(
            msg.contains("set_brightness failed"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn parse_hex_color_valid() {
        assert_eq!(parse_hex_color("#ff8000"), Rgb::new(255, 128, 0));
        assert_eq!(parse_hex_color("#000000"), Rgb::new(0, 0, 0));
        assert_eq!(parse_hex_color("aabbcc"), Rgb::new(0xaa, 0xbb, 0xcc));
    }

    #[test]
    fn parse_hex_color_invalid_returns_white() {
        assert_eq!(parse_hex_color(""), Rgb::WHITE);
        assert_eq!(parse_hex_color("#ff"), Rgb::WHITE);
    }

    #[test]
    fn power_settings_returns_defaults_without_governor() {
        let device = make_test_device();
        let iface = SettingsInterface::new(&device, true, 2, vec![], None, None, true, false);

        let power_toml = iface.get_power_settings().unwrap();
        let settings: PowerSettings = toml::from_str(&power_toml).unwrap();
        assert_eq!(settings.governor, "");
        assert!(!settings.no_turbo);
    }

    #[test]
    fn power_settings_with_governor() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        // Setup fake CPU sysfs
        let cpufreq = base.join("cpu0/cpufreq");
        std::fs::create_dir_all(&cpufreq).unwrap();
        std::fs::write(cpufreq.join("scaling_governor"), "powersave\n").unwrap();
        std::fs::write(
            cpufreq.join("scaling_available_governors"),
            "performance powersave\n",
        )
        .unwrap();
        std::fs::write(
            cpufreq.join("energy_performance_preference"),
            "balance_performance\n",
        )
        .unwrap();
        // Setup turbo
        let intel_pstate = base.join("intel_pstate");
        std::fs::create_dir_all(&intel_pstate).unwrap();
        std::fs::write(intel_pstate.join("no_turbo"), "0\n").unwrap();

        let gov = Arc::new(CpuGovernor::with_path(base));
        let device = make_test_device();
        let iface = SettingsInterface::new(&device, true, 2, vec![], Some(gov), None, true, false);

        let power_toml = iface.get_power_settings().unwrap();
        let settings: PowerSettings = toml::from_str(&power_toml).unwrap();
        assert_eq!(settings.governor, "powersave");
        assert_eq!(settings.epp, "balance_performance");
        assert!(!settings.no_turbo);
    }
}
