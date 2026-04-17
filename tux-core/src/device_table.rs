use std::sync::RwLock;

use crate::device::*;
use crate::platform::Platform;
use crate::registers::PlatformRegisters;

/// A dynamic list of devices loaded from custom overriding configuration.
pub static CUSTOM_DEVICES: RwLock<Vec<&'static DeviceDescriptor>> = RwLock::new(Vec::new());

/// Registers a dynamically loaded device descriptor (e.g. from TOML overrides).
pub fn register_custom_device(device: &'static DeviceDescriptor) {
    if let Ok(mut list) = CUSTOM_DEVICES.write() {
        list.push(device);
    }
}

/// Static table of all known TUXEDO laptop models.
///
/// Each entry maps a DMI product SKU to a full DeviceDescriptor.
/// Adding a new laptop model = adding an entry here.
pub static DEVICE_TABLE: &[DeviceDescriptor] = &[
    // ─── NB05 Platform ────────────────────────────────────────────
    DeviceDescriptor {
        name: "TUXEDO Pulse 14 Gen3",
        product_sku: "PULSE1403",
        platform: Platform::Nb05,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 184,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: false,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::None,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Nb05,
    },
    DeviceDescriptor {
        name: "TUXEDO Pulse 14 Gen4",
        product_sku: "PULSE1404",
        platform: Platform::Nb05,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 184,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: false,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::None,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Nb05,
    },
    DeviceDescriptor {
        name: "TUXEDO Pulse 15 Gen2",
        product_sku: "PULSE1502",
        platform: Platform::Nb05,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 184,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: false,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::None,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Nb05,
    },
    DeviceDescriptor {
        name: "TUXEDO InfinityFlex 14 Gen1",
        product_sku: "IFLX14I01",
        platform: Platform::Nb05,
        fans: FanCapability {
            count: 1,
            control: FanControlType::Direct,
            pwm_scale: 184,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: false,
            fan_rpm: &[true],
        },
        charging: ChargingCapability::None,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Nb05,
    },
    // ─── Uniwill Platform ─────────────────────────────────────────
    // Stellaris Gen3
    DeviceDescriptor {
        name: "TUXEDO Stellaris 15 Gen3 Intel",
        product_sku: "STELLARIS1XI03",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: true,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::Nb02Nvidia,
        registers: PlatformRegisters::Uniwill,
    },
    DeviceDescriptor {
        name: "TUXEDO Stellaris 15 Gen3 AMD",
        product_sku: "STELLARIS1XA03",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: true,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::Nb02Nvidia,
        registers: PlatformRegisters::Uniwill,
    },
    // Stellaris Gen4
    DeviceDescriptor {
        name: "TUXEDO Stellaris 15 Gen4 Intel",
        product_sku: "STELLARIS1XI04",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: true,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::Nb02Nvidia,
        registers: PlatformRegisters::Uniwill,
    },
    DeviceDescriptor {
        name: "TUXEDO Stellaris/Polaris Gen4 AMD",
        product_sku: "STEPOL1XA04",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: true,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::Nb02Nvidia,
        registers: PlatformRegisters::Uniwill,
    },
    // Stellaris Gen5
    DeviceDescriptor {
        name: "TUXEDO Stellaris 15 Gen5 Intel",
        product_sku: "STELLARIS1XI05",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: true,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::Nb02Nvidia,
        registers: PlatformRegisters::Uniwill,
    },
    DeviceDescriptor {
        name: "TUXEDO Stellaris 15 Gen5 AMD",
        product_sku: "STELLARIS1XA05",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: true,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::Nb02Nvidia,
        registers: PlatformRegisters::Uniwill,
    },
    // Stellaris Gen6
    DeviceDescriptor {
        name: "TUXEDO Stellaris 16 Gen6 Intel",
        product_sku: "STELLARIS16I06",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: true,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::Nb02Nvidia,
        registers: PlatformRegisters::Uniwill,
    },
    DeviceDescriptor {
        name: "TUXEDO Stellaris Slim 15 Gen6 Intel",
        product_sku: "STELLSL15I06",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: true,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::Nb02Nvidia,
        registers: PlatformRegisters::Uniwill,
    },
    DeviceDescriptor {
        name: "TUXEDO Stellaris Slim 15 Gen6 AMD",
        product_sku: "STELLSL15A06",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: true,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::Nb02Nvidia,
        registers: PlatformRegisters::Uniwill,
    },
    // Stellaris Gen7
    DeviceDescriptor {
        name: "TUXEDO Stellaris 17 Gen6 Intel",
        product_sku: "STELLARIS17I06",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: true,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::Nb02Nvidia,
        registers: PlatformRegisters::Uniwill,
    },
    DeviceDescriptor {
        name: "TUXEDO Stellaris 16 Gen7 Intel",
        product_sku: "STELLARIS16I07",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: true,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::Nb02Nvidia,
        registers: PlatformRegisters::Uniwill,
    },
    DeviceDescriptor {
        name: "TUXEDO Stellaris 16 Gen7 AMD",
        product_sku: "STELLARIS16A07",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: true,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::Nb02Nvidia,
        registers: PlatformRegisters::Uniwill,
    },
    // Polaris
    DeviceDescriptor {
        name: "TUXEDO Polaris 15 Gen2 Intel",
        product_sku: "POLARIS1XI02",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::Rgb3Zone,
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: true,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::Nb02Nvidia,
        registers: PlatformRegisters::Uniwill,
    },
    DeviceDescriptor {
        name: "TUXEDO Polaris 15 Gen2 AMD",
        product_sku: "POLARIS1XA02",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::Rgb3Zone,
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: true,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::Nb02Nvidia,
        registers: PlatformRegisters::Uniwill,
    },
    DeviceDescriptor {
        name: "TUXEDO Polaris 15 Gen3 Intel",
        product_sku: "POLARIS1XI03",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: true,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::Nb02Nvidia,
        registers: PlatformRegisters::Uniwill,
    },
    DeviceDescriptor {
        name: "TUXEDO Polaris 15 Gen3 AMD",
        product_sku: "POLARIS1XA03",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: true,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::Nb02Nvidia,
        registers: PlatformRegisters::Uniwill,
    },
    DeviceDescriptor {
        name: "TUXEDO Polaris 15 Gen5 AMD",
        product_sku: "POLARIS1XA05",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: true,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::Nb02Nvidia,
        registers: PlatformRegisters::Uniwill,
    },
    // InfinityBook Pro (IBP) series
    DeviceDescriptor {
        name: "TUXEDO InfinityBook Pro Gen7 MK1",
        product_sku: "IBP1XI07MK1",
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
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Uniwill,
    },
    DeviceDescriptor {
        name: "TUXEDO InfinityBook Pro Gen7 MK2",
        product_sku: "IBP1XI07MK2",
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
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Uniwill,
    },
    DeviceDescriptor {
        name: "TUXEDO InfinityBook Pro Gen8 MK1",
        product_sku: "IBP1XI08MK1",
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
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Uniwill,
    },
    DeviceDescriptor {
        name: "TUXEDO InfinityBook Pro Gen8 MK2",
        product_sku: "IBP1XI08MK2",
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
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Uniwill,
    },
    DeviceDescriptor {
        name: "TUXEDO InfinityBook Pro 14 Gen8 MK2",
        product_sku: "IBP14I08MK2",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: false,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Uniwill,
    },
    DeviceDescriptor {
        name: "TUXEDO InfinityBook Pro 16 Gen8 MK2",
        product_sku: "IBP16I08MK2",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: false,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Uniwill,
    },
    // OMNIA
    DeviceDescriptor {
        name: "TUXEDO OMNIA Gen8 MK2",
        product_sku: "OMNIA08IMK2",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite8291),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: false,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Uniwill,
    },
    // InfinityBook S series
    DeviceDescriptor {
        name: "TUXEDO InfinityBook S 14 Gen8",
        product_sku: "IBS14I08",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 1,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::White,
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: false,
            fan_rpm: &[true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Uniwill,
    },
    DeviceDescriptor {
        name: "TUXEDO InfinityBook S 15 Gen8",
        product_sku: "IBS15I08",
        platform: Platform::Uniwill,
        fans: FanCapability {
            count: 1,
            control: FanControlType::Direct,
            pwm_scale: 200,
        },
        keyboard: KeyboardType::White,
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: false,
            fan_rpm: &[true],
        },
        charging: ChargingCapability::EcProfilePriority,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Uniwill,
    },
    // InfinityBook Pro Gen6
    DeviceDescriptor {
        name: "TUXEDO InfinityBook Pro 14 Gen7",
        product_sku: "IBP14I07",
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
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Uniwill,
    },
    DeviceDescriptor {
        name: "TUXEDO InfinityBook Pro 15 Gen7",
        product_sku: "IBP15I07",
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
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Uniwill,
    },
    // ─── Clevo Platform ───────────────────────────────────────────
    DeviceDescriptor {
        name: "TUXEDO Aura 14 Gen3",
        product_sku: "AURA14GEN3",
        platform: Platform::Clevo,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 255,
        },
        keyboard: KeyboardType::Rgb3Zone,
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: false,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::Flexicharger,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Clevo,
    },
    DeviceDescriptor {
        name: "TUXEDO Aura 15 Gen3",
        product_sku: "AURA15GEN3",
        platform: Platform::Clevo,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 255,
        },
        keyboard: KeyboardType::Rgb3Zone,
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: false,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::Flexicharger,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Clevo,
    },
    // Aura 14 Gen4 / Aura 15 Gen4: hardware reports a combined SKU string.
    // The individual "AURA14GEN4" / "AURA15GEN4" strings are never seen on
    // real devices; only the combined form below is matched.
    DeviceDescriptor {
        name: "TUXEDO Aura 14/15 Gen4",
        product_sku: "AURA14GEN4 / AURA15GEN4",
        platform: Platform::Clevo,
        fans: FanCapability {
            count: 2,
            control: FanControlType::Direct,
            pwm_scale: 255,
        },
        keyboard: KeyboardType::Rgb3Zone,
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: false,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::Flexicharger,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Clevo,
    },
    // ─── NB04 Platform ────────────────────────────────────────────
    DeviceDescriptor {
        name: "TUXEDO Sirius 16 Gen1",
        product_sku: "SIRIUS1601",
        platform: Platform::Nb04,
        fans: FanCapability {
            count: 2,
            control: FanControlType::ProfileOnly,
            pwm_scale: 0,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite829x),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: true,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::None,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Nb04,
    },
    DeviceDescriptor {
        name: "TUXEDO Sirius 16 Gen2",
        product_sku: "SIRIUS1602",
        platform: Platform::Nb04,
        fans: FanCapability {
            count: 2,
            control: FanControlType::ProfileOnly,
            pwm_scale: 0,
        },
        keyboard: KeyboardType::IteHid(IteModel::Ite829x),
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: true,
            fan_rpm: &[true, true],
        },
        charging: ChargingCapability::None,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Nb04,
    },
    // ─── Tuxi Platform ────────────────────────────────────────────
    DeviceDescriptor {
        name: "TUXEDO Aura 15 Gen1 (Tuxi)",
        product_sku: "AURA15GEN1T",
        platform: Platform::Tuxi,
        fans: FanCapability {
            count: 1,
            control: FanControlType::Direct,
            pwm_scale: 255,
        },
        keyboard: KeyboardType::White,
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: false,
            fan_rpm: &[true],
        },
        charging: ChargingCapability::None,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Tuxi,
    },
    DeviceDescriptor {
        name: "TUXEDO Aura 15 Gen2 (Tuxi)",
        product_sku: "AURA15GEN2T",
        platform: Platform::Tuxi,
        fans: FanCapability {
            count: 1,
            control: FanControlType::Direct,
            pwm_scale: 255,
        },
        keyboard: KeyboardType::White,
        sensors: SensorSet {
            cpu_temp: true,
            gpu_temp: false,
            fan_rpm: &[true],
        },
        charging: ChargingCapability::None,
        tdp: None,
        tdp_source: TdpSource::None,
        gpu_power: GpuPowerCapability::None,
        registers: PlatformRegisters::Tuxi,
    },
];

