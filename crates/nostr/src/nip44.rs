//! Bounded NIP-44 v2 primitives for transport-neutral clients.

use secp256k1::{Parity, PublicKey, SecretKey, XOnlyPublicKey, ecdh};
use sha2::{Digest, Sha256};

const VERSION: u8 = 2;
const MIN_PAYLOAD_CHARS: usize = 132;
const MAX_CLIENT_PLAINTEXT_BYTES: usize = 256 * 1024;
const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn conversation_key(secret: &SecretKey, peer: &XOnlyPublicKey) -> [u8; 32] {
    let normalized = PublicKey::from_x_only_public_key(*peer, Parity::Even);
    let point = ecdh::shared_secret_point(&normalized, secret);
    hmac_sha256(b"nip44-v2", &point[..32])
}

pub fn encrypt(
    plaintext: &str,
    conversation_key: &[u8; 32],
    nonce: [u8; 32],
) -> Result<String, String> {
    let plaintext = plaintext.as_bytes();
    if plaintext.is_empty() || plaintext.len() > MAX_CLIENT_PLAINTEXT_BYTES {
        return Err(format!(
            "NIP-44 plaintext must contain 1 to {MAX_CLIENT_PLAINTEXT_BYTES} bytes"
        ));
    }
    let keys = message_keys(conversation_key, &nonce);
    let mut padded = pad(plaintext)?;
    chacha20_xor(&keys[..32], &keys[32..44], &mut padded);
    let mut authenticated = Vec::with_capacity(nonce.len() + padded.len());
    authenticated.extend_from_slice(&nonce);
    authenticated.extend_from_slice(&padded);
    let mac = hmac_sha256(&keys[44..], &authenticated);
    let mut payload = Vec::with_capacity(65 + padded.len());
    payload.push(VERSION);
    payload.extend_from_slice(&nonce);
    payload.extend_from_slice(&padded);
    payload.extend_from_slice(&mac);
    Ok(base64_encode(&payload))
}

pub fn decrypt(payload: &str, conversation_key: &[u8; 32]) -> Result<String, String> {
    if payload.starts_with('#') {
        return Err("NIP-44 payload encoding is unsupported".to_owned());
    }
    if payload.len() < MIN_PAYLOAD_CHARS
        || payload.len() > encoded_maximum(MAX_CLIENT_PLAINTEXT_BYTES)
    {
        return Err("NIP-44 payload size is outside the client bound".to_owned());
    }
    let decoded = base64_decode(payload)?;
    if decoded.len() < 99 {
        return Err("NIP-44 decoded payload is too short".to_owned());
    }
    if decoded[0] != VERSION {
        return Err(format!("NIP-44 version {} is unsupported", decoded[0]));
    }
    let nonce: [u8; 32] = decoded[1..33]
        .try_into()
        .map_err(|_| "NIP-44 nonce has the wrong length".to_owned())?;
    let ciphertext_end = decoded.len() - 32;
    let mut ciphertext = decoded[33..ciphertext_end].to_vec();
    let supplied_mac = &decoded[ciphertext_end..];
    let keys = message_keys(conversation_key, &nonce);
    let expected_mac = hmac_sha256(&keys[44..], &decoded[1..ciphertext_end]);
    if !constant_time_equal(supplied_mac, &expected_mac) {
        return Err("NIP-44 payload MAC is invalid".to_owned());
    }
    chacha20_xor(&keys[..32], &keys[32..44], &mut ciphertext);
    let plaintext = unpad(&ciphertext)?;
    String::from_utf8(plaintext.to_vec())
        .map_err(|_| "NIP-44 plaintext is not valid UTF-8".to_owned())
}

fn message_keys(conversation_key: &[u8; 32], nonce: &[u8; 32]) -> [u8; 76] {
    hkdf_expand(conversation_key, nonce)
}

