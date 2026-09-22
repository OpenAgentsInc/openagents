//! NIP-94 file metadata.
//!
//! Kind `1063` describes a shared file. `url` is where to download it,
//! `m` is its lowercase MIME type, and `x` is the SHA-256 of the bytes.
//! `ox` is the hash before a server transformation. Size, dimensions,
//! a magnet URI, a torrent infohash, a blurhash, and fallback URLs are
//! optional.
//!
//! The relay does not download the file, so it does not recompute `x`.
//! A blurhash is not decoded into pixels. Kind `1063` is a regular
//! event, so a newer record does not replace an older one. It is not
//! added to the NIP-11 list. `ox` may be omitted.

use super::hex::decode_lower_hex;
#[cfg(test)]
use super::hex::encode_lower_hex;
use super::{DomainError, Event};

const FILE_KIND: u16 = 1_063;
const BLURHASH: &[u8] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz#$%*+,-.:;=?@[]^_{|}~";

/// A kind `1063` file description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMetadata {
    pub content: String,
    pub url: String,
    pub media_type: String,
    pub sha256: String,
    pub original_sha256: Option<String>,
    pub size: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub magnet: Option<String>,
    pub infohash: Option<String>,
    pub blurhash: Option<String>,
    pub thumbnail: Option<FileImage>,
    pub preview: Option<FileImage>,
    pub summary: Option<String>,
    pub alt: Option<String>,
    pub fallbacks: Vec<String>,
    pub service: Option<String>,
}

/// A thumbnail or preview URL and its optional SHA-256.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileImage {
    pub url: String,
    pub sha256: Option<String>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn tags<'a>(event: &'a Event, name: &str) -> Vec<&'a super::Tag> {
    event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect()
}

fn http_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty() && value.len() <= 2_048 && !value.chars().any(char::is_whitespace)
}

fn media_type_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&byte)
}

fn media_type_ok(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 127
        && value == value.to_ascii_lowercase()
        && value.split_once('/').is_some_and(|(top, subtype)| {
            !top.is_empty()
                && !subtype.is_empty()
                && top.bytes().all(media_type_byte)
                && subtype.bytes().all(media_type_byte)
        })
}

fn sha256_hex(value: &str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, "file metadata hash")
        .map_err(|_| invalid("a file hash is 32 lowercase hex bytes"))?;
    Ok(value.to_owned())
}

fn one<'a>(event: &'a Event, name: &str, reason: &str) -> Result<&'a str, DomainError> {
    let found = tags(event, name);
    if found.len() != 1 {
        return Err(invalid(reason));
    }
    found[0]
        .value()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(reason))
}

fn optional<'a>(
    event: &'a Event,
    name: &str,
    reason: &str,
) -> Result<Option<&'a str>, DomainError> {
    let found = tags(event, name);
    if found.len() > 1 {
        return Err(invalid(reason));
    }
    match found.first() {
        None => Ok(None),
        Some(tag) => match tag.value() {
            Some(value) if !value.is_empty() => Ok(Some(value)),
            _ => Err(invalid(reason)),
        },
    }
}

fn byte_count(value: &str) -> Result<u64, DomainError> {
    if value.is_empty()
        || value.len() > 20
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(invalid("a file size is one unsigned integer"));
    }
    value
        .parse()
        .map_err(|_| invalid("a file size is one unsigned integer"))
}

fn pixels(value: &str) -> Result<(u32, u32), DomainError> {
    let reason = "a file dimension is <width>x<height>";
    let Some((width, height)) = value.split_once('x') else {
        return Err(invalid(reason));
    };
    if width.is_empty()
        || height.is_empty()
        || !width.bytes().all(|byte| byte.is_ascii_digit())
        || !height.bytes().all(|byte| byte.is_ascii_digit())
        || (width.len() > 1 && width.starts_with('0'))
        || (height.len() > 1 && height.starts_with('0'))
    {
        return Err(invalid(reason));
    }
    let width: u32 = width.parse().map_err(|_| invalid(reason))?;
    let height: u32 = height.parse().map_err(|_| invalid(reason))?;
    if width == 0 || height == 0 {
        return Err(invalid(reason));
    }
    Ok((width, height))
}

