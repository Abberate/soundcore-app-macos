//! The headphones' built-in equalizer presets.
//!
//! Ids and gain curves (tenths of a dB, bands 100 Hz … 12.8 kHz) are the values the
//! headphones and the official app agree on; they were documented by the OpenSCQ30
//! project. The display names are translated in the app.

use crate::types::VISIBLE_BANDS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    pub id: u16,
    /// Stable key used for translations.
    pub key: &'static str,
    pub gains: [i16; VISIBLE_BANDS],
}

const fn p(id: u16, key: &'static str, gains: [i16; VISIBLE_BANDS]) -> Preset {
    Preset { id, key, gains }
}

pub const PRESETS: [Preset; 22] = [
    p(0x00, "signature", [0, 0, 0, 0, 0, 0, 0, 0]),
    p(0x01, "acoustic", [40, 10, 20, 20, 40, 40, 40, 20]),
    p(0x02, "bass_booster", [40, 30, 10, 0, 0, 0, 0, 0]),
    p(0x03, "bass_reducer", [-40, -30, -10, 0, 0, 0, 0, 0]),
    p(0x04, "classical", [30, 30, -20, -20, 0, 20, 30, 40]),
    p(0x05, "podcast", [-30, 20, 40, 40, 30, 20, 0, -20]),
    p(0x06, "dance", [20, -30, -10, 10, 20, 20, 10, -30]),
    p(0x07, "deep", [20, 10, 30, 30, 20, -20, -40, -50]),
    p(0x08, "electronic", [30, 20, -20, 20, 10, 20, 30, 30]),
    p(0x09, "flat", [-20, -20, -10, 0, 0, 0, -20, -20]),
    p(0x0a, "hip_hop", [20, 30, -10, -10, 20, -10, 20, 30]),
    p(0x0b, "jazz", [20, 20, -20, -20, 0, 20, 30, 40]),
    p(0x0c, "latin", [0, 0, -20, -20, -20, 0, 30, 50]),
    p(0x0d, "lounge", [-10, 20, 40, 30, 0, -20, 20, 10]),
    p(0x0e, "piano", [0, 30, 30, 20, 40, 50, 30, 40]),
    p(0x0f, "pop", [-10, 10, 30, 30, 10, -10, -20, -30]),
    p(0x10, "rnb", [60, 20, -20, -20, 20, 30, 30, 40]),
    p(0x11, "rock", [30, 20, -10, -10, 10, 30, 30, 30]),
    p(0x12, "small_speakers", [40, 30, 10, 0, -20, -30, -40, -40]),
    p(0x13, "spoken_word", [-30, -20, 10, 20, 20, 10, 0, -30]),
    p(0x14, "treble_booster", [-20, -20, -20, -10, 10, 20, 20, 40]),
    p(0x15, "treble_reducer", [0, 0, 0, -20, -30, -40, -40, -60]),
];

pub fn by_id(id: u16) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_in_range() {
        for (i, p) in PRESETS.iter().enumerate() {
            assert_eq!(p.id as usize, i);
            assert!(p.gains.iter().all(|g| (-60..=60).contains(g)), "{}", p.key);
        }
        assert_eq!(by_id(0x11).unwrap().key, "rock");
        assert!(by_id(0xfefe).is_none());
    }
}
