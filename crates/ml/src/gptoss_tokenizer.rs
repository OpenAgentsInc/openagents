use std::borrow::Borrow;
use std::collections::HashSet;
use std::sync::OnceLock;

use fancy_regex::Regex;
use rustc_hash::FxHashMap as HashMap;

use crate::GgufTokenizer;

pub type Rank = u32;

// GPT-2 byte encoder/decoder mapping used by the GGUF vocab.
fn byte_encoder() -> &'static [char; 256] {
    static TABLE: OnceLock<[char; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = ['\0'; 256];
        let mut bytes = Vec::with_capacity(256);
        bytes.extend(33u8..=126u8);
        bytes.extend(161u8..=172u8);
        bytes.extend(174u8..=255u8);

        let mut seen = [false; 256];
        for &b in &bytes {
            seen[b as usize] = true;
        }

        let mut codepoints = Vec::with_capacity(256);
        for &b in &bytes {
            codepoints.push(b as u32);
        }
        let mut next = 0u32;
        for b in 0u8..=255u8 {
            if !seen[b as usize] {
                bytes.push(b);
                codepoints.push(256 + next);
                next += 1;
            }
        }

        for (b, cp) in bytes.into_iter().zip(codepoints.into_iter()) {
            table[b as usize] = char::from_u32(cp).unwrap_or('\u{FFFD}');
        }
        table
    })
}

fn byte_decoder() -> &'static HashMap<char, u8> {
    static TABLE: OnceLock<HashMap<char, u8>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut map = HashMap::default();
        for (idx, ch) in byte_encoder().iter().enumerate() {
            map.insert(*ch, idx as u8);
        }
        map
    })
}

fn byte_encode(piece: &[u8]) -> Vec<u8> {
    let table = byte_encoder();
    let mut out = Vec::with_capacity(piece.len());
    for &b in piece {
        let ch = table[b as usize];
        let mut buf = [0u8; 4];
        let encoded = ch.encode_utf8(&mut buf);
        out.extend_from_slice(encoded.as_bytes());
    }
    out
}

fn byte_decode(encoded: &[u8]) -> Result<Vec<u8>, String> {
    let text = std::str::from_utf8(encoded)
        .map_err(|err| format!("decode utf8 error: {err}"))?;
    let decoder = byte_decoder();
    let mut out = Vec::with_capacity(text.len());
    for ch in text.chars() {
        let byte = decoder
            .get(&ch)
            .copied()
            .ok_or_else(|| format!("missing byte decoder for {ch}"))?;
        out.push(byte);
    }
    Ok(out)
}

fn byte_pair_merge(ranks: &HashMap<Vec<u8>, Rank>, piece: &[u8]) -> Vec<(usize, Rank)> {
    let mut parts = Vec::with_capacity(piece.len() + 1);

    let mut min_rank: (Rank, usize) = (Rank::MAX, usize::MAX);
    for i in 0..piece.len().saturating_sub(1) {
        let rank = *ranks.get(&piece[i..i + 2]).unwrap_or(&Rank::MAX);
        if rank < min_rank.0 {
            min_rank = (rank, i);
        }
        parts.push((i, rank));
    }
    parts.push((piece.len().saturating_sub(1), Rank::MAX));
    parts.push((piece.len(), Rank::MAX));

    let get_rank = |parts: &Vec<(usize, Rank)>, i: usize| {
        if (i + 3) < parts.len() {
            *ranks
                .get(&piece[parts[i].0..parts[i + 3].0])
                .unwrap_or(&Rank::MAX)
        } else {
            Rank::MAX
        }
    };

    while min_rank.0 != Rank::MAX {
        let i = min_rank.1;
        if i > 0 {
            parts[i - 1].1 = get_rank(&parts, i - 1);
        }
        parts[i].1 = get_rank(&parts, i);
        parts.remove(i + 1);

        min_rank = (Rank::MAX, usize::MAX);
        for (idx, &(_, rank)) in parts[..parts.len().saturating_sub(1)]
            .iter()
            .enumerate()
        {
            if rank < min_rank.0 {
                min_rank = (rank, idx);
            }
        }
    }
    parts
}

