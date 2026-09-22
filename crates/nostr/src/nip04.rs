//! NIP-04 encrypted direct messages.
//!
//! The shared secret is the X coordinate of the ECDH point. It is not hashed.
//! The body is AES-256-CBC with PKCS#7 padding, written as
//! `base64(ciphertext)?iv=base64(iv)`. The pinned text marks this construction
//! unrecommended. It leaks the recipient in a `p` tag and has no MAC.

use secp256k1::{Parity, PublicKey, SecretKey, XOnlyPublicKey, ecdh};

use crate::domain::{DomainError, Event};

const SBOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

const INV_SBOX: [u8; 256] = [
    0x52, 0x09, 0x6a, 0xd5, 0x30, 0x36, 0xa5, 0x38, 0xbf, 0x40, 0xa3, 0x9e, 0x81, 0xf3, 0xd7, 0xfb,
    0x7c, 0xe3, 0x39, 0x82, 0x9b, 0x2f, 0xff, 0x87, 0x34, 0x8e, 0x43, 0x44, 0xc4, 0xde, 0xe9, 0xcb,
    0x54, 0x7b, 0x94, 0x32, 0xa6, 0xc2, 0x23, 0x3d, 0xee, 0x4c, 0x95, 0x0b, 0x42, 0xfa, 0xc3, 0x4e,
    0x08, 0x2e, 0xa1, 0x66, 0x28, 0xd9, 0x24, 0xb2, 0x76, 0x5b, 0xa2, 0x49, 0x6d, 0x8b, 0xd1, 0x25,
    0x72, 0xf8, 0xf6, 0x64, 0x86, 0x68, 0x98, 0x16, 0xd4, 0xa4, 0x5c, 0xcc, 0x5d, 0x65, 0xb6, 0x92,
    0x6c, 0x70, 0x48, 0x50, 0xfd, 0xed, 0xb9, 0xda, 0x5e, 0x15, 0x46, 0x57, 0xa7, 0x8d, 0x9d, 0x84,
    0x90, 0xd8, 0xab, 0x00, 0x8c, 0xbc, 0xd3, 0x0a, 0xf7, 0xe4, 0x58, 0x05, 0xb8, 0xb3, 0x45, 0x06,
    0xd0, 0x2c, 0x1e, 0x8f, 0xca, 0x3f, 0x0f, 0x02, 0xc1, 0xaf, 0xbd, 0x03, 0x01, 0x13, 0x8a, 0x6b,
    0x3a, 0x91, 0x11, 0x41, 0x4f, 0x67, 0xdc, 0xea, 0x97, 0xf2, 0xcf, 0xce, 0xf0, 0xb4, 0xe6, 0x73,
    0x96, 0xac, 0x74, 0x22, 0xe7, 0xad, 0x35, 0x85, 0xe2, 0xf9, 0x37, 0xe8, 0x1c, 0x75, 0xdf, 0x6e,
    0x47, 0xf1, 0x1a, 0x71, 0x1d, 0x29, 0xc5, 0x89, 0x6f, 0xb7, 0x62, 0x0e, 0xaa, 0x18, 0xbe, 0x1b,
    0xfc, 0x56, 0x3e, 0x4b, 0xc6, 0xd2, 0x79, 0x20, 0x9a, 0xdb, 0xc0, 0xfe, 0x78, 0xcd, 0x5a, 0xf4,
    0x1f, 0xdd, 0xa8, 0x33, 0x88, 0x07, 0xc7, 0x31, 0xb1, 0x12, 0x10, 0x59, 0x27, 0x80, 0xec, 0x5f,
    0x60, 0x51, 0x7f, 0xa9, 0x19, 0xb5, 0x4a, 0x0d, 0x2d, 0xe5, 0x7a, 0x9f, 0x93, 0xc9, 0x9c, 0xef,
    0xa0, 0xe0, 0x3b, 0x4d, 0xae, 0x2a, 0xf5, 0xb0, 0xc8, 0xeb, 0xbb, 0x3c, 0x83, 0x53, 0x99, 0x61,
    0x17, 0x2b, 0x04, 0x7e, 0xba, 0x77, 0xd6, 0x26, 0xe1, 0x69, 0x14, 0x63, 0x55, 0x21, 0x0c, 0x7d,
];

