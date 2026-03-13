use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Stable top-level type tag used by Apple Foundation Models transcripts.
pub const APPLE_FM_TRANSCRIPT_TYPE: &str = "FoundationModels.Transcript";

/// Current transcript format version used by the Apple FM SDK.
pub const APPLE_FM_TRANSCRIPT_VERSION: u32 = 1;

/// Reusable Rust transcript type aligned with the Apple FM transcript envelope.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleFmTranscript {
    /// Transcript schema version.
    #[serde(default = "default_transcript_version")]
    pub version: u32,
    /// Stable transcript type tag.
    #[serde(rename = "type", default = "default_transcript_type")]
    pub transcript_type: String,
    /// Transcript payload containing ordered entries.
    pub transcript: AppleFmTranscriptPayload,
}

impl Default for AppleFmTranscript {
    fn default() -> Self {
        Self {
            version: APPLE_FM_TRANSCRIPT_VERSION,
            transcript_type: APPLE_FM_TRANSCRIPT_TYPE.to_string(),
            transcript: AppleFmTranscriptPayload::default(),
        }
    }
}

impl AppleFmTranscript {
    /// Parses a transcript from a serialized JSON string.
    pub fn from_json_str(value: &str) -> Result<Self, AppleFmTranscriptError> {
        let transcript = serde_json::from_str::<Self>(value).map_err(|error| {
            AppleFmTranscriptError::Decode {
                error: error.to_string(),
            }
        })?;
        transcript.validate()?;
        Ok(transcript)
    }

    /// Serializes the transcript to a compact JSON string.
    pub fn to_json_string(&self) -> Result<String, AppleFmTranscriptError> {
        self.validate()?;
        serde_json::to_string(self).map_err(|error| AppleFmTranscriptError::Encode {
            error: error.to_string(),
        })
    }

    /// Verifies the transcript envelope matches the Apple FM contract.
    pub fn validate(&self) -> Result<(), AppleFmTranscriptError> {
        if self.transcript_type != APPLE_FM_TRANSCRIPT_TYPE {
            return Err(AppleFmTranscriptError::InvalidType {
                found: self.transcript_type.clone(),
            });
        }
        Ok(())
    }

    /// Returns the number of transcript entries.
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.transcript.entries.len()
    }
}

/// Transcript payload containing ordered Foundation Models entries.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AppleFmTranscriptPayload {
    /// Ordered transcript entries.
    #[serde(default)]
    pub entries: Vec<AppleFmTranscriptEntry>,
}

/// Transcript entry preserving known role/id fields and any extra Apple payload.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AppleFmTranscriptEntry {
    /// Optional entry identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Foundation Models transcript role.
    pub role: String,
    /// Optional content array for this entry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contents: Vec<AppleFmTranscriptContent>,
    /// Remaining role-specific payload such as tools, toolCalls, assets, or options.
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Transcript content preserving known type/id fields and any extra content payload.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AppleFmTranscriptContent {
    /// Content discriminator such as `text` or `structure`.
    #[serde(rename = "type")]
    pub content_type: String,
    /// Optional content identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Remaining content payload such as `text` or `structure`.
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl AppleFmTranscriptContent {
    /// Returns the plain-text payload if this content item is textual.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.extra.get("text").and_then(Value::as_str)
    }
}

/// Transcript parse and validation failures.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum AppleFmTranscriptError {
    /// Transcript JSON could not be parsed into the Apple FM envelope.
    #[error("invalid Apple FM transcript JSON: {error}")]
    Decode { error: String },
    /// Transcript serialization failed unexpectedly.
    #[error("failed to serialize Apple FM transcript: {error}")]
    Encode { error: String },
    /// Transcript type tag was not the expected Foundation Models value.
    #[error("invalid Apple FM transcript type '{found}'")]
    InvalidType { found: String },
    /// Both raw transcript JSON and typed transcript were provided but did not match.
    #[error("conflicting 'transcript_json' and 'transcript' values were provided")]
    ConflictingInputs,
}

const fn default_transcript_version() -> u32 {
    APPLE_FM_TRANSCRIPT_VERSION
}

fn default_transcript_type() -> String {
    APPLE_FM_TRANSCRIPT_TYPE.to_string()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use serde_json::json;

    use super::{
        APPLE_FM_TRANSCRIPT_TYPE, APPLE_FM_TRANSCRIPT_VERSION, AppleFmTranscript,
        AppleFmTranscriptContent, AppleFmTranscriptEntry, AppleFmTranscriptError,
    };

    #[test]
    fn transcript_round_trip_preserves_top_level_shape() {
        let transcript = AppleFmTranscript {
            version: APPLE_FM_TRANSCRIPT_VERSION,
            transcript_type: APPLE_FM_TRANSCRIPT_TYPE.to_string(),
            transcript: crate::transcript::AppleFmTranscriptPayload {
                entries: vec![AppleFmTranscriptEntry {
                    id: Some("entry-1".to_string()),
                    role: "user".to_string(),
                    contents: vec![AppleFmTranscriptContent {
                        content_type: "text".to_string(),
                        id: Some("content-1".to_string()),
                        extra: [("text".to_string(), json!("hello"))].into_iter().collect(),
                    }],
                    extra: [("options".to_string(), json!({}))].into_iter().collect(),
                }],
            },
        };

        let encoded = transcript.to_json_string().expect("encode transcript");
        let decoded =
            AppleFmTranscript::from_json_str(encoded.as_str()).expect("decode transcript");
        assert_eq!(decoded, transcript);
        assert_eq!(decoded.entry_count(), 1);
        assert_eq!(
            decoded.transcript.entries[0].contents[0].text(),
            Some("hello")
        );
    }

    #[test]
    fn transcript_rejects_wrong_type() {
        let error = AppleFmTranscript::from_json_str(
            r#"{"version":1,"type":"wrong","transcript":{"entries":[]}}"#,
        )
        .expect_err("wrong type should fail");

        assert_eq!(
            error,
            AppleFmTranscriptError::InvalidType {
                found: "wrong".to_string()
            }
        );
    }
}
