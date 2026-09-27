//! Simple versioned length framing for snapshot messages.
//!
//! Format (little-endian):
//! - 4-byte magic `RAF1`
//! - u32 LEN (bytes of payload)
//! - [u8; LEN] payload
//!
//! This allows multiplexed streams to delimit messages without peeking into
//! inner payloads. Inner payloads may themselves be versioned.

const FRAME_MAGIC: [u8; 4] = *b"RAF1";
const MAX_FRAME_LEN: usize = 1_048_576; // 1 MiB cap for safety

/// Write a framed message into `out`, appending to any existing bytes.
pub fn write_msg(out: &mut Vec<u8>, payload: &[u8]) {
    out.extend_from_slice(&FRAME_MAGIC);
    let len = u32::try_from(payload.len()).unwrap_or(0);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(payload);
}

/// Read a single framed message from `inp`. Returns the payload slice on success.
///
/// The returned slice borrows from `inp` and is valid as long as `inp` is.
pub fn read_msg(inp: &[u8]) -> anyhow::Result<&[u8]> {
    use anyhow::bail;
    if inp.len() < 8 {
        bail!("short frame header");
    }
    if inp[0..4] != FRAME_MAGIC {
        bail!("bad frame magic");
    }
    let mut lenb = [0u8; 4];
    lenb.copy_from_slice(&inp[4..8]);
    let len = u32::from_le_bytes(lenb) as usize;
    if len > MAX_FRAME_LEN {
        bail!("frame too large: {len} > {MAX_FRAME_LEN}");
    }
    if inp.len() < 8 + len {
        bail!("short frame payload");
    }
    Ok(&inp[8..8 + len])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_frame() {
        let payload = b"hello";
        let mut buf = Vec::new();
        write_msg(&mut buf, payload);
        let got = read_msg(&buf).expect("read");
        assert_eq!(got, payload);
    }
    #[test]
    fn rejects_wrong_version() {
        let mut buf = vec![b'B', b'A', b'D', b'!', 0, 0, 0, 0];
        assert!(read_msg(&buf).is_err());
        // fix magic but declare oversize to trigger cap
        buf[0..4].copy_from_slice(&FRAME_MAGIC);
        buf[4..8].copy_from_slice(&(u32::MAX).to_le_bytes());
        assert!(read_msg(&buf).is_err());
    }
}
