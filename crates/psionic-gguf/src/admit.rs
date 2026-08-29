use sha2::{Digest, Sha256};

use crate::format::GgufMeta;

pub const FAMILY_QWEN35: &str = "qwen35";

pub const REQUIRED_TENSORS: &[&str] = &["token_embd.weight", "output.weight", "output_norm.weight"];

pub struct Admission {
    pub family: String,
    pub digest: String,
    pub translated: usize,
}

/// Ollama blobs use `mtp.*`. llama.cpp wants `blk.N.nextn.*`.
pub fn translate_tensor_name(name: &str) -> String {
    if let Some(rest) = name.strip_prefix("mtp.") {
        let (idx, tail) = match rest.split_once('.') {
            Some((n, t)) if n.chars().all(|c| c.is_ascii_digit()) => (n, t),
            _ => ("0", rest),
        };
        return format!("blk.{idx}.nextn.{tail}");
    }
    name.to_string()
}

pub fn names_need_translate(meta: &GgufMeta) -> bool {
    meta.tensors.iter().any(|t| t.name.starts_with("mtp."))
}

pub fn admit(meta: &GgufMeta, header_bytes: &[u8]) -> Result<Admission, AdmitError> {
    let arch = meta
        .architecture()
        .ok_or(AdmitError::Family(String::from("missing")))?;
    if arch != FAMILY_QWEN35 {
        return Err(AdmitError::Family(arch.to_string()));
    }
    let translated = names_need_translate(meta);
    let names: Vec<String> = meta
        .tensors
        .iter()
        .map(|t| translate_tensor_name(&t.name))
        .collect();
    for required in REQUIRED_TENSORS {
        if !names.iter().any(|n| n == required) {
            return Err(AdmitError::MissingTensor((*required).to_string()));
        }
    }
    let mut hasher = Sha256::new();
    hasher.update(header_bytes);
    hasher.update(meta.file_size.to_le_bytes());
    let digest = format!("{:x}", hasher.finalize());
    Ok(Admission {
        family: FAMILY_QWEN35.to_string(),
        digest,
        translated: usize::from(translated),
    })
}

#[derive(Debug)]
pub enum AdmitError {
    Family(String),
    MissingTensor(String),
}

impl std::fmt::Display for AdmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Family(arch) => write!(f, "{arch}"),
            Self::MissingTensor(name) => write!(f, "{name}"),
        }
    }
}
