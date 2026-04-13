//! DMI-based platform detection for TUXEDO laptops.
//!
//! Reads DMI data from `/sys/class/dmi/id/` and sysfs to identify the
//! running hardware model, returning a resolved `DeviceDescriptor`.

use std::io;

use crate::device::DeviceDescriptor;
use crate::device_table::{fallback_for_platform, lookup_by_sku};
use crate::platform::Platform;

/// NB04 WMI GUID used for platform detection.
const NB04_WMI_GUID: &str = "80C9BAA6-AC48-4538-9234-9F81A55E7C85";

/// Clevo WMI event GUID — unique to Clevo hardware (tuxedo-drivers: clevo_wmi.ko).
const CLEVO_WMI_EVENT_GUID: &str = "ABBC0F6B-8EA1-11D1-00A0-C90629100000";

/// Uniwill WMI event GUID 2 — unique to Uniwill hardware (tuxedo-drivers: uniwill_wmi.ko).
const UNIWILL_WMI_EVENT_GUID_2: &str = "ABBC0F72-8EA1-11D1-00A0-C90629100000";

/// NB04 platform path exposed by tuxedo-drivers.
const NB04_SHIM_SYSFS_PATH: &str = "/sys/devices/platform/tuxedo_nb04_sensors/";

/// Tuxi platform path exposed by tuxedo-drivers.
const TUXI_FAN_CONTROL_SYSFS_PATH: &str = "/sys/devices/platform/tuxedo_fan_control/";

/// Curated platform hints from TCC's DMI SKU map for recent models not yet in our table.
/// Keep this list intentionally small and review-driven.
fn platform_hint_from_tcc_sku_map(sku: &str) -> Option<Platform> {
    match sku.trim() {
        // InfinityBook Pro Gen9/Gen10 AMD combined SKU strings used by TCC.
        "IBP14A09MK1 / IBP15A09MK1"
        | "IBP14A10MK1 / IBP15A10MK1"
        // TCC includes this typo variant; accept it for resilience.
        | "IIBP14A10MK1 / IBP15A10MK1" => Some(Platform::Uniwill),
        _ => None,
    }
}

/// Abstraction over DMI/sysfs access for testability.
pub trait DmiSource {
    /// Read a DMI field (e.g. "board_vendor") from sysfs.
    fn read_dmi_field(&self, field: &str) -> io::Result<String>;

    /// Check if a WMI GUID directory exists in `/sys/bus/wmi/devices/`.
    fn wmi_guid_exists(&self, guid: &str) -> bool;

    /// Check if a sysfs path exists.
    fn sysfs_path_exists(&self, path: &str) -> bool;
}

/// Real implementation that reads from `/sys/`.
pub struct SysFsDmiSource;

impl DmiSource for SysFsDmiSource {
    fn read_dmi_field(&self, field: &str) -> io::Result<String> {
        let path = format!("/sys/class/dmi/id/{field}");
        std::fs::read_to_string(&path).map(|s| s.trim().to_string())
    }

    fn wmi_guid_exists(&self, guid: &str) -> bool {
        std::path::Path::new(&format!("/sys/bus/wmi/devices/{guid}")).exists()
    }

    fn sysfs_path_exists(&self, path: &str) -> bool {
        std::path::Path::new(path).exists()
    }
}

/// DMI identification strings read from sysfs.
#[derive(Debug, Clone)]
pub struct DmiInfo {
    pub board_vendor: String,
    pub board_name: String,
    pub product_sku: String,
    pub sys_vendor: String,
    pub product_name: String,
    pub product_version: String,
}

/// Result of successful platform detection.
#[derive(Debug)]
pub struct DetectedDevice {
    /// The resolved device descriptor (from table or fallback).
    pub descriptor: &'static DeviceDescriptor,
    /// DMI identification strings.
    pub dmi: DmiInfo,
    /// `false` if using a platform fallback instead of an exact SKU match.
    pub exact_match: bool,
}