/// Fallback descriptors for each platform (conservative capabilities).
static FALLBACK_NB05: DeviceDescriptor = DeviceDescriptor {
    name: "Unknown NB05 Device",
    product_sku: "",
    platform: Platform::Nb05,
    fans: FanCapability {
        count: 1,
        control: FanControlType::Direct,
        pwm_scale: 184,
    },
    keyboard: KeyboardType::None,
    sensors: SensorSet {
        cpu_temp: true,
        gpu_temp: false,
        fan_rpm: &[true],
    },
    charging: ChargingCapability::None,
    tdp: None,
    tdp_source: TdpSource::None,
    gpu_power: GpuPowerCapability::None,
    registers: PlatformRegisters::Nb05,
};

static FALLBACK_UNIWILL: DeviceDescriptor = DeviceDescriptor {
    name: "Unknown Uniwill Device",
    product_sku: "",
    platform: Platform::Uniwill,
    fans: FanCapability {
        count: 1,
        control: FanControlType::Direct,
        pwm_scale: 200,
    },
    keyboard: KeyboardType::None,
    sensors: SensorSet {
        cpu_temp: true,
        gpu_temp: false,
        fan_rpm: &[true],
    },
    charging: ChargingCapability::None,
    tdp: None,
    tdp_source: TdpSource::None,
    gpu_power: GpuPowerCapability::None,
    registers: PlatformRegisters::Uniwill,
};

