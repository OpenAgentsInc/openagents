//! NIP-03 OpenTimestamps attestations.
//!
//! A kind `1040` event carries one `.ots` proof. The proof's digest must be
//! the `e` tag's event id, and the proof must end at exactly one Bitcoin
//! block-height attestation. Pending and non-Bitcoin attestations are refused.
//! Comparing that height to a Bitcoin block header needs a header source this
//! crate does not have.

use sha2::{Digest, Sha256};

use super::hex::decode_lower_hex;
use super::{DomainError, Event};

const MAGIC: &[u8] = b"\x00OpenTimestamps\x00\x00Proof\x00\xbf\x89\xe2\xe8\x84\xe8\x92\x94";
const BITCOIN: [u8; 8] = [0x05, 0x88, 0x96, 0x0d, 0x73, 0xd7, 0x19, 0x01];
const PENDING: [u8; 8] = [0x83, 0xdf, 0xe3, 0x0d, 0x2e, 0xf9, 0x0c, 0x8e];
const MAX_PROOF_BYTES: usize = 65_536;
const MAX_MESSAGE_BYTES: usize = 4_096;
const MAX_DEPTH: u32 = 256;

/// A kind `1040` attestation that binds one event id to one Bitcoin height.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitcoinAttestation {
    pub event_id: String,
    pub target_kind: u16,
    pub bitcoin_height: u64,
}

/// Read a kind `1040` attestation.
///
/// # Errors
///
/// Returns an invalid-event error when the tags, the base64 body, or the
/// OpenTimestamps proof do not meet the pinned rules.
pub fn open_attestation(event: &Event) -> Result<BitcoinAttestation, DomainError> {
    if event.kind != 1_040 {
        return Err(invalid("NIP-03 attestation must have kind 1040"));
    }
    let (event_id, relay) = event_reference(event)?;
    if let Some(relay) = relay
        && !relay_url(relay)
    {
        return Err(invalid("NIP-03 relay hint must be a ws:// or wss:// URL"));
    }
    let target_kind = target_kind(event)?;
    let proof = decode_base64(&event.content)?;
    if proof.len() > MAX_PROOF_BYTES {
        return Err(invalid("NIP-03 proof is larger than 65536 bytes"));
    }
    let digest = decode_lower_hex::<32>(&event_id, "attested event id")
        .map_err(|_| invalid("NIP-03 e tag must be a 32-byte hex event id"))?;
    let height = bitcoin_height(&proof, &digest)?;
    Ok(BitcoinAttestation {
        event_id,
        target_kind,
        bitcoin_height: height,
    })
}

fn event_reference(event: &Event) -> Result<(String, Option<&str>), DomainError> {
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("e"))
        .collect::<Vec<_>>();
    if tags.len() != 1 {
        return Err(invalid("NIP-03 attestation requires exactly one e tag"));
    }
    let values = tags[0].as_slice();
    if values.len() < 2 || values.len() > 3 || values[1].is_empty() {
        return Err(invalid(
            "NIP-03 e tag must name an event id and an optional relay",
        ));
    }
    Ok((values[1].clone(), values.get(2).map(String::as_str)))
}

fn target_kind(event: &Event) -> Result<u16, DomainError> {
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("k"))
        .collect::<Vec<_>>();
    if tags.len() != 1 {
        return Err(invalid("NIP-03 attestation requires exactly one k tag"));
    }
    let values = tags[0].as_slice();
    if values.len() != 2 {
        return Err(invalid("NIP-03 k tag must name one target kind"));
    }
    values[1]
        .parse::<u16>()
        .map_err(|_| invalid("NIP-03 k tag must be a kind number"))
}

