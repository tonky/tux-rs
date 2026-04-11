/// Platform-specific register maps and sysfs paths.
///
/// Each variant contains the hardware-specific addressing information
/// needed by the corresponding kernel shim.
#[derive(Debug, Clone)]
pub enum PlatformRegisters {
    Nb05(Nb05Registers),
    Nb04(Nb04Registers),
    Uniwill(UniwillRegisters),
    Clevo(ClevoRegisters),
    Tuxi(TuxiRegisters),
}

/// NB05 platform registers (EC port I/O via tuxedo-ec shim).
#[derive(Debug, Clone)]
pub struct Nb05Registers {
    /// Number of fans on this model.
    pub num_fans: u8,
    /// InfinityFlex uses single-register fan control; Pulse uses multi-register.
    pub fanctl_onereg: bool,
}

/// Uniwill platform registers (ACPI EC via tuxedo-uniwill shim).
#[derive(Debug, Clone)]
pub struct UniwillRegisters {
    /// sysfs base path for the tuxedo-uniwill platform device.
    pub sysfs_base: &'static str,
}

/// Clevo platform registers (WMI + ACPI DSM via tuxedo-clevo shim).
#[derive(Debug, Clone)]
pub struct ClevoRegisters {
    /// sysfs base path for the tuxedo-clevo platform device.
    pub sysfs_base: &'static str,
    /// Maximum number of fans supported.
    pub max_fans: u8,
}

/// NB04 platform registers (WMI AB/BS via tuxedo-nb04 shim).
#[derive(Debug, Clone)]
pub struct Nb04Registers {
    /// sysfs base path for the tuxedo-nb04 platform device.
    pub sysfs_base: &'static str,
}

/// Tuxi platform registers (ACPI TFAN via tuxedo-tuxi shim).
#[derive(Debug, Clone)]
pub struct TuxiRegisters {
    /// sysfs base path for the tuxedo-tuxi platform device.
    pub sysfs_base: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_registers_variant_access() {
        let nb05 = PlatformRegisters::Nb05(Nb05Registers {
            num_fans: 2,
            fanctl_onereg: true,
        });

        if let PlatformRegisters::Nb05(regs) = &nb05 {
            assert_eq!(regs.num_fans, 2);
            assert!(regs.fanctl_onereg);
        } else {
            panic!("Expected Nb05 variant");
        }
    }

    #[test]
    fn all_register_variants_constructible() {
        let variants: Vec<PlatformRegisters> = vec![
            PlatformRegisters::Nb05(Nb05Registers {
                num_fans: 1,
                fanctl_onereg: false,
            }),
            PlatformRegisters::Nb04(Nb04Registers {
                sysfs_base: "/sys/devices/platform/tuxedo-nb04",
            }),
            PlatformRegisters::Uniwill(UniwillRegisters {
                sysfs_base: "/sys/devices/platform/tuxedo-uniwill",
            }),
            PlatformRegisters::Clevo(ClevoRegisters {
                sysfs_base: "/sys/devices/platform/tuxedo-clevo",
                max_fans: 3,
            }),
            PlatformRegisters::Tuxi(TuxiRegisters {
                sysfs_base: "/sys/devices/platform/tuxedo-tuxi",
            }),
        ];

        assert_eq!(variants.len(), 5);
    }
}