static FALLBACK_CLEVO: DeviceDescriptor = DeviceDescriptor {
    name: "Unknown Clevo Device",
    product_sku: "",
    platform: Platform::Clevo,
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
    tdp_source: TdpSource::None,
    gpu_power: GpuPowerCapability::None,
    registers: PlatformRegisters::Clevo,
};

static FALLBACK_NB04: DeviceDescriptor = DeviceDescriptor {
    name: "Unknown NB04 Device",
    product_sku: "",
    platform: Platform::Nb04,
    fans: FanCapability {
        count: 1,
        control: FanControlType::ProfileOnly,
        pwm_scale: 0,
    },
    keyboard: KeyboardType::None,
    sensors: SensorSet {
        cpu_temp: true,
        gpu_temp: false,
        fan_rpm: &[true],
    },
    charging: ChargingCapability::None,
    tdp: None,
    tdp_source: TdpSource::None,
    gpu_power: GpuPowerCapability::None,
    registers: PlatformRegisters::Nb04,
};

static FALLBACK_TUXI: DeviceDescriptor = DeviceDescriptor {
    name: "Unknown Tuxi Device",
    product_sku: "",
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
    tdp_source: TdpSource::None,
    gpu_power: GpuPowerCapability::None,
    registers: PlatformRegisters::Tuxi,
};

