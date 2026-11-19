//! Values carried by the protocol.

/// Commands used by this app.
pub mod cmd {
    use crate::frame::Command;

    /// Ask for the full state; the answer uses the same command.
    pub const STATE: Command = [0x01, 0x01];
    /// Battery level pushed by the headphones.
    pub const BATTERY_LEVEL: Command = [0x01, 0x03];
    /// Charging state pushed by the headphones.
    pub const BATTERY_CHARGING: Command = [0x01, 0x04];
    /// Serial number and firmware version.
    pub const INFO: Command = [0x01, 0x05];
    /// Sound modes pushed after a press on the headphones' mode button.
    pub const SOUND_MODES: Command = [0x06, 0x01];
    /// Change the sound modes (answered by an empty acknowledgement).
    pub const SET_SOUND_MODES: Command = [0x06, 0x81];
    /// Change the equalizer, one gain list (answered by an empty acknowledgement).
    pub const SET_EQUALIZER: Command = [0x02, 0x81];
    /// Change the equalizer, displayed gains followed by applied gains.
    pub const SET_EQUALIZER_APPLIED: Command = [0x02, 0x83];

    /// Commands that change a setting (and are acknowledged).
    pub fn is_write(c: Command) -> bool {
        c[1] & 0x80 != 0
    }
}

/// Noise control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AmbientMode {
    NoiseCanceling,
    Transparency,
    Normal,
}

impl AmbientMode {
    pub const ALL: [AmbientMode; 3] = [Self::NoiseCanceling, Self::Normal, Self::Transparency];

    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0 => Some(Self::NoiseCanceling),
            1 => Some(Self::Transparency),
            2 => Some(Self::Normal),
            _ => None,
        }
    }

    pub fn byte(self) -> u8 {
        match self {
            Self::NoiseCanceling => 0,
            Self::Transparency => 1,
            Self::Normal => 2,
        }
    }

    /// The order the headphones' mode button cycles through.
    pub fn next(self) -> Self {
        match self {
            Self::NoiseCanceling => Self::Normal,
            Self::Normal => Self::Transparency,
            Self::Transparency => Self::NoiseCanceling,
        }
    }
}

/// The four sound-mode bytes. Only the first one matters on the Q20i; the others
/// (noise cancelling flavour, transparency flavour, custom level) are written back as
/// they were read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundModes {
    pub ambient: AmbientMode,
    pub rest: [u8; 3],
}

impl SoundModes {
    pub fn new(ambient: AmbientMode) -> Self {
        Self {
            ambient,
            rest: [0, 0, 0],
        }
    }

    pub fn parse(b: &[u8]) -> Option<Self> {
        let ambient = AmbientMode::from_byte(*b.first()?)?;
        let rest = [
            b.get(1).copied().unwrap_or(0),
            b.get(2).copied().unwrap_or(0),
            b.get(3).copied().unwrap_or(0),
        ];
        Some(Self { ambient, rest })
    }

    pub fn bytes(&self) -> [u8; 4] {
        [
            self.ambient.byte(),
            self.rest[0],
            self.rest[1],
            self.rest[2],
        ]
    }

    pub fn with_ambient(self, ambient: AmbientMode) -> Self {
        Self { ambient, ..self }
    }
}

/// Single battery, reported in steps of 0…5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Battery {
    pub level: u8,
    pub charging: bool,
}

impl Battery {
    pub const STEPS: u8 = 5;

    pub fn percent(&self) -> u8 {
        self.level.min(Self::STEPS) * (100 / Self::STEPS)
    }
}

/// Gains in tenths of a decibel. The wire byte is `gain + 120`.
pub const GAIN_MIN: i16 = -120;
pub const GAIN_MAX: i16 = 134;
/// Range offered in the editor (±6 dB), as in the official app.
pub const EDIT_MIN: i16 = -60;
pub const EDIT_MAX: i16 = 60;

pub fn gain_to_byte(g: i16) -> u8 {
    (g.clamp(GAIN_MIN, GAIN_MAX) - GAIN_MIN) as u8
}

pub fn byte_to_gain(b: u8) -> i16 {
    b as i16 + GAIN_MIN
}

