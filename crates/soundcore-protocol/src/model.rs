//! Known products and how their state answer is laid out.
//!
//! Every soundcore product speaks the same framing and most share the same commands,
//! but the full state answer (command 01 01) packs a product-specific list of fields.
//! Each product is described here as data: what it can do and the order of the fields
//! in its state answer (from the protocol documented by the OpenSCQ30 project).
//! Products not in the table still work through the commands they all share.
//!
//! The vendor control service UUID ends with the product number
//! (`0cf12d31-fac3-4553-bd80-d6832e7b3004` on the Q20i, model A3004), which identifies a
//! paired device before any data is exchanged.

use crate::eq::PresetRules;
use crate::types::{AmbientMode, BatteryScale, NoiseCancelingMode, TransparencyMode};

/// One field of a state answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// Level then charging flag (2 bytes).
    SingleBattery,
    /// Level only (1 byte).
    BatteryLevel,
    /// Left level, right level, left charging, right charging (4 bytes).
    DualBattery,
    /// Left level, right level (2 bytes).
    DualBatteryLevel,
    /// Case level (1 byte).
    CaseBattery,
    /// "xx.xx" (5 bytes).
    Firmware,
    /// Left and right "xx.xx" (10 bytes).
    DualFirmware,
    /// 16 ASCII bytes.
    Serial,
    /// Preset (2 bytes) then `channels × bands` gain bytes.
    Equalizer { channels: u8, bands: u8 },
    /// Classic sound modes (4 bytes).
    SoundModes,
    /// Product-specific sound modes, ambient mode first (n bytes).
    SoundModesV2(u8),
    /// Bytes this app does not use.
    Skip(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Headphones,
    Earbuds,
    Speaker,
}

/// How a product takes equalizer changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqWrite {
    /// Command 02 81: preset and gains.
    Plain,
    /// Command 02 83: preset, displayed gains, applied gains.
    Applied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EqSpec {
    pub channels: u8,
    pub bands: u8,
    pub write: EqWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub kind: Kind,
    /// Empty when the product has no noise control.
    pub ambient_modes: &'static [AmbientMode],
    pub noise_canceling_modes: &'static [NoiseCancelingMode],
    pub transparency_modes: &'static [TransparencyMode],
    /// The current equalizer is shown; `None` when the product has none.
    pub equalizer: Option<EqSpec>,
    /// Whether the app may change it (some products use write formats this version
    /// doesn't speak yet: the curve is then shown read-only).
    pub equalizer_writable: bool,
    pub presets: PresetRules,
    pub battery: BatteryScale,
    pub case: BatteryScale,
    /// State answer layout; empty when unknown.
    pub layout: &'static [Field],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Model {
    /// Product number, e.g. "A3004".
    pub id: &'static str,
    pub name: &'static str,
    /// Lower-case fragments of the Bluetooth names this product uses.
    pub names: &'static [&'static str],
    pub caps: Capabilities,
}

use AmbientMode::{NoiseCanceling as NC, Normal as N, Transparency as T};
use Field::*;
use NoiseCancelingMode::{Custom as NcCustom, Indoor, Outdoor, Transport};
use TransparencyMode::{FullyTransparent, Vocal};

const ALL: &[AmbientMode] = &[NC, N, T];
const NT: &[AmbientMode] = &[N, T];
const PLACES: &[NoiseCancelingMode] = &[Transport, Indoor, Outdoor];
const PLACES_CUSTOM: &[NoiseCancelingMode] = &[Transport, Indoor, Outdoor, NcCustom];
const FULL_VOCAL: &[TransparencyMode] = &[FullyTransparent, Vocal];

const fn eq(channels: u8, bands: u8, write: EqWrite) -> Option<EqSpec> {
    Some(EqSpec {
        channels,
        bands,
        write,
    })
}

/// Baseline used by the table entries below.
const BASE: Capabilities = Capabilities {
    kind: Kind::Headphones,
    ambient_modes: &[],
    noise_canceling_modes: &[],
    transparency_modes: &[],
    equalizer: None,
    equalizer_writable: false,
    presets: PresetRules::COMMON,
    battery: BatteryScale::FIVE,
    case: BatteryScale::FIVE,
    layout: &[],
};

