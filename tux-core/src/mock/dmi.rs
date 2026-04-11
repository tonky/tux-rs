//! Mock DMI source for testing platform detection.

use std::collections::HashMap;
use std::io;

use crate::dmi::DmiSource;

/// Configurable mock DMI source for testing.
///
/// Use the builder methods to configure which DMI fields, WMI GUIDs,
/// and sysfs paths are present.
pub struct MockDmiSource {
    pub fields: HashMap<String, String>,
    pub wmi_guids: Vec<String>,
    pub sysfs_paths: Vec<String>,
}

impl MockDmiSource {
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
            wmi_guids: Vec::new(),
            sysfs_paths: Vec::new(),
        }
    }

    pub fn with_field(mut self, field: &str, value: &str) -> Self {
        self.fields.insert(field.to_string(), value.to_string());
        self
    }

    pub fn with_wmi_guid(mut self, guid: &str) -> Self {
        self.wmi_guids.push(guid.to_string());
        self
    }

    pub fn with_sysfs_path(mut self, path: &str) -> Self {
        self.sysfs_paths.push(path.to_string());
        self
    }

    /// Set standard TUXEDO DMI fields with the given SKU.
    pub fn tuxedo_base(self, sku: &str) -> Self {
        self.with_field("board_vendor", "TUXEDO")
            .with_field("board_name", "TUXEDO")
            .with_field("product_sku", sku)
            .with_field("sys_vendor", "TUXEDO")
            .with_field("product_name", "TUXEDO Laptop")
            .with_field("product_version", "N/A")
    }
}

impl Default for MockDmiSource {
    fn default() -> Self {
        Self::new()
    }
}

impl DmiSource for MockDmiSource {
    fn read_dmi_field(&self, field: &str) -> io::Result<String> {
        self.fields.get(field).cloned().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("no DMI field: {field}"))
        })
    }

    fn wmi_guid_exists(&self, guid: &str) -> bool {
        self.wmi_guids.iter().any(|g| g == guid)
    }

    fn sysfs_path_exists(&self, path: &str) -> bool {
        self.sysfs_paths.iter().any(|p| p == path)
    }
}
