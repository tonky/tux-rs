//! ITE 8291 per-key RGB controller (6×21 matrix, 126 LEDs + 4 zones).
//!
//! Protocol: 8-byte SET_FEATURE reports for control commands, then
//! row-by-row data via output reports for per-key colors.

use std::io;

use super::color_scaling::ColorScaling;
use super::hidraw::HidrawOps;
use super::{KeyboardLed, Rgb};

/// Report size for control commands.
const CTRL_SIZE: usize = 8;

/// 6 rows of LEDs.
const NR_ROWS: usize = 6;
/// 21 columns per row.
const LEDS_PER_ROW: usize = 21;
/// Padding bytes before color data in each row packet.
const ROW_DATA_PADDING: usize = 2;
/// Total row data length: padding + 21*3 (RGB) = 65 bytes.
const ROW_DATA_LENGTH: usize = ROW_DATA_PADDING + LEDS_PER_ROW * 3;

/// Maximum hardware brightness (0x32 = 50).
const MAX_BRIGHTNESS: u8 = 0x32;

/// User-mode (per-key) animation code.
const PARAM_MODE_USER: u8 = 0x33;

/// Available animation modes.
const MODES: &[&str] = &[
    "static",    // per-key user mode
    "breathing", // 0x02
    "wave",      // 0x03
    "reactive",  // 0x04
    "rainbow",   // 0x05
    "ripple",    // 0x06
    "marquee",   // 0x09
    "raindrop",  // 0x0a
    "aurora",    // 0x0e
    "spark",     // 0x11
];

fn mode_to_code(mode: &str) -> Option<u8> {
    match mode {
        "static" => Some(PARAM_MODE_USER),
        "breathing" => Some(0x02),
        "wave" => Some(0x03),
        "reactive" => Some(0x04),
        "rainbow" => Some(0x05),
        "ripple" => Some(0x06),
        "marquee" => Some(0x09),
        "raindrop" => Some(0x0a),
        "aurora" => Some(0x0e),
        "spark" => Some(0x11),
        _ => None,
    }
}

pub struct Ite8291<H: HidrawOps = super::hidraw::HidrawDevice> {
    hid: H,
    scaling: ColorScaling,
    brightness: u8,
    on: bool,
    current_mode: String,
    /// Row data buffer: [row][col] → RGB packed in blue, green, red order.
    row_data: [[u8; ROW_DATA_LENGTH]; NR_ROWS],
    /// Per-zone color (4 zones, used for non-per-key modes).
    zone_colors: [Rgb; 4],
}

impl<H: HidrawOps> Ite8291<H> {
    pub fn new(hid: H) -> Self {
        Self::with_scaling(hid, ColorScaling::IDENTITY)
    }

    pub fn with_scaling(hid: H, scaling: ColorScaling) -> Self {
        Self {
            hid,
            scaling,
            brightness: MAX_BRIGHTNESS,
            on: true,
            current_mode: "static".to_string(),
            row_data: [[0u8; ROW_DATA_LENGTH]; NR_ROWS],
            zone_colors: [Rgb::WHITE; 4],
        }
    }

    fn scale_brightness(brightness: u8) -> u8 {
        super::scale_brightness(brightness, MAX_BRIGHTNESS)
    }

    /// Set a single LED color in the row buffer (with color scaling applied).
    fn set_row_col(&mut self, row: usize, col: usize, color: Rgb) {
        if row >= NR_ROWS || col >= LEDS_PER_ROW {
            return;
        }
        let scaled = self.scaling.apply(color);
        let blue_idx = ROW_DATA_PADDING + col;
        let green_idx = ROW_DATA_PADDING + LEDS_PER_ROW + col;
        let red_idx = ROW_DATA_PADDING + 2 * LEDS_PER_ROW + col;
        self.row_data[row][blue_idx] = scaled.b;
        self.row_data[row][green_idx] = scaled.g;
        self.row_data[row][red_idx] = scaled.r;
    }

    /// Fill the entire matrix with one color.
    fn fill_all(&mut self, color: Rgb) {
        for row in 0..NR_ROWS {
            for col in 0..LEDS_PER_ROW {
                self.set_row_col(row, col, color);
            }
        }
    }

    /// Send a control report (8-byte SET_FEATURE).
    fn write_control(&self, data: &[u8; CTRL_SIZE]) -> io::Result<()> {
        self.hid.set_feature(data)
    }

    /// Write per-key data to hardware: set user mode, then send 6 row packets.
    fn write_rows(&self) -> io::Result<()> {
        // Set params: 08 02 33 00 [brightness] 00 00 00
        let ctrl_params = [
            0x08,
            0x02,
            PARAM_MODE_USER,
            0x00,
            self.brightness.min(MAX_BRIGHTNESS),
            0x00,
            0x00,
            0x00,
        ];
        self.write_control(&ctrl_params)?;

        for row_index in 0..NR_ROWS {
            // Announce row: 16 00 [row] 00 00 00 00 00
            let announce = [0x16, 0x00, row_index as u8, 0x00, 0x00, 0x00, 0x00, 0x00];
            self.write_control(&announce)?;
            // Send row data as output report
            self.hid.write_output(&self.row_data[row_index])?;
        }
        Ok(())
    }