fn bitcoin_height(proof: &[u8], event_id: &[u8; 32]) -> Result<u64, DomainError> {
    let mut cursor = Cursor { proof, at: 0 };
    cursor.expect(MAGIC, "NIP-03 proof magic is not an OpenTimestamps file")?;
    if cursor.byte()? != 1 {
        return Err(invalid("NIP-03 proof major version must be 1"));
    }
    if cursor.byte()? != 0x08 {
        return Err(invalid("NIP-03 proof must hash the event id with SHA-256"));
    }
    let digest = cursor.exact(32)?;
    if digest != event_id {
        return Err(invalid(
            "NIP-03 proof digest is not the referenced event id",
        ));
    }
    let mut found = None;
    walk(&mut cursor, digest, MAX_DEPTH, &mut found)?;
    if !cursor.done() {
        return Err(invalid("NIP-03 proof has trailing bytes"));
    }
    found.ok_or_else(|| invalid("NIP-03 proof has no Bitcoin attestation"))
}

fn walk(
    cursor: &mut Cursor<'_>,
    message: &[u8],
    depth: u32,
    found: &mut Option<u64>,
) -> Result<(), DomainError> {
    if depth == 0 {
        return Err(invalid("NIP-03 proof is too deep"));
    }
    if message.is_empty() || message.len() > MAX_MESSAGE_BYTES {
        return Err(invalid("NIP-03 proof message length is out of bounds"));
    }
    let mut tag = cursor.byte()?;
    while tag == 0xff {
        let branched = cursor.byte()?;
        step(cursor, message, branched, depth, found)?;
        tag = cursor.byte()?;
    }
    step(cursor, message, tag, depth, found)
}

fn step(
    cursor: &mut Cursor<'_>,
    message: &[u8],
    tag: u8,
    depth: u32,
    found: &mut Option<u64>,
) -> Result<(), DomainError> {
    match tag {
        0x00 => attest(cursor, found),
        0x08 => {
            let next = Sha256::digest(message);
            walk(cursor, &next, depth - 1, found)
        }
        0xf0 => {
            let suffix = cursor.varbytes(MAX_MESSAGE_BYTES)?;
            if suffix.is_empty() {
                return Err(invalid("NIP-03 append argument is empty"));
            }
            let mut next = Vec::with_capacity(message.len() + suffix.len());
            next.extend_from_slice(message);
            next.extend_from_slice(suffix);
            walk(cursor, &next, depth - 1, found)
        }
        0xf1 => {
            let prefix = cursor.varbytes(MAX_MESSAGE_BYTES)?;
            if prefix.is_empty() {
                return Err(invalid("NIP-03 prepend argument is empty"));
            }
            let mut next = Vec::with_capacity(prefix.len() + message.len());
            next.extend_from_slice(prefix);
            next.extend_from_slice(message);
            walk(cursor, &next, depth - 1, found)
        }
        _ => Err(invalid(
            "NIP-03 proof uses an operation this host does not accept",
        )),
    }
}

fn attest(cursor: &mut Cursor<'_>, found: &mut Option<u64>) -> Result<(), DomainError> {
    let magic = cursor.exact(8)?;
    let payload = cursor.varbytes(8_192)?;
    if magic == PENDING {
        return Err(invalid("NIP-03 proof must not carry a pending attestation"));
    }
    if magic != BITCOIN {
        return Err(invalid(
            "NIP-03 proof must attest with Bitcoin, not another chain",
        ));
    }
    if found.is_some() {
        return Err(invalid(
            "NIP-03 proof must contain a single Bitcoin attestation",
        ));
    }
    let mut payload_cursor = Cursor {
        proof: payload,
        at: 0,
    };
    let height = payload_cursor.varuint()?;
    if !payload_cursor.done() || height == 0 {
        return Err(invalid("NIP-03 Bitcoin attestation height is invalid"));
    }
    *found = Some(height);
    Ok(())
}

