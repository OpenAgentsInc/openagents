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

/// Qwen chat wrap when GGUF has no `tokenizer.chat_template`.
pub fn render_chat(prompt: &str, template: Option<&str>) -> String {
    if let Some(t) = template {
        if t.contains("{{") {
            return t
                .replace("{{ content }}", prompt)
                .replace("{{content}}", prompt)
                .replace("{content}", prompt);
        }
        if !t.is_empty() {
            return format!("{t}\n{prompt}");
        }
    }
    format!("<|im_start|>user\n{prompt}<|im_end|>\n<|im_start|>assistant\n")
}

impl TokenizerTables {
    pub fn token_piece(&self, id: u32) -> String {
        self.tokens
            .get(id as usize)
            .cloned()
            .unwrap_or_else(|| format!("<{id}>"))
    }

    /// GPT-2 style BPE. Unknown characters become token 0 (deterministic).
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

        let mut ids = Vec::new();
        for word in split_words(text) {
            ids.extend(bpe_word(&word, &id_of, &rank));
        }
        if ids.is_empty() {
            ids.push(0);
        }
        Ok(ids)
    }
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
}
