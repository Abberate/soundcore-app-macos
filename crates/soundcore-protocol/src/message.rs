//! Typed requests and events.

use crate::frame::{Command, Frame, encode};
use crate::types::{Battery, Equalizer, SoundModes, cmd};

/// Everything the full state answer carries (Q20i layout):
///
/// ```text
/// battery level (1) | charging (1) | firmware "xx.xx" (5) | serial (16)
/// | equalizer: preset (2) + 10 gains | sound modes (4)
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StateReport {
    pub battery: Option<Battery>,
    pub firmware: Option<String>,
    pub serial: Option<String>,
    pub equalizer: Option<Equalizer>,
    pub sound_modes: Option<SoundModes>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    State(StateReport),
    SoundModes(SoundModes),
    BatteryLevel(u8),
    Charging(bool),
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

fn parse_state(b: &[u8]) -> StateReport {
    let mut r = StateReport::default();
    if b.len() >= 2 {
        r.battery = Some(Battery {
            level: b[0],
            charging: b[1] != 0,
        });
    }
    if b.len() >= 7 {
        r.firmware = text(&b[2..7]);
    }
    if b.len() >= 23 {
        r.serial = text(&b[7..23]);
    }
    if let Some((eq, used)) = b.get(23..).and_then(Equalizer::parse) {
        r.equalizer = Some(eq);
        r.sound_modes = b.get(23 + used..).and_then(SoundModes::parse);
    }
    r
}

impl Event {
    pub fn parse(f: &Frame) -> Event {
        let b = f.body.as_slice();
        if b.is_empty() && cmd::is_write(f.command) {
            return Event::Ack(f.command);
        }
        match f.command {
            cmd::STATE if !b.is_empty() => Event::State(parse_state(b)),
            cmd::SOUND_MODES => match SoundModes::parse(b) {
                Some(m) => Event::SoundModes(m),
                None => Event::Unknown(f.clone()),
            },
            cmd::BATTERY_LEVEL if !b.is_empty() => Event::BatteryLevel(b[0]),
            cmd::BATTERY_CHARGING if !b.is_empty() => Event::Charging(b[0] != 0),
            cmd::INFO => {
                // Firmware(s) first (5 bytes each), serial number last (16 bytes).
                let serial = b.len().checked_sub(16).and_then(|s| text(&b[s..]));
                Event::Info {
                    firmware: text(&b[..b.len().min(5)]),
                    serial,
                }
            }
            _ => Event::Unknown(f.clone()),
        }
    }
}

/// Encoded requests.
pub mod request {
    use super::*;

    pub fn state() -> Vec<u8> {
        encode(cmd::STATE, &[])
    }

    pub fn sound_modes(m: &SoundModes) -> Vec<u8> {
        encode(cmd::SET_SOUND_MODES, &m.bytes())
    }

    /// The Q20i takes two gain lists: the curve to display and the curve to apply.
    /// This app applies exactly the curve it shows.
    pub fn equalizer(eq: &Equalizer) -> Vec<u8> {
        let mut body = eq.bytes();
        body.extend_from_slice(&eq.bytes()[2..]);
        encode(cmd::SET_EQUALIZER_APPLIED, &body)
    }

    /// Single-list variant, kept for diagnostics.
    pub fn equalizer_single(eq: &Equalizer) -> Vec<u8> {
        encode(cmd::SET_EQUALIZER, &eq.bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{Reassembler, encode_from_device};
    use crate::types::{AmbientMode, CUSTOM_PRESET};

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
        let Event::State(s) = Event::parse(&f) else {
            panic!("state")
        };
        assert_eq!(
            s.battery,
            Some(Battery {
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
        let Event::State(s) = Event::parse(&f) else {
            panic!("state")
        };
        assert!(s.battery.is_some() && s.firmware.is_some());
        assert!(s.serial.is_none() && s.equalizer.is_none());
    }

    #[test]
    fn blank_fields_are_none() {
        let mut body = sample_state_body();
        body[2..7].copy_from_slice(&[0; 5]);
        body[7..23].copy_from_slice(&[0xff; 16]);
        let Event::State(s) = Event::parse(&Frame {
            command: cmd::STATE,
            body,
        }) else {
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
            Event::parse(&f(cmd::SOUND_MODES, &[1, 0, 0, 0])),
            Event::SoundModes(SoundModes::parse(&[1, 0, 0, 0]).unwrap())
        );
        assert_eq!(
            Event::parse(&f(cmd::BATTERY_LEVEL, &[3])),
            Event::BatteryLevel(3)
        );
        assert_eq!(
            Event::parse(&f(cmd::BATTERY_CHARGING, &[1])),
            Event::Charging(true)
        );
        assert_eq!(
            Event::parse(&f(cmd::SET_SOUND_MODES, &[])),
            Event::Ack(cmd::SET_SOUND_MODES)
        );
        assert!(matches!(
            Event::parse(&f([0x05, 0x01], &[1])),
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
    }
}
