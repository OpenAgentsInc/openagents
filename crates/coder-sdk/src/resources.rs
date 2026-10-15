//! Resource references and provider input, with bytes supplied by the owner.

use base64::{Engine as _, engine::general_purpose::STANDARD};
pub use route_contract::resources::*;
use serde_json::{Value, json};
use std::sync::Arc;

/// Exact bytes returned by an authorized resource service. The owner checks
/// image decoding and dimensions before issuing its reference.
#[derive(Clone)]
pub struct Bound {
    reference: Reference,
    bytes: Arc<Vec<u8>>,
}
impl std::fmt::Debug for Bound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bound")
            .field("reference", &self.reference)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}
impl Bound {
    pub fn new(reference: Reference, bytes: Arc<Vec<u8>>) -> Result<Self, String> {
        if !reference.valid()
            || reference.size != bytes.len()
            || reference.digest != route_contract::Digest::of_bytes(&bytes)
        {
            return Err("This attachment changed. Add it again.".into());
        }
        let matching = match reference.media_type {
            MediaType::Png => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            MediaType::Jpeg => bytes.starts_with(&[0xff, 0xd8, 0xff]),
            MediaType::Text => std::str::from_utf8(&bytes).is_ok() && !bytes.contains(&0),
            MediaType::Json => serde_json::from_slice::<Value>(&bytes).is_ok(),
        };
        if !matching {
            return Err("This attachment couldn't be read. Add it again.".into());
        }
        Ok(Self { reference, bytes })
    }
    pub fn reference(&self) -> &Reference {
        &self.reference
    }
    pub fn into_bytes(self) -> Arc<Vec<u8>> {
        self.bytes
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

pub trait Service: Send + Sync {
    fn read(&self, reference: &Reference, usage: Usage) -> Result<Bound, String>;
}

/// Translate authorized prompt images without putting their bytes into an
/// observation or hook record. Arbitrary URIs and filenames are not inputs.
pub const MAX_PROMPT_IMAGES: usize = MAX_INPUTS * 2;
pub const MAX_PROMPT_BYTES: usize = MAX_INPUT_BYTES * 2;

pub fn claude_content(
    text: &str,
    references: &[Reference],
    service: &dyn Service,
) -> Result<Vec<Value>, String> {
    if references.len() > MAX_PROMPT_IMAGES
        || references
            .iter()
            .any(|reference| !reference.allows(Usage::PromptImage))
        || references
            .iter()
            .map(|reference| reference.size)
            .sum::<usize>()
            > MAX_PROMPT_BYTES
    {
        return Err("These images exceed this coding session’s size limit.".into());
    }
    if let Some(first) = references.first()
        && references
            .iter()
            .any(|reference| reference.owner != first.owner)
    {
        return Err("These attachments belong to another message.".into());
    }
    let mut blocks = vec![json!({"type":"text","text":text})];
    for reference in references {
        let bound = service.read(reference, Usage::PromptImage)?;
        if bound.reference() != reference {
            return Err("This attachment changed. Add it again.".into());
        }
        blocks.push(json!({"type":"image","source":{"type":"base64","media_type":reference.media_type.as_str(),"data":STANDARD.encode(bound.bytes())}}));
    }
    Ok(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use route_contract::{Digest, digest_of};
    struct Memory {
        reference: Reference,
        bytes: Arc<Vec<u8>>,
    }
    impl Service for Memory {
        fn read(&self, reference: &Reference, usage: Usage) -> Result<Bound, String> {
            if reference != &self.reference || !reference.allows(usage) {
                return Err("Unavailable".into());
            }
            Bound::new(reference.clone(), self.bytes.clone())
        }
    }
    fn memory() -> Memory {
        let bytes = Arc::new(include_bytes!("../fixtures/resources/one-pixel.png").to_vec());
        let mut reference = Reference {
            schema: SCHEMA.into(),
            id: digest_of(&()),
            owner: Owner {
                host: "local".into(),
                task: "task".into(),
            },
            origin: Origin::Input {
                message: "message".into(),
            },
            digest: Digest::of_bytes(&bytes),
            media_type: MediaType::Png,
            size: bytes.len(),
            name: "image.png".into(),
            dimensions: Some(Dimensions {
                width: 1,
                height: 1,
            }),
            uses: vec![Usage::PromptImage, Usage::Preview],
        };
        reference.seal();
        Memory { reference, bytes }
    }
    #[test]
    fn claude_images_are_exact_bytes_and_never_paths_or_text_substitutes() {
        let memory = memory();
        let content = claude_content("Inspect this", &[memory.reference.clone()], &memory).unwrap();
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[1]["source"]["media_type"], "image/png");
        assert_eq!(
            STANDARD
                .decode(content[1]["source"]["data"].as_str().unwrap())
                .unwrap(),
            *memory.bytes
        );
        let mut forged = memory.reference.clone();
        forged.owner.task = "other".into();
        forged.seal();
        assert!(claude_content("Inspect", &[forged], &memory).is_err());
        assert!(Bound::new(memory.reference.clone(), Arc::new(b"changed".to_vec())).is_err());
        assert!(
            !format!(
                "{:?}",
                Bound::new(memory.reference.clone(), memory.bytes.clone()).unwrap()
            )
            .contains("[137, 80")
        );
    }
    #[test]
    fn unavailable_or_oversized_resources_fail_before_partial_input_is_returned() {
        let memory = memory();
        let mut missing = memory.reference.clone();
        missing.name = "missing.png".into();
        missing.seal();
        assert!(claude_content("Inspect", &[memory.reference.clone(), missing], &memory).is_err());
        let mut huge = memory.reference.clone();
        huge.size = usize::MAX;
        huge.seal();
        assert!(claude_content("Inspect", &[huge], &memory).is_err());
    }
}
