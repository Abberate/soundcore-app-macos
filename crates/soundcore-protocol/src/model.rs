//! Known products.
//!
//! The vendor control service UUID ends with the product number
//! (`0cf12d31-fac3-4553-bd80-d6832e7b3004` on the Q20i, model A3004), which is how a
//! paired device is identified before any data is exchanged.

use crate::types::AmbientMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Model {
    /// Product number, e.g. "A3004".
    pub id: &'static str,
    pub name: &'static str,
    /// `None` for products this version recognises but cannot drive yet.
    pub caps: Option<Capabilities>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub ambient_modes: &'static [AmbientMode],
    pub equalizer: bool,
    /// Battery steps reported (0…n).
    pub battery_steps: u8,
}

const Q20I: Capabilities = Capabilities {
    ambient_modes: &[
        AmbientMode::NoiseCanceling,
        AmbientMode::Normal,
        AmbientMode::Transparency,
    ],
    equalizer: true,
    battery_steps: 5,
};

pub const MODELS: &[Model] = &[
    Model {
        id: "A3004",
        name: "soundcore Q20i",
        caps: Some(Q20I),
    },
    Model {
        id: "A3005",
        name: "soundcore Q11i",
        caps: None,
    },
    Model {
        id: "A3025",
        name: "Soundcore Life Q20",
        caps: None,
    },
    Model {
        id: "A3027",
        name: "Soundcore Life Q35",
        caps: None,
    },
    Model {
        id: "A3028",
        name: "Soundcore Life Q30",
        caps: None,
    },
    Model {
        id: "A3029",
        name: "Soundcore Life Tune",
        caps: None,
    },
    Model {
        id: "A3040",
        name: "soundcore Space Q45",
        caps: None,
    },
    Model {
        id: "A3062",
        name: "soundcore Space One",
        caps: None,
    },
];

/// By product number ("A3004" or "3004").
pub fn by_id(id: &str) -> Option<&'static Model> {
    let digits = id.trim().trim_start_matches(['A', 'a']);
    MODELS.iter().find(|m| &m.id[1..] == digits)
}

/// From the vendor service UUID (its last two bytes carry the product number).
pub fn from_vendor_uuid(u: &[u8; 16]) -> Option<&'static Model> {
    by_id(
        &format!("{:x}{:02x}", u[14] & 0x0f, u[15])
            .to_uppercase()
            .replace(' ', ""),
    )
    .or_else(|| by_id(&format!("{:02x}{:02x}", u[14], u[15])))
}

/// From the Bluetooth name ("soundcore Q20i").
pub fn from_name(name: &str) -> Option<&'static Model> {
    let n = name.to_lowercase();
    MODELS.iter().find(|m| {
        let short = m.name.to_lowercase();
        let short = short.trim_start_matches("soundcore ");
        n.ends_with(short)
    })
}

/// From the serial number, which starts with the product number on recent products.
pub fn from_serial(serial: &str) -> Option<&'static Model> {
    serial.get(..4).and_then(by_id)
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
        assert_eq!(from_vendor_uuid(&u).unwrap().id, "A3004");
        assert_eq!(from_name("soundcore Q20i").unwrap().id, "A3004");
        assert_eq!(from_name("Soundcore Life Q30").unwrap().id, "A3028");
        assert!(from_name("CMF Buds Pro 2").is_none());
        assert_eq!(from_serial("3004ABCDEF123456").unwrap().id, "A3004");
        assert_eq!(by_id("a3004").unwrap().name, "soundcore Q20i");
        assert!(by_id("A3028").unwrap().caps.is_none());
    }
}