struct Cursor<'a> {
    proof: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn done(&self) -> bool {
        self.at == self.proof.len()
    }

    fn byte(&mut self) -> Result<u8, DomainError> {
        let byte = *self
            .proof
            .get(self.at)
            .ok_or_else(|| invalid("NIP-03 proof is truncated"))?;
        self.at += 1;
        Ok(byte)
    }

    fn exact(&mut self, length: usize) -> Result<&'a [u8], DomainError> {
        let end = self
            .at
            .checked_add(length)
            .filter(|end| *end <= self.proof.len())
            .ok_or_else(|| invalid("NIP-03 proof is truncated"))?;
        let bytes = &self.proof[self.at..end];
        self.at = end;
        Ok(bytes)
    }

    fn expect(&mut self, magic: &[u8], reason: &'static str) -> Result<(), DomainError> {
        if self.exact(magic.len())? == magic {
            Ok(())
        } else {
            Err(invalid(reason))
        }
    }

    fn varuint(&mut self) -> Result<u64, DomainError> {
        let mut value = 0_u64;
        let mut shift = 0_u32;
        loop {
            let byte = self.byte()?;
            if shift == 63 && byte > 1 {
                return Err(invalid("NIP-03 integer is out of range"));
            }
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
            if shift > 63 {
                return Err(invalid("NIP-03 integer is out of range"));
            }
        }
    }

    fn varbytes(&mut self, max: usize) -> Result<&'a [u8], DomainError> {
        let length = usize::try_from(self.varuint()?).unwrap_or(usize::MAX);
        if length > max {
            return Err(invalid("NIP-03 length is out of bounds"));
        }
        self.exact(length)
    }
}

fn relay_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("wss://")
        .or_else(|| value.strip_prefix("ws://"))
    else {
        return false;
    };
    !rest.is_empty() && value.len() <= 2_048 && !value.chars().any(char::is_whitespace)
}

fn decode_base64(value: &str) -> Result<Vec<u8>, DomainError> {
    if value.is_empty() || !value.is_ascii() {
        return Err(invalid("NIP-03 content must be base64"));
    }
    let bytes = value.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return Err(invalid("NIP-03 content must be base64"));
    }
    let mut output = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks_exact(4) {
        let mut values = [0_u8; 4];
        let mut padding = 0_usize;
        for (index, byte) in chunk.iter().enumerate() {
            values[index] = match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                b'=' if index >= 2 && chunk[index..].iter().all(|item| *item == b'=') => {
                    padding += 1;
                    0
                }
                _ => return Err(invalid("NIP-03 content must be base64")),
            };
        }
        if padding > 2 {
            return Err(invalid("NIP-03 content must be base64"));
        }
        let packed = (u32::from(values[0]) << 18)
            | (u32::from(values[1]) << 12)
            | (u32::from(values[2]) << 6)
            | u32::from(values[3]);
        output.push((packed >> 16) as u8);
        if padding < 2 {
            output.push((packed >> 8) as u8);
        }
        if padding < 1 {
            output.push(packed as u8);
        }
    }
    Ok(output)
}

fn invalid(reason: &'static str) -> DomainError {
    DomainError::InvalidEvent(reason.into())
}

/// A minimal `.ots` file used by tests: SHA-256 of the event id, then one
/// Bitcoin height, with no calendar pending attestation.
#[cfg(test)]
pub(crate) fn minimal_bitcoin_proof(event_id: &[u8; 32], height: u64) -> Vec<u8> {
    let mut proof = Vec::new();
    proof.extend_from_slice(MAGIC);
    proof.push(1);
    proof.push(0x08);
    proof.extend_from_slice(event_id);
    proof.push(0x00);
    proof.extend_from_slice(&BITCOIN);
    let mut payload = Vec::new();
    write_varuint(&mut payload, height);
    write_varuint(&mut proof, payload.len() as u64);
    proof.extend_from_slice(&payload);
    proof
}