const RCON: [u8; 11] = [
    0x00, 0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1b, 0x36,
];

const MAX_PLAINTEXT: usize = 65_536;

/// The NIP-04 shared secret: the X coordinate only, not a hash of the point.
#[must_use]
pub fn shared_key(secret: &SecretKey, peer: &XOnlyPublicKey) -> [u8; 32] {
    let point = PublicKey::from_x_only_public_key(*peer, Parity::Even);
    let shared = ecdh::shared_secret_point(&point, secret);
    let mut key = [0_u8; 32];
    key.copy_from_slice(&shared[..32]);
    key
}

/// Encrypt `plaintext` for `peer`. The caller supplies the 16-byte IV.
///
/// # Errors
///
/// Returns a sentence when the plaintext is longer than 65,536 bytes.
pub fn encrypt(
    plaintext: &str,
    secret: &SecretKey,
    peer: &XOnlyPublicKey,
    iv: [u8; 16],
) -> Result<String, String> {
    if plaintext.len() > MAX_PLAINTEXT {
        return Err(format!(
            "NIP-04 plaintext must contain at most {MAX_PLAINTEXT} bytes"
        ));
    }
    let key = shared_key(secret, peer);
    let padded = pkcs7(plaintext.as_bytes());
    let ciphertext = cbc_encrypt(&key, &iv, &padded);
    Ok(format!(
        "{}?iv={}",
        base64_encode(&ciphertext),
        base64_encode(&iv)
    ))
}

/// Decrypt a NIP-04 `content` value from `peer`.
///
/// # Errors
///
/// Returns a sentence when the content, the padding, or the plaintext is not
/// the form this host accepts.
pub fn decrypt(content: &str, secret: &SecretKey, peer: &XOnlyPublicKey) -> Result<String, String> {
    let (ciphertext, iv) = split_content(content).map_err(|error| error.to_string())?;
    let key = shared_key(secret, peer);
    let plain = cbc_decrypt(&key, &iv, &ciphertext).map_err(|error| error.to_string())?;
    String::from_utf8(plain).map_err(|_| "NIP-04 plaintext is not valid UTF-8".to_owned())
}

/// Whether a kind `4` event has the pinned envelope.
///
/// The relay can check this without the private key. It does not decrypt.
///
/// # Errors
///
/// Returns an invalid-event error when the `p` tag or the `content` form is wrong.
pub fn direct_message(event: &Event) -> Result<(), DomainError> {
    if event.kind != 4 {
        return Err(invalid("NIP-04 direct message must have kind 4"));
    }
    let recipients = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("p"))
        .collect::<Vec<_>>();
    if recipients.len() != 1 {
        return Err(invalid("NIP-04 direct message requires exactly one p tag"));
    }
    let recipient = recipients[0].as_slice();
    if recipient.len() < 2 || !is_hex32(&recipient[1]) {
        return Err(invalid("NIP-04 recipient must be a 32-byte hex public key"));
    }
    let replies = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("e"))
        .collect::<Vec<_>>();
    if replies.len() > 1 {
        return Err(invalid("NIP-04 direct message has at most one e tag"));
    }
    if let Some(reply) = replies.first() {
        let values = reply.as_slice();
        if values.len() < 2 || !is_hex32(&values[1]) {
            return Err(invalid("NIP-04 reply must be a 32-byte hex event id"));
        }
    }
    split_content(&event.content)?;
    Ok(())
}

