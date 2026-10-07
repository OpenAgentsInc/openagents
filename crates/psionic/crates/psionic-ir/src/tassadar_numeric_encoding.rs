use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable schema version for the structured numeric encoding lane.
pub const TASSADAR_STRUCTURED_NUMERIC_ENCODING_SCHEMA_VERSION: u16 = 1;

/// Bounded numeric field family supported by the encoding lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarNumericFieldFamily {
    /// Immediate numeric literals in bounded executor traces.
    Immediate,
    /// Byte or slot offsets in bounded executor traces.
    Offset,
    /// Bounded addresses in executor traces.
    Address,
}

impl TassadarNumericFieldFamily {
    /// Returns the stable field-family label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Immediate => "immediate",
            Self::Offset => "offset",
            Self::Address => "address",
        }
    }
}

/// Representation scheme for one bounded numeric field family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarNumericEncodingScheme {
    /// Legacy one-token-per-value representation.
    LegacyToken,
    /// Fixed-width binary bit tokens.
    BinaryBits,
    /// Fixed-width hexadecimal digit tokens.
    MixedRadixHex,
}

impl TassadarNumericEncodingScheme {
    /// Returns the stable scheme label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::LegacyToken => "legacy_token",
            Self::BinaryBits => "binary_bits",
            Self::MixedRadixHex => "mixed_radix_hex",
        }
    }
}

/// Public encoding config for one bounded numeric field family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStructuredNumericEncoding {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable encoding identifier.
    pub encoding_id: String,
    /// Numeric field family carried by the encoding.
    pub field_family: TassadarNumericFieldFamily,
    /// Representation scheme.
    pub scheme: TassadarNumericEncodingScheme,
    /// Fixed bit width supported by the encoding.
    pub bit_width: u8,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable encoding digest.
    pub encoding_digest: String,
}

impl TassadarStructuredNumericEncoding {
    /// Creates one structured numeric encoding.
    #[must_use]
    pub fn new(
        encoding_id: impl Into<String>,
        field_family: TassadarNumericFieldFamily,
        scheme: TassadarNumericEncodingScheme,
        bit_width: u8,
        claim_boundary: impl Into<String>,
    ) -> Self {
        let mut encoding = Self {
            schema_version: TASSADAR_STRUCTURED_NUMERIC_ENCODING_SCHEMA_VERSION,
            encoding_id: encoding_id.into(),
            field_family,
            scheme,
            bit_width: bit_width.max(1),
            claim_boundary: claim_boundary.into(),
            encoding_digest: String::new(),
        };
        encoding.encoding_digest =
            stable_digest(b"psionic_tassadar_structured_numeric_encoding|", &encoding);
        encoding
    }
}

/// Encoding or decoding failure for one bounded numeric value.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarStructuredNumericEncodingError {
    /// Value sat outside the declared bit width.
    #[error("numeric value {value} exceeds the declared bit_width={bit_width}")]
    ValueOutOfRange {
        /// Observed numeric value.
        value: u32,
        /// Declared bit width.
        bit_width: u8,
    },
    /// Token count was incompatible with the encoding scheme.
    #[error("token count {token_count} is invalid for encoding `{encoding_id}`")]
    InvalidTokenCount {
        /// Stable encoding identifier.
        encoding_id: String,
        /// Observed token count.
        token_count: usize,
    },
    /// One token was not compatible with the encoding scheme.
    #[error("token `{token}` is invalid for encoding `{encoding_id}`")]
    InvalidToken {
        /// Stable encoding identifier.
        encoding_id: String,
        /// Invalid token.
        token: String,
    },
}

/// Encodes one bounded numeric value under the selected structured representation.
pub fn encode_tassadar_numeric_value(
    value: u32,
    encoding: &TassadarStructuredNumericEncoding,
) -> Result<Vec<String>, TassadarStructuredNumericEncodingError> {
    if value >= (1_u32 << encoding.bit_width.min(31)) && encoding.bit_width < 32 {
        return Err(TassadarStructuredNumericEncodingError::ValueOutOfRange {
            value,
            bit_width: encoding.bit_width,
        });
    }
    Ok(match encoding.scheme {
        TassadarNumericEncodingScheme::LegacyToken => {
            vec![format!("{}_{}", encoding.field_family.label(), value)]
        }
        TassadarNumericEncodingScheme::BinaryBits => {
            let mut tokens = Vec::with_capacity(encoding.bit_width as usize);
            for shift in (0..encoding.bit_width).rev() {
                let bit = (value >> shift) & 1;
                tokens.push(format!("bit_{bit}"));
            }
            tokens
        }
        TassadarNumericEncodingScheme::MixedRadixHex => {
            let digit_count = ((encoding.bit_width as usize) + 3) / 4;
            let mut tokens = Vec::with_capacity(digit_count);
            for index in (0..digit_count).rev() {
                let nibble = ((value >> (index * 4)) & 0xF) as u8;
                tokens.push(format!("hex_{nibble:x}"));
            }
            tokens
        }
    })
}

