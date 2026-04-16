use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::io::AsRawFd;

/// Path to the tuxedo_io character device.
pub const TUXEDO_IO_DEVICE: &str = "/dev/tuxedo_io";

// ─── Ioctl request codes ──────────────────────────────────────────────────────
//
// Computed from vendor/tuxedo-drivers/src/tuxedo_io/tuxedo_io_ioctl.h using:
//   _IOC(dir, type, nr, size) = (dir << 30) | (size << 16) | (type << 8) | nr
//
// All pointer arguments are `int32_t*` → sizeof = 8 on 64-bit Linux.
//
//   IOCTL_MAGIC    = 0xEC
//   MAGIC_READ_CL  = 0xED  (_IOR)
//   MAGIC_WRITE_CL = 0xEE  (_IOW)
//   MAGIC_READ_UW  = 0xEF  (_IOR)
//   MAGIC_WRITE_UW = 0xF0  (_IOW / _IO)

/// `_IOR(0xEC, 0x05, int32_t*)` — check if hardware is Clevo interface (non-zero = yes).
pub const R_HWCHECK_CL: u64 = 0x8008_EC05;

/// `_IOR(0xEC, 0x06, int32_t*)` — check if hardware is Uniwill interface.
pub const R_HWCHECK_UW: u64 = 0x8008_EC06;

/// `_IOR(0xED, 0x10, int32_t*)` — Clevo fan 1 info (packed u32: duty|temp|RPM).
pub const R_CL_FANINFO1: u64 = 0x8008_ED10;
/// `_IOR(0xED, 0x11, int32_t*)` — Clevo fan 2 info.
pub const R_CL_FANINFO2: u64 = 0x8008_ED11;
/// `_IOR(0xED, 0x12, int32_t*)` — Clevo fan 3 info.
pub const R_CL_FANINFO3: u64 = 0x8008_ED12;

/// `_IOW(0xEE, 0x10, int32_t*)` — Clevo set fan speeds (packed: fan0|fan1<<8|fan2<<16).
pub const W_CL_FANSPEED: u64 = 0x4008_EE10;
/// `_IOW(0xEE, 0x11, int32_t*)` — Clevo restore fan auto mode.
pub const W_CL_FANAUTO: u64 = 0x4008_EE11;

/// `_IOR(0xEF, 0x10, int32_t*)` — Uniwill fan 1 speed (EC scale 0–200).
pub const R_UW_FANSPEED: u64 = 0x8008_EF10;
/// `_IOR(0xEF, 0x11, int32_t*)` — Uniwill fan 2 speed.
pub const R_UW_FANSPEED2: u64 = 0x8008_EF11;
/// `_IOR(0xEF, 0x12, int32_t*)` — Uniwill fan 1 temperature (°C).
pub const R_UW_FAN_TEMP: u64 = 0x8008_EF12;

/// `_IOW(0xF0, 0x10, int32_t*)` — Uniwill set fan 1 speed (EC scale 0–200).
pub const W_UW_FANSPEED: u64 = 0x4008_F010;
/// `_IOW(0xF0, 0x11, int32_t*)` — Uniwill set fan 2 speed.
pub const W_UW_FANSPEED2: u64 = 0x4008_F011;
/// `_IOW(0xF0, 0x13, int32_t*)` — Uniwill enable/disable manual fan control mode.
///
/// Write `1` to tell the EC that the driver is managing fan speeds via
/// `W_UW_FANSPEED`/`W_UW_FANSPEED2`.  Write `0` to release EC control.
/// Must be called before the first `W_UW_FANSPEED` write; without it the
/// EC's firmware thermal loop overrides manual speeds within ~1 second.
pub const W_UW_MODE_ENABLE: u64 = 0x4008_F013;
/// `_IO(0xF0, 0x14)` — Uniwill restore fan auto mode (no data argument).
pub const W_UW_FANAUTO: u64 = 0x0000_F014;

/// `_IOW(0xF0, 0x18, int32_t*)` — Uniwill set performance profile (1=power-save, 2=enthusiast, 3=overboost).
pub const W_UW_PERF_PROF: u64 = 0x4008_F018;

