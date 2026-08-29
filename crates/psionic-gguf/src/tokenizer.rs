use crate::format::{GgufMeta, GgufValue};

pub struct TokenizerTables {
    pub model: String,
    pub n_tokens: usize,
    pub n_merges: usize,
    pub bos: u64,
    pub eos: u64,
    pub tokens: Vec<String>,
    pub merges: Vec<String>,
}

pub fn load_tokenizer(meta: &GgufMeta) -> Result<TokenizerTables, String> {
    let model = meta
        .get("tokenizer.ggml.model")
        .and_then(GgufValue::as_str)
        .ok_or_else(|| String::from("missing tokenizer.ggml.model"))?
        .to_string();
    if model != "gpt2" && model != "llama" {
        return Err(model);
    }
    let tokens = meta
        .get("tokenizer.ggml.tokens")
        .and_then(GgufValue::as_string_array)
        .ok_or_else(|| String::from("tokens"))?;
    let merges = meta
        .get("tokenizer.ggml.merges")
        .and_then(GgufValue::as_string_array)
        .ok_or_else(|| String::from("merges"))?;
    let bos = meta.kv_u64("tokenizer.ggml.bos_token_id").unwrap_or(0);
    let eos = meta.kv_u64("tokenizer.ggml.eos_token_id").unwrap_or(0);
    Ok(TokenizerTables {
        model,
        n_tokens: tokens.len(),
        n_merges: merges.len(),
        bos,
        eos,
        tokens: tokens.into_iter().map(str::to_string).collect(),
        merges: merges.into_iter().map(str::to_string).collect(),
    })
}

/// Qwen chat wrap. Jinja `tokenizer.chat_template` is not executed.
pub fn render_chat(prompt: &str, template: Option<&str>) -> String {
    if let Some(t) = template {
        if t.contains("{%") || t.contains("{{ messages") {
            return qwen_user_wrap(prompt);
        }
        if t.contains("{{") && !t.contains("{%") {
            return t
                .replace("{{ content }}", prompt)
                .replace("{{content}}", prompt)
                .replace("{content}", prompt);
        }
        if !t.is_empty() {
            return format!("{t}\n{prompt}");
        }
    }
    qwen_user_wrap(prompt)
}

fn qwen_user_wrap(prompt: &str) -> String {
    // Ollama / this GGUF Jinja default `enable_thinking` to true, so the
    // assistant turn opens with `<think>\n`.
    format!("<|im_start|>user\n{prompt}<|im_end|>\n<|im_start|>assistant\n<think>\n")
}

impl TokenizerTables {
    pub fn token_piece(&self, id: u32) -> String {
        self.tokens
            .get(id as usize)
            .cloned()
            .unwrap_or_else(|| format!("<{id}>"))
    }

    /// GPT-2 / Qwen BPE. Special tokens match as whole pieces first.
    /// Unknown characters become token 0 (deterministic).
    pub fn encode(&self, text: &str) -> Result<Vec<u32>, String> {
        if self.tokens.is_empty() {
            return Err(String::from("empty vocab"));
        }
        let mut id_of = std::collections::HashMap::new();
        for (i, tok) in self.tokens.iter().enumerate() {
            id_of.entry(tok.as_str()).or_insert(i as u32);
        }
        let mut rank = std::collections::HashMap::new();
        for (i, merge) in self.merges.iter().enumerate() {
            rank.insert(merge.as_str(), i);
        }
        let specials = specials_longest(&self.tokens);
        let use_bytes = self.tokens.iter().any(|t| t == "Ċ" || t == "Ġ");

        let mut ids = Vec::new();
        for span in split_specials(text, &specials) {
            match span {
                Span::Special(tok) => {
                    if let Some(&id) = id_of.get(tok) {
                        ids.push(id);
                    }
                }
                Span::Text(piece) => {
                    for word in split_words(piece) {
                        let encoded = if use_bytes { gpt2_bytes(&word) } else { word };
                        ids.extend(bpe_word(&encoded, &id_of, &rank));
                    }
                }
            }
        }
        if ids.is_empty() {
            ids.push(0);
        }
        Ok(ids)
    }
}

enum Span<'a> {
    Special(&'a str),
    Text(&'a str),
}

fn specials_longest(tokens: &[String]) -> Vec<String> {
    let mut specials: Vec<String> = tokens
        .iter()
        .filter(|t| is_special_token(t))
        .cloned()
        .collect();
    specials.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    specials
}

fn is_special_token(token: &str) -> bool {
    (token.starts_with("<|") && token.ends_with("|>"))
        || (token.starts_with('<') && token.ends_with('>') && token.len() >= 3)
}

fn split_specials<'a>(text: &'a str, specials: &[String]) -> Vec<Span<'a>> {
    if specials.is_empty() {
        return vec![Span::Text(text)];
    }
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let mut hit: Option<&str> = None;
        for s in specials {
            if rest.starts_with(s.as_str()) {
                hit = Some(s.as_str());
                break;
            }
        }
        if let Some(s) = hit {
            out.push(Span::Special(&text[i..i + s.len()]));
            i += s.len();
        } else {
            let start = i;
            let ch = text[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            i += ch;
            while i < text.len() {
                let rest = &text[i..];
                if specials.iter().any(|s| rest.starts_with(s.as_str())) {
                    break;
                }
                i += rest.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            }
            let _ = bytes;
            out.push(Span::Text(&text[start..i]));
        }
    }
    out
}

