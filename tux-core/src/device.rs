use serde::{Deserialize, Serialize};

use crate::platform::Platform;
use crate::registers::PlatformRegisters;

/// Complete hardware description of a TUXEDO laptop model.
///
/// Each supported device has a static `DeviceDescriptor` entry in the device table.
/// Adding support for a new laptop means adding an entry — no new code paths.
#[derive(Debug, Clone)]
pub struct DeviceDescriptor {
    pub name: &'static str,
    pub product_sku: &'static str,
    pub platform: Platform,
    pub fans: FanCapability,
    pub keyboard: KeyboardType,
    pub sensors: SensorSet,
    pub charging: ChargingCapability,
    pub tdp: Option<TdpBounds>,
    pub gpu_power: GpuPowerCapability,
    pub registers: PlatformRegisters,
}

/// Fan hardware capabilities for a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FanCapability {
    /// Number of fans (0–3).
    pub count: u8,
    /// How the daemon controls fan speed.
    pub control: FanControlType,
    /// Maximum PWM value (200 for Uniwill, 255 for others).
    pub pwm_scale: u8,
}

/// How fan speed is controlled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FanControlType {
    /// Daemon can set PWM directly via sysfs.
    Direct,
    /// Fans managed by firmware power profile (NB04).
    ProfileOnly,
    /// No fan control available (passive cooling).
    None,
}

/// Keyboard backlight type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyboardType {
    /// No keyboard backlight.
    None,
    /// Single-color (white) backlight, brightness only.
    White,
    /// Discrete brightness levels (e.g., NB05: 3 levels).
    WhiteLevels(u8),
    /// Single-zone RGB.
    Rgb1Zone,
    /// Three-zone RGB.
    Rgb3Zone,
    /// Per-key RGB (kernel-driven).
    RgbPerKey,
    /// USB HID ITE controller — handled in userspace via hidraw.
    IteHid(IteModel),
}

/// ITE keyboard controller model (USB HID).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IteModel {
    /// Per-key 6×21 matrix.
    Ite8291,
    /// Lightbar variants.
    Ite8291Lb,
    /// RGB lightbar.
    Ite8297,
    /// Per-key 6×20 matrix.
    Ite829x,
}

/// Which sensors are available on the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SensorSet {
    pub cpu_temp: bool,
    pub gpu_temp: bool,
    /// Per-fan RPM availability (length matches fan count).
    pub fan_rpm: &'static [bool],
}

/// Charging control capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChargingCapability {
    /// No charging control.
    None,
    /// Clevo ACPI start/end thresholds.
    Flexicharger,
    /// Uniwill EC profile + priority registers.
    EcProfilePriority,
}

/// TDP (Thermal Design Power) bounds per power level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TdpBounds {
    pub pl1_min: u32,
    pub pl1_max: u32,
    pub pl2_min: u32,
    pub pl2_max: u32,
    pub pl4_min: Option<u32>,
    pub pl4_max: Option<u32>,
}