/// Number of bands the headphones store; the first [`VISIBLE_BANDS`] are adjustable.
pub const BANDS: usize = 10;
pub const VISIBLE_BANDS: usize = 8;
/// Centre frequencies of the adjustable bands, in hertz.
pub const BAND_HZ: [u32; VISIBLE_BANDS] = [100, 200, 400, 800, 1600, 3200, 6400, 12800];
/// What the headphones keep in the two hidden bands.
pub const HIDDEN_BANDS_DEFAULT: [i16; 2] = [0, -120];

/// Preset id the headphones use for a user-defined curve.
pub const CUSTOM_PRESET: u16 = 0xfefe;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Equalizer {
    pub preset: u16,
    pub bands: [i16; BANDS],
}

impl Default for Equalizer {
    fn default() -> Self {
        Self::preset(0).expect("signature preset")
    }
}

impl Equalizer {
    /// A built-in preset by id.
    pub fn preset(id: u16) -> Option<Self> {
        let p = crate::eq::PRESETS.iter().find(|p| p.id == id)?;
        Some(Self::with_gains(id, p.gains))
    }

    pub fn custom(gains: [i16; VISIBLE_BANDS]) -> Self {
        Self::with_gains(CUSTOM_PRESET, gains)
    }

    fn with_gains(preset: u16, gains: [i16; VISIBLE_BANDS]) -> Self {
        let mut bands = [0; BANDS];
        bands[..VISIBLE_BANDS].copy_from_slice(&gains);
        bands[VISIBLE_BANDS..].copy_from_slice(&HIDDEN_BANDS_DEFAULT);
        Self { preset, bands }
    }

    pub fn is_custom(&self) -> bool {
        self.preset == CUSTOM_PRESET
    }

    pub fn gains(&self) -> [i16; VISIBLE_BANDS] {
        let mut g = [0; VISIBLE_BANDS];
        g.copy_from_slice(&self.bands[..VISIBLE_BANDS]);
        g
    }

    /// `preset (u16 LE)` then one byte per band.
    pub fn bytes(&self) -> Vec<u8> {
        let mut out = self.preset.to_le_bytes().to_vec();
        out.extend(self.bands.iter().map(|g| gain_to_byte(*g)));
        out
    }

    /// Parses `preset + bands`; returns the value and the bytes consumed.
    pub fn parse(b: &[u8]) -> Option<(Self, usize)> {
        if b.len() < 2 + BANDS {
            return None;
        }
        let preset = u16::from_le_bytes([b[0], b[1]]);
        let mut bands = [0; BANDS];
        for (i, g) in bands.iter_mut().enumerate() {
            *g = byte_to_gain(b[2 + i]);
        }
        Some((Self { preset, bands }, 2 + BANDS))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_bytes_roundtrip() {
        assert_eq!(gain_to_byte(0), 120);
        assert_eq!(gain_to_byte(-120), 0);
        assert_eq!(gain_to_byte(60), 180);
        assert_eq!(gain_to_byte(500), 254);
        for g in -120..=134 {
            assert_eq!(byte_to_gain(gain_to_byte(g)), g);
        }
    }

    #[test]
    fn equalizer_bytes() {
        let eq = Equalizer::custom([10, 20, 30, 40, 50, 60, -10, -60]);
        let b = eq.bytes();
        assert_eq!(&b[..2], &[0xfe, 0xfe]);
        assert_eq!(b.len(), 12);
        assert_eq!(b[2], 130);
        assert_eq!(b[9], 60);
        assert_eq!(&b[10..], &[120, 0]);
        let (back, used) = Equalizer::parse(&b).unwrap();
        assert_eq!(used, 12);
        assert_eq!(back, eq);
    }

    #[test]
    fn sound_modes_keep_other_bytes() {
        let m = SoundModes::parse(&[2, 1, 0, 6]).unwrap();
        assert_eq!(m.ambient, AmbientMode::Normal);
        assert_eq!(
            m.with_ambient(AmbientMode::NoiseCanceling).bytes(),
            [0, 1, 0, 6]
        );
        assert_eq!(AmbientMode::NoiseCanceling.next(), AmbientMode::Normal);
    }

    #[test]
    fn battery_percent() {
        assert_eq!(
            Battery {
                level: 5,
                charging: false
            }
            .percent(),
            100
        );
        assert_eq!(
            Battery {
                level: 3,
                charging: true
            }
            .percent(),
            60
        );
        assert_eq!(
            Battery {
                level: 9,
                charging: false
            }
            .percent(),
            100
        );
    }
}
