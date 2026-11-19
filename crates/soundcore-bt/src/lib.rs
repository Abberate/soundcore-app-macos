//! Bluetooth Classic RFCOMM transport for soundcore headphones on macOS.
//!
//! On current macOS releases IOBluetooth is a shim over CoreBluetooth, which delivers
//! its callbacks on the **main dispatch queue**. Every IOBluetooth call is therefore
//! marshalled to the main thread; the main thread must be running a run loop (AppKit
//! in the GUI, [`run_main_loop`] in a CLI). Results come back through a callback
//! ([`LinkSink`]) so the protocol logic can live on any thread.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

/// Standard Serial Port Profile service class.
pub const SPP_UUID: [u8; 16] = uuid("00001101-0000-1000-8000-00805f9b34fb");
/// soundcore's own control service: `0cf12d31-fac3-4553-bd80-d6832e7?????`
/// (the last 20 bits vary between products).
pub const VENDOR_UUID: [u8; 16] = uuid("0cf12d31-fac3-4553-bd80-d6832e700000");

/// Whether `u` is a soundcore vendor control service UUID.
pub fn is_vendor_uuid(u: &[u8; 16]) -> bool {
    // Compare the first 108 bits: 13 full bytes plus the high nibble of byte 13.
    u[..13] == VENDOR_UUID[..13] && (u[13] & 0xF0) == (VENDOR_UUID[13] & 0xF0)
}

/// A device paired with this Mac.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairedDevice {
    pub name: String,
    /// `xx-xx-xx-xx-xx-xx` as reported by IOBluetooth.
    pub address: String,
    pub connected: bool,
    pub class_of_device: u32,
}

impl PairedDevice {
    /// Heuristic used to pre-select candidates (soundcore / Anker products).
    pub fn looks_like_soundcore(&self) -> bool {
        let n = self.name.to_lowercase();
        n.contains("soundcore") || n.starts_with("anker") || n.starts_with("life q")
    }
}

/// Which RFCOMM service to open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    /// The vendor control service when advertised, otherwise the standard SPP service.
    Control,
    /// A fixed RFCOMM channel (diagnostics).
    Channel(u8),
}

/// One RFCOMM service advertised by a device (SDP), for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceInfo {
    pub name: String,
    pub uuids: Vec<[u8; 16]>,
    pub channel: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkEvent {
    Opened { channel: u8 },
    Data(Vec<u8>),
    Closed,
    Failed(String),
}

/// Receives link events; called on the main thread, so it must be quick (e.g. a channel send).
pub type LinkSink = Box<dyn Fn(LinkEvent) + Send + 'static>;

const fn hex(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => panic!("invalid hex digit"),
    }
}

const fn uuid(s: &str) -> [u8; 16] {
    let b = s.as_bytes();
    let mut out = [0u8; 16];
    let (mut i, mut o) = (0, 0);
    while i < b.len() {
        if b[i] == b'-' {
            i += 1;
            continue;
        }
        out[o] = (hex(b[i]) << 4) | hex(b[i + 1]);
        o += 1;
        i += 2;
    }
    out
}

/// `0cf12d31-fac3-…` form.
pub fn format_uuid(u: &[u8; 16]) -> String {
    let h: String = u.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

/// Normalises `AA:BB:CC:DD:EE:FF` / `aa-bb-cc-dd-ee-ff` to `aa-bb-cc-dd-ee-ff`.
pub fn normalize_address(addr: &str) -> String {
    addr.trim().replace(':', "-").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vendor_uuid_mask() {
        assert!(is_vendor_uuid(&uuid(
            "0cf12d31-fac3-4553-bd80-d6832e7abcde"
        )));
        assert!(is_vendor_uuid(&VENDOR_UUID));
        assert!(!is_vendor_uuid(&uuid(
            "0cf12d31-fac3-4553-bd80-d6832e8abcde"
        )));
        assert!(!is_vendor_uuid(&SPP_UUID));
        assert_eq!(
            format_uuid(&SPP_UUID),
            "00001101-0000-1000-8000-00805f9b34fb"
        );
    }
}