/// GPU power control capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuPowerCapability {
    /// No GPU power control.
    None,
    /// Uniwill NB02 cTGP/Dynamic Boost via kernel sysfs.
    Nb02Nvidia,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registers::PlatformRegisters;
    use serde::{Deserialize, Serialize};

    #[test]
    fn construct_uniwill_descriptor() {
        let desc = DeviceDescriptor {
            name: "TUXEDO InfinityBook Pro 16 Gen8",
            product_sku: "STELLARIS1XI05",
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
            charging: ChargingCapability::EcProfilePriority,
            tdp: None,
            gpu_power: GpuPowerCapability::None,
            registers: PlatformRegisters::Uniwill,
        };

        assert_eq!(desc.platform, Platform::Uniwill);
        assert_eq!(desc.fans.count, 2);
        assert_eq!(desc.fans.pwm_scale, 200);
        assert!(desc.sensors.cpu_temp);
        assert!(!desc.sensors.gpu_temp);
        assert_eq!(desc.sensors.fan_rpm.len(), 2);
    }

    #[test]
    fn construct_nb05_descriptor() {
        let desc = DeviceDescriptor {
            name: "TUXEDO Pulse 14 Gen4",
            product_sku: "NB05DATA",
            platform: Platform::Nb05,
            fans: FanCapability {
                count: 1,
                control: FanControlType::Direct,
                pwm_scale: 255,
            },
            keyboard: KeyboardType::WhiteLevels(3),
            sensors: SensorSet {
                cpu_temp: true,
                gpu_temp: false,
                fan_rpm: &[true],
            },
            charging: ChargingCapability::None,
            tdp: Some(TdpBounds {
                pl1_min: 5,
                pl1_max: 28,
                pl2_min: 10,
                pl2_max: 40,
                pl4_min: None,
                pl4_max: None,
            }),
            gpu_power: GpuPowerCapability::None,
            registers: PlatformRegisters::Nb05,
        };

        assert_eq!(desc.platform, Platform::Nb05);
        assert_eq!(desc.fans.count, 1);
        assert!(desc.tdp.is_some());
        let tdp = desc.tdp.unwrap();
        assert_eq!(tdp.pl1_max, 28);
    }

    #[test]
    fn construct_nb04_descriptor() {
        let desc = DeviceDescriptor {
            name: "TUXEDO Sirius 16 Gen1",
            product_sku: "NB04SKU",
            platform: Platform::Nb04,
            fans: FanCapability {
                count: 2,
                control: FanControlType::ProfileOnly,
                pwm_scale: 0,
            },
            keyboard: KeyboardType::IteHid(IteModel::Ite8291),
            sensors: SensorSet {
                cpu_temp: true,
                gpu_temp: true,
                fan_rpm: &[true, true],
            },
            charging: ChargingCapability::None,
            tdp: None,
            gpu_power: GpuPowerCapability::None,
            registers: PlatformRegisters::Nb04,
        };

        assert_eq!(desc.platform, Platform::Nb04);
        assert_eq!(desc.fans.control, FanControlType::ProfileOnly);
        assert_eq!(desc.keyboard, KeyboardType::IteHid(IteModel::Ite8291));
    }

    #[test]
    fn construct_clevo_descriptor() {
        let desc = DeviceDescriptor {
            name: "TUXEDO Stellaris 17 Gen5",
            product_sku: "CLEVOSKU",
            platform: Platform::Clevo,
            fans: FanCapability {
                count: 3,
                control: FanControlType::Direct,
                pwm_scale: 255,
            },
            keyboard: KeyboardType::RgbPerKey,
            sensors: SensorSet {
                cpu_temp: true,
                gpu_temp: true,
                fan_rpm: &[true, true, true],
            },
            charging: ChargingCapability::Flexicharger,
            tdp: None,
            gpu_power: GpuPowerCapability::None,
            registers: PlatformRegisters::Clevo,
        };

        assert_eq!(desc.fans.count, 3);
        assert_eq!(desc.charging, ChargingCapability::Flexicharger);
        assert_eq!(desc.sensors.fan_rpm.len(), 3);
    }

    #[test]
    fn construct_tuxi_descriptor() {
        let desc = DeviceDescriptor {
            name: "TUXEDO Aura 15 Gen3",
            product_sku: "TUXISKU",
            platform: Platform::Tuxi,
            fans: FanCapability {
                count: 1,
                control: FanControlType::Direct,
                pwm_scale: 255,
            },
            keyboard: KeyboardType::None,
            sensors: SensorSet {
                cpu_temp: true,
                gpu_temp: false,
                fan_rpm: &[true],
            },
            charging: ChargingCapability::None,
            tdp: None,
            gpu_power: GpuPowerCapability::None,
            registers: PlatformRegisters::Tuxi,
        };

        assert_eq!(desc.platform, Platform::Tuxi);
        assert_eq!(desc.keyboard, KeyboardType::None);
    }

    #[test]
    fn fan_control_type_serialize_roundtrip() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct W {
            v: FanControlType,
        }
        for variant in [
            FanControlType::Direct,
            FanControlType::ProfileOnly,
            FanControlType::None,
        ] {
            let w = W { v: variant };
            let s = toml::to_string(&w).unwrap();
            let back: W = toml::from_str(&s).unwrap();
            assert_eq!(w, back);
        }
    }

    #[test]
    fn keyboard_type_serialize_roundtrip() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct W {
            v: KeyboardType,
        }
        for variant in [
            KeyboardType::None,
            KeyboardType::White,
            KeyboardType::WhiteLevels(3),
            KeyboardType::Rgb1Zone,
            KeyboardType::Rgb3Zone,
            KeyboardType::RgbPerKey,
            KeyboardType::IteHid(IteModel::Ite8291),
            KeyboardType::IteHid(IteModel::Ite829x),
        ] {
            let w = W { v: variant };
            let s = toml::to_string(&w).unwrap();
            let back: W = toml::from_str(&s).unwrap();
            assert_eq!(w, back);
        }
    }

    #[test]
    fn charging_capability_serialize_roundtrip() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct W {
            v: ChargingCapability,
        }
        for variant in [
            ChargingCapability::None,
            ChargingCapability::Flexicharger,
            ChargingCapability::EcProfilePriority,
        ] {
            let w = W { v: variant };
            let s = toml::to_string(&w).unwrap();
            let back: W = toml::from_str(&s).unwrap();
            assert_eq!(w, back);
        }
    }
}
