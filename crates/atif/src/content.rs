//! Versions and multimodal content: what a reader accepts, and how.
//!
//! This crate writes one version, [`SCHEMA_VERSION`]. It reads every 1.x
//! version the upstream specification (Harbor RFC 0001) has published,
//! because every 1.x revision only added optional fields: a v1.7 document is
//! a valid v1.8 document with an older label. [`upgrade`] relabels one in
//! memory; it never rewrites recorded bytes.
//!
//! # What v1.8 added
//!
//! ATIF-v1.8 added exactly one thing over v1.7: a [`ContentPart`] of type
//! `audio`, with an [`AudioSource`] that mirrors [`ImageSource`] (`media_type`
//! and `path`) plus an optional `duration_sec`. Alongside it the RFC requires
//! consumers to reject a source whose kind disagrees with `type`, and asks
//! them to normalize common alternate audio MIME spellings (`audio/mp3` above
//! all). Nothing was removed or renamed.
//!
//! The rules here follow the reference Pydantic models in Harbor
//! (`src/harbor/models/trajectories/content.py`): every content object is
//! closed, a text part carries `text` and no `source`, a media part carries a
//! `source` of its own kind and no `text`. Like the reference, a feature is
//! not gated by the document's declared version: a v1.7 document holding an
//! audio part validates, as it does upstream.

use serde_json::{Map, Value};

/// The version of the format this crate writes.
pub const SCHEMA_VERSION: &str = "ATIF-v1.8";

/// Every version a reader accepts, oldest first. The last is
/// [`SCHEMA_VERSION`].
pub const SUPPORTED_SCHEMA_VERSIONS: [&str; 9] = [
    "ATIF-v1.0",
    "ATIF-v1.1",
    "ATIF-v1.2",
    "ATIF-v1.3",
    "ATIF-v1.4",
    "ATIF-v1.5",
    "ATIF-v1.6",
    "ATIF-v1.7",
    SCHEMA_VERSION,
];

/// Whether a reader accepts a document declaring `version`.
#[must_use]
pub fn supported(version: &str) -> bool {
    SUPPORTED_SCHEMA_VERSIONS.contains(&version)
}

/// Relabels a document of an older supported version as [`SCHEMA_VERSION`],
/// in memory, and returns the version it declared.
///
/// Every 1.x revision is additive, so no field changes: the relabel is the
/// whole upgrade. A document already at the current version is left alone.
///
/// # Errors
///
/// Returns a message when `schema_version` is missing or names a version
/// this crate does not read. The document is then untouched.
pub fn upgrade(document: &mut Value) -> Result<String, String> {
    let declared = document
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| "the document declares no schema_version".to_string())?
        .to_string();
    if !supported(&declared) {
        return Err(format!("unsupported ATIF version {declared:?}"));
    }
    document["schema_version"] = Value::String(SCHEMA_VERSION.to_string());
    Ok(declared)
}

/// The image MIME types an [`ImageSource`] may declare (v1.6+).
pub const IMAGE_MEDIA_TYPES: [&str; 4] = ["image/jpeg", "image/png", "image/gif", "image/webp"];

/// The canonical audio MIME types an [`AudioSource`] may declare (v1.8+).
pub const AUDIO_MEDIA_TYPES: [&str; 8] = [
    "audio/wav",
    "audio/mpeg",
    "audio/mp4",
    "audio/aac",
    "audio/ogg",
    "audio/flac",
    "audio/webm",
    "audio/aiff",
];

/// Alternate audio spellings and the canonical type each normalizes to.
pub const AUDIO_MEDIA_TYPE_ALIASES: [(&str, &str); 11] = [
    ("audio/mp3", "audio/mpeg"),
    ("audio/mpga", "audio/mpeg"),
    ("audio/x-mpeg", "audio/mpeg"),
    ("audio/x-wav", "audio/wav"),
    ("audio/wave", "audio/wav"),
    ("audio/vnd.wave", "audio/wav"),
    ("audio/x-m4a", "audio/mp4"),
    ("audio/m4a", "audio/mp4"),
    ("audio/x-aac", "audio/aac"),
    ("audio/x-flac", "audio/flac"),
    ("audio/x-aiff", "audio/aiff"),
];

