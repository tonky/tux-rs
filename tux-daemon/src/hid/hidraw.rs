//! Low-level hidraw ioctl wrappers for USB HID feature reports.
//!
//! Uses `nix` ioctl macros to send SET_FEATURE and GET_FEATURE reports
//! to `/dev/hidrawN` devices.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use nix::libc;

/// Raw HID device info returned by `HIDIOCGRAWINFO`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HidrawInfo {
    pub bus_type: u32,
    pub vendor_id: u16,
    pub product_id: u16,
}

/// Kernel `hidraw_devinfo` struct layout.
#[repr(C)]
struct hidraw_devinfo {
    bustype: u32,
    vendor: i16,
    product: i16,
}

// ioctl numbers from <linux/hidraw.h>
const HIDIOCGRAWINFO: libc::c_ulong = 0x80084803; // _IOR('H', 0x03, struct hidraw_devinfo)

// HIDIOCSFEATURE(len) = _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x06, len)
fn hidiocsfeature(len: usize) -> libc::c_ulong {
    let dir = 3u64; // _IOC_WRITE | _IOC_READ
    let typ = b'H' as u64;
    let nr = 0x06u64;
    let size = len as u64;
    ((dir << 30) | (typ << 8) | nr | (size << 16)) as libc::c_ulong
}

// HIDIOCGFEATURE(len) = _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x07, len)
fn hidiocgfeature(len: usize) -> libc::c_ulong {
    let dir = 3u64;
    let typ = b'H' as u64;
    let nr = 0x07u64;
    let size = len as u64;
    ((dir << 30) | (typ << 8) | nr | (size << 16)) as libc::c_ulong
}

/// Handle to an opened `/dev/hidrawN` device.
pub struct HidrawDevice {
    file: File,
    path: PathBuf,
    info: HidrawInfo,
}

impl std::fmt::Debug for HidrawDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HidrawDevice")
            .field("path", &self.path)
            .field("info", &self.info)
            .finish()
    }
}

impl HidrawDevice {
    /// Open a `/dev/hidrawN` device and query its info.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new().read(true).write(true).open(&path)?;
        let info = Self::query_info(&file)?;
        Ok(Self { file, path, info })
    }

    /// Get device info (VID, PID, bus type).
    pub fn info(&self) -> &HidrawInfo {
        &self.info
    }

    /// Device path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Send a SET_FEATURE report.
    pub fn set_feature(&self, data: &[u8]) -> io::Result<()> {
        let fd = self.file.as_raw_fd();
        let ret = unsafe { libc::ioctl(fd, hidiocsfeature(data.len()), data.as_ptr()) };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Send a GET_FEATURE report.
    pub fn get_feature(&self, buf: &mut [u8]) -> io::Result<usize> {
        let fd = self.file.as_raw_fd();
        let ret = unsafe { libc::ioctl(fd, hidiocgfeature(buf.len()), buf.as_mut_ptr()) };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(ret as usize)
    }

    /// Write an output report (interrupt endpoint data).
    pub fn write_output(&self, data: &[u8]) -> io::Result<()> {
        use std::io::Write;
        (&self.file).write_all(data)
    }

    fn query_info(file: &File) -> io::Result<HidrawInfo> {
        let fd = file.as_raw_fd();
        let mut info = hidraw_devinfo {
            bustype: 0,
            vendor: 0,
            product: 0,
        };
        let ret = unsafe { libc::ioctl(fd, HIDIOCGRAWINFO, &mut info) };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(HidrawInfo {
            bus_type: info.bustype,
            vendor_id: info.vendor as u16,
            product_id: info.product as u16,
        })
    }
}

/// Trait for abstracting hidraw operations (enables mock testing).
pub trait HidrawOps: Send + Sync {
    fn set_feature(&self, data: &[u8]) -> io::Result<()>;
    fn get_feature(&self, buf: &mut [u8]) -> io::Result<usize>;
    fn write_output(&self, data: &[u8]) -> io::Result<()>;
    fn product_id(&self) -> u16;
}

impl HidrawOps for HidrawDevice {
    fn set_feature(&self, data: &[u8]) -> io::Result<()> {
        self.set_feature(data)
    }

    fn get_feature(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.get_feature(buf)
    }

    fn write_output(&self, data: &[u8]) -> io::Result<()> {
        self.write_output(data)
    }

    fn product_id(&self) -> u16 {
        self.info.product_id
    }
}

/// Mock HID device for testing.
#[cfg(test)]
pub struct MockHidraw {
    pub pid: u16,
    pub reports: std::sync::Mutex<Vec<Vec<u8>>>,
    pub output_reports: std::sync::Mutex<Vec<Vec<u8>>>,
}

#[cfg(test)]
impl MockHidraw {
    pub fn new(pid: u16) -> Self {
        Self {
            pid,
            reports: std::sync::Mutex::new(Vec::new()),
            output_reports: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn sent_reports(&self) -> Vec<Vec<u8>> {
        self.reports.lock().unwrap().clone()
    }

    pub fn sent_output_reports(&self) -> Vec<Vec<u8>> {
        self.output_reports.lock().unwrap().clone()
    }
}

#[cfg(test)]
impl HidrawOps for MockHidraw {
    fn set_feature(&self, data: &[u8]) -> io::Result<()> {
        self.reports.lock().unwrap().push(data.to_vec());
        Ok(())
    }

    fn get_feature(&self, buf: &mut [u8]) -> io::Result<usize> {
        // Return zeros
        buf.fill(0);
        Ok(buf.len())
    }

    fn write_output(&self, data: &[u8]) -> io::Result<()> {
        self.output_reports.lock().unwrap().push(data.to_vec());
        Ok(())
    }

    fn product_id(&self) -> u16 {
        self.pid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_hidraw_records_feature_reports() {
        let mock = MockHidraw::new(0x8291);
        mock.set_feature(&[0x08, 0x01, 0x00]).unwrap();
        mock.set_feature(&[0x09, 0x02, 0x32]).unwrap();
        let reports = mock.sent_reports();
        assert_eq!(reports.len(), 2);
        assert_eq!(reports[0], vec![0x08, 0x01, 0x00]);
        assert_eq!(reports[1], vec![0x09, 0x02, 0x32]);
    }

    #[test]
    fn mock_hidraw_records_output_reports() {
        let mock = MockHidraw::new(0x8291);
        mock.write_output(&[0x00; 65]).unwrap();
        assert_eq!(mock.sent_output_reports().len(), 1);
    }

    #[test]
    fn ioctl_numbers_valid() {
        // Verify our ioctl number calculations produce sane values
        let set_feat = hidiocsfeature(8);
        let get_feat = hidiocgfeature(64);
        // Both should be non-zero
        assert_ne!(set_feat, 0);
        assert_ne!(get_feat, 0);
        // SET and GET should differ (different nr)
        assert_ne!(hidiocsfeature(64), hidiocgfeature(64));
    }
}
