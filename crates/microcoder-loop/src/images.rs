//! Images a person attached to a task, carried to the model natively on
//! every step's user message: the Codex Responses API's `input_image` with a
//! data URL (what `codex exec --image` sends), and Claude Code's
//! `stream-json` user message with base64 `image` blocks. The bytes come
//! from the task store already checked against their digest; nothing here
//! reads a path. Evidence records each image by digest and size, never its
//! bytes ([`redacted`]).

use base64::Engine as _;
use serde_json::{Value, json};
use std::sync::Arc;

/// One attached image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputImage {
    /// `image/png` or `image/jpeg`.
    pub media_type: String,
    pub bytes: Arc<Vec<u8>>,
}

impl InputImage {
    #[must_use]
    pub fn base64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(self.bytes.as_slice())
    }

    /// The `sha256:<hex>` digest of the bytes.
    #[must_use]
    pub fn digest(&self) -> String {
        nostr::contracts::digest_bytes(&self.bytes)
    }

    /// The Responses API content item.
    #[must_use]
    pub fn codex(&self) -> Value {
        json!({"type":"input_image","image_url":format!("data:{};base64,{}",self.media_type,self.base64())})
    }

    /// The Anthropic Messages content block Claude Code's `stream-json`
    /// input takes.
    #[must_use]
    pub fn claude(&self) -> Value {
        json!({"type":"image","source":{"type":"base64","media_type":self.media_type,"data":self.base64()}})
    }

    /// What evidence keeps of the image.
    #[must_use]
    pub fn record(&self) -> Value {
        json!({"media_type":self.media_type,"bytes":self.bytes.len(),"digest":self.digest()})
    }
}

/// `value` with every inline image's data replaced by its digest and size,
/// for retained evidence: a Responses `input_image` data URL or a base64
/// `image` source.
#[must_use]
pub fn redacted(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(redacted).collect()),
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, item) in map {
                let replaced = match (key.as_str(), item) {
                    ("image_url", Value::String(url)) => url
                        .split_once(";base64,")
                        .filter(|(head, _)| head.starts_with("data:"))
                        .map(|(head, data)| summary(&head["data:".len()..], data)),
                    ("data", Value::String(data))
                        if map.get("type").and_then(Value::as_str) == Some("base64") =>
                    {
                        let media = map.get("media_type").and_then(Value::as_str).unwrap_or("");
                        Some(summary(media, data))
                    }
                    _ => None,
                };
                out.insert(key.clone(), replaced.unwrap_or_else(|| redacted(item)));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

fn summary(media_type: &str, data: &str) -> Value {
    match base64::engine::general_purpose::STANDARD.decode(data) {
        Ok(bytes) => json!({"media_type":media_type,"bytes":bytes.len(),
            "digest":nostr::contracts::digest_bytes(&bytes)}),
        Err(_) => json!({"media_type":media_type,"unreadable":true}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_keeps_digests_not_bytes() {
        let image = InputImage {
            media_type: "image/png".into(),
            bytes: Arc::new(b"\x89PNG\r\n\x1a\nxyz".to_vec()),
        };
        let request = json!({"input":[{"content":[{"type":"input_text","text":"t"},image.codex()]}],
            "claude":[image.claude()]});
        let kept = redacted(&request);
        let text = kept.to_string();
        assert!(!text.contains(&image.base64()), "{text}");
        assert_eq!(kept["input"][0]["content"][1]["image_url"], image.record());
        assert_eq!(kept["claude"][0]["source"]["data"], image.record());
        assert_eq!(kept["input"][0]["content"][0]["text"], "t");
    }
}
