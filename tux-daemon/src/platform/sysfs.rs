use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use tracing::debug;

// ─── Shared fan sysfs constants ──────────────────────────────────────────────

/// `fan{N}_pwm_enable` value: fan is under manual (user) control.
pub const PWM_ENABLE_MANUAL: u8 = 1;

/// `fan{N}_pwm_enable` value: fan is under automatic (firmware) control.
pub const PWM_ENABLE_AUTO: u8 = 2;

// ─── Shared fan sysfs helpers ─────────────────────────────────────────────────

/// Build a 1-indexed fan attribute name (e.g. `fan1_pwm`, `fan2_input`).
pub fn fan_attr(fan_index: u8, suffix: &str) -> String {
    format!("fan{}_{}", fan_index + 1, suffix)
}

/// Validate a 0-based fan index against the backend's fan count.
pub fn check_fan_index(fan_index: u8, num_fans: u8) -> io::Result<()> {
    if fan_index >= num_fans {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("fan index {fan_index} out of range (num_fans={num_fans})"),
        ))
    } else {
        Ok(())
    }
}

/// Walk `base` and return the path of the first `hwmonN` subdirectory.
///
/// Used by backends that expose sensors through a dynamically-numbered hwmon
/// device (e.g. `tuxedo_nb05_sensors`, `tuxedo_fan_control`, `tuxedo_nb04_sensors`).
pub fn discover_hwmon(base: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(Path::new(base)).ok()?;
    entries
        .flatten()
        .find(|e| e.file_name().to_string_lossy().starts_with("hwmon"))
        .map(|e| e.path())
}

/// Helper for reading/writing sysfs attributes under a platform device directory.
pub struct SysfsReader {
    base_path: PathBuf,
}

impl SysfsReader {
    pub fn new(base: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base.into(),
        }
    }

    /// Check whether the sysfs base directory exists.
    pub fn available(&self) -> bool {
        self.base_path.is_dir()
    }

    /// Check whether a specific attribute file exists.
    pub fn exists(&self, attr: &str) -> bool {
        self.base_path.join(attr).exists()
    }

    pub fn read_u8(&self, attr: &str) -> io::Result<u8> {
        self.read_str(attr)?
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn read_u16(&self, attr: &str) -> io::Result<u16> {
        self.read_str(attr)?
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn read_u32(&self, attr: &str) -> io::Result<u32> {
        self.read_str(attr)?
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn write_u8(&self, attr: &str, value: u8) -> io::Result<()> {
        self.write_str(attr, &value.to_string())
    }

    pub fn write_u32(&self, attr: &str, value: u32) -> io::Result<()> {
        self.write_str(attr, &value.to_string())
    }

    pub fn read_str(&self, attr: &str) -> io::Result<String> {
        let path = self.base_path.join(attr);
        let result = fs::read_to_string(&path).map(|s| s.trim().to_string());
        debug!("sysfs read  {path:?} → {result:?}");
        result
    }

    pub fn write_str(&self, attr: &str, value: &str) -> io::Result<()> {
        let path = self.base_path.join(attr);
        debug!("sysfs write {path:?} ← {value:?}");
        let result = fs::write(&path, value);
        debug!("sysfs write {path:?} result={result:?}");
        result
    }

    /// Read raw bytes from a binary sysfs attribute at a given offset.
    pub fn pread(&self, attr: &str, offset: u64, len: usize) -> io::Result<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};
        let path = self.base_path.join(attr);
        let mut f = fs::File::open(&path)?;
        f.seek(SeekFrom::Start(offset))?;
        let mut buf = vec![0u8; len];
        f.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Write raw bytes to a binary sysfs attribute at a given offset.
    pub fn pwrite(&self, attr: &str, offset: u64, data: &[u8]) -> io::Result<()> {
        use std::io::{Seek, SeekFrom, Write};
        let path = self.base_path.join(attr);
        let mut f = fs::OpenOptions::new().write(true).open(&path)?;
        f.seek(SeekFrom::Start(offset))?;
        f.write_all(data)
    }
}

impl std::fmt::Debug for SysfsReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SysfsReader")
            .field("base_path", &self.base_path)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, SysfsReader) {
        let dir = TempDir::new().unwrap();
        let reader = SysfsReader::new(dir.path());
        (dir, reader)
    }

    #[test]
    fn read_u8_valid() {
        let (dir, reader) = setup();
        fs::write(dir.path().join("cpu_temp"), "65\n").unwrap();
        assert_eq!(reader.read_u8("cpu_temp").unwrap(), 65);
    }

    #[test]
    fn read_u8_trims_whitespace() {
        let (dir, reader) = setup();
        fs::write(dir.path().join("val"), "  42 \n").unwrap();
        assert_eq!(reader.read_u8("val").unwrap(), 42);
    }

    #[test]
    fn read_u16_valid() {
        let (dir, reader) = setup();
        fs::write(dir.path().join("rpm"), "3200\n").unwrap();
        assert_eq!(reader.read_u16("rpm").unwrap(), 3200);
    }

    #[test]
    fn read_u32_valid() {
        let (dir, reader) = setup();
        fs::write(dir.path().join("fan_info"), "4294967295\n").unwrap();
        assert_eq!(reader.read_u32("fan_info").unwrap(), u32::MAX);
    }

    #[test]
    fn write_u8_roundtrip() {
        let (dir, reader) = setup();
        reader.write_u8("fan_mode", 1).unwrap();
        let content = fs::read_to_string(dir.path().join("fan_mode")).unwrap();
        assert_eq!(content, "1");
    }

    #[test]
    fn write_u32_roundtrip() {
        let (_dir, reader) = setup();
        reader.write_u32("fan_speed", 0x00_1A_2B_3C).unwrap();
        assert_eq!(reader.read_u32("fan_speed").unwrap(), 0x001A2B3C);
    }

    #[test]
    fn read_nonexistent_attr_errors() {
        let (_dir, reader) = setup();
        assert!(reader.read_u8("nonexistent").is_err());
    }

    #[test]
    fn read_invalid_data_errors() {
        let (dir, reader) = setup();
        fs::write(dir.path().join("bad"), "notanumber\n").unwrap();
        assert!(reader.read_u8("bad").is_err());
    }

    #[test]
    fn exists_check() {
        let (dir, reader) = setup();
        assert!(!reader.exists("foo"));
        fs::write(dir.path().join("foo"), "bar").unwrap();
        assert!(reader.exists("foo"));
    }

    #[test]
    fn available_check() {
        let (_dir, reader) = setup();
        assert!(reader.available());

        let missing = SysfsReader::new("/sys/devices/platform/nonexistent");
        assert!(!missing.available());
    }

    #[test]
    fn pread_pwrite_roundtrip() {
        let (dir, reader) = setup();
        // Create a binary file with known content
        let mut data = vec![0u8; 256];
        data[100] = 0xAB;
        data[101] = 0xCD;
        fs::write(dir.path().join("ec_ram"), &data).unwrap();

        // Read at offset
        let result = reader.pread("ec_ram", 100, 2).unwrap();
        assert_eq!(result, vec![0xAB, 0xCD]);

        // Write at offset
        reader.pwrite("ec_ram", 100, &[0x11, 0x22]).unwrap();
        let result = reader.pread("ec_ram", 100, 2).unwrap();
        assert_eq!(result, vec![0x11, 0x22]);
    }
}
