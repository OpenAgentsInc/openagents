//! NIP-96 HTTP file storage.
//!
//! A kind `10096` event lists the `https://` servers a user uploads to.
//! `parse_storage_document` reads `/.well-known/nostr/nip96.json`.
//! `parse_upload_response` reads the upload JSON, including the NIP-94
//! tags `url` and `ox`. `parse_processing_status` reads a delayed job.
//!
//! The relay does not upload, download, or delete files, and it does not
//! check a NIP-98 payload hash against file bytes. NIP-96 is
//! unrecommended, so kind `10096` is not added to the NIP-11 list.

use serde_json::Value;

use super::hex::decode_lower_hex;
#[cfg(test)]
use super::hex::encode_lower_hex;
use super::{DomainError, Event};

const SERVER_KIND: u16 = 10_096;

/// One storage plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoragePlan {
    pub name: String,
    pub nip98_required: bool,
    pub url: Option<String>,
    pub max_byte_size: Option<u64>,
    pub expiration_days: Option<(u64, u64)>,
    pub transformations: Vec<(String, Vec<String>)>,
}

/// A `nip96.json` document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageDocument {
    pub api_url: String,
    pub download_url: Option<String>,
    pub delegated_to_url: Option<String>,
    pub supported_nips: Vec<u16>,
    pub tos_url: Option<String>,
    pub content_types: Vec<String>,
    pub free: Option<StoragePlan>,
    pub plans: Vec<(String, StoragePlan)>,
}

/// The file tags returned after an upload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadedFile {
    pub url: String,
    pub original_sha256: String,
    pub sha256: Option<String>,
    pub media_type: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub content: String,
}

/// An upload JSON body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadResponse {
    pub success: bool,
    pub message: String,
    pub processing_url: Option<String>,
    pub file: Option<UploadedFile>,
}

/// A delayed-processing status body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessingStatus {
    pub failed: bool,
    pub message: String,
    pub percentage: Option<u8>,
}

/// A kind `10096` server list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileServers {
    pub servers: Vec<String>,
    pub content: String,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn https_url(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty() && value.len() <= 2_048 && !value.chars().any(char::is_whitespace)
}

fn token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&byte)
}

fn content_type_ok(value: &str) -> bool {
    let Some((top, subtype)) = value.split_once('/') else {
        return false;
    };
    !top.is_empty()
        && top.bytes().all(token_byte)
        && (subtype == "*" || (!subtype.is_empty() && subtype.bytes().all(token_byte)))
        && value.len() <= 127
        && value == value.to_ascii_lowercase()
}

fn sha256_hex(value: &str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, "stored file hash")
        .map_err(|_| invalid("a stored file hash is 32 lowercase hex bytes"))?;
    Ok(value.to_owned())
}

fn object<'a>(
    value: &'a Value,
    reason: &'static str,
) -> Result<&'a serde_json::Map<String, Value>, DomainError> {
    value.as_object().ok_or_else(|| invalid(reason))
}

