//! NIP-19 bech32 encoding of bare keys: `npub` for an x-only public key and
//! `nsec` for a secret key.
//!
//! The bech32 codec is implemented here from BIP-173 with no padding
//! leniency, and a decoded key is checked against the curve before it is
//! returned, so a caller never receives 32 bytes that are not a key.

use std::fmt;

use secp256k1::{SecretKey, XOnlyPublicKey};

/// The human-readable part of an encoded x-only public key.
pub const NPUB: &str = "npub";
/// The human-readable part of an encoded secret key.
pub const NSEC: &str = "nsec";

const CHARSET: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
const GENERATOR: [u32; 5] = [
    0x3b6a_57b2,
    0x2650_8e6d,
    0x1ea1_19fa,
    0x3d42_33dd,
    0x2a14_62b3,
];
const MAX_ENCODED_CHARS: usize = 90;
const CHECKSUM_CHARS: usize = 6;

/// Why a bech32 string did not decode to the requested key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Nip19Error {
    /// The string is longer than the 90 characters bech32 allows, or too
    /// short to carry a separator, a prefix, and a checksum.
    InvalidLength,
    /// Upper and lower case letters are mixed.
    MixedCase,
    /// The string carries no `1` separator, or an empty prefix.
    MissingSeparator,
    /// A data character is outside the bech32 alphabet.
    InvalidCharacter,
    /// The checksum does not verify.
    InvalidChecksum,
    /// The data part does not convert to whole bytes.
    InvalidPadding,
    /// The prefix names another kind of value.
    WrongPrefix {
        expected: &'static str,
        actual: String,
    },
    /// The decoded bytes are not a valid key on the curve.
    InvalidKey,
}

impl fmt::Display for Nip19Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength => f.write_str("bech32 string has an invalid length"),
            Self::MixedCase => f.write_str("bech32 string mixes upper and lower case"),
            Self::MissingSeparator => f.write_str("bech32 string has no prefix separator"),
            Self::InvalidCharacter => {
                f.write_str("bech32 string has a character outside its alphabet")
            }
            Self::InvalidChecksum => f.write_str("bech32 checksum does not verify"),
            Self::InvalidPadding => f.write_str("bech32 data does not convert to whole bytes"),
            Self::WrongPrefix { expected, actual } => {
                write!(f, "expected a {expected} value, received {actual}")
            }
            Self::InvalidKey => f.write_str("decoded bytes are not a valid secp256k1 key"),
        }
    }
}

impl std::error::Error for Nip19Error {}

/// Encode an x-only public key as `npub1...`.
pub fn encode_npub(pubkey: &[u8; 32]) -> String {
    encode(NPUB, pubkey).expect("a 32-byte payload fits the bech32 length limit")
}

/// Decode `npub1...` to the x-only public key it carries, refusing a value
/// with another prefix or one that is not a point on the curve.
pub fn decode_npub(text: &str) -> Result<[u8; 32], Nip19Error> {
    let bytes = decode_key(text, NPUB)?;
    XOnlyPublicKey::from_byte_array(bytes).map_err(|_| Nip19Error::InvalidKey)?;
    Ok(bytes)
}

/// Encode a secret key as `nsec1...`.
pub fn encode_nsec(secret: &[u8; 32]) -> String {
    encode(NSEC, secret).expect("a 32-byte payload fits the bech32 length limit")
}

/// Decode `nsec1...` to the secret key it carries, refusing a value with
/// another prefix or one outside the curve order.
pub fn decode_nsec(text: &str) -> Result<[u8; 32], Nip19Error> {
    let bytes = decode_key(text, NSEC)?;
    SecretKey::from_byte_array(bytes).map_err(|_| Nip19Error::InvalidKey)?;
    Ok(bytes)
}

fn decode_key(text: &str, expected: &'static str) -> Result<[u8; 32], Nip19Error> {
    let (prefix, bytes) = decode(text)?;
    if prefix != expected {
        return Err(Nip19Error::WrongPrefix {
            expected,
            actual: prefix,
        });
    }
    <[u8; 32]>::try_from(bytes).map_err(|_| Nip19Error::InvalidLength)
}

/// Encode bytes as a lowercase bech32 string under `prefix`.
pub fn encode(prefix: &str, data: &[u8]) -> Result<String, Nip19Error> {
    if prefix.is_empty() || !prefix.bytes().all(|byte| (33..=126).contains(&byte)) {
        return Err(Nip19Error::MissingSeparator);
    }
    let prefix = prefix.to_ascii_lowercase();
    let mut words = to_words(data);
    let checksum = checksum(&prefix, &words);
    words.extend_from_slice(&checksum);
    let mut encoded = String::with_capacity(prefix.len() + 1 + words.len());
    encoded.push_str(&prefix);
    encoded.push('1');
    for word in words {
        encoded.push(char::from(CHARSET[usize::from(word)]));
    }
    if encoded.len() > MAX_ENCODED_CHARS {
        return Err(Nip19Error::InvalidLength);
    }
    Ok(encoded)
}

