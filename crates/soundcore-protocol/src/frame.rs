//! Wire framing.
//!
//! ```text
//! to the headphones:   08 ee 00 00 00 | command (2) | length (u16 LE) | body | checksum
//! from the headphones: 09 ff 00 00 01 | command (2) | length (u16 LE) | body | checksum
//! ```
//!
//! `length` counts the whole frame, header and checksum included. The checksum is the
//! 8-bit sum of every byte that precedes it.

/// A two-byte command identifier, e.g. `[0x01, 0x01]`.
pub type Command = [u8; 2];

pub const TO_DEVICE: [u8; 5] = [0x08, 0xee, 0x00, 0x00, 0x00];
pub const FROM_DEVICE: [u8; 5] = [0x09, 0xff, 0x00, 0x00, 0x01];

/// Header (5) + command (2) + length (2) + checksum (1).
const OVERHEAD: usize = 10;
/// Anything longer is treated as a corrupted length field.
const MAX_FRAME: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub command: Command,
    pub body: Vec<u8>,
}

/// 8-bit additive checksum.
pub fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |acc, b| acc.wrapping_add(*b))
}

fn build(header: [u8; 5], command: Command, body: &[u8]) -> Vec<u8> {
    let len = (OVERHEAD + body.len()) as u16;
    let mut out = Vec::with_capacity(OVERHEAD + body.len());
    out.extend_from_slice(&header);
    out.extend_from_slice(&command);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(body);
    out.push(checksum(&out));
    out
}

/// A frame for the headphones.
pub fn encode(command: Command, body: &[u8]) -> Vec<u8> {
    build(TO_DEVICE, command, body)
}

/// A frame as the headphones send it (demo mode and tests).
pub fn encode_from_device(command: Command, body: &[u8]) -> Vec<u8> {
    build(FROM_DEVICE, command, body)
}

/// Splits the incoming byte stream into frames, resynchronising on garbage.
#[derive(Debug, Default)]
pub struct Reassembler {
    buf: Vec<u8>,
    /// Bytes thrown away while resynchronising (diagnostics).
    pub dropped: usize,
}

impl Reassembler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, data: &[u8]) -> Vec<Frame> {
        self.buf.extend_from_slice(data);
        let mut frames = Vec::new();
        loop {
            // Align on a header.
            match find(&self.buf, &FROM_DEVICE) {
                Some(0) => {}
                Some(n) => {
                    self.dropped += n;
                    self.buf.drain(..n);
                }
                None => {
                    // Keep a possible header prefix at the end.
                    let keep = (1..FROM_DEVICE.len())
                        .rev()
                        .find(|&k| self.buf.ends_with(&FROM_DEVICE[..k]))
                        .unwrap_or(0);
                    let n = self.buf.len() - keep;
                    self.dropped += n;
                    self.buf.drain(..n);
                    return frames;
                }
            }
            if self.buf.len() < 9 {
                return frames;
            }
            let len = u16::from_le_bytes([self.buf[7], self.buf[8]]) as usize;
            if !(OVERHEAD..=MAX_FRAME).contains(&len) {
                self.skip_one();
                continue;
            }
            if self.buf.len() < len {
                return frames;
            }
            if checksum(&self.buf[..len - 1]) != self.buf[len - 1] {
                self.skip_one();
                continue;
            }
            frames.push(Frame {
                command: [self.buf[5], self.buf[6]],
                body: self.buf[9..len - 1].to_vec(),
            });
            self.buf.drain(..len);
        }
    }

    fn skip_one(&mut self) {
        self.dropped += 1;
        self.buf.drain(..1);
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_state_bytes() {
        assert_eq!(
            encode([0x01, 0x01], &[]),
            vec![0x08, 0xee, 0x00, 0x00, 0x00, 0x01, 0x01, 0x0a, 0x00, 0x02]
        );
    }

    #[test]
    fn length_counts_whole_frame() {
        let f = encode([0x06, 0x81], &[2, 1, 0, 6]);
        assert_eq!(f.len(), 14);
        assert_eq!(u16::from_le_bytes([f[7], f[8]]), 14);
        assert_eq!(*f.last().unwrap(), checksum(&f[..13]));
    }

    #[test]
    fn reassembles_split_and_noisy_stream() {
        let a = encode_from_device([0x06, 0x01], &[0, 1, 0, 6]);
        let b = encode_from_device([0x01, 0x03], &[4]);
        let mut stream = vec![0x00, 0x13, 0x09];
        stream.extend_from_slice(&a);
        stream.extend_from_slice(&b);
        let mut r = Reassembler::new();
        let mut frames = Vec::new();
        for chunk in stream.chunks(3) {
            frames.extend(r.push(chunk));
        }
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].command, [0x06, 0x01]);
        assert_eq!(frames[0].body, vec![0, 1, 0, 6]);
        assert_eq!(frames[1].command, [0x01, 0x03]);
        assert_eq!(frames[1].body, vec![4]);
        assert_eq!(r.dropped, 3);
    }

    #[test]
    fn rejects_bad_checksum_and_recovers() {
        let mut bad = encode_from_device([0x01, 0x03], &[4]);
        let n = bad.len();
        bad[n - 1] ^= 0xff;
        let good = encode_from_device([0x01, 0x04], &[1]);
        let mut r = Reassembler::new();
        let mut frames = r.push(&bad);
        frames.extend(r.push(&good));
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].command, [0x01, 0x04]);
    }

    #[test]
    fn empty_body_ack() {
        let ack = encode_from_device([0x06, 0x81], &[]);
        let frames = Reassembler::new().push(&ack);
        assert_eq!(
            frames,
            vec![Frame {
                command: [0x06, 0x81],
                body: vec![]
            }]
        );
    }
}