fn byte_pair_encode(piece: &[u8], ranks: &HashMap<Vec<u8>, Rank>) -> Result<Vec<Rank>, String> {
    if piece.len() == 1 {
        let rank = ranks
            .get(piece)
            .copied()
            .ok_or_else(|| "missing single-byte token".to_string())?;
        return Ok(vec![rank]);
    }
    let merged = byte_pair_merge(ranks, piece);
    let mut out = Vec::with_capacity(merged.len().saturating_sub(1));
    for part in merged.windows(2) {
        let bytes = &piece[part[0].0..part[1].0];
        let rank = ranks
            .get(bytes)
            .copied()
            .ok_or_else(|| "missing bpe token".to_string())?;
        out.push(rank);
    }
    Ok(out)
}

pub struct CoreBpe {
    encoder: HashMap<Vec<u8>, Rank>,
    special_tokens_encoder: HashMap<String, Rank>,
    decoder: HashMap<Rank, Vec<u8>>,
    special_tokens_decoder: HashMap<Rank, Vec<u8>>,
    regex: Regex,
    special_regex: Regex,
}

impl CoreBpe {
    pub fn new<E, SE>(encoder: E, special_tokens_encoder: SE, pattern: &str) -> Result<Self, String>
    where
        E: IntoIterator<Item = (Vec<u8>, Rank)>,
        SE: IntoIterator<Item = (String, Rank)>,
    {
        let encoder = HashMap::from_iter(encoder);
        let special_tokens_encoder = HashMap::from_iter(special_tokens_encoder);

        let regex = Regex::new(pattern).map_err(|err| format!("regex error: {err}"))?;

        let special_regex = if special_tokens_encoder.is_empty() {
            Regex::new("$^").map_err(|err| format!("regex error: {err}"))?
        } else {
            let parts = special_tokens_encoder
                .keys()
                .map(|s| fancy_regex::escape(s))
                .collect::<Vec<_>>();
            Regex::new(&parts.join("|")).map_err(|err| format!("regex error: {err}"))?
        };

        let decoder: HashMap<Rank, Vec<u8>> =
            encoder.iter().map(|(k, v)| (*v, k.clone())).collect();
        if encoder.len() != decoder.len() {
            return Err("encoder has duplicate ranks".to_string());
        }
        let special_tokens_decoder: HashMap<Rank, Vec<u8>> = special_tokens_encoder
            .iter()
            .map(|(k, v)| (*v, k.as_bytes().to_vec()))
            .collect();

        Ok(Self {
            encoder,
            special_tokens_encoder,
            decoder,
            special_tokens_decoder,
            regex,
            special_regex,
        })
    }

    pub fn encode_ordinary(&self, text: &str) -> Result<Vec<Rank>, String> {
        let mut out = Vec::new();
        for mat in self.regex.find_iter(text) {
            let mat = mat.map_err(|err| format!("regex error: {err}"))?;
            let piece = byte_encode(mat.as_str().as_bytes());
            if let Some(token) = self.encoder.get(&piece) {
                out.push(*token);
            } else {
                out.extend(byte_pair_encode(&piece, &self.encoder)?);
            }
        }
        Ok(out)
    }

    pub fn encode(
        &self,
        text: &str,
        allowed_special: &HashSet<&str>,
    ) -> Result<(Vec<Rank>, usize), String> {
        let mut out = Vec::new();
        let mut start = 0usize;
        let mut last_piece_token_len = 0usize;

        loop {
            let mut next_special = None;
            let mut start_find = start;
            loop {
                let candidate = self
                    .special_regex
                    .find_from_pos(text, start_find)
                    .map_err(|err| format!("regex error: {err}"))?;
                match candidate {
                    Some(m) => {
                        if allowed_special.contains(&text[m.start()..m.end()]) {
                            next_special = Some(m);
                            break;
                        }
                        start_find = m.start().saturating_add(1);
                    }
                    None => break,
                }
            }

            let end = next_special.map_or(text.len(), |m| m.start());
            for mat in self.regex.find_iter(&text[start..end]) {
                let mat = mat.map_err(|err| format!("regex error: {err}"))?;
                let piece = byte_encode(mat.as_str().as_bytes());
                if let Some(token) = self.encoder.get(&piece) {
                    last_piece_token_len = 1;
                    out.push(*token);
                } else {
                    let tokens = byte_pair_encode(&piece, &self.encoder)?;
                    last_piece_token_len = tokens.len();
                    out.extend(tokens);
                }
            }

            match next_special {
                Some(m) => {
                    let piece = m.as_str();
                    let token = self
                        .special_tokens_encoder
                        .get(piece)
                        .copied()
                        .ok_or_else(|| "missing special token".to_string())?;
                    out.push(token);
                    start = m.end();
                    last_piece_token_len = 0;
                }
                None => break,
            }
        }

        Ok((out, last_piece_token_len))
    }