/// The canonical spelling of an audio MIME type: trimmed, lowercased, and
/// with a known alias replaced. An unknown type comes back normalized but
/// unaccepted; [`AudioSource::from_value`] refuses it.
#[must_use]
pub fn normalize_audio_media_type(media_type: &str) -> String {
    let normalized = media_type.trim().to_ascii_lowercase();
    AUDIO_MEDIA_TYPE_ALIASES
        .iter()
        .find(|(alias, _)| *alias == normalized)
        .map_or(normalized, |(_, canonical)| (*canonical).to_string())
}

/// An image referenced by path or URL (v1.6+).
#[derive(Clone, Debug, PartialEq)]
pub struct ImageSource {
    /// One of [`IMAGE_MEDIA_TYPES`].
    pub media_type: String,
    /// A relative or absolute file path, or a URL.
    pub path: String,
}

/// Audio referenced by path or URL (v1.8+).
#[derive(Clone, Debug, PartialEq)]
pub struct AudioSource {
    /// One of [`AUDIO_MEDIA_TYPES`], after normalization.
    pub media_type: String,
    /// A relative or absolute file path, or a URL.
    pub path: String,
    /// Duration in seconds, when known. Never negative.
    pub duration_sec: Option<f64>,
}

/// One part of a multimodal message or observation (v1.6+; audio v1.8+).
#[derive(Clone, Debug, PartialEq)]
pub enum ContentPart {
    /// Text.
    Text(String),
    /// An image.
    Image(ImageSource),
    /// Audio.
    Audio(AudioSource),
}

fn closed(object: &Map<String, Value>, allowed: &[&str], what: &str) -> Result<(), String> {
    match object.keys().find(|key| !allowed.contains(&key.as_str())) {
        Some(key) => Err(format!("{what} does not allow the field {key:?}")),
        None => Ok(()),
    }
}

fn path_of(object: &Map<String, Value>, what: &str) -> Result<String, String> {
    object
        .get("path")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("{what} requires a string 'path'"))
}

impl ImageSource {
    /// Reads and validates an image source.
    ///
    /// # Errors
    ///
    /// Returns a message when the value is not a closed object with a
    /// supported `media_type` and a string `path`.
    pub fn from_value(value: &Value) -> Result<Self, String> {
        let object = value
            .as_object()
            .ok_or_else(|| "an image source must be an object".to_string())?;
        closed(object, &["media_type", "path"], "an image source")?;
        let media_type = object
            .get("media_type")
            .and_then(Value::as_str)
            .filter(|media_type| IMAGE_MEDIA_TYPES.contains(media_type))
            .ok_or_else(|| {
                format!(
                    "an image source's media_type must be one of {IMAGE_MEDIA_TYPES:?}, got {}",
                    object.get("media_type").unwrap_or(&Value::Null)
                )
            })?;
        Ok(ImageSource {
            media_type: media_type.to_string(),
            path: path_of(object, "an image source")?,
        })
    }

    /// The source as the document spells it.
    #[must_use]
    pub fn to_value(&self) -> Value {
        serde_json::json!({ "media_type": self.media_type, "path": self.path })
    }
}

impl AudioSource {
    /// Reads and validates an audio source, normalizing an alias to the
    /// canonical media type.
    ///
    /// # Errors
    ///
    /// Returns a message when the value is not a closed object with a
    /// supported `media_type`, a string `path`, and, when present, a
    /// non-negative numeric `duration_sec`.
    pub fn from_value(value: &Value) -> Result<Self, String> {
        let object = value
            .as_object()
            .ok_or_else(|| "an audio source must be an object".to_string())?;
        closed(
            object,
            &["media_type", "path", "duration_sec"],
            "an audio source",
        )?;
        let media_type = object
            .get("media_type")
            .and_then(Value::as_str)
            .map(normalize_audio_media_type)
            .filter(|media_type| AUDIO_MEDIA_TYPES.contains(&media_type.as_str()))
            .ok_or_else(|| {
                format!(
                    "an audio source's media_type must be one of {AUDIO_MEDIA_TYPES:?}, got {}",
                    object.get("media_type").unwrap_or(&Value::Null)
                )
            })?;
        let duration_sec = match object.get("duration_sec") {
            None | Some(Value::Null) => None,
            Some(value) => match value.as_f64() {
                Some(seconds) if seconds >= 0.0 => Some(seconds),
                _ => {
                    return Err(format!(
                        "an audio source's duration_sec must be a number >= 0, got {value}"
                    ));
                }
            },
        };
        Ok(AudioSource {
            media_type,
            path: path_of(object, "an audio source")?,
            duration_sec,
        })
    }