// ─── Trait ────────────────────────────────────────────────────────────────────

/// Abstraction over the `/dev/tuxedo_io` ioctl interface.
///
/// Implemented by `TuxedoIoDevice` (real hardware) and `MockTuxedoIo` (tests).
pub trait TuxedoIo: Send + Sync {
    /// Issue a read-type ioctl (`_IOR`). The 32-bit result is returned.
    fn read_i32(&self, cmd: u64) -> io::Result<i32>;

    /// Issue a write-type ioctl (`_IOW`) with a 32-bit argument.
    fn write_i32(&self, cmd: u64, val: i32) -> io::Result<()>;

    /// Issue a no-data ioctl (`_IO`). Passes 0 as the kernel argument.
    fn ioctl_noarg(&self, cmd: u64) -> io::Result<()>;
}

// ─── Real implementation ──────────────────────────────────────────────────────

/// Opens and holds `/dev/tuxedo_io`.
pub struct TuxedoIoDevice {
    file: File,
}

impl TuxedoIoDevice {
    /// Open the character device. Returns `None` if tuxedo_io is not loaded.
    pub fn open() -> Option<Self> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(TUXEDO_IO_DEVICE)
            .ok()
            .map(|file| Self { file })
    }

    /// Return `true` if the kernel identifies this as Clevo hardware.
    pub fn is_clevo_hardware(&self) -> bool {
        self.read_i32(R_HWCHECK_CL).map(|v| v != 0).unwrap_or(false)
    }

    /// Return `true` if the kernel identifies this as Uniwill hardware.
    pub fn is_uniwill_hardware(&self) -> bool {
        self.read_i32(R_HWCHECK_UW).map(|v| v != 0).unwrap_or(false)
    }
}

impl TuxedoIo for TuxedoIoDevice {
    fn read_i32(&self, cmd: u64) -> io::Result<i32> {
        let mut val: i32 = 0;
        // SAFETY: cmd is a valid tuxedo_io ioctl code; val is a properly aligned i32.
        let ret =
            unsafe { nix::libc::ioctl(self.file.as_raw_fd(), cmd as nix::libc::c_ulong, &mut val) };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(val)
    }

