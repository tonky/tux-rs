use serde::Deserialize;

use crate::device::*;
use crate::platform::Platform;
use crate::registers::*;

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
    Nb05 { num_fans: u8, fanctl_onereg: bool },
    Nb04 { sysfs_base: String },
    Uniwill { sysfs_base: String },
    Clevo { sysfs_base: String, max_fans: u8 },
    Tuxi { sysfs_base: String },
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
            CustomPlatformRegisters::Nb05 {
                num_fans,
                fanctl_onereg,
            } => PlatformRegisters::Nb05(Nb05Registers {
                num_fans,
                fanctl_onereg,
            }),
            CustomPlatformRegisters::Nb04 { sysfs_base } => {
                PlatformRegisters::Nb04(Nb04Registers {
                    sysfs_base: Box::leak(sysfs_base.into_boxed_str()),
                })
            }
            CustomPlatformRegisters::Uniwill { sysfs_base } => {
                PlatformRegisters::Uniwill(UniwillRegisters {
                    sysfs_base: Box::leak(sysfs_base.into_boxed_str()),
                })
            }
            CustomPlatformRegisters::Clevo {
                sysfs_base,
                max_fans,
            } => PlatformRegisters::Clevo(ClevoRegisters {
                sysfs_base: Box::leak(sysfs_base.into_boxed_str()),
                max_fans,
            }),
            CustomPlatformRegisters::Tuxi { sysfs_base } => {
                PlatformRegisters::Tuxi(TuxiRegisters {
                    sysfs_base: Box::leak(sysfs_base.into_boxed_str()),
                })
            }
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