    /// The source as the document spells it, omitting an unknown duration.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut value = serde_json::json!({ "media_type": self.media_type, "path": self.path });
        if let Some(seconds) = self.duration_sec {
            value["duration_sec"] = serde_json::json!(seconds);
        }
        value
    }
}

impl ContentPart {
    /// Reads and validates one content part.
    ///
    /// # Errors
    ///
    /// Returns a message when the part is not a closed object, names an
    /// unknown `type`, lacks the field its type requires, carries the field
    /// its type forbids, or has a source whose kind disagrees with `type`.
    pub fn from_value(value: &Value) -> Result<Self, String> {
        let object = value
            .as_object()
            .ok_or_else(|| "a content part must be an object".to_string())?;
        closed(object, &["type", "text", "source"], "a content part")?;
        let kind = object
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| "a content part requires a string 'type'".to_string())?;
        let text = object.get("text").filter(|text| !text.is_null());
        let source = object.get("source").filter(|source| !source.is_null());
        match kind {
            "text" => {
                if source.is_some() {
                    return Err("'source' field is not allowed when type='text'".to_string());
                }
                text.and_then(Value::as_str)
                    .map(|text| ContentPart::Text(text.to_string()))
                    .ok_or_else(|| "'text' field is required when type='text'".to_string())
            }
            "image" | "audio" => {
                let Some(source) = source else {
                    return Err(format!("'source' field is required when type='{kind}'"));
                };
                if text.is_some() {
                    return Err(format!("'text' field is not allowed when type='{kind}'"));
                }
                // The kind check the RFC makes normative from v1.8: a source
                // that reads as the other kind is a mislabel, not a match.
                if kind == "image" {
                    ImageSource::from_value(source)
                        .map(ContentPart::Image)
                        .map_err(|error| match AudioSource::from_value(source) {
                            Ok(_) => {
                                "type='image' requires an image source, but the source is audio"
                                    .to_string()
                            }
                            Err(_) => error,
                        })
                } else {
                    AudioSource::from_value(source)
                        .map(ContentPart::Audio)
                        .map_err(|error| match ImageSource::from_value(source) {
                            Ok(_) => {
                                "type='audio' requires an audio source, but the source is an image"
                                    .to_string()
                            }
                            Err(_) => error,
                        })
                }
            }
            other => Err(format!(
                "a content part's type must be 'text', 'image', or 'audio', got {other:?}"
            )),
        }
    }

    /// The part as the document spells it.
    #[must_use]
    pub fn to_value(&self) -> Value {
        match self {
            ContentPart::Text(text) => serde_json::json!({ "type": "text", "text": text }),
            ContentPart::Image(source) => {
                serde_json::json!({ "type": "image", "source": source.to_value() })
            }
            ContentPart::Audio(source) => {
                serde_json::json!({ "type": "audio", "source": source.to_value() })
            }
        }
    }

    /// Whether the part is media rather than text.
    #[must_use]
    pub fn is_media(&self) -> bool {
        !matches!(self, ContentPart::Text(_))
    }
}

/// Checks a `message` or observation `content` value: a string, or (v1.6+)
/// an array of content parts. `null` is accepted where `nullable`.
fn check_content(value: &Value, nullable: bool, at: &str, errors: &mut Vec<String>) {
    match value {
        Value::String(_) => {}
        Value::Null if nullable => {}
        Value::Array(parts) => {
            for (index, part) in parts.iter().enumerate() {
                if let Err(error) = ContentPart::from_value(part) {
                    errors.push(format!("{at}[{index}]: {error}"));
                }
            }
        }
        other => errors.push(format!(
            "{at}: must be a string or an array of content parts, got {other}"
        )),
    }
}