/// Errors that can occur during platform detection.
#[derive(Debug, thiserror::Error)]
pub enum DetectionError {
    /// Cannot read DMI data from sysfs (permissions, missing files).
    #[error("cannot access DMI data: {0}")]
    NoDmiAccess(#[from] io::Error),

    /// DMI data was read but no TUXEDO platform could be identified.
    #[error("unknown platform — DMI: board_vendor={}, product_sku={}", .dmi.board_vendor, .dmi.product_sku)]
    UnknownPlatform { dmi: Box<DmiInfo> },

    /// Platform was identified but its kernel shim is not loaded.
    #[error("kernel shim not loaded for platform {platform}")]
    NoKernelShim { platform: Platform },
}

/// Read DMI info from the given source.
pub fn read_dmi_info(source: &dyn DmiSource) -> Result<DmiInfo, DetectionError> {
    Ok(DmiInfo {
        board_vendor: source.read_dmi_field("board_vendor")?,
        board_name: source.read_dmi_field("board_name")?,
        product_sku: source.read_dmi_field("product_sku")?,
        sys_vendor: source.read_dmi_field("sys_vendor")?,
        product_name: source.read_dmi_field("product_name")?,
        product_version: source.read_dmi_field("product_version")?,
    })
}

/// Detect the platform from DMI info and sysfs probing.
fn detect_platform(source: &dyn DmiSource, dmi: &DmiInfo) -> Option<Platform> {
    // NB05: board_vendor == "NB05"
    if dmi.board_vendor.eq_ignore_ascii_case("NB05") {
        return Some(Platform::Nb05);
    }

    // NB04: WMI GUID present
    if source.wmi_guid_exists(NB04_WMI_GUID) {
        return Some(Platform::Nb04);
    }

    // Uniwill: tuxedo-drivers exposes WMI event GUID 2 unique to Uniwill hardware.
    if source.wmi_guid_exists(UNIWILL_WMI_EVENT_GUID_2) {
        return Some(Platform::Uniwill);
    }

    // Clevo: tuxedo-drivers exposes WMI event GUID unique to Clevo hardware.
    if source.wmi_guid_exists(CLEVO_WMI_EVENT_GUID) {
        return Some(Platform::Clevo);
    }

    // Tuxi: tuxedo-drivers registers a tuxedo_fan_control platform device.
    if source.sysfs_path_exists(TUXI_FAN_CONTROL_SYSFS_PATH) {
        return Some(Platform::Tuxi);
    }

    // TCC-proven recent SKU strings can provide a safe platform hint.
    if let Some(platform) = platform_hint_from_tcc_sku_map(&dmi.product_sku) {
        return Some(platform);
    }

    // NB02 platforms are Uniwill-based in tuxedo-drivers.
    // Some firmware variants do not expose the Uniwill WMI GUID reliably,
    // so treat NB02 board vendor as a final Uniwill fallback hint.
    if dmi.board_vendor.eq_ignore_ascii_case("NB02") {
        return Some(Platform::Uniwill);
    }

    None
}

/// Try exact SKU lookup, then tokenized lookup for combined SKU strings.
///
/// Some hardware reports composite values like "SKU_A / SKU_B" in DMI.
/// We keep exact match first, then try slash-delimited tokens.
fn lookup_descriptor_for_sku(sku: &str) -> Option<&'static DeviceDescriptor> {
    if let Some(descriptor) = lookup_by_sku(sku) {
        return Some(descriptor);
    }

    if !sku.contains('/') {
        return None;
    }

    for token in sku
        .split('/')
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        if let Some(descriptor) = lookup_by_sku(token) {
            return Some(descriptor);
        }
    }

    None
}

