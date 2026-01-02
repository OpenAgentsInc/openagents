use crate::error::Result;
use crate::http::fetch_bytes;

#[derive(Clone, Debug)]
pub struct Tokenizer {
    inner: tokenizers::Tokenizer,
}

impl Tokenizer {
    pub fn from_bytes(json: &[u8]) -> Result<Self> {
        let inner = tokenizers::Tokenizer::from_bytes(json)?;
        Ok(Self { inner })
    }

    #[cfg(feature = "native")]
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let inner = tokenizers::Tokenizer::from_file(path)?;
        Ok(Self { inner })
    }

    pub async fn from_url(url: &str) -> Result<Self> {
        let bytes = fetch_bytes(url).await?;
        Self::from_bytes(&bytes)
    }

    pub fn encode(&self, text: &str, add_special: bool) -> Result<Vec<u32>> {
        let encoding = self.inner.encode(text, add_special)?;
        Ok(encoding.get_ids().to_vec())
    }

    pub fn decode(&self, tokens: &[u32], skip_special: bool) -> Result<String> {
        Ok(self.inner.decode(tokens, skip_special)?)
    }

    pub fn token_id(&self, token: &str) -> Option<u32> {
        self.inner.get_vocab(true).get(token).copied()
    }

    pub fn vocab_size(&self) -> usize {
        self.inner.get_vocab(true).len()
    }

    pub fn bos_token_id(&self) -> Option<u32> {
        self.find_token_id(&[
            "<s>",
            "<bos>",
            "<BOS>",
            "[BOS]",
            "<|bos|>",
            "<|begin_of_text|>",
            "<|startoftext|>",
            "[CLS]",
        ])
    }

    pub fn eos_token_id(&self) -> Option<u32> {
        self.find_token_id(&[
            "</s>",
            "<eos>",
            "<EOS>",
            "[EOS]",
            "<|eos|>",
            "<|end_of_text|>",
            "<|endoftext|>",
            "[SEP]",
        ])
    }

    pub fn pad_token_id(&self) -> Option<u32> {
        self.find_token_id(&["<pad>", "<PAD>", "[PAD]", "<|pad|>"])
    }

    pub fn inner(&self) -> &tokenizers::Tokenizer {
        &self.inner
    }

    fn find_token_id(&self, candidates: &[&str]) -> Option<u32> {
        for &token in candidates {
            if let Some(id) = self.token_id(token) {
                return Some(id);
            }
        }
        None
    }
}