fn split_content(content: &str) -> Result<(Vec<u8>, [u8; 16]), DomainError> {
    let Some((body, iv_text)) = content.split_once("?iv=") else {
        return Err(invalid("NIP-04 content must carry ?iv="));
    };
    if body.contains("?iv=") || iv_text.contains("?iv=") {
        return Err(invalid("NIP-04 content must carry one ?iv="));
    }
    let ciphertext = decode_base64(body)?;
    let iv = decode_base64(iv_text)?;
    if ciphertext.is_empty() || !ciphertext.len().is_multiple_of(16) || iv.len() != 16 {
        return Err(invalid(
            "NIP-04 ciphertext must be whole blocks and the IV must be 16 bytes",
        ));
    }
    let mut iv_bytes = [0_u8; 16];
    iv_bytes.copy_from_slice(&iv);
    Ok((ciphertext, iv_bytes))
}

fn pkcs7(plaintext: &[u8]) -> Vec<u8> {
    let pad = 16 - (plaintext.len() % 16);
    let mut out = Vec::with_capacity(plaintext.len() + pad);
    out.extend_from_slice(plaintext);
    out.extend(std::iter::repeat_n(pad as u8, pad));
    out
}

fn unpad(padded: &[u8]) -> Result<Vec<u8>, DomainError> {
    let Some(pad) = padded.last().copied() else {
        return Err(invalid("NIP-04 padding is missing"));
    };
    let pad = usize::from(pad);
    if pad == 0 || pad > 16 || padded.len() < pad {
        return Err(invalid("NIP-04 padding is invalid"));
    }
    if padded[padded.len() - pad..]
        .iter()
        .any(|byte| *byte != pad as u8)
    {
        return Err(invalid("NIP-04 padding is invalid"));
    }
    Ok(padded[..padded.len() - pad].to_vec())
}

fn cbc_encrypt(key: &[u8; 32], iv: &[u8; 16], padded: &[u8]) -> Vec<u8> {
    let round_keys = expand_key(key);
    let mut previous = *iv;
    let mut out = Vec::with_capacity(padded.len());
    for chunk in padded.chunks_exact(16) {
        let mut block = [0_u8; 16];
        for index in 0..16 {
            block[index] = chunk[index] ^ previous[index];
        }
        encrypt_block(&mut block, &round_keys);
        out.extend_from_slice(&block);
        previous = block;
    }
    out
}

fn cbc_decrypt(key: &[u8; 32], iv: &[u8; 16], ciphertext: &[u8]) -> Result<Vec<u8>, DomainError> {
    let round_keys = expand_key(key);
    let mut previous = *iv;
    let mut padded = Vec::with_capacity(ciphertext.len());
    for chunk in ciphertext.chunks_exact(16) {
        let mut block = [0_u8; 16];
        block.copy_from_slice(chunk);
        let cipher = block;
        decrypt_block(&mut block, &round_keys);
        for index in 0..16 {
            block[index] ^= previous[index];
        }
        padded.extend_from_slice(&block);
        previous = cipher;
    }
    unpad(&padded)
}

fn expand_key(key: &[u8; 32]) -> [u8; 240] {
    let mut words = [0_u8; 240];
    words[..32].copy_from_slice(key);
    let mut offset = 32;
    let mut rcon_index = 1;
    while offset < 240 {
        let mut temp = [
            words[offset - 4],
            words[offset - 3],
            words[offset - 2],
            words[offset - 1],
        ];
        if offset.is_multiple_of(32) {
            temp = sub_word(rot_word(temp));
            temp[0] ^= RCON[rcon_index];
            rcon_index += 1;
        } else if offset % 32 == 16 {
            temp = sub_word(temp);
        }
        for byte in temp {
            words[offset] = words[offset - 32] ^ byte;
            offset += 1;
        }
    }
    words
}

fn rot_word(word: [u8; 4]) -> [u8; 4] {
    [word[1], word[2], word[3], word[0]]
}