/// Capabilities assumed for products without a description: the commands soundcore
/// products share, no state answer layout.
pub const fn generic(kind: Kind) -> Capabilities {
    Capabilities {
        kind,
        ambient_modes: ALL,
        equalizer: eq(1, 10, EqWrite::Plain),
        equalizer_writable: true,
        ..BASE
    }
}

const EARBUDS: Capabilities = Capabilities {
    kind: Kind::Earbuds,
    ..BASE
};

// Shared layouts.
const LIFE_Q: &[Field] = &[
    SingleBattery,
    Equalizer {
        channels: 1,
        bands: 8,
    },
    Skip(23), // gender, age range, basic hear id
    SoundModes,
    Firmware,
    Serial,
];
const DOT2: &[Field] = &[
    Skip(2),
    DualBattery,
    Equalizer {
        channels: 2,
        bands: 8,
    },
    Skip(12),
    SoundModes,
];
const P2_MINI: &[Field] = &[
    Skip(2),
    DualBattery,
    DualFirmware,
    Serial,
    Equalizer {
        channels: 1,
        bands: 10,
    },
];

pub const MODELS: &[Model] = &[
    // ---- headphones ------------------------------------------------------------------
    Model {
        id: "A3004",
        name: "soundcore Q20i",
        names: &["q20i"],
        caps: Capabilities {
            ambient_modes: ALL,
            equalizer: eq(1, 10, EqWrite::Applied),
            equalizer_writable: true,
            layout: &[
                SingleBattery,
                Firmware,
                Serial,
                Equalizer {
                    channels: 1,
                    bands: 10,
                },
                SoundModes,
            ],
            ..BASE
        },
    },
    Model {
        id: "A3005",
        name: "soundcore Q11i",
        names: &["q11i"],
        caps: Capabilities {
            equalizer: eq(1, 10, EqWrite::Applied),
            equalizer_writable: true,
            presets: PresetRules::V2_ALT_BASS,
            battery: BatteryScale::TEN_PLUS_ONE,
            layout: &[
                SingleBattery,
                Firmware,
                Serial,
                Equalizer {
                    channels: 1,
                    bands: 10,
                },
            ],
            ..BASE
        },
    },
    Model {
        id: "A3027",
        name: "Soundcore Life Q35",
        names: &["life q35"],
        caps: Capabilities {
            ambient_modes: ALL,
            noise_canceling_modes: PLACES,
            equalizer: eq(1, 8, EqWrite::Plain),
            equalizer_writable: true,
            layout: LIFE_Q,
            ..BASE
        },
    },
    Model {
        id: "A3030",
        name: "Soundcore Life Tune Pro",
        names: &["life tune pro"],
        caps: Capabilities {
            ambient_modes: ALL,
            noise_canceling_modes: PLACES,
            equalizer: eq(1, 8, EqWrite::Plain),
            equalizer_writable: true,
            layout: LIFE_Q,
            ..BASE
        },
    },
    Model {
        id: "A3028",
        name: "Soundcore Life Q30",
        names: &["life q30", "q30"],
        caps: Capabilities {
            ambient_modes: ALL,
            noise_canceling_modes: PLACES,
            equalizer: eq(1, 8, EqWrite::Plain),
            equalizer_writable: true,
            layout: LIFE_Q,
            ..BASE
        },
    },
    Model {
        id: "A3029",
        name: "Soundcore Life Tune",
        names: &["life tune"],
        caps: Capabilities {
            ambient_modes: ALL,
            noise_canceling_modes: PLACES,
            equalizer: eq(1, 8, EqWrite::Plain),
            equalizer_writable: true,
            layout: LIFE_Q,
            ..BASE
        },
    },
    Model {
        id: "A3033",
        name: "Soundcore Life 2 Neo",
        names: &["life 2 neo"],
        caps: Capabilities {
            equalizer: eq(1, 8, EqWrite::Plain),
            equalizer_writable: true,
            layout: &[
                SingleBattery,
                Equalizer {
                    channels: 1,
                    bands: 8,
                },
                Firmware,
                Serial,
            ],
            ..BASE
        },
    },
    Model {
        id: "A3035",
        name: "soundcore Space One",
        names: &["space one"],
        caps: Capabilities {
            ambient_modes: ALL,
            equalizer: eq(1, 10, EqWrite::Plain),
            presets: PresetRules::V2_ALT_BASS,
            layout: &[
                BatteryLevel,
                Skip(1),
                Firmware,
                Serial,
                Equalizer {
                    channels: 1,
                    bands: 10,
                },
                Skip(34),
                SoundModesV2(6),
            ],
            ..BASE
        },
    },
    Model {
        id: "A3040",
        name: "soundcore Space Q45",
        names: &["space q45", "q45"],
        caps: Capabilities {
            ambient_modes: ALL,
            equalizer: eq(1, 10, EqWrite::Plain),
            presets: PresetRules {
                bass_booster_id: None,
                ..PresetRules::V2
            },
            layout: &[
                BatteryLevel,
                Skip(1),
                Firmware,
                Serial,
                Equalizer {
                    channels: 1,
                    bands: 10,
                },
                Skip(14),
                SoundModesV2(6),
            ],
            ..BASE
        },
    },
    Model {
        id: "A3062",
        name: "soundcore Space One Pro",
        names: &["space one pro"],
        caps: Capabilities {
            ambient_modes: ALL,
            equalizer: eq(1, 10, EqWrite::Plain),
            presets: PresetRules::V2_ALT_BASS,
            battery: BatteryScale::TEN_PLUS_ONE,
            layout: &[
                SingleBattery,
                Firmware,
                Serial,
                Equalizer {
                    channels: 1,
                    bands: 10,
                },
                Skip(34),
                SoundModesV2(6),
            ],
            ..BASE
        },
    },
    Model {
        id: "D1402",
        name: "soundcore Space 2",
        names: &["space 2"],
        caps: Capabilities {
            ambient_modes: ALL,
            equalizer: eq(1, 10, EqWrite::Plain),
            presets: PresetRules::V2_ALT_BASS,
            battery: BatteryScale::TEN_PLUS_ONE,
            layout: &[
                SingleBattery,
                Firmware,
                Serial,
                Equalizer {
                    channels: 1,
                    bands: 10,
                },
                Skip(36),
                SoundModesV2(6),
            ],
            ..BASE
        },
    },
    // ---- earbuds ---------------------------------------------------------------------
    Model {
        id: "A3031",
        name: "Soundcore Vortex",
        names: &["vortex"],
        caps: Capabilities {
            ambient_modes: ALL,
            noise_canceling_modes: PLACES,
            equalizer: eq(2, 8, EqWrite::Plain),
            equalizer_writable: true,
            layout: DOT2,
            ..EARBUDS
        },
    },
    Model {
        id: "A3931",
        name: "Soundcore Life Dot 2 NC",
        names: &["life dot 2 nc", "dot 2 nc"],
        caps: Capabilities {
            ambient_modes: ALL,
            noise_canceling_modes: PLACES,
            transparency_modes: FULL_VOCAL,
            equalizer: eq(2, 8, EqWrite::Applied),
            equalizer_writable: true,
            layout: DOT2,
            ..EARBUDS
        },
    },
    Model {
        id: "A3935",
        name: "Soundcore Life A2 NC",
        names: &["life a2 nc", "a2 nc"],
        caps: Capabilities {
            ambient_modes: ALL,
            noise_canceling_modes: PLACES,
            transparency_modes: FULL_VOCAL,
            equalizer: eq(2, 8, EqWrite::Applied),
            equalizer_writable: true,
            layout: DOT2,
            ..EARBUDS
        },
    },
    Model {
        id: "A3909",
        name: "Soundcore Liberty 2 Pro",
        names: &["liberty 2 pro"],
        caps: Capabilities {
            layout: &[Skip(2), DualBattery],
            ..EARBUDS
        },
    },
    Model {
        id: "A3926",
        name: "Soundcore Life Dot 2S",
        names: &["life dot 2s", "dot 2s"],
        caps: Capabilities {
            equalizer: eq(2, 8, EqWrite::Plain),
            layout: &[
                Skip(2),
                DualBattery,
                Equalizer {
                    channels: 2,
                    bands: 8,
                },
            ],
            ..EARBUDS
        },
    },
    Model {
        id: "A3930",
        name: "Soundcore Liberty 2 Pro+",
        names: &["liberty 2 pro+", "liberty 2 pro plus"],
        caps: Capabilities {
            ambient_modes: NT,
            equalizer: eq(2, 8, EqWrite::Plain),
            layout: &[
                Skip(2),
                DualBattery,
                Equalizer {
                    channels: 2,
                    bands: 8,
                },
                Skip(53),
                SoundModes,
            ],
            ..EARBUDS
        },
    },
    Model {
        id: "A3951",
        name: "Soundcore Liberty Air 2 Pro",
        names: &["liberty air 2 pro"],
        caps: Capabilities {
            ambient_modes: ALL,
            noise_canceling_modes: PLACES_CUSTOM,
            transparency_modes: FULL_VOCAL,
            equalizer: eq(2, 8, EqWrite::Plain),
            layout: &[
                Skip(2),
                DualBattery,
                Equalizer {
                    channels: 2,
                    bands: 8,
                },
                Skip(53),
                SoundModes,
            ],
            ..EARBUDS
        },
    },
    Model {
        id: "A3933",
        name: "Soundcore Life Note 3",
        names: &["life note 3"],
        caps: Capabilities {
            ambient_modes: ALL,
            noise_canceling_modes: PLACES,
            transparency_modes: FULL_VOCAL,
            equalizer: eq(2, 10, EqWrite::Plain),
            equalizer_writable: true,
            layout: &[
                Skip(2),
                DualBattery,
                DualFirmware,
                Serial,
                Equalizer {
                    channels: 2,
                    bands: 10,
                },
                Skip(62),
                SoundModes,
                Skip(5),
                CaseBattery,
            ],
            ..EARBUDS
        },
    },
    Model {
        id: "A3939",
        name: "Soundcore Life P3",
        names: &["life p3"],
        caps: Capabilities {
            ambient_modes: ALL,
            noise_canceling_modes: PLACES,
            transparency_modes: FULL_VOCAL,
            equalizer: eq(2, 10, EqWrite::Plain),
            equalizer_writable: true,
            layout: &[
                Skip(2),
                DualBattery,
                DualFirmware,
                Serial,
                Equalizer {
                    channels: 2,
                    bands: 10,
                },
                Skip(62),
                SoundModes,
                Skip(5),
                CaseBattery,
            ],
            ..EARBUDS
        },
    },
    Model {
        id: "A3945",
        name: "Soundcore Life Note 3S",
        names: &["life note 3s"],
        caps: Capabilities {
            equalizer: eq(2, 10, EqWrite::Plain),
            equalizer_writable: true,
            layout: &[
                Skip(2),
                DualBattery,
                DualFirmware,
                Serial,
                Equalizer {
                    channels: 2,
                    bands: 10,
                },
                Skip(15),
                CaseBattery,
            ],
            ..EARBUDS
        },
    },
    Model {
        id: "A3944",
        name: "Soundcore Life P2 Mini",
        names: &["life p2 mini", "p2 mini"],
        caps: Capabilities {
            equalizer: eq(1, 10, EqWrite::Applied),
            equalizer_writable: true,
            presets: PresetRules {
                custom: false,
                ..PresetRules::COMMON
            },
            layout: P2_MINI,
            ..EARBUDS
        },
    },
    Model {
        id: "A3948",
        name: "soundcore A20i",
        names: &["a20i"],
        caps: Capabilities {
            equalizer: eq(1, 10, EqWrite::Applied),
            equalizer_writable: true,
            presets: PresetRules::V2,
            layout: P2_MINI,
            ..EARBUDS
        },
    },
    Model {
        id: "A3949",
        name: "soundcore P20i / P25i / R50i",
        names: &["p20i", "p25i", "r50i"],
        caps: Capabilities {
            equalizer: eq(1, 10, EqWrite::Applied),
            equalizer_writable: true,
            presets: PresetRules {
                custom: false,
                ..PresetRules::V2
            },
            layout: P2_MINI,
            ..EARBUDS
        },
    },
    Model {
        id: "A3936",
        name: "soundcore Space A40",
        names: &["space a40", "a40"],
        caps: Capabilities {
            ambient_modes: ALL,
            equalizer: eq(2, 10, EqWrite::Plain),
            presets: PresetRules::V2,
            case: BatteryScale::TEN,
            layout: &[
                Skip(2),
                DualBattery,
                DualFirmware,
                Serial,
                Equalizer {
                    channels: 2,
                    bands: 10,
                },
                Skip(63),
                SoundModesV2(6),
                Skip(1),
                CaseBattery,
            ],
            ..EARBUDS
        },
    },
    Model {
        id: "A3947",
        name: "soundcore Liberty 4 NC",
        names: &["liberty 4 nc"],
        caps: Capabilities {
            ambient_modes: ALL,
            equalizer: eq(2, 10, EqWrite::Plain),
            presets: PresetRules::V2,
            layout: &[
                Skip(2),
                DualBattery,
                DualFirmware,
                Serial,
                Skip(5),
                Equalizer {
                    channels: 2,
                    bands: 10,
                },
                Skip(67),
                SoundModesV2(7),
                Skip(6),
                CaseBattery,
            ],
            ..EARBUDS
        },
    },
    Model {
        id: "A3952",
        name: "soundcore Liberty 3 Pro",
        names: &["liberty 3 pro"],
        caps: Capabilities {
            ambient_modes: ALL,
            equalizer: eq(2, 10, EqWrite::Plain),
            layout: &[
                Skip(2),
                DualBattery,
                DualFirmware,
                Serial,
                Equalizer {
                    channels: 2,
                    bands: 10,
                },
                Skip(67),
                SoundModesV2(6),
                Skip(3),
                CaseBattery,
            ],
            ..EARBUDS
        },
    },
    Model {
        id: "A3953",
        name: "soundcore Liberty 4",
        names: &["liberty 4"],
        caps: Capabilities {
            ambient_modes: ALL,
            equalizer: eq(2, 10, EqWrite::Plain),
            presets: PresetRules::V2,
            // Sound modes follow a variable-length block: they arrive with pushes.
            layout: &[
                Skip(2),
                DualBattery,
                DualFirmware,
                Serial,
                Equalizer {
                    channels: 2,
                    bands: 10,
                },
            ],
            ..EARBUDS
        },
    },
    Model {
        id: "A3954",
        name: "soundcore Liberty 4 Pro",
        names: &["liberty 4 pro"],
        caps: Capabilities {
            ambient_modes: ALL,
            equalizer: eq(2, 10, EqWrite::Plain),
            presets: PresetRules::V2,
            battery: BatteryScale::HUNDRED,
            case: BatteryScale::TEN_PLUS_ONE,
            layout: &[
                Skip(2),
                DualBattery,
                DualFirmware,
                Serial,
                Skip(5),
                CaseBattery,
                Skip(6),
                Equalizer {
                    channels: 2,
                    bands: 10,
                },
                Skip(59),
                SoundModesV2(4),
            ],
            ..EARBUDS
        },
    },
    Model {
        id: "A3955",
        name: "soundcore P40i",
        names: &["p40i"],
        caps: Capabilities {
            ambient_modes: ALL,
            equalizer: eq(2, 10, EqWrite::Plain),
            presets: PresetRules::V2,
            layout: &[
                Skip(2),
                DualBattery,
                DualFirmware,
                Serial,
                Skip(5),
                CaseBattery,
                Equalizer {
                    channels: 2,
                    bands: 10,
                },
                Skip(59),
                SoundModesV2(7),
            ],
            ..EARBUDS
        },
    },
    Model {
        id: "A3957",
        name: "soundcore Liberty 5",
        names: &["liberty 5"],
        caps: Capabilities {
            ambient_modes: ALL,
            equalizer: eq(2, 10, EqWrite::Plain),
            presets: PresetRules::V2,
            battery: BatteryScale::TEN_PLUS_ONE,
            case: BatteryScale::TEN_PLUS_ONE,
            layout: &[
                Skip(2),
                DualBattery,
                DualFirmware,
                Serial,
                Skip(5),
                CaseBattery,
                Equalizer {
                    channels: 2,
                    bands: 10,
                },
                Skip(59),
                SoundModesV2(7),
            ],
            ..EARBUDS
        },
    },
    Model {
        id: "A3959",
        name: "soundcore P30i / R50i NC",
        names: &["p30i", "r50i nc"],
        caps: Capabilities {
            ambient_modes: ALL,
            equalizer: eq(1, 10, EqWrite::Applied),
            equalizer_writable: true,
            presets: PresetRules::V2,
            battery: BatteryScale::TEN,
            layout: &[
                Skip(2),
                DualBattery,
                DualFirmware,
                Serial,
                Equalizer {
                    channels: 1,
                    bands: 10,
                },
                Skip(20),
                SoundModesV2(7),
            ],
            ..EARBUDS
        },
    },
    Model {
        id: "A3968",
        name: "soundcore Sport X20",
        names: &["sport x20", "x20"],
        caps: Capabilities {
            ambient_modes: ALL,
            equalizer: eq(2, 10, EqWrite::Plain),
            presets: PresetRules::V2,
            layout: &[
                Skip(2),
                DualBattery,
                DualFirmware,
                Serial,
                Skip(5),
                CaseBattery,
                Equalizer {
                    channels: 2,
                    bands: 10,
                },
                Skip(57),
                SoundModesV2(6),
            ],
            ..EARBUDS
        },
    },
    Model {
        id: "A3876",
        name: "soundcore V20i",
        names: &["v20i"],
        caps: Capabilities {
            equalizer: eq(1, 10, EqWrite::Applied),
            equalizer_writable: true,
            presets: PresetRules {
                only: &[0],
                ..PresetRules::COMMON
            },
            battery: BatteryScale::TEN_PLUS_ONE,
            layout: &[
                Skip(2),
                DualBatteryLevel,
                DualFirmware,
                Serial,
                Skip(6),
                Equalizer {
                    channels: 2,
                    bands: 10,
                },
            ],
            ..EARBUDS
        },
    },
    Model {
        id: "A3330",
        name: "soundcore C30i",
        names: &["c30i"],
        caps: Capabilities {
            equalizer: eq(1, 10, EqWrite::Applied),
            equalizer_writable: true,
            presets: PresetRules {
                only: &[0],
                ..PresetRules::COMMON
            },
            layout: &[
                Skip(2),
                DualBatteryLevel,
                DualFirmware,
                Serial,
                Skip(5),
                CaseBattery,
                Skip(14),
                Equalizer {
                    channels: 1,
                    bands: 10,
                },
            ],
            ..EARBUDS
        },
    },
    Model {
        id: "A3388",
        name: "soundcore AeroClip",
        names: &["aeroclip"],
        caps: Capabilities {
            equalizer: eq(1, 10, EqWrite::Applied),
            equalizer_writable: true,
            presets: PresetRules::V2,
            battery: BatteryScale::TEN_PLUS_ONE,
            case: BatteryScale::TEN_PLUS_ONE,
            layout: &[
                Skip(2),
                DualBatteryLevel,
                DualFirmware,
                Serial,
                Skip(5),
                CaseBattery,
                Skip(14),
                Equalizer {
                    channels: 1,
                    bands: 10,
                },
            ],
            ..EARBUDS
        },
    },
    Model {
        id: "D1101",
        name: "soundcore C50i",
        names: &["c50i"],
        caps: Capabilities {
            equalizer: eq(2, 10, EqWrite::Plain),
            equalizer_writable: true,
            presets: PresetRules {
                only: &[0x00, 0x02, 0x04, 0x05, 0x14],
                ..PresetRules::COMMON
            },
            battery: BatteryScale::TEN_PLUS_ONE,
            layout: &[
                Skip(2),
                DualBatteryLevel,
                DualFirmware,
                Serial,
                Equalizer {
                    channels: 1,
                    bands: 10,
                },
            ],
            ..EARBUDS
        },
    },
    Model {
        id: "D1202",
        name: "soundcore P31i / R60i NC",
        names: &["p31i", "r60i"],
        caps: Capabilities {
            ambient_modes: ALL,
            equalizer: eq(2, 10, EqWrite::Plain),
            presets: PresetRules::V2,
            battery: BatteryScale::TEN_PLUS_ONE,
            case: BatteryScale::TEN_PLUS_ONE,
            layout: &[
                Skip(2),
                DualBattery,
                DualFirmware,
                Serial,
                Skip(5),
                CaseBattery,
                Equalizer {
                    channels: 2,
                    bands: 10,
                },
                Skip(59),
                SoundModesV2(8),
            ],
            ..EARBUDS
        },
    },
    Model {
        id: "D1301",
        name: "soundcore Sleep A30",
        names: &["sleep a30"],
        caps: Capabilities {
            layout: &[Skip(2), DualBatteryLevel, DualFirmware, Serial],
            ..EARBUDS
        },
    },
];

