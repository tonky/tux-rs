//! Enumerate `/dev/hidraw*` devices and construct ITE keyboard drivers.

use std::io;
use std::path::PathBuf;

use tracing::{debug, info, warn};

use super::KeyboardLed;
use super::color_scaling;
use super::hidraw::{HidrawDevice, HidrawInfo};
use super::ite829x::Ite829x;
use super::ite8291::Ite8291;
use super::ite8291_lb::Ite8291Lb;
use super::ite8297::Ite8297;
use super::sysfs_kbd::{SysfsRgbKeyboard, SysfsWhiteKeyboard};

/// ITE vendor ID on USB.
const ITE_VENDOR_ID: u16 = 0x048d;

/// Known ITE product IDs and their chip family.
const ITE_PRODUCTS: &[(u16, IteFamily)] = &[
    (0x8291, IteFamily::Ite8291),
    (0x600a, IteFamily::Ite8291), // Newer ITE 8291 variant
    (0x600b, IteFamily::Ite8291), // ITE 8291 variant
    (0xce00, IteFamily::Ite8291), // ITE 8291 variant
    (0x6010, IteFamily::Ite8291Lb),
    (0x7000, IteFamily::Ite8291Lb),
    (0x7001, IteFamily::Ite8291Lb),
    (0x8297, IteFamily::Ite8297),
    (0x8910, IteFamily::Ite829x),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IteFamily {
    Ite8291,
    Ite8291Lb,
    Ite8297,
    Ite829x,
}

/// Scan `/dev/hidraw*` and construct keyboard LED drivers for ITE devices.
pub fn discover_keyboards() -> Vec<Box<dyn KeyboardLed>> {
    discover_keyboards_with_sku("/dev", "")
}

/// Discover keyboards with a specific product SKU for color scaling.
pub fn discover_keyboards_for_device(product_sku: &str) -> Vec<Box<dyn KeyboardLed>> {
    discover_keyboards_with_sku("/dev", product_sku)
}

/// Scan a directory for hidraw devices (testable with custom base path).
fn discover_keyboards_with_sku(dev_dir: &str, product_sku: &str) -> Vec<Box<dyn KeyboardLed>> {
    let mut keyboards: Vec<Box<dyn KeyboardLed>> = Vec::new();

    let entries = match std::fs::read_dir(dev_dir) {
        Ok(e) => e,
        Err(e) => {
            warn!("failed to read {dev_dir}: {e}");
            return keyboards;
        }
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !name_str.starts_with("hidraw") {
            continue;
        }

        let path = entry.path();
        match try_open_ite_keyboard(&path, product_sku) {
            Ok(Some(kb)) => {
                info!(
                    "discovered ITE keyboard: {} at {}",
                    kb.device_type(),
                    path.display()
                );
                keyboards.push(kb);
            }
            Ok(None) => {
                debug!("skipping non-ITE device {}", path.display());
            }
            Err(e) => {
                debug!("failed to probe {}: {e}", path.display());
            }
        }
    }

    keyboards
}

fn try_open_ite_keyboard(
    path: &PathBuf,
    product_sku: &str,
) -> io::Result<Option<Box<dyn KeyboardLed>>> {
    let dev = HidrawDevice::open(path)?;
    let info = dev.info();

    if info.vendor_id != ITE_VENDOR_ID {
        return Ok(None);
    }

    let family = ITE_PRODUCTS
        .iter()
        .find(|(pid, _)| *pid == info.product_id)
        .map(|(_, family)| *family);

    let scaling = color_scaling::scale_for_model(product_sku, info.product_id);

    match family {
        Some(IteFamily::Ite8291) => Ok(Some(Box::new(Ite8291::with_scaling(dev, scaling)))),
        Some(IteFamily::Ite8291Lb) => Ok(Some(Box::new(Ite8291Lb::with_scaling(dev, scaling)))),
        Some(IteFamily::Ite8297) => Ok(Some(Box::new(Ite8297::with_scaling(dev, scaling)))),
        Some(IteFamily::Ite829x) => Ok(Some(Box::new(Ite829x::with_scaling(dev, scaling)))),
        None => {
            debug!(
                "ITE device at {} has unknown PID 0x{:04x}",
                path.display(),
                info.product_id
            );
            Ok(None)
        }
    }
}

/// Check if a HidrawInfo matches a known ITE keyboard device.
pub fn is_ite_keyboard(info: &HidrawInfo) -> bool {
    info.vendor_id == ITE_VENDOR_ID && ITE_PRODUCTS.iter().any(|(pid, _)| *pid == info.product_id)
}

/// Default sysfs LED class path.
const SYSFS_LEDS_DIR: &str = "/sys/class/leds";

/// Discover sysfs-based keyboard backlights (both RGB and white-only).
///
/// Scans `leds_dir` for `rgb:kbd_backlight` entries (with `multi_intensity`)
/// and `white:kbd_backlight` entries (brightness-only).
pub fn discover_sysfs_keyboards() -> Vec<Box<dyn KeyboardLed>> {
    discover_sysfs_keyboards_in(SYSFS_LEDS_DIR)
}

fn discover_sysfs_keyboards_in(leds_dir: &str) -> Vec<Box<dyn KeyboardLed>> {
    let mut keyboards: Vec<Box<dyn KeyboardLed>> = Vec::new();
    let path = std::path::Path::new(leds_dir);

    if !path.exists() {
        return keyboards;
    }

    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(e) => {
            debug!("failed to read {leds_dir}: {e}");
            return keyboards;
        }
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let led_path = entry.path();

        // Match RGB keyboards: "rgb:kbd_backlight", "rgb:kbd_backlight_1", etc.
        if name_str.starts_with("rgb:kbd_backlight") {
            // Require multi_intensity to confirm RGB capability.
            if !led_path.join("multi_intensity").exists() {
                debug!("skipping {} — no multi_intensity", led_path.display());
                continue;
            }

            match SysfsRgbKeyboard::open(&led_path) {
                Ok(kb) => {
                    info!("discovered sysfs RGB keyboard at {}", led_path.display());
                    keyboards.push(Box::new(kb));
                }
                Err(e) => {
                    warn!(
                        "failed to open sysfs keyboard at {}: {e}",
                        led_path.display()
                    );
                }
            }
            continue;
        }

        // Match white keyboards: "white:kbd_backlight", "white:kbd_backlight_1", etc.
        if name_str.starts_with("white:kbd_backlight") {
            match SysfsWhiteKeyboard::open(&led_path) {
                Ok(kb) => {
                    info!("discovered sysfs white keyboard at {}", led_path.display());
                    keyboards.push(Box::new(kb));
                }
                Err(e) => {
                    warn!(
                        "failed to open sysfs white keyboard at {}: {e}",
                        led_path.display()
                    );
                }
            }
        }
    }

    keyboards
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_ite_keyboard_known_pids() {
        for &(pid, _) in ITE_PRODUCTS {
            let info = HidrawInfo {
                bus_type: 3, // USB
                vendor_id: ITE_VENDOR_ID,
                product_id: pid,
            };
            assert!(
                is_ite_keyboard(&info),
                "PID 0x{pid:04x} should be recognized"
            );
        }
    }

    #[test]
    fn is_ite_keyboard_unknown_pid() {
        let info = HidrawInfo {
            bus_type: 3,
            vendor_id: ITE_VENDOR_ID,
            product_id: 0x9999,
        };
        assert!(!is_ite_keyboard(&info));
    }

    #[test]
    fn is_ite_keyboard_wrong_vendor() {
        let info = HidrawInfo {
            bus_type: 3,
            vendor_id: 0x1234,
            product_id: 0x8291,
        };
        assert!(!is_ite_keyboard(&info));
    }

    #[test]
    fn discover_in_nonexistent_dir_returns_empty() {
        let result = discover_keyboards_with_sku("/nonexistent_path_for_test", "");
        assert!(result.is_empty());
    }

    #[test]
    fn discover_sysfs_nonexistent_returns_empty() {
        let result = discover_sysfs_keyboards_in("/nonexistent_path_for_sysfs_test");
        assert!(result.is_empty());
    }

    #[test]
    fn discover_sysfs_finds_rgb_keyboard() {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tmp/test_sysfs_discover");
        let _ = std::fs::remove_dir_all(&base);

        // Create a mock sysfs LED entry.
        let led_dir = base.join("rgb:kbd_backlight");
        std::fs::create_dir_all(&led_dir).unwrap();
        std::fs::write(led_dir.join("max_brightness"), "200\n").unwrap();
        std::fs::write(led_dir.join("brightness"), "0\n").unwrap();
        std::fs::write(led_dir.join("multi_intensity"), "0 0 0\n").unwrap();

        let kbs = discover_sysfs_keyboards_in(base.to_str().unwrap());
        assert_eq!(kbs.len(), 1);
        assert_eq!(kbs[0].device_type(), "sysfs_rgb");
        assert_eq!(kbs[0].available_modes(), vec!["static"]);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn discover_sysfs_skips_non_rgb() {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tmp/test_sysfs_discover_skip");
        let _ = std::fs::remove_dir_all(&base);

        // LED entry without multi_intensity → not RGB, should be skipped.
        let led_dir = base.join("rgb:kbd_backlight");
        std::fs::create_dir_all(&led_dir).unwrap();
        std::fs::write(led_dir.join("max_brightness"), "200\n").unwrap();
        std::fs::write(led_dir.join("brightness"), "0\n").unwrap();
        // No multi_intensity file.

        let kbs = discover_sysfs_keyboards_in(base.to_str().unwrap());
        assert!(kbs.is_empty());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn discover_sysfs_finds_white_keyboard() {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tmp/test_sysfs_discover_white");
        let _ = std::fs::remove_dir_all(&base);

        // Create a mock white keyboard LED entry.
        let led_dir = base.join("white:kbd_backlight");
        std::fs::create_dir_all(&led_dir).unwrap();
        std::fs::write(led_dir.join("max_brightness"), "2\n").unwrap();
        std::fs::write(led_dir.join("brightness"), "0\n").unwrap();

        let kbs = discover_sysfs_keyboards_in(base.to_str().unwrap());
        assert_eq!(kbs.len(), 1);
        assert_eq!(kbs[0].device_type(), "sysfs_white");
        assert_eq!(kbs[0].available_modes(), vec!["static"]);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn discover_sysfs_finds_both_rgb_and_white() {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tmp/test_sysfs_discover_both");
        let _ = std::fs::remove_dir_all(&base);

        // RGB keyboard.
        let rgb_dir = base.join("rgb:kbd_backlight");
        std::fs::create_dir_all(&rgb_dir).unwrap();
        std::fs::write(rgb_dir.join("max_brightness"), "200\n").unwrap();
        std::fs::write(rgb_dir.join("brightness"), "0\n").unwrap();
        std::fs::write(rgb_dir.join("multi_intensity"), "0 0 0\n").unwrap();

        // White keyboard.
        let white_dir = base.join("white:kbd_backlight");
        std::fs::create_dir_all(&white_dir).unwrap();
        std::fs::write(white_dir.join("max_brightness"), "2\n").unwrap();
        std::fs::write(white_dir.join("brightness"), "0\n").unwrap();

        let kbs = discover_sysfs_keyboards_in(base.to_str().unwrap());
        assert_eq!(kbs.len(), 2);

        let types: Vec<&str> = kbs.iter().map(|k| k.device_type()).collect();
        assert!(types.contains(&"sysfs_rgb"));
        assert!(types.contains(&"sysfs_white"));

        let _ = std::fs::remove_dir_all(&base);
    }
}