fn sub_word(word: [u8; 4]) -> [u8; 4] {
    [
        SBOX[word[0] as usize],
        SBOX[word[1] as usize],
        SBOX[word[2] as usize],
        SBOX[word[3] as usize],
    ]
}

fn encrypt_block(block: &mut [u8; 16], round_keys: &[u8; 240]) {
    add_round_key(block, &round_keys[..16]);
    for round in 1..14 {
        sub_bytes(block);
        shift_rows(block);
        mix_columns(block);
        let start = round * 16;
        add_round_key(block, &round_keys[start..start + 16]);
    }
    sub_bytes(block);
    shift_rows(block);
    add_round_key(block, &round_keys[224..]);
}

fn decrypt_block(block: &mut [u8; 16], round_keys: &[u8; 240]) {
    add_round_key(block, &round_keys[224..]);
    for round in (1..14).rev() {
        inv_shift_rows(block);
        inv_sub_bytes(block);
        let start = round * 16;
        add_round_key(block, &round_keys[start..start + 16]);
        inv_mix_columns(block);
    }
    inv_shift_rows(block);
    inv_sub_bytes(block);
    add_round_key(block, &round_keys[..16]);
}

fn add_round_key(block: &mut [u8; 16], key: &[u8]) {
    for (byte, key_byte) in block.iter_mut().zip(key) {
        *byte ^= key_byte;
    }
}

fn sub_bytes(block: &mut [u8; 16]) {
    for byte in block {
        *byte = SBOX[*byte as usize];
    }
}

fn inv_sub_bytes(block: &mut [u8; 16]) {
    for byte in block {
        *byte = INV_SBOX[*byte as usize];
    }
}

fn shift_rows(block: &mut [u8; 16]) {
    let first = block[1];
    block[1] = block[5];
    block[5] = block[9];
    block[9] = block[13];
    block[13] = first;
    let (second, sixth) = (block[2], block[6]);
    block[2] = block[10];
    block[6] = block[14];
    block[10] = second;
    block[14] = sixth;
    let third = block[15];
    block[15] = block[11];
    block[11] = block[7];
    block[7] = block[3];
    block[3] = third;
}

fn inv_shift_rows(block: &mut [u8; 16]) {
    let first = block[13];
    block[13] = block[9];
    block[9] = block[5];
    block[5] = block[1];
    block[1] = first;
    let (second, sixth) = (block[2], block[6]);
    block[2] = block[10];
    block[6] = block[14];
    block[10] = second;
    block[14] = sixth;
    let third = block[3];
    block[3] = block[7];
    block[7] = block[11];
    block[11] = block[15];
    block[15] = third;
}

fn mix_columns(block: &mut [u8; 16]) {
    for column in 0..4 {
        let index = column * 4;
        let a = [
            block[index],
            block[index + 1],
            block[index + 2],
            block[index + 3],
        ];
        block[index] = xtime(a[0]) ^ xtime(a[1]) ^ a[1] ^ a[2] ^ a[3];
        block[index + 1] = a[0] ^ xtime(a[1]) ^ xtime(a[2]) ^ a[2] ^ a[3];
        block[index + 2] = a[0] ^ a[1] ^ xtime(a[2]) ^ xtime(a[3]) ^ a[3];
        block[index + 3] = xtime(a[0]) ^ a[0] ^ a[1] ^ a[2] ^ xtime(a[3]);
    }
}

fn inv_mix_columns(block: &mut [u8; 16]) {
    for column in 0..4 {
        let index = column * 4;
        let a = [
            block[index],
            block[index + 1],
            block[index + 2],
            block[index + 3],
        ];
        block[index] = mul(a[0], 0x0e) ^ mul(a[1], 0x0b) ^ mul(a[2], 0x0d) ^ mul(a[3], 0x09);
        block[index + 1] = mul(a[0], 0x09) ^ mul(a[1], 0x0e) ^ mul(a[2], 0x0b) ^ mul(a[3], 0x0d);
        block[index + 2] = mul(a[0], 0x0d) ^ mul(a[1], 0x09) ^ mul(a[2], 0x0e) ^ mul(a[3], 0x0b);
        block[index + 3] = mul(a[0], 0x0b) ^ mul(a[1], 0x0d) ^ mul(a[2], 0x09) ^ mul(a[3], 0x0e);
    }
}