/// Detect the running TUXEDO laptop model.
///
/// # Detection flow
///
/// 1. Read DMI strings from sysfs
/// 2. Try exact SKU match in the device table
/// 3. If no match, detect platform heuristically (board_vendor, WMI, sysfs)
/// 4. Return platform fallback if platform detected but SKU unknown
/// 5. Return error if no platform can be identified
pub fn detect_device(source: &dyn DmiSource) -> Result<DetectedDevice, DetectionError> {
    let dmi = read_dmi_info(source)?;

    // Step 1: Try exact SKU match
    if let Some(descriptor) = lookup_descriptor_for_sku(&dmi.product_sku) {
        return Ok(DetectedDevice {
            descriptor,
            dmi,
            exact_match: true,
        });
    }

    // Step 2: Heuristic platform detection
    let Some(platform) = detect_platform(source, &dmi) else {
        return Err(DetectionError::UnknownPlatform { dmi: Box::new(dmi) });
    };

    // Step 2.5: For platforms detected via non-sysfs signals (NB04 via WMI),
    // verify the tuxedo-drivers kernel module is actually loaded.
    // NB05 is detected by board_vendor and has no sysfs platform path to verify.
    // Uniwill/Clevo/Tuxi are detected BY sysfs/WMI, so driver is inherently present.
    // NB04: verify tuxedo_nb04_sensors is loaded (tuxedo-drivers platform device).
    if platform == Platform::Nb04 && !source.sysfs_path_exists(NB04_SHIM_SYSFS_PATH) {
        return Err(DetectionError::NoKernelShim { platform });
    }

    // Step 3: Return platform fallback
    let descriptor = fallback_for_platform(platform);
    Ok(DetectedDevice {
        descriptor,
        dmi,
        exact_match: false,
    })
}

