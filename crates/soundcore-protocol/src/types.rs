//! Values carried by the protocol.

/// Commands used by this app.
pub mod cmd {
    use crate::frame::Command;

    /// Ask for the full state; the answer uses the same command.
    pub const STATE: Command = [0x01, 0x01];
    /// Battery level (request with an empty body, or pushed by the device).
    pub const BATTERY_LEVEL: Command = [0x01, 0x03];
    /// Charging state (request with an empty body, or pushed by the device).
    pub const BATTERY_CHARGING: Command = [0x01, 0x04];
    /// Serial number and firmware version(s).
    pub const INFO: Command = [0x01, 0x05];
    /// Sound modes pushed after a press on the device.
    pub const SOUND_MODES: Command = [0x06, 0x01];
    /// Change the sound modes (answered by an empty acknowledgement).
    pub const SET_SOUND_MODES: Command = [0x06, 0x81];
    /// Change the equalizer, one gain list per channel.
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

    /// The order the Q20i's mode button cycles through.
    pub fn next(self) -> Self {
        match self {
            Self::NoiseCanceling => Self::Normal,
            Self::Normal => Self::Transparency,
            Self::Transparency => Self::NoiseCanceling,
        }
    }
}

/// Noise cancelling flavour (devices that offer a choice).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoiseCancelingMode {
    Transport,
    Outdoor,
    Indoor,
    Custom,
}

impl NoiseCancelingMode {
    pub fn from_byte(b: u8) -> Option<Self> {
        [Self::Transport, Self::Outdoor, Self::Indoor, Self::Custom]
            .get(b as usize)
            .copied()
    }

    pub fn byte(self) -> u8 {
        self as u8
    }
}

/// Transparency flavour (devices that offer a choice).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransparencyMode {
    FullyTransparent,
    Vocal,
}

impl TransparencyMode {
    pub fn from_byte(b: u8) -> Option<Self> {
        [Self::FullyTransparent, Self::Vocal]
            .get(b as usize)
            .copied()
    }

    pub fn byte(self) -> u8 {
        self as u8
    }
}

/// Sound-mode bytes as the device sent them. The first byte is always the ambient
/// mode. On the classic 4-byte layout the next ones are the noise cancelling flavour,
/// the transparency flavour and a custom level; newer products send longer,
/// product-specific bodies. Bytes this app doesn't change are written back exactly as
/// they were read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundModes {
    pub ambient: AmbientMode,
    raw: [u8; 8],
    len: u8,
}

impl SoundModes {
    pub fn new(ambient: AmbientMode) -> Self {
        let mut raw = [0; 8];
        raw[0] = ambient.byte();
        Self {
            ambient,
            raw,
            len: 4,
        }
    }

    /// Up to 8 bytes; the classic layout is 4.
    pub fn parse(b: &[u8]) -> Option<Self> {
        let ambient = AmbientMode::from_byte(*b.first()?)?;
        let len = b.len().min(8);
        let mut raw = [0; 8];
        raw[..len].copy_from_slice(&b[..len]);
        Some(Self {
            ambient,
            raw,
            len: len.max(4) as u8,
        })
    }

    pub fn bytes(&self) -> Vec<u8> {
        let mut out = self.raw[..self.len as usize].to_vec();
        out[0] = self.ambient.byte();
        out
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn with_ambient(self, ambient: AmbientMode) -> Self {
        Self { ambient, ..self }
    }

    pub fn noise_canceling_mode(&self) -> Option<NoiseCancelingMode> {
        NoiseCancelingMode::from_byte(self.raw[1])
    }

    pub fn transparency_mode(&self) -> Option<TransparencyMode> {
        TransparencyMode::from_byte(self.raw[2])
    }

    pub fn with_noise_canceling(mut self, m: NoiseCancelingMode) -> Self {
        self.raw[1] = m.byte();
        self
    }

    pub fn with_transparency(mut self, m: TransparencyMode) -> Self {
        self.raw[2] = m.byte();
        self
    }
}

/// One battery, reported in steps (0…`steps`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Level {
    pub level: u8,
    pub charging: bool,
}

/// Batteries of a device: a single one (headphones, speakers) or left/right buds
/// with an optional case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Batteries {
    pub single: Option<Level>,
    pub left: Option<Level>,
    pub right: Option<Level>,
    pub case: Option<Level>,
}

impl Batteries {
    pub fn is_dual(&self) -> bool {
        self.left.is_some() || self.right.is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.single.is_none() && !self.is_dual() && self.case.is_none()
    }

    /// Applies a level answer: one byte for a single battery, two for left/right,
    /// three when the case follows.
    pub fn set_levels(&mut self, b: &[u8]) {
        let set = |slot: &mut Option<Level>, v: u8| slot.get_or_insert_default().level = v;
        match b.len() {
            0 => {}
            1 => set(&mut self.single, b[0]),
            n => {
                set(&mut self.left, b[0]);
                set(&mut self.right, b[1]);
                if n >= 3 {
                    set(&mut self.case, b[2]);
                }
            }
        }
    }

    /// Same layout as [`Self::set_levels`], for charging flags.
    pub fn set_charging(&mut self, b: &[u8]) {
        let set = |slot: &mut Option<Level>, v: u8| slot.get_or_insert_default().charging = v != 0;
        match b.len() {
            0 => {}
            1 => set(&mut self.single, b[0]),
            n => {
                set(&mut self.left, b[0]);
                set(&mut self.right, b[1]);
                if n >= 3 {
                    set(&mut self.case, b[2]);
                }
            }
        }
    }
}

/// Percentage of a level reported in `steps`.
pub fn percent(level: u8, steps: u8) -> u8 {
    let steps = steps.max(1);
    (level.min(steps) as u16 * 100 / steps as u16) as u8
}