fn infohash_ok(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn blurhash_ok(value: &str) -> bool {
    (6..=100).contains(&value.len()) && value.bytes().all(|byte| BLURHASH.contains(&byte))
}

fn image_tag(event: &Event, name: &str) -> Result<Option<FileImage>, DomainError> {
    let found = tags(event, name);
    if found.len() > 1 {
        return Err(invalid("a file image URL is http:// or https://"));
    }
    let Some(tag) = found.first() else {
        return Ok(None);
    };
    let Some(url) = tag.value().filter(|value| http_url(value)) else {
        return Err(invalid("a file image URL is http:// or https://"));
    };
    let sha256 = match tag.as_slice().get(2) {
        None => None,
        Some(value) if value.is_empty() => None,
        Some(value) => Some(sha256_hex(value)?),
    };
    Ok(Some(FileImage {
        url: url.to_owned(),
        sha256,
    }))
}

/// Read a kind `1063` file description.
///
/// # Errors
///
/// Returns a sentence when the URL, MIME type, hash, or an optional tag is refused.
pub fn open_file_metadata(event: &Event) -> Result<FileMetadata, DomainError> {
    if event.kind != FILE_KIND {
        return Err(invalid("a file description has kind 1063"));
    }
    let url = one(event, "url", "a file URL is http:// or https://")?;
    if !http_url(url) {
        return Err(invalid("a file URL is http:// or https://"));
    }
    let media_type = one(event, "m", "a file type is one lowercase MIME type")?;
    if !media_type_ok(media_type) {
        return Err(invalid("a file type is one lowercase MIME type"));
    }
    let sha256 = sha256_hex(one(event, "x", "a file hash is 32 lowercase hex bytes")?)?;
    let original_sha256 = match optional(event, "ox", "a file hash is 32 lowercase hex bytes")? {
        None => None,
        Some(value) => Some(sha256_hex(value)?),
    };
    let size = match optional(event, "size", "a file size is one unsigned integer")? {
        None => None,
        Some(value) => Some(byte_count(value)?),
    };
    let (width, height) = match optional(event, "dim", "a file dimension is <width>x<height>")? {
        None => (None, None),
        Some(value) => {
            let (width, height) = pixels(value)?;
            (Some(width), Some(height))
        }
    };
    let magnet = match optional(event, "magnet", "a file magnet URI starts with magnet:?")? {
        None => None,
        Some(value)
            if value.starts_with("magnet:?")
                && value.len() <= 2_048
                && !value.chars().any(char::is_whitespace) =>
        {
            Some(value.to_owned())
        }
        Some(_) => return Err(invalid("a file magnet URI starts with magnet:?")),
    };
    let infohash = match optional(
        event,
        "i",
        "a torrent infohash is 40 or 64 lowercase hex characters",
    )? {
        None => None,
        Some(value) if infohash_ok(value) => Some(value.to_owned()),
        Some(_) => {
            return Err(invalid(
                "a torrent infohash is 40 or 64 lowercase hex characters",
            ));
        }
    };
    let blurhash = match optional(event, "blurhash", "a blurhash uses the blurhash alphabet")? {
        None => None,
        Some(value) if blurhash_ok(value) => Some(value.to_owned()),
        Some(_) => return Err(invalid("a blurhash uses the blurhash alphabet")),
    };
    let mut fallbacks = Vec::new();
    for tag in tags(event, "fallback") {
        let Some(value) = tag.value().filter(|value| http_url(value)) else {
            return Err(invalid("a file fallback URL is http:// or https://"));
        };
        fallbacks.push(value.to_owned());
    }
    let service = match optional(event, "service", "a file service name is non-empty")? {
        None => None,
        Some(value) if value.len() <= 128 && !value.chars().any(char::is_whitespace) => {
            Some(value.to_owned())
        }
        Some(_) => return Err(invalid("a file service name is non-empty")),
    };
    Ok(FileMetadata {
        content: event.content.clone(),
        url: url.to_owned(),
        media_type: media_type.to_owned(),
        sha256,
        original_sha256,
        size,
        width,
        height,
        magnet,
        infohash,
        blurhash,
        thumbnail: image_tag(event, "thumb")?,
        preview: image_tag(event, "image")?,
        summary: optional(event, "summary", "a file summary is non-empty")?.map(str::to_owned),
        alt: optional(event, "alt", "a file alt text is non-empty")?.map(str::to_owned),
        fallbacks,
        service,
    })
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::*;
    use crate::domain::{DomainError, EventClass, RelaySigner, Tag, compare_replacement};

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"94".repeat(32)).unwrap()
    }

    fn digest(bytes: &[u8]) -> String {
        encode_lower_hex(&Sha256::digest(bytes))
    }

    use super::encode_lower_hex;

    #[test]
    fn a_file_keeps_its_hash_and_does_not_replace() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/94.md"
        ))
        .unwrap();
        assert!(text.contains("1063"));
        assert!(text.contains("blurhash"));
        assert!(text.contains("magnet"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "94.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "94.md")
        );

        let bytes = b"file-bytes";
        let original = b"original-bytes";
        let hash = digest(bytes);
        let original_hash = digest(original);
        let infohash = "ab".repeat(20);
        let author = signer();
        let event = author.sign(
            1_700_000_000,
            FILE_KIND,
            vec![
                Tag::new(vec![
                    "url".into(),
                    "https://cdn.example/files/photo.jpg".into(),
                ]),
                Tag::new(vec!["m".into(), "image/jpeg".into()]),
                Tag::new(vec!["x".into(), hash.clone()]),
                Tag::new(vec!["ox".into(), original_hash.clone()]),
                Tag::new(vec!["size".into(), bytes.len().to_string()]),
                Tag::new(vec!["dim".into(), "640x480".into()]),
                Tag::new(vec![
                    "magnet".into(),
                    format!("magnet:?xt=urn:btih:{infohash}&dn=photo"),
                ]),
                Tag::new(vec!["i".into(), infohash.clone()]),
                Tag::new(vec![
                    "blurhash".into(),
                    "LEHV6nWB2yk8pyo0adR*.7kCMdnj".into(),
                ]),
                Tag::new(vec![
                    "thumb".into(),
                    "https://cdn.example/files/photo-thumb.jpg".into(),
                    hash.clone(),
                ]),
                Tag::new(vec![
                    "image".into(),
                    "https://cdn.example/files/photo-preview.jpg".into(),
                ]),
                Tag::new(vec!["summary".into(), "A hillside".into()]),
                Tag::new(vec!["alt".into(), "Green hill under a blue sky".into()]),
                Tag::new(vec![
                    "fallback".into(),
                    "https://mirror.example/photo.jpg".into(),
                ]),
                Tag::new(vec!["service".into(), "nip96".into()]),
            ],
            "A photo of a hill.".into(),
        );
        event.validate_structure().unwrap();
        assert_eq!(event.class(), EventClass::Regular);
        let file = open_file_metadata(&event).unwrap();
        assert_eq!(file.url, "https://cdn.example/files/photo.jpg");
        assert_eq!(file.media_type, "image/jpeg");
        assert_eq!(file.sha256, hash);
        assert_eq!(
            file.original_sha256.as_deref(),
            Some(original_hash.as_str())
        );
        assert_eq!(file.size, Some(bytes.len() as u64));
        assert_eq!(file.width, Some(640));
        assert_eq!(file.height, Some(480));
        assert_eq!(file.infohash.as_deref(), Some(infohash.as_str()));
        assert_eq!(file.fallbacks.len(), 1);
        assert_eq!(
            file.thumbnail
                .as_ref()
                .and_then(|image| image.sha256.as_deref()),
            Some(hash.as_str())
        );
        let again = author.sign(
            1_700_000_100,
            FILE_KIND,
            vec![
                Tag::new(vec![
                    "url".into(),
                    "https://cdn.example/files/other.jpg".into(),
                ]),
                Tag::new(vec!["m".into(), "image/jpeg".into()]),
                Tag::new(vec!["x".into(), hash]),
            ],
            String::new(),
        );
        assert!(matches!(
            compare_replacement(&event, &again),
            Err(DomainError::NotReplaceable)
        ));
        let upper = author.sign(
            1_700_000_200,
            FILE_KIND,
            vec![
                Tag::new(vec![
                    "url".into(),
                    "https://cdn.example/files/photo.jpg".into(),
                ]),
                Tag::new(vec!["m".into(), "Image/JPEG".into()]),
                Tag::new(vec!["x".into(), digest(bytes)]),
            ],
            String::new(),
        );
        assert!(upper.validate_structure().is_err());
        let wide = author.sign(
            1_700_000_300,
            FILE_KIND,
            vec![
                Tag::new(vec![
                    "url".into(),
                    "https://cdn.example/files/photo.jpg".into(),
                ]),
                Tag::new(vec!["m".into(), "image/jpeg".into()]),
                Tag::new(vec!["x".into(), digest(bytes)]),
                Tag::new(vec!["dim".into(), "640x".into()]),
            ],
            String::new(),
        );
        assert!(wide.validate_structure().is_err());
    }
}