    pub fn encode_with_special_tokens(&self, text: &str) -> Result<Vec<Rank>, String> {
        let allowed = self.special_tokens();
        Ok(self.encode(text, &allowed)?.0)
    }

    pub fn decode_bytes<S, E>(&self, tokens: S) -> Result<Vec<u8>, String>
    where
        S: IntoIterator<Item = E>,
        E: Borrow<Rank>,
    {
        let token_iter = tokens.into_iter();
        let (lower, _upper) = token_iter.size_hint();
        let mut out = Vec::with_capacity(lower * 2);
        for token in token_iter {
            let &token = token.borrow();
            let token_bytes = match self.decoder.get(&token) {
                Some(bytes) => bytes,
                None => self
                    .special_tokens_decoder
                    .get(&token)
                    .ok_or_else(|| format!("invalid token {token}"))?,
            };
            out.extend(token_bytes);
        }
        byte_decode(&out)
    }

    pub fn decode_utf8<S, E>(&self, tokens: S) -> Result<String, String>
    where
        S: IntoIterator<Item = E>,
        E: Borrow<Rank>,
    {
        let bytes = self.decode_bytes(tokens)?;
        String::from_utf8(bytes).map_err(|err| format!("decode utf8 error: {err}"))
    }

    pub fn decode_utf8_lossy(&self, tokens: &[Rank]) -> String {
        match self.decode_bytes(tokens) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
            Err(_) => String::new(),
        }
    }

    pub fn special_tokens(&self) -> HashSet<&str> {
        self.special_tokens_encoder
            .keys()
            .map(|s| s.as_str())
            .collect()
    }

    pub fn is_special_token(&self, token: Rank) -> bool {
        self.special_tokens_decoder.contains_key(&token)
    }
}

pub struct GptOssTokenizer {
    bpe: CoreBpe,
    vocab: GgufTokenizer,
}

impl GptOssTokenizer {
    pub fn from_gguf(vocab: GgufTokenizer) -> Result<Self, String> {
        if vocab.token_types.is_empty() {
            return Err("tokenizer token_types missing".to_string());
        }
        if vocab.token_types.len() != vocab.tokens.len() {
            return Err("tokenizer token_types length mismatch".to_string());
        }
        let mut encoder = Vec::new();
        let mut special_tokens = Vec::new();

        for (idx, token) in vocab.tokens.iter().enumerate() {
            let rank = idx as Rank;
            let token_type = vocab.token_types.get(idx).copied().unwrap_or(1);
            if token_type == 1 {
                encoder.push((token.as_bytes().to_vec(), rank));
            } else {
                special_tokens.push((token.clone(), rank));
            }
        }

        let bpe = CoreBpe::new(encoder, special_tokens, &vocab.pattern)?;
        Ok(Self { bpe, vocab })
    }

    pub fn encode_with_special_tokens(&self, text: &str) -> Result<Vec<Rank>, String> {
        self.bpe.encode_with_special_tokens(text)
    }

    pub fn decode_utf8_lossy(&self, tokens: &[Rank]) -> String {
        self.bpe.decode_utf8_lossy(tokens)
    }

    pub fn token_text(&self, token: Rank) -> String {
        self.vocab
            .tokens
            .get(token as usize)
            .cloned()
            .unwrap_or_else(|| format!("<tok:{token}>"))
    }

    pub fn token_id(&self, token: &str) -> Option<Rank> {
        self.vocab
            .tokens
            .iter()
            .position(|candidate| candidate == token)
            .map(|idx| idx as Rank)
    }

    pub fn is_special_token(&self, token: Rank) -> bool {
        self.bpe.is_special_token(token)
    }

    pub fn bos_token_id(&self) -> Option<Rank> {
        self.vocab.bos_token_id
    }

    pub fn eos_token_id(&self) -> Option<Rank> {
        self.vocab.eos_token_id
    }

    pub fn pad_token_id(&self) -> Option<Rank> {
        self.vocab.pad_token_id
    }
}