/// Decode a bech32 string to its lowercase prefix and its bytes.
pub fn decode(text: &str) -> Result<(String, Vec<u8>), Nip19Error> {
    if text.len() > MAX_ENCODED_CHARS || !text.is_ascii() {
        return Err(Nip19Error::InvalidLength);
    }
    let has_lower = text.bytes().any(|byte| byte.is_ascii_lowercase());
    let has_upper = text.bytes().any(|byte| byte.is_ascii_uppercase());
    if has_lower && has_upper {
        return Err(Nip19Error::MixedCase);
    }
    let lowered = text.to_ascii_lowercase();
    let separator = lowered.rfind('1').ok_or(Nip19Error::MissingSeparator)?;
    if separator == 0 {
        return Err(Nip19Error::MissingSeparator);
    }
    let (prefix, data) = lowered.split_at(separator);
    let data = &data[1..];
    if data.len() < CHECKSUM_CHARS || !prefix.bytes().all(|byte| (33..=126).contains(&byte)) {
        return Err(Nip19Error::InvalidLength);
    }
    let mut words = Vec::with_capacity(data.len());
    for byte in data.bytes() {
        let word = CHARSET
            .iter()
            .position(|candidate| *candidate == byte)
            .ok_or(Nip19Error::InvalidCharacter)?;
        words.push(word as u8);
    }
    if polymod(&expand_prefix(prefix), &words) != 1 {
        return Err(Nip19Error::InvalidChecksum);
    }
    let payload = &words[..words.len() - CHECKSUM_CHARS];
    Ok((prefix.to_owned(), from_words(payload)?))
}

fn checksum(prefix: &str, words: &[u8]) -> [u8; CHECKSUM_CHARS] {
    let mut padded = words.to_vec();
    padded.extend_from_slice(&[0; CHECKSUM_CHARS]);
    let polymod = polymod(&expand_prefix(prefix), &padded) ^ 1;
    let mut checksum = [0; CHECKSUM_CHARS];
    for (index, word) in checksum.iter_mut().enumerate() {
        *word = ((polymod >> (5 * (5 - index))) & 31) as u8;
    }
    checksum
}

fn expand_prefix(prefix: &str) -> Vec<u8> {
    let mut expanded = Vec::with_capacity(prefix.len() * 2 + 1);
    expanded.extend(prefix.bytes().map(|byte| byte >> 5));
    expanded.push(0);
    expanded.extend(prefix.bytes().map(|byte| byte & 31));
    expanded
}

fn polymod(expanded_prefix: &[u8], words: &[u8]) -> u32 {
    let mut checksum: u32 = 1;
    for value in expanded_prefix.iter().chain(words) {
        let top = checksum >> 25;
        checksum = ((checksum & 0x01ff_ffff) << 5) ^ u32::from(*value);
        for (bit, generator) in GENERATOR.iter().enumerate() {
            if (top >> bit) & 1 == 1 {
                checksum ^= generator;
            }
        }
    }
    checksum
}

/// Regroup 8-bit bytes into 5-bit words, padding the final word with zeros.
fn to_words(data: &[u8]) -> Vec<u8> {
    let mut words = Vec::with_capacity(data.len().div_ceil(5) * 8);
    let mut accumulator: u32 = 0;
    let mut bits: u8 = 0;
    for byte in data {
        accumulator = (accumulator << 8) | u32::from(*byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            words.push(((accumulator >> bits) & 31) as u8);
        }
    }
    if bits > 0 {
        words.push(((accumulator << (5 - bits)) & 31) as u8);
    }
    words
}

/// Regroup 5-bit words into bytes, refusing leftover bits that are either
/// a whole word or nonzero, as BIP-173 requires.
fn from_words(words: &[u8]) -> Result<Vec<u8>, Nip19Error> {
    let mut bytes = Vec::with_capacity(words.len() * 5 / 8);
    let mut accumulator: u32 = 0;
    let mut bits: u8 = 0;
    for word in words {
        accumulator = (accumulator << 5) | u32::from(*word);
        bits += 5;
        while bits >= 8 {
            bits -= 8;
            bytes.push(((accumulator >> bits) & 0xff) as u8);
        }
    }
    if bits >= 5 || (accumulator & ((1 << bits) - 1)) != 0 {
        return Err(Nip19Error::InvalidPadding);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bip173_valid_strings_decode_and_round_trip() {
        for text in [
            "A12UEL5L",
            "a12uel5l",
            "an83characterlonghumanreadablepartthatcontainsthenumber1andtheexcludedcharactersbio1tt5tgs",
            "abcdef1qpzry9x8gf2tvdw0s3jn54khce6mua7lmqqqxw",
            "11qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqc8247j",
            "split1checkupstagehandshakeupstreamerranterredcaperred2y9e3w",
        ] {
            let (prefix, bytes) = decode(text).unwrap_or_else(|error| panic!("{text}: {error}"));
            assert_eq!(encode(&prefix, &bytes).unwrap(), text.to_ascii_lowercase());
        }
    }

    #[test]
    fn bip173_invalid_strings_are_refused() {
        for (text, expected) in [
            ("\u{20}1nwldj5", Nip19Error::InvalidLength),
            ("\u{7f}1axkwrx", Nip19Error::InvalidLength),
            (
                "an84characterslonghumanreadablepartthatcontainsthenumber1andtheexcludedcharactersbio1569pvx",
                Nip19Error::InvalidLength,
            ),
            ("pzry9x0s0muk", Nip19Error::MissingSeparator),
            ("1pzry9x0s0muk", Nip19Error::MissingSeparator),
            ("x1b4n0q5v", Nip19Error::InvalidCharacter),
            ("li1dgmt3", Nip19Error::InvalidLength),
            ("de1lg7wt\u{ff}", Nip19Error::InvalidLength),
            ("A1G7SGD8", Nip19Error::InvalidChecksum),
            ("10a06t8", Nip19Error::MissingSeparator),
            ("1qzzfhee", Nip19Error::MissingSeparator),
        ] {
            assert_eq!(decode(text), Err(expected), "{text:?}");
        }
    }

    #[test]
    fn mixed_case_is_refused() {
        assert_eq!(decode("A12ueL5L"), Err(Nip19Error::MixedCase));
    }

    #[test]
    fn a_zero_secret_is_not_a_key() {
        let encoded = encode_nsec(&[0; 32]);
        assert_eq!(decode_nsec(&encoded), Err(Nip19Error::InvalidKey));
    }
}