/// Build a copy-paste diagnostics block for startup detection failures.
pub fn startup_detection_debug_block(source: &dyn DmiSource) -> String {
    let mut lines = Vec::new();
    lines.push("tux-rs startup diagnostics".to_string());

    match read_dmi_info(source) {
        Ok(dmi) => {
            lines.push(format!("dmi.board_vendor={}", dmi.board_vendor));
            lines.push(format!("dmi.board_name={}", dmi.board_name));
            lines.push(format!("dmi.product_sku={}", dmi.product_sku));
            lines.push(format!("dmi.sys_vendor={}", dmi.sys_vendor));
            lines.push(format!("dmi.product_name={}", dmi.product_name));
            lines.push(format!("dmi.product_version={}", dmi.product_version));

            if let Some(platform) = detect_platform(source, &dmi) {
                lines.push(format!("detect.platform_guess={platform:?}"));
            } else {
                lines.push("detect.platform_guess=none".to_string());
            }

            let tcc_hint = platform_hint_from_tcc_sku_map(&dmi.product_sku)
                .map(|p| format!("{p:?}"))
                .unwrap_or_else(|| "none".to_string());
            lines.push(format!("detect.platform_hint_from_tcc_sku={tcc_hint}"));
        }
        Err(e) => {
            lines.push(format!("dmi.read_error={e}"));
            lines.push("detect.platform_guess=unknown".to_string());
            lines.push("detect.platform_hint_from_tcc_sku=unknown".to_string());
        }
    }

    lines.push(format!(
        "probe.wmi.nb04_guid={}",
        source.wmi_guid_exists(NB04_WMI_GUID)
    ));
    lines.push(format!(
        "probe.wmi.uniwill_guid_2={}",
        source.wmi_guid_exists(UNIWILL_WMI_EVENT_GUID_2)
    ));
    lines.push(format!(
        "probe.wmi.clevo_guid={}",
        source.wmi_guid_exists(CLEVO_WMI_EVENT_GUID)
    ));
    lines.push(format!(
        "probe.sysfs.nb04_shim={}",
        source.sysfs_path_exists(NB04_SHIM_SYSFS_PATH)
    ));
    lines.push(format!(
        "probe.sysfs.tuxi_fan_control={}",
        source.sysfs_path_exists(TUXI_FAN_CONTROL_SYSFS_PATH)
    ));

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::dmi::MockDmiSource;

    #[test]
    fn exact_sku_match() {
        let source = MockDmiSource::new().tuxedo_base("PULSE1403");
        let result = detect_device(&source).unwrap();
        assert!(result.exact_match);
        assert_eq!(result.descriptor.product_sku, "PULSE1403");
        assert_eq!(result.descriptor.platform, Platform::Nb05);
    }

    #[test]
    fn nb05_board_vendor_fallback() {
        let source = MockDmiSource::new()
            .tuxedo_base("UNKNOWN_NB05_SKU")
            .with_field("board_vendor", "NB05");
        let result = detect_device(&source).unwrap();
        assert!(!result.exact_match);
        assert_eq!(result.descriptor.platform, Platform::Nb05);
    }

    #[test]
    fn nb02_board_vendor_fallback_maps_to_uniwill() {
        let source = MockDmiSource::new()
            .tuxedo_base("UNKNOWN_NB02_SKU")
            .with_field("board_vendor", "NB02");
        let result = detect_device(&source).unwrap();
        assert!(!result.exact_match);
        assert_eq!(result.descriptor.platform, Platform::Uniwill);
    }

    #[test]
    fn split_sku_matches_known_token() {
        let source = MockDmiSource::new().tuxedo_base("IBP14I08MK2 / UNKNOWN_VARIANT");
        let result = detect_device(&source).unwrap();
        assert!(result.exact_match);
        assert_eq!(result.descriptor.product_sku, "IBP14I08MK2");
        assert_eq!(result.descriptor.platform, Platform::Uniwill);
    }

    #[test]
    fn issue_8_gen9_amd_combined_sku_detects_uniwill_fallback() {
        let source = MockDmiSource::new()
            .tuxedo_base("IBP14A09MK1 / IBP15A09MK1")
            .with_field("board_vendor", "NB02");
        let result = detect_device(&source).unwrap();
        assert!(!result.exact_match);
        assert_eq!(result.descriptor.platform, Platform::Uniwill);
    }

    #[test]
    fn tcc_gen10_combined_sku_hints_uniwill_without_board_vendor() {
        let source = MockDmiSource::new()
            .tuxedo_base("IBP14A10MK1 / IBP15A10MK1")
            .with_field("board_vendor", "UNKNOWN_VENDOR");
        let result = detect_device(&source).unwrap();
        assert!(!result.exact_match);
        assert_eq!(result.descriptor.platform, Platform::Uniwill);
    }

    #[test]
    fn tcc_typo_gen10_combined_sku_hints_uniwill() {
        let source = MockDmiSource::new()
            .tuxedo_base("IIBP14A10MK1 / IBP15A10MK1")
            .with_field("board_vendor", "UNKNOWN_VENDOR");
        let result = detect_device(&source).unwrap();
        assert!(!result.exact_match);
        assert_eq!(result.descriptor.platform, Platform::Uniwill);
    }

    #[test]
    fn nb04_wmi_guid_detection() {
        let source = MockDmiSource::new()
            .tuxedo_base("UNKNOWN_NB04_SKU")
            .with_wmi_guid(NB04_WMI_GUID)
            .with_sysfs_path("/sys/devices/platform/tuxedo_nb04_sensors/");
        let result = detect_device(&source).unwrap();
        assert!(!result.exact_match);
        assert_eq!(result.descriptor.platform, Platform::Nb04);
    }

    #[test]
    fn nb04_wmi_without_shim_returns_no_kernel_shim() {
        // WMI GUID present (BIOS advertises NB04) but tuxedo_nb04_sensors not loaded
        let source = MockDmiSource::new()
            .tuxedo_base("UNKNOWN_NB04_SKU")
            .with_wmi_guid(NB04_WMI_GUID);
        let err = detect_device(&source).unwrap_err();
        assert!(matches!(
            err,
            DetectionError::NoKernelShim {
                platform: Platform::Nb04
            }
        ));
        let msg = err.to_string();
        assert!(msg.contains("kernel shim not loaded"));
        assert!(msg.contains("NB04"));
    }

    #[test]
    fn uniwill_sysfs_fallback() {
        let source = MockDmiSource::new()
            .tuxedo_base("UNKNOWN_UW_SKU")
            .with_wmi_guid(UNIWILL_WMI_EVENT_GUID_2);
        let result = detect_device(&source).unwrap();
        assert!(!result.exact_match);
        assert_eq!(result.descriptor.platform, Platform::Uniwill);
    }

    #[test]
    fn clevo_sysfs_fallback() {
        let source = MockDmiSource::new()
            .tuxedo_base("UNKNOWN_CLEVO_SKU")
            .with_wmi_guid(CLEVO_WMI_EVENT_GUID);
        let result = detect_device(&source).unwrap();
        assert!(!result.exact_match);
        assert_eq!(result.descriptor.platform, Platform::Clevo);
    }

    #[test]
    fn tuxi_sysfs_fallback() {
        let source = MockDmiSource::new()
            .tuxedo_base("UNKNOWN_TUXI_SKU")
            .with_sysfs_path("/sys/devices/platform/tuxedo_fan_control/");
        let result = detect_device(&source).unwrap();
        assert!(!result.exact_match);
        assert_eq!(result.descriptor.platform, Platform::Tuxi);
    }

    #[test]
    fn unknown_platform_error() {
        let source = MockDmiSource::new().tuxedo_base("TOTALLY_UNKNOWN");
        let err = detect_device(&source).unwrap_err();
        assert!(matches!(err, DetectionError::UnknownPlatform { .. }));
        let msg = err.to_string();
        assert!(msg.contains("unknown platform"));
        assert!(msg.contains("TUXEDO")); // board_vendor from tuxedo_base
    }

    #[test]
    fn no_dmi_access_error() {
        // Empty source — no fields at all
        let source = MockDmiSource::new();
        let err = detect_device(&source).unwrap_err();
        assert!(matches!(err, DetectionError::NoDmiAccess(_)));
    }

    #[test]
    fn dmi_info_populated() {
        let source = MockDmiSource::new()
            .with_field("board_vendor", "NB05")
            .with_field("board_name", "NB05_BOARD")
            .with_field("product_sku", "PULSE1403")
            .with_field("sys_vendor", "TUXEDO")
            .with_field("product_name", "TUXEDO Pulse 14 Gen3")
            .with_field("product_version", "Rev1");
        let result = detect_device(&source).unwrap();
        assert_eq!(result.dmi.board_vendor, "NB05");
        assert_eq!(result.dmi.board_name, "NB05_BOARD");
        assert_eq!(result.dmi.product_sku, "PULSE1403");
        assert_eq!(result.dmi.sys_vendor, "TUXEDO");
        assert_eq!(result.dmi.product_name, "TUXEDO Pulse 14 Gen3");
        assert_eq!(result.dmi.product_version, "Rev1");
    }

    #[test]
    fn detection_error_messages_are_meaningful() {
        // UnknownPlatform
        let source = MockDmiSource::new().tuxedo_base("NOPE");
        let err = detect_device(&source).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown platform"));
        assert!(msg.contains("TUXEDO"));
        assert!(msg.contains("NOPE"));

        // NoDmiAccess
        let source = MockDmiSource::new();
        let err = detect_device(&source).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("cannot access DMI data"));
    }

    #[test]
    fn exact_sku_match_ibp16_gen8() {
        // InfinityBook Pro 16 Gen 8 (Uniwill platform) — priority regression test.
        // Must always exact-match by SKU so the correct Uniwill backend is selected.
        let source = MockDmiSource::new().tuxedo_base("IBP16I08MK2");
        let result = detect_device(&source).unwrap();
        assert!(
            result.exact_match,
            "IBP16I08MK2 must match by SKU, not platform fallback"
        );
        assert_eq!(result.descriptor.product_sku, "IBP16I08MK2");
        assert_eq!(result.descriptor.platform, Platform::Uniwill);
    }

    #[test]
    fn exact_sku_match_aura_gen4_combined() {
        // The actual hardware SKU string reported by Aura 14 Gen4 / Aura 15 Gen4
        let source = MockDmiSource::new().tuxedo_base("AURA14GEN4 / AURA15GEN4");
        let result = detect_device(&source).unwrap();
        assert!(result.exact_match);
        assert_eq!(result.descriptor.product_sku, "AURA14GEN4 / AURA15GEN4");
        assert_eq!(result.descriptor.platform, Platform::Clevo);
    }

    #[test]
    fn startup_debug_block_contains_copy_paste_fields() {
        let source = MockDmiSource::new()
            .tuxedo_base("IBP14A09MK1 / IBP15A09MK1")
            .with_field("board_vendor", "NB02");
        let report = startup_detection_debug_block(&source);

        assert!(report.contains("tux-rs startup diagnostics"));
        assert!(report.contains("dmi.board_vendor=NB02"));
        assert!(report.contains("dmi.product_sku=IBP14A09MK1 / IBP15A09MK1"));
        assert!(report.contains("detect.platform_guess=Uniwill"));
        assert!(report.contains("detect.platform_hint_from_tcc_sku=Uniwill"));
        assert!(report.contains("probe.wmi.nb04_guid="));
        assert!(report.contains("probe.wmi.uniwill_guid_2="));
        assert!(report.contains("probe.wmi.clevo_guid="));
        assert!(report.contains("probe.sysfs.nb04_shim="));
        assert!(report.contains("probe.sysfs.tuxi_fan_control="));
    }

    // ── tuxedo-drivers detection paths ──────────────────────────────────────

    #[test]
    fn nb04_tuxedo_drivers_shim_path() {
        // WMI GUID present + tuxedo_nb04_sensors (tuxedo-drivers) loaded
        let source = MockDmiSource::new()
            .tuxedo_base("UNKNOWN_NB04_SKU")
            .with_wmi_guid(NB04_WMI_GUID)
            .with_sysfs_path("/sys/devices/platform/tuxedo_nb04_sensors/");
        let result = detect_device(&source).unwrap();
        assert!(!result.exact_match);
        assert_eq!(result.descriptor.platform, Platform::Nb04);
    }

    #[test]
    fn nb04_wmi_without_either_shim_returns_no_kernel_shim() {
        // WMI GUID present (BIOS advertises NB04) but neither tux-kmod nor tuxedo-drivers loaded
        let source = MockDmiSource::new()
            .tuxedo_base("UNKNOWN_NB04_SKU")
            .with_wmi_guid(NB04_WMI_GUID);
        let err = detect_device(&source).unwrap_err();
        assert!(matches!(
            err,
            DetectionError::NoKernelShim {
                platform: Platform::Nb04
            }
        ));
    }

    #[test]
    fn clevo_wmi_guid_detection_tuxedo_drivers() {
        // tuxedo-drivers: no tuxedo-clevo sysfs path, but WMI event GUID present
        let source = MockDmiSource::new()
            .tuxedo_base("UNKNOWN_CLEVO_SKU")
            .with_wmi_guid(CLEVO_WMI_EVENT_GUID);
        let result = detect_device(&source).unwrap();
        assert!(!result.exact_match);
        assert_eq!(result.descriptor.platform, Platform::Clevo);
    }

    #[test]
    fn uniwill_wmi_guid_detection_tuxedo_drivers() {
        // tuxedo-drivers: no tuxedo-uniwill sysfs path, but WMI event GUID 2 present
        let source = MockDmiSource::new()
            .tuxedo_base("UNKNOWN_UW_SKU")
            .with_wmi_guid(UNIWILL_WMI_EVENT_GUID_2);
        let result = detect_device(&source).unwrap();
        assert!(!result.exact_match);
        assert_eq!(result.descriptor.platform, Platform::Uniwill);
    }

    #[test]
    fn tuxi_tuxedo_drivers_platform_path() {
        // tuxedo-drivers: tuxedo_fan_control platform device instead of tuxedo-tuxi
        let source = MockDmiSource::new()
            .tuxedo_base("UNKNOWN_TUXI_SKU")
            .with_sysfs_path("/sys/devices/platform/tuxedo_fan_control/");
        let result = detect_device(&source).unwrap();
        assert!(!result.exact_match);
        assert_eq!(result.descriptor.platform, Platform::Tuxi);
    }

    #[test]
    fn uwiwill_wmi_guid_detected_before_clevo() {
        // If both Uniwill and Clevo WMI GUIDs are somehow present, Uniwill wins
        let source = MockDmiSource::new()
            .tuxedo_base("AMBIGUOUS_SKU")
            .with_wmi_guid(UNIWILL_WMI_EVENT_GUID_2)
            .with_wmi_guid(CLEVO_WMI_EVENT_GUID);
        let result = detect_device(&source).unwrap();
        assert_eq!(result.descriptor.platform, Platform::Uniwill);
    }

    #[test]
    #[ignore] // Only runs on actual TUXEDO hardware with sysfs
    fn real_sysfs_dmi_source() {
        let source = SysFsDmiSource;
        // Just verify we can read at least board_vendor without panicking
        let result = source.read_dmi_field("board_vendor");
        assert!(result.is_ok(), "should be able to read board_vendor");
        let vendor = result.unwrap();
        assert!(!vendor.is_empty(), "board_vendor should not be empty");
    }
}