/// Decodes one bounded numeric value from the selected structured representation.
pub fn decode_tassadar_numeric_value(
    tokens: &[String],
    encoding: &TassadarStructuredNumericEncoding,
) -> Result<u32, TassadarStructuredNumericEncodingError> {
    match encoding.scheme {
        TassadarNumericEncodingScheme::LegacyToken => {
            if tokens.len() != 1 {
                return Err(TassadarStructuredNumericEncodingError::InvalidTokenCount {
                    encoding_id: encoding.encoding_id.clone(),
                    token_count: tokens.len(),
                });
            }
            let token = &tokens[0];
            let prefix = format!("{}_", encoding.field_family.label());
            let value = token
                .strip_prefix(&prefix)
                .ok_or_else(|| TassadarStructuredNumericEncodingError::InvalidToken {
                    encoding_id: encoding.encoding_id.clone(),
                    token: token.clone(),
                })?
                .parse::<u32>()
                .map_err(|_| TassadarStructuredNumericEncodingError::InvalidToken {
                    encoding_id: encoding.encoding_id.clone(),
                    token: token.clone(),
                })?;
            Ok(value)
        }
        TassadarNumericEncodingScheme::BinaryBits => {
            if tokens.len() != encoding.bit_width as usize {
                return Err(TassadarStructuredNumericEncodingError::InvalidTokenCount {
                    encoding_id: encoding.encoding_id.clone(),
                    token_count: tokens.len(),
                });
            }
            let mut value = 0_u32;
            for token in tokens {
                value <<= 1;
                match token.as_str() {
                    "bit_0" => {}
                    "bit_1" => value |= 1,
                    _ => {
                        return Err(TassadarStructuredNumericEncodingError::InvalidToken {
                            encoding_id: encoding.encoding_id.clone(),
                            token: token.clone(),
                        });
                    }
                }
            }
            Ok(value)
        }
        TassadarNumericEncodingScheme::MixedRadixHex => {
            let digit_count = ((encoding.bit_width as usize) + 3) / 4;
            if tokens.len() != digit_count {
                return Err(TassadarStructuredNumericEncodingError::InvalidTokenCount {
                    encoding_id: encoding.encoding_id.clone(),
                    token_count: tokens.len(),
                });
            }
            let mut value = 0_u32;
            for token in tokens {
                let digit = token
                    .strip_prefix("hex_")
                    .and_then(|digit| u32::from_str_radix(digit, 16).ok())
                    .ok_or_else(|| TassadarStructuredNumericEncodingError::InvalidToken {
                        encoding_id: encoding.encoding_id.clone(),
                        token: token.clone(),
                    })?;
                value = (value << 4) | digit;
            }
            Ok(value)
        }
    }
}

/// Returns the canonical structured numeric encodings for the bounded research lane.
#[must_use]
pub fn tassadar_structured_numeric_encodings() -> Vec<TassadarStructuredNumericEncoding> {
    let claim_boundary = "structured numeric encoding changes only the learned representation of bounded numeric fields; it must decode exactly back to the same runtime semantics and does not imply arbitrary numeric closure or general learned exactness";
    let mut encodings = Vec::new();
    for field_family in [
        TassadarNumericFieldFamily::Immediate,
        TassadarNumericFieldFamily::Offset,
        TassadarNumericFieldFamily::Address,
    ] {
        encodings.push(TassadarStructuredNumericEncoding::new(
            format!("tassadar.numeric.{}.legacy_u8.v1", field_family.label()),
            field_family,
            TassadarNumericEncodingScheme::LegacyToken,
            8,
            claim_boundary,
        ));
        encodings.push(TassadarStructuredNumericEncoding::new(
            format!("tassadar.numeric.{}.binary_u8.v1", field_family.label()),
            field_family,
            TassadarNumericEncodingScheme::BinaryBits,
            8,
            claim_boundary,
        ));
        encodings.push(TassadarStructuredNumericEncoding::new(
            format!("tassadar.numeric.{}.hex_u8.v1", field_family.label()),
            field_family,
            TassadarNumericEncodingScheme::MixedRadixHex,
            8,
            claim_boundary,
        ));
    }
    encodings
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("structured numeric encoding should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        decode_tassadar_numeric_value, encode_tassadar_numeric_value,
        tassadar_structured_numeric_encodings, TassadarNumericEncodingScheme,
    };

    #[test]
    fn structured_numeric_encodings_round_trip_bounded_values() {
        let encodings = tassadar_structured_numeric_encodings();
        for encoding in encodings {
            for value in [0_u32, 1, 15, 42, 255] {
                let tokens = encode_tassadar_numeric_value(value, &encoding)
                    .expect("encoding should succeed");
                let decoded = decode_tassadar_numeric_value(tokens.as_slice(), &encoding)
                    .expect("decoding should succeed");
                assert_eq!(decoded, value);
                match encoding.scheme {
                    TassadarNumericEncodingScheme::LegacyToken => assert_eq!(tokens.len(), 1),
                    TassadarNumericEncodingScheme::BinaryBits => assert_eq!(tokens.len(), 8),
                    TassadarNumericEncodingScheme::MixedRadixHex => assert_eq!(tokens.len(), 2),
                }
            }
        }
    }
}