/// By product number ("A3004", "3004", "D1402" or "1402").
pub fn by_id(id: &str) -> Option<&'static Model> {
    let id = id.trim().to_uppercase();
    MODELS.iter().find(|m| m.id == id).or_else(|| {
        MODELS
            .iter()
            .find(|m| m.id[1..] == *id.trim_start_matches(['A', 'D']))
    })
}

/// Product number from the vendor service UUID (its last two bytes, e.g. `30 04` → "3004").
pub fn product_number(u: &[u8; 16]) -> String {
    format!("{:02x}{:02x}", u[14], u[15])
}

pub fn from_vendor_uuid(u: &[u8; 16]) -> Option<&'static Model> {
    by_id(&product_number(u))
}

/// From the Bluetooth name ("soundcore Q20i"); the most specific name wins.
pub fn from_name(name: &str) -> Option<&'static Model> {
    let n = name.to_lowercase();
    MODELS
        .iter()
        .flat_map(|m| m.names.iter().map(move |frag| (m, *frag)))
        .filter(|(_, frag)| n.contains(frag))
        .max_by_key(|(_, frag)| frag.len())
        .map(|(m, _)| m)
}

/// From the serial number, which starts with the product number on most products.
pub fn from_serial(serial: &str) -> Option<&'static Model> {
    serial
        .get(..4)
        .filter(|s| s.chars().all(|c| c.is_ascii_digit()))
        .and_then(by_id)
}

