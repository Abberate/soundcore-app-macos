//! soundcore headphones control protocol (RFCOMM).
//!
//! Pure logic, no I/O: framing and checksum, reassembly, typed requests and events,
//! the built-in equalizer presets and a [`DeviceState`] that events update.
//! Protocol knowledge comes from the OpenSCQ30 project (GPL-3.0), whose documentation
//! of the soundcore wire format made this implementation possible.

pub mod eq;
pub mod frame;
pub mod message;
pub mod model;
pub mod state;
pub mod types;

pub use frame::{Command, Frame, Reassembler};
pub use message::{Event, StateReport, request};
pub use model::{Capabilities, Model};
pub use state::DeviceState;
pub use types::{AmbientMode, Battery, Equalizer, SoundModes, cmd};
