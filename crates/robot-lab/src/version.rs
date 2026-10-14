//! Portable version, note, and chat-selection values.
use crate::{Document, Reference, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Version {
    pub schema: String,
    pub document: String,
    pub number: u64,
    pub parent: Option<Reference>,
    pub author: String,
    pub summary: String,
    pub created_unix_ms: u64,
    pub content_sha256: String,
    pub content: Document,
}

impl Version {
    pub fn reference(&self) -> Result<Reference> {
        Ok(Reference {
            document: self.document.clone(),
            version: format!("{:x}", Sha256::digest(serde_json::to_vec(self)?)),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Note {
    pub subject: Reference,
    pub episode_step: Option<u32>,
    pub author: String,
    pub text: String,
    pub created_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub scene: Reference,
    pub episode: Option<Reference>,
    pub step: Option<u32>,
    pub comparison: Option<Reference>,
    pub camera: String,
}
impl Selection {
    pub fn sha256(&self) -> Result<String> {
        Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(self)?)))
    }
}
