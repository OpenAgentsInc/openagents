//! Log probabilities of output tokens.

use std::collections::HashMap;
use std::sync::LazyLock;

use super::decoder::StreamTokenizer;

/// Backend log probability data for one generated token.
#[derive(Debug, Clone, PartialEq)]
pub struct InferenceTokenLogprob {
    /// Log probability of the generated token.
    pub logprob: f32,
    /// The most likely alternatives as token IDs and log probabilities.
    pub top_logprobs: Vec<(u32, f32)>,
}

/// The log probability of one output token, as protocols report it.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenLogprob {
    /// The token text. A token whose bytes are not valid UTF-8 is written as
    /// `\xNN` escapes of its bytes.
    pub token: String,
    pub logprob: f32,
    /// The bytes the token stands for. They might not be valid UTF-8; see
    /// [`token`](Self::token).
    pub bytes: Vec<u8>,
    pub top_logprobs: Vec<TopLogprob>,
}

/// One of the most likely alternatives at an output token position.
#[derive(Debug, Clone, PartialEq)]
pub struct TopLogprob {
    pub token: String,
    pub logprob: f32,
    pub bytes: Vec<u8>,
}

/// The byte-level vocabulary characters mapped to the bytes they stand for.
static BYTES_BY_BYTE_LEVEL_CHAR: LazyLock<HashMap<char, u8>> = LazyLock::new(|| {
    let mut bytes: Vec<u8> = (b'!'..=b'~')
        .chain(0xA1..=0xAC)
        .chain(0xAE..=0xFF)
        .collect();
    let mut chars: Vec<u32> = bytes.iter().map(|&byte| u32::from(byte)).collect();
    let mut next = 0;
    for byte in 0..=255u8 {
        if !bytes.contains(&byte) {
            bytes.push(byte);
            chars.push(256 + next);
            next += 1;
        }
    }
    chars
        .into_iter()
        .zip(bytes)
        .map(|(ch, byte)| {
            (
                char::from_u32(ch).expect("byte-level characters are valid"),
                byte,
            )
        })
        .collect()
});

/// Return the bytes and text of a byte-level vocabulary token.
///
/// A token with a character outside the byte-level alphabet, such as a special
/// token, stands for its own UTF-8 bytes.
fn byte_level_decode(vocab_token: &str) -> (Vec<u8>, String) {
    let bytes = vocab_token
        .chars()
        .map(|ch| BYTES_BY_BYTE_LEVEL_CHAR.get(&ch).copied())
        .collect::<Option<Vec<u8>>>()
        .unwrap_or_else(|| vocab_token.as_bytes().to_vec());
    let token = match String::from_utf8(bytes.clone()) {
        Ok(token) => token,
        Err(_) => bytes.iter().map(|byte| format!("\\x{byte:02x}")).collect(),
    };
    (bytes, token)
}

/// Return the bytes and text of a token ID. An unknown ID is an empty token.
fn decode_token(tokenizer: &dyn StreamTokenizer, token_id: u32) -> (Vec<u8>, String) {
    byte_level_decode(&tokenizer.id_to_token(token_id).unwrap_or_default())
}

/// Build the log probabilities of decoded tokens.
pub(super) fn create_logprobs(
    tokenizer: &dyn StreamTokenizer,
    token_ids: &[u32],
    logprobs: &[InferenceTokenLogprob],
) -> Vec<TokenLogprob> {
    token_ids
        .iter()
        .zip(logprobs)
        .map(|(&token_id, logprob)| {
            let (bytes, token) = decode_token(tokenizer, token_id);
            TokenLogprob {
                token,
                logprob: logprob.logprob,
                bytes,
                top_logprobs: logprob
                    .top_logprobs
                    .iter()
                    .map(|&(token_id, logprob)| {
                        let (bytes, token) = decode_token(tokenizer, token_id);
                        TopLogprob {
                            token,
                            logprob,
                            bytes,
                        }
                    })
                    .collect(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_level_tokens_decode_to_their_bytes() {
        // `Ġ` stands for a space; `ä½` stands for the first two bytes of `你`.
        assert_eq!(
            byte_level_decode("Ġhi"),
            (b" hi".to_vec(), " hi".to_string())
        );
        assert_eq!(
            byte_level_decode("ä½"),
            (vec![0xe4, 0xbd], "\\xe4\\xbd".to_string())
        );
        // Characters outside the byte-level alphabet stand for themselves.
        assert_eq!(
            byte_level_decode("</think>"),
            (b"</think>".to_vec(), "</think>".to_string())
        );
        assert_eq!(
            byte_level_decode("<｜User｜>"),
            ("<｜User｜>".as_bytes().to_vec(), "<｜User｜>".to_string())
        );
    }
}