fn gpt2_bytes(text: &str) -> String {
    text.as_bytes().iter().copied().map(encode_byte).collect()
}

fn byte_encoder() -> [char; 256] {
    let mut bs: Vec<u8> = (b'!'..=b'~').collect();
    bs.extend(0xA1u8..=0xAC);
    bs.extend(0xAEu8..=0xFF);
    let mut cs: Vec<u32> = bs.iter().map(|b| *b as u32).collect();
    let mut n = 0u32;
    for b in 0u8..=255 {
        if !bs.contains(&b) {
            bs.push(b);
            cs.push(256 + n);
            n += 1;
        }
    }
    let mut out = ['\0'; 256];
    for (b, c) in bs.into_iter().zip(cs.into_iter()) {
        out[b as usize] = char::from_u32(c).unwrap_or('\u{FFFD}');
    }
    out
}

fn encode_byte(b: u8) -> char {
    static TABLE: std::sync::OnceLock<[char; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(byte_encoder)[b as usize]
}

fn split_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
            words.push(ch.to_string());
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words
}

fn bpe_word(
    word: &str,
    id_of: &std::collections::HashMap<&str, u32>,
    rank: &std::collections::HashMap<&str, usize>,
) -> Vec<u32> {
    if let Some(&id) = id_of.get(word) {
        return vec![id];
    }
    let mut symbols: Vec<String> = word.chars().map(|c| c.to_string()).collect();
    if symbols.is_empty() {
        return vec![0];
    }
    loop {
        let mut best: Option<(usize, usize)> = None;
        for i in 0..symbols.len().saturating_sub(1) {
            let pair = format!("{} {}", symbols[i], symbols[i + 1]);
            if let Some(&r) = rank.get(pair.as_str()) {
                if best.map(|(br, _)| r < br).unwrap_or(true) {
                    best = Some((r, i));
                }
            }
        }
        let Some((_, i)) = best else {
            break;
        };
        let merged = format!("{}{}", symbols[i], symbols[i + 1]);
        symbols[i] = merged;
        symbols.remove(i + 1);
    }
    symbols
        .iter()
        .map(|s| id_of.get(s.as_str()).copied().unwrap_or(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::write_qwen35_fixture;
    use crate::parse_path;

    #[test]
    fn fixture_encodes_hello_deterministically() {
        let dir = std::env::temp_dir().join("psionic-gguf-tok");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("qwen35.gguf");
        write_qwen35_fixture(&path).unwrap();
        let meta = parse_path(&path).unwrap();
        let tok = load_tokenizer(&meta).unwrap();
        let rendered = render_chat("hello", None);
        let ids = tok.encode(&rendered).unwrap();
        assert!(!ids.is_empty());
        let again = tok.encode(&rendered).unwrap();
        assert_eq!(ids, again);
    }

    #[test]
    fn specials_are_atomic_and_newline_is_gpt2_byte() {
        let tok = TokenizerTables {
            model: String::from("gpt2"),
            n_tokens: 6,
            n_merges: 0,
            bos: 0,
            eos: 1,
            tokens: vec![
                "<|im_start|>".into(),
                "<|im_end|>".into(),
                "user".into(),
                "hello".into(),
                "assistant".into(),
                "Ċ".into(),
                "<think>".into(),
            ],
            merges: vec![],
        };
        let rendered = render_chat("hello", Some("{% messages %}"));
        let ids = tok.encode(&rendered).unwrap();
        assert_eq!(
            ids,
            vec![0, 2, 5, 3, 1, 5, 0, 4, 5, 6, 5],
            "wrap must match Ollama thinking-on prompt, got {ids:?}"
        );
    }

    #[test]
    fn twenty_seven_b_hello_prompt_ids_match_ollama_when_blob_present() {
        let Ok(home) = std::env::var("HOME") else {
            return;
        };
        let path = std::path::PathBuf::from(home).join(
            ".ollama/models/blobs/sha256-2bb22714289826d7b9e0ba376c3ce47d08bce39abe598745857c44d88c09bdbf",
        );
        if !path.is_file() {
            return;
        }
        let meta = crate::parse_path(&path).unwrap();
        let tok = load_tokenizer(&meta).unwrap();
        let ids = tok.encode(&render_chat("hello", None)).unwrap();
        assert_eq!(
            ids,
            vec![248045, 846, 198, 14556, 248046, 198, 248045, 74455, 198, 248068, 198],
            "prompt IDs must match Ollama context[:11], got {ids:?}"
        );
    }
}
