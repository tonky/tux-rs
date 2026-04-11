use serde::{Deserialize, Serialize};
use std::fmt;

/// Hardware platform families for TUXEDO laptops.
///
/// Each platform uses a different hardware access method and kernel shim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Platform {
    /// EC port I/O via tuxedo-ec shim (Pulse, InfinityFlex)
    Nb05,
    /// WMI AB/BS methods via tuxedo-nb04 shim (Sirius)
    Nb04,
    /// ACPI EC methods via tuxedo-uniwill shim (InfinityBook)
    Uniwill,
    /// WMI + ACPI DSM via tuxedo-clevo shim (Polaris, Stellaris)
    Clevo,
    /// ACPI TFAN evaluation via tuxedo-tuxi shim (Aura)
    Tuxi,
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Platform::Nb05 => write!(f, "NB05"),
            Platform::Nb04 => write!(f, "NB04"),
            Platform::Uniwill => write!(f, "Uniwill"),
            Platform::Clevo => write!(f, "Clevo"),
            Platform::Tuxi => write!(f, "Tuxi"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_all_platforms() {
        assert_eq!(Platform::Nb05.to_string(), "NB05");
        assert_eq!(Platform::Nb04.to_string(), "NB04");
        assert_eq!(Platform::Uniwill.to_string(), "Uniwill");
        assert_eq!(Platform::Clevo.to_string(), "Clevo");
        assert_eq!(Platform::Tuxi.to_string(), "Tuxi");
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Wrapper {
            platform: Platform,
        }

        for platform in [
            Platform::Nb05,
            Platform::Nb04,
            Platform::Uniwill,
            Platform::Clevo,
            Platform::Tuxi,
        ] {
            let w = Wrapper { platform };
            let serialized = toml::to_string(&w).unwrap();
            let deserialized: Wrapper = toml::from_str(&serialized).unwrap();
            assert_eq!(w, deserialized);
        }
    }

    #[test]
    fn platform_is_copy() {
        let p = Platform::Uniwill;
        let p2 = p;
        assert_eq!(p, p2);
    }
}
