//! NIP-B7 Blossom media.
//!
//! A kind `10063` event is one replaceable list of Blossom servers per
//! author. A `server` tag is an `http://` or `https://` URL. When a media
//! URL the author published stops answering, a client reads that list
//! and asks each server for the same 64-character hex digest, with the
//! original file extension when the URL carried one. The client then
//! checks the downloaded bytes against that digest.
//!
//! The relay stores the list and does not download or hash any file.
//! NIP-B7 is a draft, so kind `10063` stays off the NIP-11 list. The
//! separate NIP-96 server list is kind `10096` and lives in
//! [`super::storage`].

use sha2::{Digest, Sha256};

use super::hex::decode_lower_hex;
#[cfg(test)]
use super::hex::encode_lower_hex;
use super::{DomainError, Event};

const SERVER_LIST_KIND: u16 = 10_063;

/// A kind `10063` Blossom server list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerList {
    /// The `server` tag URLs in the order the author listed them.
    pub servers: Vec<String>,
}

/// A media digest a URL names: the 64-character hex tail and the file
/// extension the URL carried, when it carried one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaReference {
    /// The lowercase hex SHA-256 the path ends with.
    pub sha256: String,
    /// The extension after that hex, when the URL has one.
    pub extension: Option<String>,
}

/// Read a kind `10063` event into a server list.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind, no `server`
/// tag, or a `server` value that is not a valid `http://` or `https://`
/// URL.
pub fn open_server_list(event: &Event) -> Result<ServerList, DomainError> {
    if event.kind != SERVER_LIST_KIND {
        return Err(DomainError::InvalidEvent(
            "a Blossom server list is kind 10063".into(),
        ));
    }
    let servers: Vec<String> = event.tag_values("server").map(str::to_string).collect();
    if servers.is_empty() || servers.iter().any(|url| !valid_http_url(url)) {
        return Err(DomainError::InvalidEvent(
            "kind 10063 requires valid http:// or https:// server tags".into(),
        ));
    }
    Ok(ServerList { servers })
}

/// The media digest a URL names, when its path ends with 64 hex
/// characters and an optional `.extension`.
///
/// The digest must be a hex-decodable 32 bytes; a URL whose tail is not
/// hex names nothing and returns `None`.
#[must_use]
pub fn media_reference(url: &str) -> Option<MediaReference> {
    let path = url.split(['?', '#']).next()?;
    let segment = path.rsplit('/').next()?;
    let (digest, extension) = match segment.split_once('.') {
        Some((digest, extension)) if !extension.is_empty() => (digest, Some(extension.to_string())),
        _ => (segment, None),
    };
    if decode_lower_hex::<32>(digest, "sha256").is_ok() {
        Some(MediaReference {
            sha256: digest.to_string(),
            extension,
        })
    } else {
        None
    }
}

/// The URL a client asks `server` for: the digest as the path, with the
/// extension when the original URL carried one.
#[must_use]
pub fn recovery_url(server: &str, media: &MediaReference) -> String {
    let base = server.trim_end_matches('/');
    match &media.extension {
        Some(extension) => format!("{base}/{}.{}", media.sha256, extension),
        None => format!("{base}/{}", media.sha256),
    }
}

/// Whether `bytes` hash to the digest the reference names. A client
/// checks every download against this before showing the media.
#[must_use]
pub fn verifies_media(media: &MediaReference, bytes: &[u8]) -> bool {
    decode_lower_hex::<32>(&media.sha256, "sha256").is_ok_and(|expected| {
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        digest == expected
    })
}

fn valid_http_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty() && value.len() <= 2_048 && !value.chars().any(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EventClass, RelaySigner, Tag};

    fn sign(kind: u16, tags: Vec<Tag>, content: &str) -> Event {
        let signer = RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap();
        signer.sign(1_700_000_000, kind, tags, content.to_string())
    }

    fn server_list(urls: &[&str]) -> Event {
        sign(
            SERVER_LIST_KIND,
            urls.iter()
                .map(|url| Tag::new(vec!["server".into(), (*url).into()]))
                .collect(),
            "",
        )
    }

    #[test]
    fn a_server_list_recovers_media_by_its_sha256() {
        let event = server_list(&["https://blossom.self.hosted", "https://cdn.blossom.cloud"]);
        let list = open_server_list(&event).unwrap();
        assert_eq!(list.servers.len(), 2);
        assert_eq!(
            EventClass::from_kind(SERVER_LIST_KIND),
            EventClass::Replaceable
        );

        let bytes = b"a downloaded image";
        let hashed: [u8; 32] = Sha256::digest(bytes).into();
        let digest = encode_lower_hex(&hashed);
        let url = format!("https://cdn.blossom.cloud/{digest}.png");
        let media = media_reference(&url).unwrap();
        assert_eq!(media.sha256, digest);
        assert_eq!(media.extension.as_deref(), Some("png"));

        // The dead host is skipped; the same digest is asked of the
        // author's other servers, extension kept.
        assert_eq!(
            recovery_url(&list.servers[0], &media),
            format!("https://blossom.self.hosted/{digest}.png")
        );
        assert!(verifies_media(&media, bytes));
        assert!(!verifies_media(&media, b"different bytes"));
    }

    #[test]
    fn a_digest_without_an_extension_recovers_the_bare_path() {
        let digest = "ab".repeat(32);
        let media = media_reference(&format!("https://host/{digest}")).unwrap();
        assert_eq!(media.extension, None);
        assert_eq!(
            recovery_url("https://mirror.example/", &media),
            format!("https://mirror.example/{digest}")
        );
        // A query or fragment after the path does not reach the digest.
        assert_eq!(
            media_reference(&format!("https://host/{digest}.jpg?v=2")),
            Some(MediaReference {
                sha256: digest.clone(),
                extension: Some("jpg".into()),
            })
        );
    }

    #[test]
    fn non_hex_tails_and_malformed_lists_are_refused() {
        assert_eq!(media_reference("https://host/readme.txt"), None);
        assert_eq!(media_reference("https://host/"), None);
        // 64 characters that are not hex name nothing.
        assert_eq!(
            media_reference(&format!("https://host/{}", "zz".repeat(32))),
            None
        );

        assert!(open_server_list(&sign(1, Vec::new(), "note")).is_err());
        assert!(open_server_list(&sign(SERVER_LIST_KIND, Vec::new(), "")).is_err());
        assert!(open_server_list(&server_list(&["ftp://nope"])).is_err());
        assert!(open_server_list(&server_list(&["https://ok.example", "not a url"])).is_err());
    }
}
