use serde::Deserialize;

use crate::device::*;
use crate::platform::Platform;
use crate::registers::PlatformRegisters;

/// A dynamic version of `DeviceDescriptor` that owns its strings.
/// Deserialized from `custom_devices.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomDeviceDescriptor {
    pub name: String,
    pub product_sku: String,
    pub platform: Platform,
    pub fans: FanCapability,
    pub keyboard: KeyboardType,
    pub sensors: CustomSensorSet,
    pub charging: ChargingCapability,
    pub tdp: Option<TdpBounds>,
    pub gpu_power: GpuPowerCapability,
    pub registers: CustomPlatformRegisters,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomSensorSet {
    pub cpu_temp: bool,
    pub gpu_temp: bool,
    pub fan_rpm: Vec<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum CustomPlatformRegisters {
    Nb05,
    Nb04,
    Uniwill,
    Clevo,
    Tuxi,
}

impl CustomDeviceDescriptor {
    /// Leaks the custom device descriptor, giving it a `'static` lifetime.
    /// This is safe and necessary because the core hardware model relies on
    /// static descriptions that are loaded once at startup and live forever.
    pub fn leak(self) -> &'static DeviceDescriptor {
        let fan_rpm_slice = self.sensors.fan_rpm.into_boxed_slice();
        let sensors = SensorSet {
            cpu_temp: self.sensors.cpu_temp,
            gpu_temp: self.sensors.gpu_temp,
            fan_rpm: Box::leak(fan_rpm_slice),
        };

        let registers = match self.registers {
            CustomPlatformRegisters::Nb05 => PlatformRegisters::Nb05,
            CustomPlatformRegisters::Nb04 => PlatformRegisters::Nb04,
            CustomPlatformRegisters::Uniwill => PlatformRegisters::Uniwill,
            CustomPlatformRegisters::Clevo => PlatformRegisters::Clevo,
            CustomPlatformRegisters::Tuxi => PlatformRegisters::Tuxi,
        };

        let desc = DeviceDescriptor {
            name: Box::leak(self.name.into_boxed_str()),
            product_sku: Box::leak(self.product_sku.into_boxed_str()),
            platform: self.platform,
            fans: self.fans,
            keyboard: self.keyboard,
            sensors,
            charging: self.charging,
            tdp: self.tdp,
            gpu_power: self.gpu_power,
            registers,
        };

        Box::leak(Box::new(desc))
    }
}
