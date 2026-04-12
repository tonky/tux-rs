/// Platform discriminant stored alongside a device descriptor.
///
/// All hardware-specific addressing has moved into the `td_*` backends which
/// use hard-coded tuxedo-drivers sysfs paths. This enum is kept for
/// pattern-matching and documentation purposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformRegisters {
    Nb05,
    Nb04,
    Uniwill,
    Clevo,
    Tuxi,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_registers_all_variants_constructible() {
        let variants = [
            PlatformRegisters::Nb05,
            PlatformRegisters::Nb04,
            PlatformRegisters::Uniwill,
            PlatformRegisters::Clevo,
            PlatformRegisters::Tuxi,
        ];
        assert_eq!(variants.len(), 5);
    }

    #[test]
    fn platform_registers_equality() {
        assert_eq!(PlatformRegisters::Nb05, PlatformRegisters::Nb05);
        assert_ne!(PlatformRegisters::Nb05, PlatformRegisters::Nb04);
    }
}