/// How a product reports a battery: raw values 0…`max - offset`, shown as
/// `(raw + offset) / max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryScale {
    pub max: u8,
    pub offset: u8,
}

impl BatteryScale {
    pub const FIVE: BatteryScale = BatteryScale { max: 5, offset: 0 };
    pub const TEN: BatteryScale = BatteryScale { max: 10, offset: 0 };
    pub const TEN_PLUS_ONE: BatteryScale = BatteryScale { max: 10, offset: 1 };
    pub const HUNDRED: BatteryScale = BatteryScale {
        max: 100,
        offset: 0,
    };

    pub fn percent(&self, raw: u8) -> u8 {
        percent(raw.saturating_add(self.offset), self.max)
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

/// Number of bands kept per channel; the first [`VISIBLE_BANDS`] are adjustable.
pub const BANDS: usize = 10;
pub const VISIBLE_BANDS: usize = 8;
/// Centre frequencies of the adjustable bands, in hertz.
pub const BAND_HZ: [u32; VISIBLE_BANDS] = [100, 200, 400, 800, 1600, 3200, 6400, 12800];
/// What devices keep in the two hidden bands.
pub const HIDDEN_BANDS_DEFAULT: [i16; 2] = [0, -120];

/// Preset id the devices use for a user-defined curve.
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

    /// A preset as a product lists it (its own id and gains).
    pub fn from_preset(p: &crate::eq::Preset) -> Self {
        Self::with_gains(p.id, p.gains)
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

    /// Gains of one channel on the wire (`bands` bytes).
    pub fn band_bytes(&self, bands: usize) -> Vec<u8> {
        (0..bands)
            .map(|i| gain_to_byte(self.bands.get(i).copied().unwrap_or(0)))
            .collect()
    }

    /// `preset (u16 LE)` then the gains, repeated for each channel.
    pub fn bytes_for(&self, channels: usize, bands: usize) -> Vec<u8> {
        let mut out = self.preset.to_le_bytes().to_vec();
        for _ in 0..channels {
            out.extend(self.band_bytes(bands));
        }
        out
    }

    /// Single channel, ten bands (the Q20i layout).
    pub fn bytes(&self) -> Vec<u8> {
        self.bytes_for(1, BANDS)
    }

    /// Parses `preset + channels × bands`; keeps the first present channel.
    /// Returns the value and the bytes consumed.
    pub fn parse_with(b: &[u8], channels: usize, bands: usize) -> Option<(Self, usize)> {
        let used = 2 + channels * bands;
        if b.len() < used {
            return None;
        }
        let preset = u16::from_le_bytes([b[0], b[1]]);
        let channel = (0..channels)
            .map(|c| &b[2 + c * bands..2 + (c + 1) * bands])
            .find(|ch| ch[0] != 0xff)?;
        let mut gains = [0; BANDS];
        gains[VISIBLE_BANDS..].copy_from_slice(&HIDDEN_BANDS_DEFAULT);
        for (i, g) in gains.iter_mut().enumerate().take(bands) {
            *g = byte_to_gain(channel[i]);
        }
        Some((
            Self {
                preset,
                bands: gains,
            },
            used,
        ))
    }

    pub fn parse(b: &[u8]) -> Option<(Self, usize)> {
        Self::parse_with(b, 1, BANDS)
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
    fn two_channel_equalizer() {
        let eq = Equalizer::preset(0x11).unwrap();
        let b = eq.bytes_for(2, 8);
        assert_eq!(b.len(), 2 + 16);
        let (back, used) = Equalizer::parse_with(&b, 2, 8).unwrap();
        assert_eq!(used, 18);
        assert_eq!(back.gains(), eq.gains());
        // First channel absent: the second one is used.
        let mut c = b.clone();
        c[2..10].fill(0xff);
        assert_eq!(
            Equalizer::parse_with(&c, 2, 8).unwrap().0.gains(),
            eq.gains()
        );
    }

    #[test]
    fn sound_modes_keep_other_bytes() {
        let m = SoundModes::parse(&[2, 1, 0, 6]).unwrap();
        assert_eq!(m.ambient, AmbientMode::Normal);
        assert_eq!(
            m.with_ambient(AmbientMode::NoiseCanceling).bytes(),
            [0, 1, 0, 6]
        );
        assert_eq!(m.noise_canceling_mode(), Some(NoiseCancelingMode::Outdoor));
        assert_eq!(
            m.with_transparency(TransparencyMode::Vocal).bytes(),
            [2, 1, 1, 6]
        );
        // Longer, product-specific bodies keep their length and unknown bytes.
        let v2 = SoundModes::parse(&[0, 0x35, 0, 1, 1, 3, 9]).unwrap();
        assert_eq!(v2.len(), 7);
        assert_eq!(
            v2.with_ambient(AmbientMode::Normal).bytes(),
            [2, 0x35, 0, 1, 1, 3, 9]
        );
        assert_eq!(AmbientMode::NoiseCanceling.next(), AmbientMode::Normal);
    }

    #[test]
    fn batteries() {
        let mut b = Batteries::default();
        b.set_levels(&[4]);
        b.set_charging(&[1]);
        assert_eq!(
            b.single,
            Some(Level {
                level: 4,
                charging: true
            })
        );
        assert!(!b.is_dual());
        let mut d = Batteries::default();
        d.set_levels(&[3, 5]);
        d.set_charging(&[0, 1]);
        assert!(d.is_dual());
        assert_eq!(
            d.right,
            Some(Level {
                level: 5,
                charging: true
            })
        );
        assert_eq!(percent(3, 5), 60);
        assert_eq!(percent(9, 5), 100);
        assert_eq!(percent(1, 0), 100);
    }
}