/// Find device by exact DMI product SKU match.
pub fn lookup_by_sku(sku: &str) -> Option<&'static DeviceDescriptor> {
    if let Some(d) = CUSTOM_DEVICES
        .read()
        .ok()
        .and_then(|list| list.iter().find(|d| d.product_sku == sku).copied())
    {
        return Some(d);
    }
    DEVICE_TABLE.iter().find(|d| d.product_sku == sku)
}

/// Find all devices for a given platform.
pub fn devices_for_platform(platform: Platform) -> Vec<&'static DeviceDescriptor> {
    let mut devices = Vec::new();
    if let Ok(list) = CUSTOM_DEVICES.read() {
        devices.extend(list.iter().filter(|d| d.platform == platform).copied());
    }
    devices.extend(DEVICE_TABLE.iter().filter(|d| d.platform == platform));
    devices
}

/// Get the fallback descriptor for a platform (conservative capabilities).
pub fn fallback_for_platform(platform: Platform) -> &'static DeviceDescriptor {
    match platform {
        Platform::Nb05 => &FALLBACK_NB05,
        Platform::Nb04 => &FALLBACK_NB04,
        Platform::Uniwill => &FALLBACK_UNIWILL,
        Platform::Clevo => &FALLBACK_CLEVO,
        Platform::Tuxi => &FALLBACK_TUXI,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_skus_unique() {
        let mut seen = HashSet::new();
        for device in DEVICE_TABLE {
            assert!(
                seen.insert(device.product_sku),
                "Duplicate SKU: {}",
                device.product_sku
            );
        }
    }

    #[test]
    fn every_platform_has_at_least_one_device() {
        for platform in [
            Platform::Nb05,
            Platform::Nb04,
            Platform::Uniwill,
            Platform::Clevo,
            Platform::Tuxi,
        ] {
            let devices = devices_for_platform(platform);
            assert!(
                !devices.is_empty(),
                "No devices for platform {:?}",
                platform
            );
        }
    }

    #[test]
    fn lookup_known_sku() {
        let device = lookup_by_sku("PULSE1403").expect("PULSE1403 should exist");
        assert_eq!(device.name, "TUXEDO Pulse 14 Gen3");
        assert_eq!(device.platform, Platform::Nb05);
        assert_eq!(device.fans.count, 2);
    }

    #[test]
    fn lookup_unknown_sku_returns_none() {
        assert!(lookup_by_sku("NONEXISTENT").is_none());
        assert!(lookup_by_sku("").is_none());
    }

    #[test]
    fn devices_for_platform_correct_subset() {
        let nb05_devices = devices_for_platform(Platform::Nb05);
        for device in &nb05_devices {
            assert_eq!(device.platform, Platform::Nb05);
        }

        let uniwill_devices = devices_for_platform(Platform::Uniwill);
        for device in &uniwill_devices {
            assert_eq!(device.platform, Platform::Uniwill);
        }
    }

    #[test]
    fn fallback_for_each_platform() {
        for platform in [
            Platform::Nb05,
            Platform::Nb04,
            Platform::Uniwill,
            Platform::Clevo,
            Platform::Tuxi,
        ] {
            let fb = fallback_for_platform(platform);
            assert_eq!(fb.platform, platform);
            assert!(fb.product_sku.is_empty());
            assert!(fb.fans.count >= 1);
        }
    }

    #[test]
    fn all_entries_have_nonempty_name_and_sku() {
        for device in DEVICE_TABLE {
            assert!(!device.name.is_empty(), "Empty name in device table");
            assert!(
                !device.product_sku.is_empty(),
                "Empty SKU for device: {}",
                device.name
            );
        }
    }

    #[test]
    fn table_has_minimum_entries() {
        assert!(
            DEVICE_TABLE.len() >= 38,
            "Device table has {} entries, expected >= 38",
            DEVICE_TABLE.len()
        );
    }

    #[test]
    fn lookup_each_platform_specific_device() {
        // NB05
        assert!(lookup_by_sku("IFLX14I01").is_some());
        // Uniwill
        assert!(lookup_by_sku("STELLARIS1XI05").is_some());
        // Clevo
        assert!(lookup_by_sku("AURA14GEN3").is_some());
        // NB04
        assert!(lookup_by_sku("SIRIUS1601").is_some());
        // Tuxi
        assert!(lookup_by_sku("AURA15GEN1T").is_some());
    }

    #[test]
    fn nb05_infinityflex_has_one_fan() {
        let device = lookup_by_sku("IFLX14I01").unwrap();
        assert_eq!(device.fans.count, 1);
        assert_eq!(device.platform, Platform::Nb05);
        assert_eq!(device.registers, PlatformRegisters::Nb05);
    }

    #[test]
    fn nb04_uses_profile_only_control() {
        let nb04_devices = devices_for_platform(Platform::Nb04);
        for device in &nb04_devices {
            assert_eq!(device.fans.control, FanControlType::ProfileOnly);
        }
    }

    #[test]
    fn gpu_power_only_on_uniwill() {
        for device in DEVICE_TABLE {
            if device.gpu_power != GpuPowerCapability::None {
                assert_eq!(
                    device.platform,
                    Platform::Uniwill,
                    "Non-Uniwill device '{}' has GPU power control",
                    device.name
                );
            }
        }
    }

    #[test]
    fn gpu_power_matches_gpu_temp_on_uniwill() {
        let uniwill_devices = devices_for_platform(Platform::Uniwill);
        for device in &uniwill_devices {
            if device.sensors.gpu_temp {
                assert_eq!(
                    device.gpu_power,
                    GpuPowerCapability::Nb02Nvidia,
                    "Uniwill device '{}' has gpu_temp but no GPU power",
                    device.name
                );
            }
        }
    }
}
