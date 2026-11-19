//! Device state assembled from events.

use crate::message::Event;
use crate::model::{self, Capabilities, Kind, Model};
use crate::types::{Batteries, Equalizer, SoundModes};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceState {
    /// The product, when it is in the table.
    pub model: Option<&'static Model>,
    /// Product number read from the vendor service UUID or the serial ("3951"), even
    /// for products the table does not describe.
    pub product: Option<String>,
    /// What the app may offer: from the table, or generic for unknown products.
    pub caps: Capabilities,
    pub batteries: Batteries,
    pub firmware: Option<String>,
    pub serial: Option<String>,
    pub equalizer: Option<Equalizer>,
    pub sound_modes: Option<SoundModes>,
    /// Something answered: the device is talking to us.
    pub ready: bool,
}

impl Default for DeviceState {
    fn default() -> Self {
        Self::new(None, Kind::Headphones)
    }
}

impl DeviceState {
    pub fn new(model: Option<&'static Model>, kind: Kind) -> Self {
        Self {
            model,
            product: model.map(|m| m.id[1..].to_string()),
            caps: model.map(|m| m.caps).unwrap_or(model::generic(kind)),
            batteries: Batteries::default(),
            firmware: None,
            serial: None,
            equalizer: None,
            sound_modes: None,
            ready: false,
        }
    }

    pub fn set_model(&mut self, m: &'static Model) {
        self.model = Some(m);
        self.product = Some(m.id[1..].to_string());
        self.caps = m.caps;
    }

    /// Product number ("A3951") for display.
    pub fn product_id(&self) -> Option<String> {
        self.product.as_ref().map(|p| format!("A{p}"))
    }

    /// Whether this product is described by the table.
    pub fn described(&self) -> bool {
        self.model.is_some_and(|m| !m.caps.layout.is_empty())
    }

    /// Whether the sound modes can be written: their bytes are known, or the product
    /// uses the classic 4-byte layout that can be built from scratch.
    pub fn can_write_sound_modes(&self) -> bool {
        self.sound_modes.is_some()
            || self.model.is_none()
            || self.caps.layout.contains(&crate::model::Field::SoundModes)
    }

    /// Applies an event; returns whether anything visible changed.
    pub fn apply(&mut self, ev: &Event) -> bool {
        let before = self.clone();
        match ev {
            Event::State(s) => {
                let b = &s.batteries;
                if !b.is_empty() {
                    self.batteries = *b;
                }
                if s.firmware.is_some() {
                    self.firmware.clone_from(&s.firmware);
                }
                if let Some(serial) = &s.serial {
                    self.on_serial(serial);
                }
                if s.equalizer.is_some() {
                    self.equalizer = s.equalizer;
                }
                if s.sound_modes.is_some() {
                    self.sound_modes = s.sound_modes;
                }
                self.ready = true;
            }
            Event::SoundModes(m) => self.sound_modes = Some(*m),
            Event::BatteryLevels(l) => {
                self.batteries.set_levels(l);
                self.ready = true;
            }
            Event::Charging(c) => self.batteries.set_charging(c),
            Event::Info { firmware, serial } => {
                if firmware.is_some() {
                    self.firmware.clone_from(firmware);
                }
                if let Some(serial) = serial {
                    self.on_serial(serial);
                }
                self.ready = true;
            }
            Event::Ack(_) | Event::Unknown(_) => {}
        }
        *self != before
    }

    fn on_serial(&mut self, serial: &str) {
        self.serial = Some(serial.to_string());
        if self.model.is_none() {
            if let Some(m) = model::from_serial(serial) {
                self.set_model(m);
            } else if self.product.is_none()
                && serial.len() >= 4
                && serial[..4].chars().all(|c| c.is_ascii_digit())
            {
                self.product = Some(serial[..4].to_string());
            }
        }
    }

    /// Optimistic update after a write.
    pub fn set_sound_modes(&mut self, m: SoundModes) {
        self.sound_modes = Some(m);
    }

    pub fn set_equalizer(&mut self, eq: Equalizer) {
        self.equalizer = Some(eq);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AmbientMode, Level};

    #[test]
    fn pushes_update_state() {
        let mut s = DeviceState::new(model::by_id("A3004"), Kind::Headphones);
        assert!(s.apply(&Event::BatteryLevels(vec![2])));
        assert!(s.apply(&Event::Charging(vec![1])));
        assert_eq!(
            s.batteries.single,
            Some(Level {
                level: 2,
                charging: true
            })
        );
        let m = SoundModes::new(AmbientMode::Transparency);
        assert!(s.apply(&Event::SoundModes(m)));
        assert!(!s.apply(&Event::SoundModes(m)));
    }

    #[test]
    fn unknown_product_from_serial() {
        let mut s = DeviceState::new(None, Kind::Earbuds);
        s.apply(&Event::Info {
            firmware: Some("01.10".into()),
            serial: Some("9999ABCDEF123456".into()),
        });
        assert_eq!(s.product_id().as_deref(), Some("A9999"));
        assert!(s.model.is_none() && s.ready);
        assert_eq!(s.caps.kind, Kind::Earbuds);
    }
}
