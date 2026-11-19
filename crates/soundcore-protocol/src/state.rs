//! Device state assembled from events.

use crate::message::Event;
use crate::model::{self, Model};
use crate::types::{Battery, Equalizer, SoundModes};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeviceState {
    pub model: Option<&'static Model>,
    pub battery: Option<Battery>,
    pub firmware: Option<String>,
    pub serial: Option<String>,
    pub equalizer: Option<Equalizer>,
    pub sound_modes: Option<SoundModes>,
    /// A full state answer arrived.
    pub ready: bool,
}

impl DeviceState {
    pub fn new(model: Option<&'static Model>) -> Self {
        Self {
            model,
            ..Default::default()
        }
    }

    /// Applies an event; returns whether anything visible changed.
    pub fn apply(&mut self, ev: &Event) -> bool {
        let before = self.clone();
        match ev {
            Event::State(s) => {
                if s.battery.is_some() {
                    self.battery = s.battery;
                }
                if s.firmware.is_some() {
                    self.firmware.clone_from(&s.firmware);
                }
                if let Some(serial) = &s.serial {
                    self.serial = Some(serial.clone());
                    if self.model.is_none() {
                        self.model = model::from_serial(serial);
                    }
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
            Event::BatteryLevel(l) => {
                let b = self.battery.get_or_insert_default();
                b.level = *l;
            }
            Event::Charging(c) => {
                let b = self.battery.get_or_insert_default();
                b.charging = *c;
            }
            Event::Info { firmware, serial } => {
                if firmware.is_some() {
                    self.firmware.clone_from(firmware);
                }
                if serial.is_some() {
                    self.serial.clone_from(serial);
                }
            }
            Event::Ack(_) | Event::Unknown(_) => {}
        }
        *self != before
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
    use crate::types::AmbientMode;

    #[test]
    fn pushes_update_state() {
        let mut s = DeviceState::new(model::by_id("A3004"));
        assert!(s.apply(&Event::BatteryLevel(2)));
        assert!(s.apply(&Event::Charging(true)));
        assert_eq!(
            s.battery,
            Some(Battery {
                level: 2,
                charging: true
            })
        );
        let m = SoundModes::new(AmbientMode::Transparency);
        assert!(s.apply(&Event::SoundModes(m)));
        assert!(!s.apply(&Event::SoundModes(m)));
        assert!(!s.ready);
    }
}