#[cfg(test)]
fn write_varuint(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::super::Tag;

    fn event(id: &str, kind: &str, content: &str, relay: bool) -> Event {
        let mut tags = vec![Tag::new(if relay {
            vec!["e".into(), id.into(), "wss://relay.example".into()]
        } else {
            vec!["e".into(), id.into()]
        })];
        tags.push(Tag::new(vec!["k".into(), kind.into()]));
        Event {
            id: "ab".repeat(32),
            pubkey: "cd".repeat(32),
            created_at: 1,
            kind: 1_040,
            tags,
            content: content.to_owned(),
            sig: "ef".repeat(64),
        }
    }

    fn standard_base64(bytes: &[u8]) -> String {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let b0 = chunk[0];
            let b1 = chunk.get(1).copied().unwrap_or(0);
            let b2 = chunk.get(2).copied().unwrap_or(0);
            let packed = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
            out.push(ALPHABET[((packed >> 18) & 63) as usize] as char);
            out.push(ALPHABET[((packed >> 12) & 63) as usize] as char);
            if chunk.len() > 1 {
                out.push(ALPHABET[((packed >> 6) & 63) as usize] as char);
            } else {
                out.push('=');
            }
            if chunk.len() > 2 {
                out.push(ALPHABET[(packed & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
        out
    }

    #[test]
    fn a_bitcoin_proof_binds_the_event_id_and_one_height() {
        let id = [0x11_u8; 32];
        let hex_id = "11".repeat(32);
        let proof = minimal_bitcoin_proof(&id, 810_391);
        assert!(proof.starts_with(MAGIC));
        assert_eq!(&proof[proof.len() - 13..], &{
            let mut tail = vec![0x00];
            tail.extend_from_slice(&BITCOIN);
            tail.extend_from_slice(&[0x03, 0x97, 0xbb, 0x31]);
            tail
        });
        let opened =
            open_attestation(&event(&hex_id, "1", &standard_base64(&proof), true)).unwrap();
        assert_eq!(opened.event_id, hex_id);
        assert_eq!(opened.target_kind, 1);
        assert_eq!(opened.bitcoin_height, 810_391);

        let wrong = open_attestation(&event(
            &"22".repeat(32),
            "1",
            &standard_base64(&proof),
            false,
        ));
        assert!(wrong.is_err(), "{wrong:?}");
    }

    #[test]
    fn a_pending_attestation_and_a_second_bitcoin_attestation_are_refused() {
        let id = [0x11_u8; 32];
        let mut pending = Vec::new();
        pending.extend_from_slice(MAGIC);
        pending.extend_from_slice(&[1, 0x08]);
        pending.extend_from_slice(&id);
        pending.push(0x00);
        pending.extend_from_slice(&PENDING);
        let url = b"https://alice.btc.calendar.opentimestamps.org";
        let mut payload = Vec::new();
        write_varuint(&mut payload, url.len() as u64);
        payload.extend_from_slice(url);
        write_varuint(&mut pending, payload.len() as u64);
        pending.extend_from_slice(&payload);
        let encoded = standard_base64(&pending);
        assert!(open_attestation(&event(&"11".repeat(32), "1", &encoded, false)).is_err());

        let mut two = minimal_bitcoin_proof(&id, 1);
        // Fork a second Bitcoin attestation before the existing one.
        let insert_at = MAGIC.len() + 1 + 1 + 32;
        let mut second = vec![0xff, 0x00];
        second.extend_from_slice(&BITCOIN);
        second.extend_from_slice(&[0x01, 0x02]);
        two.splice(insert_at..insert_at, second);
        assert!(
            open_attestation(&event(&"11".repeat(32), "1", &standard_base64(&two), false)).is_err()
        );
    }

    #[test]
    fn an_append_then_sha256_still_reaches_the_bitcoin_attestation() {
        let id = [0x22_u8; 32];
        let suffix = b"ots";
        let mut proof = Vec::new();
        proof.extend_from_slice(MAGIC);
        proof.extend_from_slice(&[1, 0x08]);
        proof.extend_from_slice(&id);
        proof.push(0xf0);
        write_varuint(&mut proof, suffix.len() as u64);
        proof.extend_from_slice(suffix);
        proof.push(0x08);
        proof.push(0x00);
        proof.extend_from_slice(&BITCOIN);
        proof.extend_from_slice(&[0x01, 0x05]);
        let opened = open_attestation(&event(
            &"22".repeat(32),
            "1040",
            &standard_base64(&proof),
            false,
        ))
        .unwrap();
        assert_eq!(opened.bitcoin_height, 5);
        assert_eq!(opened.target_kind, 1_040);
    }
}
