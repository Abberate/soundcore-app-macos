//! Typed requests and events.

use crate::frame::{Command, Frame, encode};
use crate::model::Field;
use crate::types::{Batteries, Equalizer, Level, SoundModes, cmd};

/// What a full state answer carried. Every field is optional: layouts differ between
/// products, and unknown products only answer the individual requests.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StateReport {
    pub batteries: Batteries,
    pub firmware: Option<String>,
    pub serial: Option<String>,
    pub equalizer: Option<Equalizer>,
    pub sound_modes: Option<SoundModes>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    State(StateReport),
    SoundModes(SoundModes),
    /// One level (single battery) or left, right[, case].
    BatteryLevels(Vec<u8>),
    /// Same layout as [`Event::BatteryLevels`].
    Charging(Vec<u8>),
    Info {
        firmware: Option<String>,
        serial: Option<String>,
    },
    /// Empty answer to a write.
    Ack(Command),
    Unknown(Frame),
}

/// Printable ASCII text, `None` when the field is blank (all 0x00 or 0xff).
fn text(b: &[u8]) -> Option<String> {
    if b.iter().all(|&c| c == 0 || c == 0xff) {
        return None;
    }
    let s: String = b
        .iter()
        .filter(|c| c.is_ascii_graphic() || **c == b' ')
        .map(|&c| c as char)
        .collect();
    let s = s.trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// Firmware of one or two parts ("02.61", or "02.61 / 02.61" for left and right).
fn firmware_pair(b: &[u8]) -> Option<String> {
    match (text(&b[..5]), b.get(5..10).and_then(text)) {
        (Some(l), Some(r)) if l != r => Some(format!("{l} / {r}")),
        (Some(l), _) => Some(l),
        (None, r) => r,
    }
}

/// Walks a product's state layout.
pub fn parse_state(b: &[u8], layout: &[Field]) -> StateReport {
    let mut r = StateReport::default();
    let mut at = 0usize;
    for field in layout {
        let rest = &b[at.min(b.len())..];
        let used = match *field {
            Field::SingleBattery if rest.len() >= 2 => {
                r.batteries.single = Some(Level {
                    level: rest[0],
                    charging: rest[1] != 0,
                });
                2
            }
            Field::BatteryLevel if !rest.is_empty() => {
                r.batteries.single = Some(Level {
                    level: rest[0],
                    charging: false,
                });
                1
            }
            Field::DualBatteryLevel if rest.len() >= 2 => {
                r.batteries.left = Some(Level {
                    level: rest[0],
                    charging: false,
                });
                r.batteries.right = Some(Level {
                    level: rest[1],
                    charging: false,
                });
                2
            }
            Field::DualBattery if rest.len() >= 4 => {
                r.batteries.left = Some(Level {
                    level: rest[0],
                    charging: rest[2] != 0,
                });
                r.batteries.right = Some(Level {
                    level: rest[1],
                    charging: rest[3] != 0,
                });
                4
            }
            Field::CaseBattery if !rest.is_empty() => {
                r.batteries.case = Some(Level {
                    level: rest[0],
                    charging: false,
                });
                1
            }
            Field::Firmware if rest.len() >= 5 => {
                r.firmware = text(&rest[..5]);
                5
            }
            Field::DualFirmware if rest.len() >= 10 => {
                r.firmware = firmware_pair(&rest[..10]);
                10
            }
            Field::Serial if rest.len() >= 16 => {
                r.serial = text(&rest[..16]);
                16
            }
            Field::Equalizer { channels, bands } => {
                match Equalizer::parse_with(rest, channels as usize, bands as usize) {
                    Some((eq, used)) => {
                        r.equalizer = Some(eq);
                        used
                    }
                    None => break,
                }
            }
            Field::SoundModes if rest.len() >= 4 => match SoundModes::parse(&rest[..4]) {
                Some(m) => {
                    r.sound_modes = Some(m);
                    4
                }
                None => 4,
            },
            Field::SoundModesV2(n) if rest.len() >= n as usize => {
                r.sound_modes = SoundModes::parse(&rest[..n as usize]);
                n as usize
            }
            Field::Skip(n) if rest.len() >= n as usize => n as usize,
            _ => break,
        };
        at += used;
    }
    r
}

impl Event {
    /// `layout` describes the product's state answer (empty when unknown).
    pub fn parse(f: &Frame, layout: &[Field]) -> Event {
        let b = f.body.as_slice();
        if b.is_empty() && cmd::is_write(f.command) {
            return Event::Ack(f.command);
        }
        match f.command {
            cmd::STATE if !b.is_empty() && !layout.is_empty() => {
                Event::State(parse_state(b, layout))
            }
            cmd::SOUND_MODES => match SoundModes::parse(b) {
                Some(m) => Event::SoundModes(m),
                None => Event::Unknown(f.clone()),
            },
            cmd::BATTERY_LEVEL if !b.is_empty() => {
                Event::BatteryLevels(b[..b.len().min(3)].to_vec())
            }
            cmd::BATTERY_CHARGING if !b.is_empty() => Event::Charging(b[..b.len().min(3)].to_vec()),
            cmd::INFO if b.len() >= 16 => {
                // Firmware(s) first (5 bytes each), serial number last (16 bytes).
                let serial = text(&b[b.len() - 16..]);
                let fw = &b[..b.len() - 16];
                let firmware = match fw.len() {
                    n if n >= 10 => firmware_pair(&fw[..10]),
                    n if n >= 5 => text(&fw[..5]),
                    _ => None,
                };
                Event::Info { firmware, serial }
            }
            _ => Event::Unknown(f.clone()),
        }
    }
}

/// Encoded requests.
pub mod request {
    use super::*;
    use crate::model::{EqSpec, EqWrite};

    pub fn state() -> Vec<u8> {
        encode(cmd::STATE, &[])
    }

    pub fn battery_levels() -> Vec<u8> {
        encode(cmd::BATTERY_LEVEL, &[])
    }

    pub fn charging() -> Vec<u8> {
        encode(cmd::BATTERY_CHARGING, &[])
    }

    pub fn info() -> Vec<u8> {
        encode(cmd::INFO, &[])
    }

    pub fn sound_modes(m: &SoundModes) -> Vec<u8> {
        encode(cmd::SET_SOUND_MODES, &m.bytes())
    }

    /// Equalizer write in the product's format. With [`EqWrite::Applied`], the device
    /// takes the curve to display and the curve to apply; this app applies exactly the
    /// curve it shows.
    pub fn equalizer_for(eq: &Equalizer, spec: &EqSpec) -> Vec<u8> {
        let (channels, bands) = (spec.channels as usize, spec.bands as usize);
        match spec.write {
            EqWrite::Plain => encode(cmd::SET_EQUALIZER, &eq.bytes_for(channels, bands)),
            EqWrite::Applied => {
                let mut body = eq.bytes_for(channels, bands);
                for _ in 0..channels {
                    body.extend(eq.band_bytes(bands));
                }
                encode(cmd::SET_EQUALIZER_APPLIED, &body)
            }
        }
    }

    /// The Q20i format.
    pub fn equalizer(eq: &Equalizer) -> Vec<u8> {
        equalizer_for(
            eq,
            &EqSpec {
                channels: 1,
                bands: 10,
                write: EqWrite::Applied,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{Reassembler, encode_from_device};
    use crate::model;
    use crate::types::{AmbientMode, CUSTOM_PRESET};

    fn q20i() -> &'static [Field] {
        model::by_id("A3004").unwrap().caps.layout
    }

    pub(crate) fn sample_state_body() -> Vec<u8> {
        let mut b = vec![4, 0];
        b.extend_from_slice(b"02.61");
        b.extend_from_slice(b"3004ABCDEF123456");
        b.extend_from_slice(&Equalizer::preset(0x11).unwrap().bytes());
        b.extend_from_slice(&[0, 0, 0, 0]);
        b
    }

    #[test]
    fn parses_full_state() {
        let frame = encode_from_device(cmd::STATE, &sample_state_body());
        let f = Reassembler::new().push(&frame).remove(0);
        let Event::State(s) = Event::parse(&f, q20i()) else {
            panic!("state")
        };
        assert_eq!(
            s.batteries.single,
            Some(Level {
                level: 4,
                charging: false
            })
        );
        assert_eq!(s.firmware.as_deref(), Some("02.61"));
        assert_eq!(s.serial.as_deref(), Some("3004ABCDEF123456"));
        assert_eq!(s.equalizer.unwrap().preset, 0x11);
        assert_eq!(
            s.equalizer.unwrap().gains(),
            [30, 20, -10, -10, 10, 30, 30, 30]
        );
        assert_eq!(s.sound_modes.unwrap().ambient, AmbientMode::NoiseCanceling);
    }

    #[test]
    fn truncated_state_is_partial() {
        let body = sample_state_body();
        let f = Frame {
            command: cmd::STATE,
            body: body[..10].to_vec(),
        };
        let Event::State(s) = Event::parse(&f, q20i()) else {
            panic!("state")
        };
        assert!(s.batteries.single.is_some() && s.firmware.is_some());
        assert!(s.serial.is_none() && s.equalizer.is_none());
    }

    #[test]
    fn unknown_layout_ignores_state() {
        let f = Frame {
            command: cmd::STATE,
            body: sample_state_body(),
        };
        assert!(matches!(Event::parse(&f, &[]), Event::Unknown(_)));
    }

    #[test]
    fn dual_battery_layout() {
        let layout = [
            Field::DualBattery,
            Field::DualFirmware,
            Field::Serial,
            Field::SoundModes,
        ];
        let mut b = vec![5, 3, 0, 1];
        b.extend_from_slice(b"01.2301.24");
        b.extend_from_slice(b"3951ABCDEF123456");
        b.extend_from_slice(&[1, 0, 1, 0]);
        let s = parse_state(&b, &layout);
        assert_eq!(
            s.batteries.left,
            Some(Level {
                level: 5,
                charging: false
            })
        );
        assert_eq!(
            s.batteries.right,
            Some(Level {
                level: 3,
                charging: true
            })
        );
        assert_eq!(s.firmware.as_deref(), Some("01.23 / 01.24"));
        assert_eq!(s.sound_modes.unwrap().ambient, AmbientMode::Transparency);
    }

    #[test]
    fn blank_fields_are_none() {
        let mut body = sample_state_body();
        body[2..7].copy_from_slice(&[0; 5]);
        body[7..23].copy_from_slice(&[0xff; 16]);
        let Event::State(s) = Event::parse(
            &Frame {
                command: cmd::STATE,
                body,
            },
            q20i(),
        ) else {
            panic!("state")
        };
        assert!(s.firmware.is_none() && s.serial.is_none());
    }

    #[test]
    fn pushes_and_acks() {
        let f = |c, b: &[u8]| Frame {
            command: c,
            body: b.to_vec(),
        };
        assert_eq!(
            Event::parse(&f(cmd::SOUND_MODES, &[1, 0, 0, 0]), &[]),
            Event::SoundModes(SoundModes::parse(&[1, 0, 0, 0]).unwrap())
        );
        assert_eq!(
            Event::parse(&f(cmd::BATTERY_LEVEL, &[3]), &[]),
            Event::BatteryLevels(vec![3])
        );
        assert_eq!(
            Event::parse(&f(cmd::BATTERY_CHARGING, &[1, 0]), &[]),
            Event::Charging(vec![1, 0])
        );
        assert_eq!(
            Event::parse(&f(cmd::SET_SOUND_MODES, &[]), &[]),
            Event::Ack(cmd::SET_SOUND_MODES)
        );
        let mut info = b"01.1001.11".to_vec();
        info.extend_from_slice(b"3951ABCDEF123456");
        assert_eq!(
            Event::parse(&f(cmd::INFO, &info), &[]),
            Event::Info {
                firmware: Some("01.10 / 01.11".into()),
                serial: Some("3951ABCDEF123456".into())
            }
        );
        assert!(matches!(
            Event::parse(&f([0x05, 0x01], &[1]), &[]),
            Event::Unknown(_)
        ));
    }

    #[test]
    fn equalizer_request_layout() {
        let eq = Equalizer::custom([0, 10, 20, 30, 40, 50, 60, -60]);
        let r = request::equalizer(&eq);
        assert_eq!(&r[5..7], &cmd::SET_EQUALIZER_APPLIED);
        let body = &r[9..r.len() - 1];
        assert_eq!(body.len(), 2 + 10 + 10);
        assert_eq!(u16::from_le_bytes([body[0], body[1]]), CUSTOM_PRESET);
        assert_eq!(&body[2..12], &body[12..22]);
        assert_eq!(request::state(), vec![8, 0xee, 0, 0, 0, 1, 1, 10, 0, 2]);
        let plain = request::equalizer_for(
            &eq,
            &model::EqSpec {
                channels: 2,
                bands: 8,
                write: model::EqWrite::Plain,
            },
        );
        assert_eq!(&plain[5..7], &cmd::SET_EQUALIZER);
        assert_eq!(plain.len(), 10 + 2 + 16);
    }
}