fn hkdf_expand(key: &[u8], info: &[u8]) -> [u8; 76] {
    let mut output = [0_u8; 76];
    let mut previous = Vec::new();
    let mut written = 0;
    for counter in 1_u8..=3 {
        let mut input = Vec::with_capacity(previous.len() + info.len() + 1);
        input.extend_from_slice(&previous);
        input.extend_from_slice(info);
        input.push(counter);
        previous = hmac_sha256(key, &input).to_vec();
        let take = (output.len() - written).min(previous.len());
        output[written..written + take].copy_from_slice(&previous[..take]);
        written += take;
    }
    output
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut normalized = [0_u8; 64];
    if key.len() > normalized.len() {
        normalized[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        normalized[..key.len()].copy_from_slice(key);
    }
    let mut inner_pad = [0x36_u8; 64];
    let mut outer_pad = [0x5c_u8; 64];
    for index in 0..64 {
        inner_pad[index] ^= normalized[index];
        outer_pad[index] ^= normalized[index];
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(message);
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_digest);
    outer.finalize().into()
}

fn pad(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let padded_length = padded_length(plaintext.len())?;
    let extended = plaintext.len() >= 65_536;
    let prefix_length = if extended { 6 } else { 2 };
    let mut padded = vec![0_u8; prefix_length + padded_length];
    if extended {
        let length = u32::try_from(plaintext.len())
            .map_err(|_| "NIP-44 plaintext exceeds the protocol length".to_owned())?;
        padded[2..6].copy_from_slice(&length.to_be_bytes());
    } else {
        let length = u16::try_from(plaintext.len())
            .map_err(|_| "NIP-44 short plaintext length is invalid".to_owned())?;
        padded[..2].copy_from_slice(&length.to_be_bytes());
    }
    padded[prefix_length..prefix_length + plaintext.len()].copy_from_slice(plaintext);
    Ok(padded)
}

fn unpad(padded: &[u8]) -> Result<&[u8], String> {
    if padded.len() < 2 {
        return Err("NIP-44 padding is truncated".to_owned());
    }
    let short = u16::from_be_bytes([padded[0], padded[1]]);
    let (length, prefix_length) = if short == 0 {
        if padded.len() < 6 {
            return Err("NIP-44 extended padding is truncated".to_owned());
        }
        let length = u32::from_be_bytes([padded[2], padded[3], padded[4], padded[5]]) as usize;
        if length < 65_536 {
            return Err("NIP-44 extended padding length is noncanonical".to_owned());
        }
        (length, 6_usize)
    } else {
        (usize::from(short), 2_usize)
    };
    if length == 0 || length > MAX_CLIENT_PLAINTEXT_BYTES {
        return Err("NIP-44 plaintext length is outside the client bound".to_owned());
    }
    let expected = prefix_length
        .checked_add(padded_length(length)?)
        .ok_or_else(|| "NIP-44 padded length overflow".to_owned())?;
    if padded.len() != expected || prefix_length + length > padded.len() {
        return Err("NIP-44 padding length is invalid".to_owned());
    }
    if padded[prefix_length + length..]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err("NIP-44 padding is not zero-filled".to_owned());
    }
    Ok(&padded[prefix_length..prefix_length + length])
}

fn padded_length(length: usize) -> Result<usize, String> {
    if length == 0 || length > MAX_CLIENT_PLAINTEXT_BYTES {
        return Err("NIP-44 plaintext length is outside the client bound".to_owned());
    }
    if length <= 32 {
        return Ok(32);
    }
    let next_power = length
        .checked_next_power_of_two()
        .ok_or_else(|| "NIP-44 padded length overflow".to_owned())?;
    let chunk = if next_power <= 256 {
        32
    } else {
        next_power / 8
    };
    Ok(chunk * ((length - 1) / chunk + 1))
}

fn encoded_maximum(maximum_plaintext: usize) -> usize {
    let raw = 65 + 6 + maximum_plaintext.next_power_of_two();
    raw.div_ceil(3) * 4
}

fn chacha20_xor(key: &[u8], nonce: &[u8], data: &mut [u8]) {
    let mut counter = 0_u32;
    for chunk in data.chunks_mut(64) {
        let block = chacha20_block(key, nonce, counter);
        for (byte, mask) in chunk.iter_mut().zip(block) {
            *byte ^= mask;
        }
        counter = counter.wrapping_add(1);
    }
}

fn chacha20_block(key: &[u8], nonce: &[u8], counter: u32) -> [u8; 64] {
    let mut state = [0_u32; 16];
    state[..4].copy_from_slice(&[0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574]);
    for (index, bytes) in key.chunks_exact(4).enumerate() {
        state[4 + index] = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    }
    state[12] = counter;
    for (index, bytes) in nonce.chunks_exact(4).enumerate() {
        state[13 + index] = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    }
    let initial = state;
    for _ in 0..10 {
        quarter_round(&mut state, 0, 4, 8, 12);
        quarter_round(&mut state, 1, 5, 9, 13);
        quarter_round(&mut state, 2, 6, 10, 14);
        quarter_round(&mut state, 3, 7, 11, 15);
        quarter_round(&mut state, 0, 5, 10, 15);
        quarter_round(&mut state, 1, 6, 11, 12);
        quarter_round(&mut state, 2, 7, 8, 13);
        quarter_round(&mut state, 3, 4, 9, 14);
    }
    let mut output = [0_u8; 64];
    for index in 0..16 {
        output[index * 4..index * 4 + 4]
            .copy_from_slice(&state[index].wrapping_add(initial[index]).to_le_bytes());
    }
    output
}