fn xtime(value: u8) -> u8 {
    let shifted = value << 1;
    if value & 0x80 == 0 {
        shifted
    } else {
        shifted ^ 0x1b
    }
}

fn mul(mut left: u8, mut right: u8) -> u8 {
    let mut product = 0;
    for _ in 0..8 {
        if right & 1 != 0 {
            product ^= left;
        }
        left = xtime(left);
        right >>= 1;
    }
    product
}

fn is_hex32(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn invalid(reason: &'static str) -> DomainError {
    DomainError::InvalidEvent(reason.into())
}

fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        let packed = (u32::from(first) << 16) | (u32::from(second) << 8) | u32::from(third);
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

fn decode_base64(value: &str) -> Result<Vec<u8>, DomainError> {
    if value.is_empty() || !value.is_ascii() || !value.len().is_multiple_of(4) {
        return Err(invalid("NIP-04 content must be base64"));
    }
    let mut output = Vec::with_capacity(value.len() / 4 * 3);
    for chunk in value.as_bytes().chunks_exact(4) {
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
                _ => return Err(invalid("NIP-04 content must be base64")),
            };
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

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(bytes: impl AsRef<[u8]>) -> String {
        const DIGITS: &[u8] = b"0123456789abcdef";
        let bytes = bytes.as_ref();
        let mut out = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            out.push(DIGITS[(byte >> 4) as usize] as char);
            out.push(DIGITS[(byte & 0x0f) as usize] as char);
        }
        out
    }

    fn secret(byte: u8) -> SecretKey {
        SecretKey::from_byte_array([byte; 32]).unwrap()
    }

    fn peer(secret: &SecretKey) -> XOnlyPublicKey {
        secret.x_only_public_key(&secp256k1::Secp256k1::new()).0
    }

    #[test]
    fn aes_256_matches_the_fips_197_block_vector() {
        let key = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b,
            0x1c, 0x1d, 0x1e, 0x1f,
        ];
        let mut block = [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ];
        let rounds = expand_key(&key);
        encrypt_block(&mut block, &rounds);
        assert_eq!(
            block,
            [
                0x8e, 0xa2, 0xb7, 0xca, 0x51, 0x67, 0x45, 0xbf, 0xea, 0xfc, 0x49, 0x90, 0x4b, 0x49,
                0x60, 0x89
            ]
        );
        decrypt_block(&mut block, &rounds);
        assert_eq!(
            block,
            [
                0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
                0xee, 0xff
            ]
        );
    }

    #[test]
    fn a_direct_message_round_trips_and_keeps_a_mention_as_text() {
        let alice = secret(1);
        let bob = secret(2);
        assert_eq!(
            shared_key(&alice, &peer(&bob)),
            shared_key(&bob, &peer(&alice))
        );
        let plaintext = "hello npub1alice, this stays text";
        let content = encrypt(plaintext, &alice, &peer(&bob), [7; 16]).unwrap();
        assert!(content.contains("?iv="));
        assert_eq!(decrypt(&content, &bob, &peer(&alice)).unwrap(), plaintext);
        assert!(decrypt(&content, &secret(3), &peer(&alice)).is_err());

        let mut event = Event {
            id: "ab".repeat(32),
            pubkey: encode(peer(&alice).serialize()),
            created_at: 1,
            kind: 4,
            tags: vec![crate::domain::Tag::new(vec![
                "p".into(),
                encode(peer(&bob).serialize()),
            ])],
            content,
            sig: "cd".repeat(64),
        };
        direct_message(&event).unwrap();
        event.content = "not-a-message".into();
        assert!(direct_message(&event).is_err());
    }
}