    fn write_i32(&self, cmd: u64, val: i32) -> io::Result<()> {
        let mut arg = val;
        // SAFETY: cmd is a valid tuxedo_io ioctl code; arg is a properly aligned i32.
        let ret =
            unsafe { nix::libc::ioctl(self.file.as_raw_fd(), cmd as nix::libc::c_ulong, &mut arg) };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn ioctl_noarg(&self, cmd: u64) -> io::Result<()> {
        // SAFETY: cmd is a valid _IO ioctl; no data pointer needed.
        let ret =
            unsafe { nix::libc::ioctl(self.file.as_raw_fd(), cmd as nix::libc::c_ulong, 0i32) };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

// ─── Mock implementation ──────────────────────────────────────────────────────

/// In-process mock for unit tests. Pre-program read responses and inspect writes.
#[cfg(test)]
pub struct MockTuxedoIo {
    /// Pre-programmed return values for read-type ioctls.
    reads: std::collections::HashMap<u64, i32>,
    /// All write_i32 calls recorded as (cmd, value).
    pub writes: std::sync::Mutex<Vec<(u64, i32)>>,
    /// All ioctl_noarg calls recorded as cmd.
    pub noarg_calls: std::sync::Mutex<Vec<u64>>,
    /// When set, all read-type ioctls return an error.
    fail_reads: std::sync::atomic::AtomicBool,
    /// When set, all write-type ioctls and no-arg ioctls return an error.
    fail_writes: std::sync::atomic::AtomicBool,
}

#[cfg(test)]
impl Default for MockTuxedoIo {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl MockTuxedoIo {
    pub fn new() -> Self {
        Self {
            reads: std::collections::HashMap::new(),
            writes: std::sync::Mutex::new(Vec::new()),
            noarg_calls: std::sync::Mutex::new(Vec::new()),
            fail_reads: std::sync::atomic::AtomicBool::new(false),
            fail_writes: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Pre-program a response for a read-type ioctl.
    pub fn set_read(&mut self, cmd: u64, val: i32) {
        self.reads.insert(cmd, val);
    }

    /// When `fail` is `true`, all read-type ioctls will return an error.
    pub fn set_fail_reads(&self, fail: bool) {
        self.fail_reads
            .store(fail, std::sync::atomic::Ordering::Relaxed);
    }

    /// When `fail` is `true`, all write-type and no-arg ioctls will return an error.
    pub fn set_fail_writes(&self, fail: bool) {
        self.fail_writes
            .store(fail, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
impl TuxedoIo for MockTuxedoIo {
    fn read_i32(&self, cmd: u64) -> io::Result<i32> {
        if self.fail_reads.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(io::Error::other("simulated read failure"));
        }
        self.reads.get(&cmd).copied().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("no mock for cmd {cmd:#x}"))
        })
    }

    fn write_i32(&self, cmd: u64, val: i32) -> io::Result<()> {
        if self.fail_writes.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(io::Error::other("simulated write failure"));
        }
        self.writes.lock().unwrap().push((cmd, val));
        Ok(())
    }

    fn ioctl_noarg(&self, cmd: u64) -> io::Result<()> {
        if self.fail_writes.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(io::Error::other("simulated write failure"));
        }
        self.noarg_calls.lock().unwrap().push(cmd);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_read_returns_programmed_value() {
        let mut m = MockTuxedoIo::new();
        m.set_read(R_CL_FANINFO1, 0x00401E32u32 as i32);
        assert_eq!(m.read_i32(R_CL_FANINFO1).unwrap(), 0x00401E32u32 as i32);
    }

    #[test]
    fn mock_read_missing_returns_error() {
        let m = MockTuxedoIo::new();
        assert!(m.read_i32(R_CL_FANINFO1).is_err());
    }

    #[test]
    fn mock_write_records_calls() {
        let m = MockTuxedoIo::new();
        m.write_i32(W_CL_FANSPEED, 0x804020).unwrap();
        let recorded = m.writes.lock().unwrap();
        assert_eq!(recorded[0], (W_CL_FANSPEED, 0x804020));
    }

    #[test]
    fn mock_noarg_records_calls() {
        let m = MockTuxedoIo::new();
        m.ioctl_noarg(W_UW_FANAUTO).unwrap();
        let calls = m.noarg_calls.lock().unwrap();
        assert_eq!(calls[0], W_UW_FANAUTO);
    }

    #[test]
    fn mock_fail_reads_blocks_all_reads() {
        let mut m = MockTuxedoIo::new();
        m.set_read(R_CL_FANINFO1, 42);
        m.set_fail_reads(true);
        assert!(
            m.read_i32(R_CL_FANINFO1).is_err(),
            "should error when fail_reads is set"
        );
    }

    #[test]
    fn mock_fail_writes_blocks_writes_and_noarg() {
        let m = MockTuxedoIo::new();
        m.set_fail_writes(true);
        assert!(
            m.write_i32(W_CL_FANSPEED, 0).is_err(),
            "write_i32 should error"
        );
        assert!(
            m.ioctl_noarg(W_UW_FANAUTO).is_err(),
            "ioctl_noarg should error"
        );
    }

    #[test]
    fn mock_fail_reads_does_not_affect_writes() {
        let m = MockTuxedoIo::new();
        m.set_fail_reads(true);
        assert!(
            m.write_i32(W_CL_FANSPEED, 0).is_ok(),
            "writes should still work"
        );
    }

    #[test]
    fn mock_fail_writes_does_not_affect_reads() {
        let mut m = MockTuxedoIo::new();
        m.set_read(R_CL_FANINFO1, 42);
        m.set_fail_writes(true);
        assert_eq!(
            m.read_i32(R_CL_FANINFO1).unwrap(),
            42,
            "reads should still work"
        );
    }
}