fn take_string(
    map: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<String>, DomainError> {
    match map.get(key) {
        None => Ok(None),
        Some(Value::String(value)) if !value.is_empty() => Ok(Some(value.clone())),
        Some(Value::String(_)) => Ok(Some(String::new())),
        Some(_) => Err(invalid("a storage field is a string")),
    }
}

fn https_field(
    map: &serde_json::Map<String, Value>,
    key: &str,
    required: bool,
) -> Result<Option<String>, DomainError> {
    match take_string(map, key)? {
        None if required => Err(invalid("a storage URL is https://")),
        None => Ok(None),
        Some(value) if value.is_empty() && !required => Ok(Some(String::new())),
        Some(value) if https_url(&value) => Ok(Some(value)),
        Some(_) => Err(invalid("a storage URL is https://")),
    }
}

fn plan(value: &Value) -> Result<StoragePlan, DomainError> {
    let map = object(value, "a storage plan is an object")?;
    let Some(name) = take_string(map, "name")?.filter(|name| !name.is_empty() && name.len() <= 128)
    else {
        return Err(invalid("a storage plan has a name"));
    };
    let nip98_required = match map.get("is_nip98_required") {
        None => true,
        Some(Value::Bool(value)) => *value,
        Some(_) => return Err(invalid("a storage plan states whether NIP-98 is required")),
    };
    let url = match https_field(map, "url", false)? {
        Some(value) if value.is_empty() => {
            return Err(invalid("a storage URL is https://"));
        }
        other => other,
    };
    let max_byte_size = match map.get("max_byte_size") {
        None => None,
        Some(Value::Number(number)) => Some(
            number
                .as_u64()
                .ok_or_else(|| invalid("a storage plan byte size is a non-negative integer"))?,
        ),
        Some(_) => {
            return Err(invalid(
                "a storage plan byte size is a non-negative integer",
            ));
        }
    };
    let expiration_days = match map.get("file_expiration") {
        None => None,
        Some(Value::Array(items)) if items.len() == 2 => {
            let mut days = [0_u64; 2];
            for (index, item) in items.iter().enumerate() {
                days[index] = item.as_u64().ok_or_else(|| {
                    invalid("a storage plan expiration is two non-negative day counts")
                })?;
            }
            if days[1] != 0 && days[0] > days[1] {
                return Err(invalid(
                    "a storage plan expiration starts on or before it ends",
                ));
            }
            Some((days[0], days[1]))
        }
        Some(_) => {
            return Err(invalid(
                "a storage plan expiration is two non-negative day counts",
            ));
        }
    };
    let mut transformations = Vec::new();
    if let Some(value) = map.get("media_transformations") {
        let listed = object(value, "a storage transformation list is an object")?;
        for (media, names) in listed {
            if media.is_empty() || media.len() > 32 || media.chars().any(char::is_whitespace) {
                return Err(invalid("a storage transformation names a media kind"));
            }
            let Some(names) = names.as_array() else {
                return Err(invalid("a storage transformation lists names"));
            };
            let mut kept = Vec::new();
            for name in names {
                let Some(name) = name.as_str() else {
                    return Err(invalid("a storage transformation lists names"));
                };
                if name.is_empty() || name.len() > 64 || name.chars().any(char::is_whitespace) {
                    return Err(invalid("a storage transformation lists names"));
                }
                kept.push(name.to_owned());
            }
            transformations.push((media.clone(), kept));
        }
    }
    for key in map.keys() {
        if !matches!(
            key.as_str(),
            "name"
                | "is_nip98_required"
                | "url"
                | "max_byte_size"
                | "file_expiration"
                | "media_transformations"
        ) {
            return Err(invalid("a storage plan uses the documented fields"));
        }
    }
    Ok(StoragePlan {
        name,
        nip98_required,
        url,
        max_byte_size,
        expiration_days,
        transformations,
    })
}

/// Read a `nip96.json` document.
///
/// # Errors
///
/// Returns a sentence when a URL, plan, or delegated document is refused.
pub fn parse_storage_document(body: &str) -> Result<StorageDocument, DomainError> {
    let value: Value =
        serde_json::from_str(body).map_err(|_| invalid("a storage document is JSON"))?;
    let map = object(&value, "a storage document is a JSON object")?;
    if map.contains_key("delegated_to_url") {
        if map.len() != 2 || map.get("api_url").and_then(Value::as_str) != Some("") {
            return Err(invalid(
                "a delegated storage document has an empty api_url and no other fields",
            ));
        }
        let Some(url) = map
            .get("delegated_to_url")
            .and_then(Value::as_str)
            .filter(|url| https_url(url))
        else {
            return Err(invalid("a storage URL is https://"));
        };
        return Ok(StorageDocument {
            api_url: String::new(),
            download_url: None,
            delegated_to_url: Some(url.to_owned()),
            supported_nips: Vec::new(),
            tos_url: None,
            content_types: Vec::new(),
            free: None,
            plans: Vec::new(),
        });
    }
    for key in map.keys() {
        if !matches!(
            key.as_str(),
            "api_url" | "download_url" | "supported_nips" | "tos_url" | "content_types" | "plans"
        ) {
            return Err(invalid("a storage document uses the documented fields"));
        }
    }
    let Some(api_url) = https_field(map, "api_url", true)?.filter(|url| !url.is_empty()) else {
        return Err(invalid("a storage URL is https://"));
    };
    let download_url = match https_field(map, "download_url", false)? {
        Some(value) if value.is_empty() => None,
        other => other,
    };
    let tos_url = match https_field(map, "tos_url", false)? {
        Some(value) if value.is_empty() => None,
        other => other,
    };
    let mut supported_nips = Vec::new();
    if let Some(value) = map.get("supported_nips") {
        let Some(items) = value.as_array() else {
            return Err(invalid("a storage document lists NIP numbers"));
        };
        for item in items {
            let number = item
                .as_u64()
                .ok_or_else(|| invalid("a storage document lists NIP numbers"))?;
            let number = u16::try_from(number)
                .map_err(|_| invalid("a storage document lists NIP numbers"))?;
            supported_nips.push(number);
        }
    }
    let mut content_types = Vec::new();
    if let Some(value) = map.get("content_types") {
        let Some(items) = value.as_array() else {
            return Err(invalid("a storage document lists content types"));
        };
        for item in items {
            let Some(item) = item.as_str().filter(|value| content_type_ok(value)) else {
                return Err(invalid("a storage document lists content types"));
            };
            content_types.push(item.to_owned());
        }
    }
    let mut free = None;
    let mut plans = Vec::new();
    if let Some(value) = map.get("plans") {
        let listed = object(value, "a storage document lists plans")?;
        for (name, value) in listed {
            let parsed = plan(value)?;
            if name == "free" {
                free = Some(parsed);
            } else if name.is_empty() || name.len() > 64 || name.chars().any(char::is_whitespace) {
                return Err(invalid("a storage plan key is non-empty"));
            } else {
                plans.push((name.clone(), parsed));
            }
        }
    }
    Ok(StorageDocument {
        api_url,
        download_url,
        delegated_to_url: None,
        supported_nips,
        tos_url,
        content_types,
        free,
        plans,
    })
}

fn pixels(value: &str) -> Result<(u32, u32), DomainError> {
    let Some((width, height)) = value.split_once('x') else {
        return Err(invalid("a stored file dimension is <width>x<height>"));
    };
    if width.is_empty()
        || height.is_empty()
        || !width.bytes().all(|byte| byte.is_ascii_digit())
        || !height.bytes().all(|byte| byte.is_ascii_digit())
        || (width.len() > 1 && width.starts_with('0'))
        || (height.len() > 1 && height.starts_with('0'))
    {
        return Err(invalid("a stored file dimension is <width>x<height>"));
    }
    let width: u32 = width
        .parse()
        .map_err(|_| invalid("a stored file dimension is <width>x<height>"))?;
    let height: u32 = height
        .parse()
        .map_err(|_| invalid("a stored file dimension is <width>x<height>"))?;
    if width == 0 || height == 0 {
        return Err(invalid("a stored file dimension is <width>x<height>"));
    }
    Ok((width, height))
}

fn uploaded_file(value: &Value) -> Result<UploadedFile, DomainError> {
    let map = object(value, "an upload file event is an object")?;
    let Some(tags) = map.get("tags").and_then(Value::as_array) else {
        return Err(invalid("an upload file event lists tags"));
    };
    let mut url = None;
    let mut original = None;
    let mut sha256 = None;
    let mut media_type = None;
    let mut width = None;
    let mut height = None;
    for tag in tags {
        let Some(tag) = tag.as_array() else {
            return Err(invalid("an upload file tag is a list of strings"));
        };
        if tag.is_empty() || tag.iter().any(|item| !item.is_string()) {
            return Err(invalid("an upload file tag is a list of strings"));
        }
        let name = tag[0].as_str().unwrap_or_default();
        let first = tag.get(1).and_then(Value::as_str).unwrap_or_default();
        match name {
            "url" if url.is_none() && https_url(first) => url = Some(first.to_owned()),
            "ox" if original.is_none() => original = Some(sha256_hex(first)?),
            "x" if sha256.is_none() => sha256 = Some(sha256_hex(first)?),
            "m" if media_type.is_none() && content_type_ok(first) && !first.ends_with("/*") => {
                media_type = Some(first.to_owned());
            }
            "dim" if width.is_none() => {
                let (parsed_width, parsed_height) = pixels(first)?;
                width = Some(parsed_width);
                height = Some(parsed_height);
            }
            "url" | "ox" | "x" | "m" | "dim" => {
                return Err(invalid("an upload file event has one url and one ox"));
            }
            _ if !first.is_empty() => {}
            _ => return Err(invalid("an upload file tag has a value")),
        }
    }
    let Some(url) = url else {
        return Err(invalid("an upload file event has one url and one ox"));
    };
    let Some(original_sha256) = original else {
        return Err(invalid("an upload file event has one url and one ox"));
    };
    let content = match map.get("content") {
        None => String::new(),
        Some(Value::String(value)) => value.clone(),
        Some(_) => return Err(invalid("an upload file event content is a string")),
    };
    Ok(UploadedFile {
        url,
        original_sha256,
        sha256,
        media_type,
        width,
        height,
        content,
    })
}

/// Read an upload JSON body.
///
/// # Errors
///
/// Returns a sentence when the status or the file tags are refused.
pub fn parse_upload_response(body: &str) -> Result<UploadResponse, DomainError> {
    let value: Value =
        serde_json::from_str(body).map_err(|_| invalid("an upload response is JSON"))?;
    let map = object(&value, "an upload response is a JSON object")?;
    let success = match map.get("status").and_then(Value::as_str) {
        Some("success") => true,
        Some("error") => false,
        _ => return Err(invalid("an upload status is success or error")),
    };
    let Some(message) = take_string(map, "message")?.filter(|message| !message.is_empty()) else {
        return Err(invalid("an upload response has a message"));
    };
    let processing_url = match https_field(map, "processing_url", false)? {
        Some(value) if value.is_empty() || !success => {
            return Err(invalid("a storage URL is https://"));
        }
        other => other,
    };
    let file = match map.get("nip94_event") {
        None if success => return Err(invalid("a successful upload includes the file event")),
        None => None,
        Some(_) if !success => return Err(invalid("a failed upload omits the file event")),
        Some(value) => Some(uploaded_file(value)?),
    };
    Ok(UploadResponse {
        success,
        message,
        processing_url,
        file,
    })
}

/// Read a delayed-processing JSON body.
///
/// # Errors
///
/// Returns a sentence when the status or percentage is refused.
pub fn parse_processing_status(body: &str) -> Result<ProcessingStatus, DomainError> {
    let value: Value =
        serde_json::from_str(body).map_err(|_| invalid("a processing status is JSON"))?;
    let map = object(&value, "a processing status is a JSON object")?;
    let failed = match map.get("status").and_then(Value::as_str) {
        Some("processing") => false,
        Some("error") => true,
        _ => return Err(invalid("a processing status is processing or error")),
    };
    let Some(message) = take_string(map, "message")?.filter(|message| !message.is_empty()) else {
        return Err(invalid("a processing status has a message"));
    };
    let percentage = match map.get("percentage") {
        None => None,
        Some(Value::Number(number)) => {
            let number = number
                .as_u64()
                .ok_or_else(|| invalid("a processing percentage is an integer from 0 to 100"))?;
            let number = u8::try_from(number)
                .map_err(|_| invalid("a processing percentage is an integer from 0 to 100"))?;
            if number > 100 {
                return Err(invalid(
                    "a processing percentage is an integer from 0 to 100",
                ));
            }
            Some(number)
        }
        Some(_) => {
            return Err(invalid(
                "a processing percentage is an integer from 0 to 100",
            ));
        }
    };
    Ok(ProcessingStatus {
        failed,
        message,
        percentage,
    })
}

/// Read a kind `10096` file-server list.
///
/// # Errors
///
/// Returns a sentence when a server URL is missing or not `https://`.
pub fn open_file_servers(event: &Event) -> Result<FileServers, DomainError> {
    if event.kind != SERVER_KIND {
        return Err(invalid("a file server list has kind 10096"));
    }
    let mut servers = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("server")) {
        let Some(value) = tag.value().filter(|value| https_url(value)) else {
            return Err(invalid("a file server is an https:// URL"));
        };
        if servers.iter().any(|seen: &String| seen == value) {
            return Err(invalid("a file server list names each server once"));
        }
        servers.push(value.to_owned());
    }
    if servers.is_empty() {
        return Err(invalid("a file server list names an https:// URL"));
    }
    Ok(FileServers {
        servers,
        content: event.content.clone(),
    })
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::*;
    use crate::domain::{EventClass, RelaySigner, ReplacementDecision, Tag, compare_replacement};

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"96".repeat(32)).unwrap()
    }

    fn digest(bytes: &[u8]) -> String {
        encode_lower_hex(&Sha256::digest(bytes))
    }

    use super::encode_lower_hex;

    #[test]
    fn a_file_server_list_replaces_and_an_upload_keeps_the_original_hash() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/96.md"
        ))
        .unwrap();
        assert!(text.contains("10096"));
        assert!(text.contains("api_url"));
        assert!(text.contains("nip94_event"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "96.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "96.md")
        );

        let document = parse_storage_document(
            r#"{"api_url":"https://files.example/api","download_url":"https://cdn.example/files","supported_nips":[60],"content_types":["image/jpeg","audio/*"],"plans":{"free":{"name":"Free Tier","max_byte_size":10485760,"file_expiration":[14,90],"media_transformations":{"image":["resizing"]}}}}"#,
        )
        .unwrap();
        assert_eq!(document.api_url, "https://files.example/api");
        assert_eq!(document.supported_nips, vec![60]);
        assert_eq!(document.content_types[1], "audio/*");
        let free = document.free.unwrap();
        assert!(free.nip98_required);
        assert_eq!(free.max_byte_size, Some(10_485_760));
        assert_eq!(free.expiration_days, Some((14, 90)));
        let delegated =
            parse_storage_document(r#"{"api_url":"","delegated_to_url":"https://files.example"}"#)
                .unwrap();
        assert_eq!(
            delegated.delegated_to_url.as_deref(),
            Some("https://files.example")
        );
        assert!(parse_storage_document(
            r#"{"api_url":"https://files.example/api","delegated_to_url":"https://files.example"}"#
        )
        .is_err());

        let original = digest(b"original-bytes");
        let saved = digest(b"saved-bytes");
        let upload = parse_upload_response(&format!(
            r#"{{"status":"success","message":"Upload successful.","nip94_event":{{"content":"","tags":[["url","https://cdn.example/files/{original}.png"],["ox","{original}"],["x","{saved}"],["m","image/png"],["dim","800x600"]]}}}}"#
        ))
        .unwrap();
        assert!(upload.success);
        let file = upload.file.unwrap();
        assert_eq!(file.original_sha256, original);
        assert_eq!(file.sha256.as_deref(), Some(saved.as_str()));
        assert_eq!(file.width, Some(800));
        assert_eq!(file.height, Some(600));
        assert!(parse_upload_response(
            r#"{"status":"success","message":"Upload successful.","nip94_event":{"tags":[["url","https://cdn.example/file.png"]]}}"#,
        )
        .is_err());
        let processing = parse_processing_status(
            r#"{"status":"processing","message":"Processing.","percentage":15}"#,
        )
        .unwrap();
        assert!(!processing.failed);
        assert_eq!(processing.percentage, Some(15));
        assert!(
            parse_processing_status(
                r#"{"status":"processing","message":"Processing.","percentage":101}"#,
            )
            .is_err()
        );

        let author = signer();
        let list = author.sign(
            1_700_000_000,
            SERVER_KIND,
            vec![
                Tag::new(vec!["server".into(), "https://file.server.one".into()]),
                Tag::new(vec!["server".into(), "https://file.server.two".into()]),
            ],
            String::new(),
        );
        list.validate_structure().unwrap();
        assert_eq!(list.class(), EventClass::Replaceable);
        let opened = open_file_servers(&list).unwrap();
        assert_eq!(opened.servers.len(), 2);
        let newer = author.sign(
            1_700_000_100,
            SERVER_KIND,
            vec![Tag::new(vec![
                "server".into(),
                "https://file.server.one".into(),
            ])],
            String::new(),
        );
        assert_eq!(
            compare_replacement(&list, &newer).unwrap(),
            ReplacementDecision::ReplaceCurrent
        );
        let plain = author.sign(
            1_700_000_200,
            SERVER_KIND,
            vec![Tag::new(vec![
                "server".into(),
                "http://file.server.one".into(),
            ])],
            String::new(),
        );
        assert!(plain.validate_structure().is_err());
    }
}
