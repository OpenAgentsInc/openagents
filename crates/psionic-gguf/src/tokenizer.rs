use crate::format::{GgufMeta, GgufValue};

pub struct TokenizerTables {
    pub model: String,
    pub n_tokens: usize,
    pub n_merges: usize,
    pub bos: u64,
    pub eos: u64,
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
    })
}
