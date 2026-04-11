//! D-Bus Keyboard interface: `com.tuxedocomputers.tccd.Keyboard`.
//!
//! Provides brightness, color, and mode control for ITE HID keyboards.

use zbus::interface;

use crate::hid::KeyboardLed;
use crate::hid::Rgb;
use crate::hid::SharedKeyboard;

/// D-Bus object implementing the Keyboard interface.
pub struct KeyboardInterface {
    keyboards: Vec<SharedKeyboard>,
}

impl KeyboardInterface {
    pub fn new(keyboards: Vec<SharedKeyboard>) -> Self {
        Self { keyboards }
    }

    fn with_keyboard<F, R>(&self, index: usize, f: F) -> zbus::fdo::Result<R>
    where
        F: FnOnce(&mut Box<dyn KeyboardLed>) -> std::io::Result<R>,
    {
        let kb = self.keyboards.get(index).ok_or_else(|| {
            zbus::fdo::Error::InvalidArgs(format!("invalid keyboard index: {index}"))
        })?;
        let mut guard = kb
            .lock()
            .map_err(|e| zbus::fdo::Error::Failed(format!("lock poisoned: {e}")))?;
        f(&mut guard).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }
}

#[interface(name = "com.tuxedocomputers.tccd.Keyboard")]
impl KeyboardInterface {
    /// Get the number of discovered keyboard LED controllers.
    fn keyboard_count(&self) -> u32 {
        self.keyboards.len() as u32
    }

    /// Set brightness for a keyboard (0–255).
    fn set_brightness(&self, keyboard_index: u32, brightness: u8) -> zbus::fdo::Result<()> {
        self.with_keyboard(keyboard_index as usize, |kb| kb.set_brightness(brightness))
    }

    /// Set color for a zone on a keyboard.
    fn set_color(
        &self,
        keyboard_index: u32,
        zone: u8,
        red: u8,
        green: u8,
        blue: u8,
    ) -> zbus::fdo::Result<()> {
        self.with_keyboard(keyboard_index as usize, |kb| {
            if zone >= kb.zone_count() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "zone {zone} out of range (device has {} zones)",
                        kb.zone_count()
                    ),
                ));
            }
            kb.set_color(zone, Rgb::new(red, green, blue))
        })
    }

    /// Set animation mode for a keyboard.
    fn set_mode(&self, keyboard_index: u32, mode: &str) -> zbus::fdo::Result<()> {
        self.with_keyboard(keyboard_index as usize, |kb| kb.set_mode(mode))
    }

    /// Turn off a keyboard's LEDs.
    fn turn_off(&self, keyboard_index: u32) -> zbus::fdo::Result<()> {
        self.with_keyboard(keyboard_index as usize, |kb| kb.turn_off())
    }

    /// Turn on a keyboard's LEDs.
    fn turn_on(&self, keyboard_index: u32) -> zbus::fdo::Result<()> {
        self.with_keyboard(keyboard_index as usize, |kb| kb.turn_on())
    }

    /// Flush buffered state to hardware.
    fn flush(&self, keyboard_index: u32) -> zbus::fdo::Result<()> {
        self.with_keyboard(keyboard_index as usize, |kb| kb.flush())
    }

    /// Get keyboard info as TOML.
    fn get_keyboard_info(&self) -> zbus::fdo::Result<String> {
        let mut infos = Vec::new();
        for (i, kb) in self.keyboards.iter().enumerate() {
            let guard = kb
                .lock()
                .map_err(|e| zbus::fdo::Error::Failed(format!("lock poisoned: {e}")))?;
            infos.push(format!(
                "[[keyboards]]\nindex = {i}\ndevice_type = \"{}\"\nzone_count = {}\navailable_modes = {:?}\n",
                guard.device_type(),
                guard.zone_count(),
                guard.available_modes(),
            ));
        }
        Ok(infos.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    struct MockKb {
        brightness: u8,
        color: Rgb,
        on: bool,
        mode: String,
    }

    impl MockKb {
        fn new() -> Self {
            Self {
                brightness: 255,
                color: Rgb::WHITE,
                on: true,
                mode: "static".to_string(),
            }
        }
    }

    impl KeyboardLed for MockKb {
        fn set_brightness(&mut self, b: u8) -> std::io::Result<()> {
            self.brightness = b;
            Ok(())
        }
        fn set_color(&mut self, _zone: u8, c: Rgb) -> std::io::Result<()> {
            self.color = c;
            Ok(())
        }
        fn set_mode(&mut self, m: &str) -> std::io::Result<()> {
            self.mode = m.to_string();
            Ok(())
        }
        fn zone_count(&self) -> u8 {
            4
        }
        fn turn_off(&mut self) -> std::io::Result<()> {
            self.on = false;
            Ok(())
        }
        fn turn_on(&mut self) -> std::io::Result<()> {
            self.on = true;
            Ok(())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
        fn device_type(&self) -> &str {
            "mock"
        }
        fn available_modes(&self) -> Vec<String> {
            vec!["static".to_string()]
        }
    }

    fn wrap(kb: MockKb) -> SharedKeyboard {
        Arc::new(Mutex::new(Box::new(kb)))
    }

    #[test]
    fn keyboard_count_matches_registered() {
        let iface = KeyboardInterface::new(vec![wrap(MockKb::new()), wrap(MockKb::new())]);
        assert_eq!(iface.keyboards.len(), 2);
    }

    #[test]
    fn invalid_index_returns_error() {
        let iface = KeyboardInterface::new(vec![]);
        let result = iface.with_keyboard(0, |kb| kb.flush());
        assert!(result.is_err());
    }

    #[test]
    fn set_operations_succeed() {
        let iface = KeyboardInterface::new(vec![wrap(MockKb::new())]);
        assert!(iface.with_keyboard(0, |kb| kb.set_brightness(128)).is_ok());
        assert!(
            iface
                .with_keyboard(0, |kb| kb.set_color(0, Rgb::new(255, 0, 0)))
                .is_ok()
        );
        assert!(iface.with_keyboard(0, |kb| kb.turn_off()).is_ok());
        assert!(iface.with_keyboard(0, |kb| kb.turn_on()).is_ok());
    }
}