fn quarter_round(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    state[a] = state[a].wrapping_add(state[b]);
    state[d] ^= state[a];
    state[d] = state[d].rotate_left(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] ^= state[c];
    state[b] = state[b].rotate_left(12);
    state[a] = state[a].wrapping_add(state[b]);
    state[d] ^= state[a];
    state[d] = state[d].rotate_left(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] ^= state[c];
    state[b] = state[b].rotate_left(7);
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (left, right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

fn base64_encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        encoded.push(char::from(BASE64[usize::from(first >> 2)]));
        encoded.push(char::from(
            BASE64[usize::from(((first & 0x03) << 4) | (second >> 4))],
        ));
        encoded.push(if chunk.len() > 1 {
            char::from(BASE64[usize::from(((second & 0x0f) << 2) | (third >> 6))])
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            char::from(BASE64[usize::from(third & 0x3f)])
        } else {
            '='
        });
    }
    encoded
}

fn base64_decode(value: &str) -> Result<Vec<u8>, String> {
    let bytes = value.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return Err("NIP-44 payload base64 length is invalid".to_owned());
    }
    let mut decoded = Vec::with_capacity(bytes.len() / 4 * 3);
    for (index, chunk) in bytes.chunks_exact(4).enumerate() {
        let last = index + 1 == bytes.len() / 4;
        let a = base64_value(chunk[0])?;
        let b = base64_value(chunk[1])?;
        let c = if chunk[2] == b'=' {
            if !last || chunk[3] != b'=' || b & 0x0f != 0 {
                return Err("NIP-44 payload base64 padding is invalid".to_owned());
            }
            0
        } else {
            base64_value(chunk[2])?
        };
        let d = if chunk[3] == b'=' {
            if !last || c & 0x03 != 0 {
                return Err("NIP-44 payload base64 padding is invalid".to_owned());
            }
            0
        } else {
            if chunk[2] == b'=' {
                return Err("NIP-44 payload base64 padding is invalid".to_owned());
            }
            base64_value(chunk[3])?
        };
        decoded.push((a << 2) | (b >> 4));
        if chunk[2] != b'=' {
            decoded.push((b << 4) | (c >> 2));
        }
        if chunk[3] != b'=' {
            decoded.push((c << 6) | d);
        }
    }
    if base64_encode(&decoded) != value {
        return Err("NIP-44 payload base64 is noncanonical".to_owned());
    }
    Ok(decoded)
}

fn base64_value(byte: u8) -> Result<u8, String> {
    match byte {
        b'A'..=b'Z' => Ok(byte - b'A'),
        b'a'..=b'z' => Ok(byte - b'a' + 26),
        b'0'..=b'9' => Ok(byte - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => Err("NIP-44 payload is not canonical base64".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_nip44_vector_and_tamper_boundaries() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/nip44/conversation.json"
        ))
        .unwrap();
        let secret_one =
            SecretKey::from_byte_array(hex_32(fixture["secret_one"].as_str().unwrap())).unwrap();
        let secret_two =
            SecretKey::from_byte_array(hex_32(fixture["secret_two"].as_str().unwrap())).unwrap();
        let secp = secp256k1::Secp256k1::new();
        let peer_two = secret_two.public_key(&secp).x_only_public_key().0;
        let peer_one = secret_one.public_key(&secp).x_only_public_key().0;
        let key = conversation_key(&secret_one, &peer_two);
        assert_eq!(
            lower_hex(&key),
            fixture["conversation_key"].as_str().unwrap()
        );
        assert_eq!(key, conversation_key(&secret_two, &peer_one));
        let nonce = hex_32(fixture["nonce"].as_str().unwrap());
        let plaintext = fixture["plaintext"].as_str().unwrap();
        let payload = encrypt(plaintext, &key, nonce).unwrap();
        assert_eq!(payload, fixture["payload"].as_str().unwrap());
        assert_eq!(decrypt(&payload, &key).unwrap(), plaintext);
        let mut tampered = payload.into_bytes();
        tampered[80] = if tampered[80] == b'A' { b'B' } else { b'A' };
        assert!(decrypt(std::str::from_utf8(&tampered).unwrap(), &key).is_err());
    }

    fn lower_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn hex_32(value: &str) -> [u8; 32] {
        let mut bytes = [0_u8; 32];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            bytes[index] = (hex_digit(pair[0]) << 4) | hex_digit(pair[1]);
        }
        bytes
    }

    fn hex_digit(byte: u8) -> u8 {
        match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => panic!("fixture contains invalid lowercase hex"),
        }
    }
}