/// Checks what this crate owns in a document: a supported
/// `schema_version`, and every step `message` and observation result
/// `content` as a string or a list of valid content parts, recursing into
/// `subagent_trajectories`. Returns every problem found; empty means none.
///
/// This is not the whole RFC. The rest of a document's shape is checked
/// against Harbor's own models by the Terminal-Bench contract test.
#[must_use]
pub fn validate(document: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    validate_into(document, "", &mut errors);
    errors
}

fn validate_into(document: &Value, at: &str, errors: &mut Vec<String>) {
    match document.get("schema_version").and_then(Value::as_str) {
        Some(version) if supported(version) => {}
        Some(version) => errors.push(format!(
            "{at}schema_version: unsupported ATIF version {version:?}"
        )),
        None => errors.push(format!("{at}schema_version: missing")),
    }
    for (index, step) in document
        .get("steps")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        if let Some(message) = step.get("message") {
            check_content(
                message,
                false,
                &format!("{at}steps[{index}].message"),
                errors,
            );
        }
        for (result_index, result) in step
            .pointer("/observation/results")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            if let Some(content) = result.get("content") {
                check_content(
                    content,
                    true,
                    &format!("{at}steps[{index}].observation.results[{result_index}].content"),
                    errors,
                );
            }
        }
    }
    for (index, child) in document
        .get("subagent_trajectories")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        validate_into(
            child,
            &format!("{at}subagent_trajectories[{index}]."),
            errors,
        );
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn this_crate_writes_the_newest_version_it_reads() {
        assert_eq!(SCHEMA_VERSION, "ATIF-v1.8");
        assert_eq!(SUPPORTED_SCHEMA_VERSIONS.last(), Some(&SCHEMA_VERSION));
    }

    #[test]
    fn every_published_1x_version_reads_and_nothing_else_does() {
        for minor in 0..=8 {
            assert!(supported(&format!("ATIF-v1.{minor}")), "v1.{minor}");
        }
        for version in ["ATIF-v1.9", "ATIF-v2.0", "ATIF-v1.7 ", "atif-v1.8", ""] {
            assert!(!supported(version), "{version:?}");
        }
    }

    #[test]
    fn a_v1_7_document_upgrades_on_read_by_relabel_alone() {
        let mut document = json!({"schema_version": "ATIF-v1.7", "steps": [{"message": "hi"}]});
        let before = document["steps"].clone();
        assert_eq!(upgrade(&mut document).as_deref(), Ok("ATIF-v1.7"));
        assert_eq!(document["schema_version"], SCHEMA_VERSION);
        assert_eq!(document["steps"], before);
        assert!(validate(&document).is_empty());
    }

    #[test]
    fn an_unknown_version_is_refused_and_left_alone() {
        let mut document = json!({"schema_version": "ATIF-v2.0"});
        assert!(upgrade(&mut document).is_err());
        assert_eq!(document["schema_version"], "ATIF-v2.0");
        assert!(upgrade(&mut json!({})).is_err());
        assert_eq!(validate(&document).len(), 1);
    }

    #[test]
    fn an_audio_part_reads_and_round_trips() {
        let value = json!({"type": "audio", "source": {
            "media_type": "audio/wav", "path": "audio/question.wav", "duration_sec": 3.2}});
        let part = ContentPart::from_value(&value).unwrap();
        assert!(part.is_media());
        assert_eq!(part.to_value(), value);
        let bare = json!({"type": "audio", "source": {"media_type": "audio/ogg", "path": "a.ogg"}});
        assert_eq!(ContentPart::from_value(&bare).unwrap().to_value(), bare);
    }

    #[test]
    fn audio_aliases_normalize_to_the_canonical_type() {
        for (alias, canonical) in AUDIO_MEDIA_TYPE_ALIASES {
            let source =
                AudioSource::from_value(&json!({"media_type": alias, "path": "a"})).unwrap();
            assert_eq!(source.media_type, canonical, "{alias}");
        }
        let shouting = AudioSource::from_value(&json!({"media_type": " AUDIO/MP3 ", "path": "a"}));
        assert_eq!(shouting.unwrap().media_type, "audio/mpeg");
        assert!(AudioSource::from_value(&json!({"media_type": "audio/pcm", "path": "a"})).is_err());
    }

    #[test]
    fn image_types_are_not_normalized() {
        assert!(ImageSource::from_value(&json!({"media_type": "image/png", "path": "p"})).is_ok());
        assert!(ImageSource::from_value(&json!({"media_type": "IMAGE/PNG", "path": "p"})).is_err());
        assert!(ImageSource::from_value(&json!({"media_type": "image/jpg", "path": "p"})).is_err());
    }

    #[test]
    fn a_source_whose_kind_disagrees_with_type_is_refused() {
        let image = json!({"media_type": "image/png", "path": "i.png"});
        let audio = json!({"media_type": "audio/wav", "path": "a.wav"});
        let error =
            ContentPart::from_value(&json!({"type": "audio", "source": image})).unwrap_err();
        assert!(error.contains("requires an audio source"), "{error}");
        let error =
            ContentPart::from_value(&json!({"type": "image", "source": audio})).unwrap_err();
        assert!(error.contains("requires an image source"), "{error}");
    }

    #[test]
    fn each_part_carries_exactly_the_field_its_type_needs() {
        let refused = [
            json!({"type": "text"}),
            json!({"type": "text", "text": "t", "source": {"media_type": "image/png", "path": "p"}}),
            json!({"type": "audio"}),
            json!({"type": "audio", "text": "t", "source": {"media_type": "audio/wav", "path": "p"}}),
            json!({"type": "video", "source": {"media_type": "video/mp4", "path": "p"}}),
            json!({"type": "text", "text": "t", "caption": "extra"}),
            json!({"type": "audio", "source": {"media_type": "audio/wav", "path": "p", "rate": 1}}),
            json!({"type": "audio", "source": {"media_type": "audio/wav", "path": "p", "duration_sec": -1}}),
            json!({"type": "audio", "source": {"media_type": "audio/wav", "path": "p", "duration_sec": "3"}}),
            json!({"type": "audio", "source": {"media_type": "audio/wav"}}),
            json!({"type": "image", "source": {"media_type": "image/png", "path": "p", "duration_sec": 1}}),
        ];
        for value in refused {
            assert!(ContentPart::from_value(&value).is_err(), "{value}");
        }
        let text = json!({"type": "text", "text": "t", "source": null});
        assert_eq!(
            ContentPart::from_value(&text),
            Ok(ContentPart::Text("t".to_string()))
        );
    }

    #[test]
    fn a_document_validates_messages_observations_and_subagents() {
        let document = json!({
            "schema_version": SCHEMA_VERSION,
            "steps": [
                {"message": [{"type": "text", "text": "Answer the recording."},
                             {"type": "audio", "source": {"media_type": "audio/mp3", "path": "q.mp3"}}]},
                {"message": "ok", "observation": {"results": [
                    {"content": null},
                    {"content": [{"type": "image", "source": {"media_type": "image/png", "path": "s.png"}}]},
                ]}},
            ],
            "subagent_trajectories": [{"schema_version": "ATIF-v1.7", "steps": [{"message": "child"}]}],
        });
        assert_eq!(validate(&document), Vec::<String>::new());

        let broken = json!({
            "schema_version": "ATIF-v1.8",
            "steps": [{"message": 7, "observation": {"results": [{"content": [{"type": "audio"}]}]}}],
            "subagent_trajectories": [{"schema_version": "ATIF-v3", "steps": []}],
        });
        let errors = validate(&broken);
        assert_eq!(errors.len(), 3, "{errors:?}");
        assert!(errors[0].starts_with("steps[0].message"));
        assert!(errors[1].starts_with("steps[0].observation.results[0].content[0]"));
        assert!(errors[2].starts_with("subagent_trajectories[0].schema_version"));
    }
}
