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

/// How a product's preset list differs from the common one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetRules {
    /// Later products use a brighter Rock curve.
    pub rock_v2: bool,
    /// Id the product uses for Bass Booster (0x7e7e on some), `None` when absent.
    pub bass_booster_id: Option<u16>,
    /// Restricts the list to these common ids (empty: all of them).
    pub only: &'static [u16],
    /// Whether a custom curve can be stored.
    pub custom: bool,
}

impl PresetRules {
    pub const COMMON: PresetRules = PresetRules {
        rock_v2: false,
        bass_booster_id: Some(0x02),
        only: &[],
        custom: true,
    };
    pub const V2: PresetRules = PresetRules {
        rock_v2: true,
        ..Self::COMMON
    };
    pub const V2_ALT_BASS: PresetRules = PresetRules {
        bass_booster_id: Some(0x7e7e),
        ..Self::V2
    };

    /// Presets offered for this product, with the id to send and the gains.
    pub fn list(&self) -> Vec<Preset> {
        PRESETS
            .iter()
            .filter(|p| self.only.is_empty() || self.only.contains(&p.id))
            .filter_map(|p| {
                let mut p = *p;
                if p.key == "bass_booster" {
                    p.id = self.bass_booster_id?;
                }
                if p.key == "rock" && self.rock_v2 {
                    p.gains = [30, 20, -10, -10, 10, 30, 40, 50];
                }
                Some(p)
            })
            .collect()
    }

    /// The preset a device reports by id.
    pub fn find(&self, id: u16) -> Option<Preset> {
        self.list().into_iter().find(|p| p.id == id)
    }
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

    #[test]
    fn preset_rules() {
        assert_eq!(PresetRules::COMMON.list().len(), 22);
        let v2 = PresetRules::V2_ALT_BASS;
        assert_eq!(v2.find(0x7e7e).unwrap().key, "bass_booster");
        assert!(v2.find(0x02).is_none());
        assert_eq!(v2.find(0x11).unwrap().gains[7], 50);
        let few = PresetRules {
            only: &[0, 4],
            ..PresetRules::COMMON
        };
        assert_eq!(few.list().len(), 2);
    }
}
