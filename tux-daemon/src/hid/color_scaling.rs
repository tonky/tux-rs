//! Per-model RGB color scaling corrections.
//!
//! Different LED hardware in different TUXEDO models produces uneven
//! color output. These scaling factors normalize the appearance.

use super::Rgb;

/// Per-channel RGB scaling factors (0.0–1.0).
#[derive(Debug, Clone, Copy)]
pub struct ColorScaling {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl ColorScaling {
    pub const IDENTITY: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
    };

    /// Apply scaling to an RGB color.
    pub fn apply(&self, color: Rgb) -> Rgb {
        Rgb {
            r: (color.r as f32 * self.r).round().min(255.0) as u8,
            g: (color.g as f32 * self.g).round().min(255.0) as u8,
            b: (color.b as f32 * self.b).round().min(255.0) as u8,
        }
    }
}

/// Look up color scaling for a given product SKU and USB PID.
///
/// Based on the quirk tables from the C kernel drivers.
pub fn scale_for_model(product_sku: &str, usb_pid: u16) -> ColorScaling {
    match (product_sku, usb_pid) {
        // ITE 8291 per-key scaling
        ("STEPOL1XA04", 0x600a) => ColorScaling {
            r: 126.0 / 255.0,
            g: 1.0,
            b: 1.0,
        },
        ("STELLARIS1XI05", 0x600a) => ColorScaling {
            r: 200.0 / 255.0,
            g: 1.0,
            b: 220.0 / 255.0,
        },
        ("STELLARIS1XA05", _) => ColorScaling {
            r: 128.0 / 255.0,
            g: 1.0,
            b: 1.0,
        },
        ("STELLARIS17I06", 0xce00) => ColorScaling {
            r: 1.0,
            g: 180.0 / 255.0,
            b: 180.0 / 255.0,
        },
        ("STELLARIS17I06", 0x600a) => ColorScaling {
            r: 200.0 / 255.0,
            g: 1.0,
            b: 220.0 / 255.0,
        },
        // ITE 8291 lightbar scaling
        ("STEPOL1XA04", 0x6010) | ("STELLARIS1XI05", 0x6010) | ("STELLARIS17I06", 0x6010) => {
            ColorScaling {
                r: 1.0,
                g: 100.0 / 255.0,
                b: 100.0 / 255.0,
            }
        }
        // Default: slight green/blue reduction (matches C driver fallback)
        (_, 0x8291) | (_, 0x600a) | (_, 0x600b) | (_, 0xce00) => ColorScaling {
            r: 1.0,
            g: 126.0 / 255.0,
            b: 120.0 / 255.0,
        },
        // No scaling by default for other chips
        _ => ColorScaling::IDENTITY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_scaling_preserves_color() {
        let color = Rgb::new(128, 64, 255);
        let result = ColorScaling::IDENTITY.apply(color);
        assert_eq!(result, color);
    }

    #[test]
    fn scaling_reduces_channels() {
        let scaling = ColorScaling {
            r: 0.5,
            g: 1.0,
            b: 0.5,
        };
        let result = scaling.apply(Rgb::new(200, 100, 200));
        assert_eq!(result.r, 100);
        assert_eq!(result.g, 100);
        assert_eq!(result.b, 100);
    }

    #[test]
    fn scaling_clamps_to_255() {
        // Even with identity, values can't exceed 255
        let result = ColorScaling::IDENTITY.apply(Rgb::new(255, 255, 255));
        assert_eq!(result, Rgb::WHITE);
    }

    #[test]
    fn known_model_has_scaling() {
        let s = scale_for_model("STEPOL1XA04", 0x600a);
        assert!(s.r < 0.6); // red reduced
        assert_eq!(s.g, 1.0);
    }

    #[test]
    fn unknown_model_8291_gets_default() {
        let s = scale_for_model("UNKNOWN", 0x8291);
        assert!(s.g < 0.6); // default reduces green
        assert!(s.b < 0.6); // default reduces blue
    }

    #[test]
    fn unknown_model_unknown_pid_gets_identity() {
        let s = scale_for_model("UNKNOWN", 0x9999);
        assert_eq!(s.r, 1.0);
        assert_eq!(s.g, 1.0);
        assert_eq!(s.b, 1.0);
    }
}