    /// Write power-off command.
    fn write_off(&self) -> io::Result<()> {
        self.write_control(&[0x08, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
    }

    /// Write animation mode with current brightness.
    fn write_mode(&self, mode_code: u8) -> io::Result<()> {
        if mode_code == PARAM_MODE_USER {
            return self.write_rows();
        }
        // Set params: 08 02 [mode] [speed] [brightness] 08 [behaviour] 00
        let ctrl = [
            0x08,
            0x02,
            mode_code,
            0x05, // mid-speed
            self.brightness.min(MAX_BRIGHTNESS),
            0x08,
            0x00,
            0x00,
        ];
        self.write_control(&ctrl)
    }
}

impl<H: HidrawOps> KeyboardLed for Ite8291<H> {
    fn set_brightness(&mut self, brightness: u8) -> io::Result<()> {
        self.brightness = Self::scale_brightness(brightness);
        if self.on { self.flush() } else { Ok(()) }
    }

    fn set_color(&mut self, zone: u8, color: Rgb) -> io::Result<()> {
        if (zone as usize) < 4 {
            self.zone_colors[zone as usize] = color;
        }
        // In static mode, fill entire keyboard with zone 0 color for simplicity
        if zone == 0 {
            self.fill_all(color);
        }
        Ok(())
    }

    fn set_mode(&mut self, mode: &str) -> io::Result<()> {
        if mode_to_code(mode).is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown mode: {mode}"),
            ));
        }
        self.current_mode = mode.to_string();
        if self.on { self.flush() } else { Ok(()) }
    }

    fn zone_count(&self) -> u8 {
        4
    }

    fn turn_off(&mut self) -> io::Result<()> {
        self.on = false;
        self.write_off()
    }

    fn turn_on(&mut self) -> io::Result<()> {
        self.on = true;
        self.flush()
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.on {
            return self.write_off();
        }
        let code = mode_to_code(&self.current_mode).unwrap_or(PARAM_MODE_USER);
        self.write_mode(code)
    }

    fn device_type(&self) -> &str {
        "ite8291"
    }

    fn available_modes(&self) -> Vec<String> {
        MODES.iter().map(|s| (*s).to_string()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hid::hidraw::MockHidraw;

    fn make_driver() -> Ite8291<MockHidraw> {
        Ite8291::new(MockHidraw::new(0x8291))
    }

    #[test]
    fn brightness_scales_to_device_range() {
        assert_eq!(Ite8291::<MockHidraw>::scale_brightness(0), 0);
        assert_eq!(Ite8291::<MockHidraw>::scale_brightness(255), MAX_BRIGHTNESS);
        assert_eq!(Ite8291::<MockHidraw>::scale_brightness(128), 25); // ~50%
    }

    #[test]
    fn set_color_fills_row_data() {
        let mut drv = make_driver();
        let color = Rgb::new(0xFF, 0x00, 0x80);
        drv.set_color(0, color).unwrap();
        // zone 0 fills all — verify first LED
        assert_eq!(drv.row_data[0][ROW_DATA_PADDING], 0x80); // blue
        assert_eq!(drv.row_data[0][ROW_DATA_PADDING + LEDS_PER_ROW], 0x00); // green
        assert_eq!(drv.row_data[0][ROW_DATA_PADDING + 2 * LEDS_PER_ROW], 0xFF); // red
    }

    #[test]
    fn flush_writes_control_and_row_data() {
        let mut drv = make_driver();
        drv.fill_all(Rgb::WHITE);
        drv.flush().unwrap();
        // Should have: 1 ctrl_params + 6 * (1 announce + 1 row output)
        let reports = drv.hid.sent_reports();
        assert_eq!(reports.len(), 7); // 1 params + 6 announces
        let outputs = drv.hid.sent_output_reports();
        assert_eq!(outputs.len(), 6); // 6 rows
    }

    #[test]
    fn turn_off_sends_off_command() {
        let mut drv = make_driver();
        drv.turn_off().unwrap();
        let reports = drv.hid.sent_reports();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0][0], 0x08);
        assert_eq!(reports[0][1], 0x01);
        // All remaining bytes should be 0
        assert!(reports[0][2..].iter().all(|&b| b == 0));
    }

    #[test]
    fn invalid_mode_returns_error() {
        let mut drv = make_driver();
        assert!(drv.set_mode("nonexistent").is_err());
    }

    #[test]
    fn available_modes_includes_static() {
        let drv = make_driver();
        let modes = drv.available_modes();
        assert!(modes.contains(&"static".to_string()));
        assert!(modes.contains(&"breathing".to_string()));
        assert_eq!(modes.len(), 10);
    }

    #[test]
    fn set_mode_breathing_writes_animation() {
        let mut drv = make_driver();
        drv.set_mode("breathing").unwrap();
        let reports = drv.hid.sent_reports();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0][0], 0x08);
        assert_eq!(reports[0][1], 0x02);
        assert_eq!(reports[0][2], 0x02); // breathing mode code
    }

    #[test]
    fn device_type_ite8291() {
        let drv = make_driver();
        assert_eq!(drv.device_type(), "ite8291");
    }

    #[test]
    fn row_col_out_of_bounds_is_noop() {
        let mut drv = make_driver();
        // These should not panic
        drv.set_row_col(7, 0, Rgb::WHITE);
        drv.set_row_col(0, 25, Rgb::WHITE);
    }
}