/// Best guess at what kind of product a name designates.
pub fn kind_from_name(name: &str) -> Kind {
    if let Some(m) = from_name(name) {
        return m.caps.kind;
    }
    let n = name.to_lowercase();
    let earbuds = [
        "liberty",
        "life dot",
        "life note",
        "life p",
        "life a",
        "space a",
        "aerofit",
        "aeroclip",
        "sport",
        "r50",
        "r60",
        "p20",
        "p25",
        "p30",
        "p31",
        "p40",
        "a20",
        "a25",
        "a30",
        "c30",
        "c40",
        "c50",
        "v20",
        "dot",
        "note",
        "buds",
    ];
    let speakers = [
        "motion", "boom", "flare", "select", "rave", "mini", "glow", "icon",
    ];
    if earbuds.iter().any(|k| n.contains(k)) {
        Kind::Earbuds
    } else if speakers.iter().any(|k| n.contains(k)) {
        Kind::Speaker
    } else {
        Kind::Headphones
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_q20i() {
        let u = [
            0x0c, 0xf1, 0x2d, 0x31, 0xfa, 0xc3, 0x45, 0x53, 0xbd, 0x80, 0xd6, 0x83, 0x2e, 0x7b,
            0x30, 0x04,
        ];
        assert_eq!(product_number(&u), "3004");
        assert_eq!(from_vendor_uuid(&u).unwrap().id, "A3004");
        assert_eq!(from_name("soundcore Q20i").unwrap().id, "A3004");
        assert!(from_name("CMF Buds Pro 2").is_none());
        assert_eq!(from_serial("3004ABCDEF123456").unwrap().id, "A3004");
        assert_eq!(by_id("a3004").unwrap().name, "soundcore Q20i");
        assert_eq!(by_id("1402").unwrap().id, "D1402");
    }

    #[test]
    fn most_specific_name_wins() {
        assert_eq!(from_name("soundcore Liberty 4 NC").unwrap().id, "A3947");
        assert_eq!(from_name("soundcore Liberty 4 Pro").unwrap().id, "A3954");
        assert_eq!(from_name("soundcore Liberty 4").unwrap().id, "A3953");
        assert_eq!(from_name("Soundcore Life Note 3S").unwrap().id, "A3945");
        assert_eq!(from_name("soundcore Space One Pro").unwrap().id, "A3062");
        assert_eq!(from_name("soundcore R50i NC").unwrap().id, "A3959");
    }

    #[test]
    fn kinds_from_names() {
        assert_eq!(kind_from_name("soundcore Liberty 4 NC"), Kind::Earbuds);
        assert_eq!(kind_from_name("Soundcore Life Q30"), Kind::Headphones);
        assert_eq!(kind_from_name("Soundcore Motion+"), Kind::Speaker);
    }

    #[test]
    fn table_is_consistent() {
        for (i, a) in MODELS.iter().enumerate() {
            for b in &MODELS[i + 1..] {
                assert_ne!(a.id, b.id);
            }
            // A writable equalizer needs its format, and a layout that reads it.
            if a.caps.equalizer_writable {
                assert!(a.caps.equalizer.is_some(), "{}", a.id);
            }
            let has_eq_field = a
                .caps
                .layout
                .iter()
                .any(|f| matches!(f, Field::Equalizer { .. }));
            if a.caps.equalizer.is_some() {
                assert!(has_eq_field, "{} shows an equalizer it never reads", a.id);
            }
        }
    }
}
