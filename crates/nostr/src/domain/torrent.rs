//! NIP-35 torrent indexes.
//!
//! Kind `2003` carries one BitTorrent v1 info hash, the file list, and
//! enough tags to build a magnet link. No torrent file is stored. Kind
//! `2004` is a comment and uses the NIP-10 `e` tag markers, with the
//! deprecated positional form still accepted.
//!
//! The info hash is not checked against torrent bytes. This crate does not
//! fetch a tracker or a media database.

use super::hex::decode_lower_hex;
use super::{DomainError, Event, Tag};

const TORRENT_KIND: u16 = 2_003;
const COMMENT_KIND: u16 = 2_004;

/// One file inside a torrent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TorrentFile {
    pub path: String,
    pub bytes: u64,
}

/// A catalog identifier from an `i` tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogId {
    Category(Vec<String>),
    Newznab(u32),
    Imdb(String),
    Tmdb { media: String, id: u64 },
    Ttvdb { media: String, id: u64 },
    Mal { media: String, id: u64 },
    Anilist { media: String, id: u64 },
}

/// A kind `2003` torrent index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Torrent {
    pub title: Option<String>,
    pub description: String,
    /// Lowercase hex or uppercase base32, the form used in `urn:btih`.
    pub info_hash: String,
    pub files: Vec<TorrentFile>,
    pub trackers: Vec<String>,
    pub catalogs: Vec<CatalogId>,
    pub topics: Vec<String>,
}

/// A kind `2004` comment on a torrent thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TorrentComment {
    pub content: String,
    /// Thread root when the comment names one.
    pub root_id: Option<String>,
    /// Event this comment replies to. For a top-level comment this is the torrent.
    pub parent_id: String,
    pub mentions: Vec<String>,
    pub participants: Vec<String>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn is_relay(value: &str) -> bool {
    (value.starts_with("ws://") || value.starts_with("wss://"))
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

fn is_tracker(value: &str) -> bool {
    (value.starts_with("udp://") || value.starts_with("http://") || value.starts_with("https://"))
        && value.len() > "udp://".len()
        && !value.chars().any(char::is_whitespace)
}

fn info_hash(value: &str) -> Result<String, DomainError> {
    let hex = value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    if hex {
        return Ok(value.to_ascii_lowercase());
    }
    let base32 = value.len() == 32
        && value
            .bytes()
            .all(|byte| matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'2'..=b'7'));
    if base32 {
        return Ok(value.to_ascii_uppercase());
    }
    Err(invalid(
        "a torrent info hash is 40 hex characters or 32 base32 characters",
    ))
}

fn file_path(value: &str) -> Result<String, DomainError> {
    if value.is_empty() || value.starts_with('/') || value.contains('\0') {
        return Err(invalid("a torrent file path is a relative path"));
    }
    if value
        .split('/')
        .any(|segment| segment.is_empty() || segment == "..")
    {
        return Err(invalid("a torrent file path is a relative path"));
    }
    Ok(value.to_owned())
}

fn catalog(value: &str) -> Result<CatalogId, DomainError> {
    if let Some(path) = value.strip_prefix("tcat:") {
        let segments: Vec<String> = path.split(',').map(str::trim).map(str::to_owned).collect();
        if segments.iter().any(String::is_empty) {
            return Err(invalid("a tcat path has no empty segments"));
        }
        return Ok(CatalogId::Category(segments));
    }
    if let Some(id) = value.strip_prefix("newznab:") {
        let id = id
            .parse::<u32>()
            .map_err(|_| invalid("a newznab category is a number"))?;
        return Ok(CatalogId::Newznab(id));
    }
    if let Some(id) = value.strip_prefix("imdb:") {
        let digits = id.strip_prefix("tt").filter(|digits| {
            !digits.is_empty() && digits.chars().all(|char| char.is_ascii_digit())
        });
        let Some(digits) = digits else {
            return Err(invalid("an imdb id is tt followed by digits"));
        };
        return Ok(CatalogId::Imdb(format!("tt{digits}")));
    }
    if let Some(rest) = value.strip_prefix("tmdb:") {
        return media_id(rest, "tmdb").map(|(media, id)| CatalogId::Tmdb { media, id });
    }
    if let Some(rest) = value.strip_prefix("ttvdb:") {
        return media_id(rest, "ttvdb").map(|(media, id)| CatalogId::Ttvdb { media, id });
    }
    if let Some(rest) = value.strip_prefix("mal:") {
        return typed_id(rest, &["anime", "manga"]).map(|(media, id)| CatalogId::Mal { media, id });
    }
    if let Some(rest) = value.strip_prefix("anilist:") {
        return media_id(rest, "anilist").map(|(media, id)| CatalogId::Anilist { media, id });
    }
    Err(invalid(
        "a torrent catalog id uses tcat, newznab, imdb, tmdb, ttvdb, mal, or anilist",
    ))
}

fn media_id(rest: &str, name: &str) -> Result<(String, u64), DomainError> {
    let Some((media, id)) = rest.split_once(':') else {
        return Err(invalid(&format!(
            "a {name} id names a media type and a number"
        )));
    };
    if media.is_empty() || !media.chars().all(|char| char.is_ascii_lowercase()) {
        return Err(invalid(&format!(
            "a {name} id names a media type and a number"
        )));
    }
    let id = id
        .parse::<u64>()
        .map_err(|_| invalid(&format!("a {name} id names a media type and a number")))?;
    Ok((media.to_owned(), id))
}

fn typed_id(rest: &str, allowed: &[&str]) -> Result<(String, u64), DomainError> {
    let (media, id) = media_id(rest, "catalog")?;
    if !allowed.contains(&media.as_str()) {
        return Err(invalid("a mal id is anime or manga"));
    }
    Ok((media, id))
}

fn one_text<'a>(tags: &'a [Tag], name: &str) -> Result<Option<&'a str>, DomainError> {
    let mut found = tags.iter().filter(|tag| tag.name() == Some(name));
    let Some(tag) = found.next() else {
        return Ok(None);
    };
    if found.next().is_some() {
        return Err(invalid("a torrent repeats a single-value tag"));
    }
    tag.value()
        .filter(|value| !value.is_empty())
        .map(Some)
        .ok_or_else(|| invalid("a torrent tag is empty"))
}

/// Read a kind `2003` torrent.
pub fn open_torrent(event: &Event) -> Result<Torrent, DomainError> {
    if event.kind != TORRENT_KIND {
        return Err(invalid("a torrent has kind 2003"));
    }
    let Some(hash) = one_text(&event.tags, "x")? else {
        return Err(invalid("a torrent requires one info hash"));
    };
    let mut files = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("file")) {
        let Some(path) = tag.value() else {
            return Err(invalid("a torrent file path is a relative path"));
        };
        let Some(size) = tag.as_slice().get(2) else {
            return Err(invalid("a torrent file size is a number of bytes"));
        };
        let bytes = size
            .parse::<u64>()
            .map_err(|_| invalid("a torrent file size is a number of bytes"))?;
        files.push(TorrentFile {
            path: file_path(path)?,
            bytes,
        });
    }
    if files.is_empty() {
        return Err(invalid("a torrent lists at least one file"));
    }
    let mut trackers = Vec::new();
    for tag in event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("tracker"))
    {
        let Some(url) = tag.value().filter(|url| is_tracker(url)) else {
            return Err(invalid("a tracker is a udp, http, or https URL"));
        };
        trackers.push(url.to_owned());
    }
    let mut catalogs = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("i")) {
        let Some(value) = tag.value() else {
            return Err(invalid(
                "a torrent catalog id uses tcat, newznab, imdb, tmdb, ttvdb, mal, or anilist",
            ));
        };
        catalogs.push(catalog(value)?);
    }
    let topics = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("t"))
        .map(|tag| {
            tag.value()
                .filter(|topic| !topic.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| invalid("a torrent topic is empty"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Torrent {
        title: one_text(&event.tags, "title")?.map(str::to_owned),
        description: event.content.clone(),
        info_hash: info_hash(hash)?,
        files,
        trackers,
        catalogs,
        topics,
    })
}

/// `magnet:?xt=urn:btih:` plus one `tr` parameter per tracker.
pub fn magnet_uri(torrent: &Torrent) -> String {
    let mut uri = format!("magnet:?xt=urn:btih:{}", torrent.info_hash);
    for tracker in &torrent.trackers {
        uri.push_str("&tr=");
        uri.push_str(&percent_encode(tracker));
    }
    uri
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(char::from(byte));
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

/// Read a kind `2004` torrent comment.
pub fn open_torrent_comment(event: &Event) -> Result<TorrentComment, DomainError> {
    if event.kind != COMMENT_KIND {
        return Err(invalid("a torrent comment has kind 2004"));
    }
    let references = event_references(&event.tags)?;
    let participants = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("p"))
        .map(|tag| {
            let Some(value) = tag.value() else {
                return Err(invalid("a torrent comment participant must be a pubkey"));
            };
            decode_lower_hex::<32>(value, "participant")
                .map_err(|_| invalid("a torrent comment participant must be a pubkey"))?;
            Ok(value.to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TorrentComment {
        content: event.content.clone(),
        root_id: references.root_id,
        parent_id: references.parent_id,
        mentions: references.mentions,
        participants,
    })
}

struct ThreadRefs {
    root_id: Option<String>,
    parent_id: String,
    mentions: Vec<String>,
}

fn event_references(tags: &[Tag]) -> Result<ThreadRefs, DomainError> {
    let mut marked = false;
    let mut positional = Vec::new();
    let mut root_id = None;
    let mut reply_id = None;
    let mut mentions = Vec::new();
    for tag in tags.iter().filter(|tag| tag.name() == Some("e")) {
        let Some(id) = tag.value() else {
            return Err(invalid(
                "a torrent comment event id is 32 lowercase hex bytes",
            ));
        };
        let id = decode_lower_hex::<32>(id, "torrent comment event")
            .map(|_| id.to_owned())
            .map_err(|_| invalid("a torrent comment event id is 32 lowercase hex bytes"))?;
        if let Some(relay) = tag.as_slice().get(2)
            && !relay.is_empty()
            && !is_relay(relay)
        {
            return Err(invalid(
                "a torrent comment relay hint must be ws:// or wss://",
            ));
        }
        if let Some(pubkey) = tag.as_slice().get(4) {
            decode_lower_hex::<32>(pubkey, "referenced author")
                .map_err(|_| invalid("a referenced author must be a pubkey"))?;
        }
        match tag.as_slice().get(3).map(String::as_str) {
            Some("root") => {
                marked = true;
                if root_id.replace(id).is_some() {
                    return Err(invalid("a torrent comment has one root"));
                }
            }
            Some("reply") => {
                marked = true;
                if reply_id.replace(id).is_some() {
                    return Err(invalid("a torrent comment has one reply parent"));
                }
            }
            Some(_) => return Err(invalid("a torrent comment marker is root or reply")),
            None => positional.push(id),
        }
    }
    if marked {
        if !positional.is_empty() {
            mentions = positional;
        }
        let parent_id = reply_id
            .clone()
            .or_else(|| root_id.clone())
            .ok_or_else(|| invalid("a torrent comment names the torrent"))?;
        return Ok(ThreadRefs {
            root_id,
            parent_id,
            mentions,
        });
    }
    match positional.len() {
        0 => Err(invalid("a torrent comment names the torrent")),
        1 => Ok(ThreadRefs {
            root_id: None,
            parent_id: positional.remove(0),
            mentions,
        }),
        _ => {
            let parent_id = positional.pop().expect("len > 1");
            let root_id = positional.remove(0);
            mentions = positional;
            Ok(ThreadRefs {
                root_id: Some(root_id),
                parent_id,
                mentions,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EventClass, RelaySigner, Tag};

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap()
    }

    fn sign(kind: u16, tags: Vec<Tag>, content: &str) -> Event {
        signer().sign(1_700_000_000, kind, tags, content.into())
    }

    #[test]
    fn a_torrent_builds_a_magnet_and_a_comment_names_it() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/35.md"
        ))
        .unwrap();
        assert!(text.contains("kind 2003"));
        assert!(text.contains("urn:btih"));
        assert!(text.contains("kind 2004"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "35.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "35.md")
        );

        let hash = "0123456789abcdef0123456789abcdef01234567";
        let torrent = sign(
            TORRENT_KIND,
            vec![
                Tag::new(vec!["title".into(), "Dune".into()]),
                Tag::new(vec!["x".into(), hash.to_ascii_uppercase()]),
                Tag::new(vec![
                    "file".into(),
                    "info/example.txt".into(),
                    "1024".into(),
                ]),
                Tag::new(vec!["file".into(), "info/more.bin".into(), "2048".into()]),
                Tag::new(vec!["tracker".into(), "udp://tracker.example:1337".into()]),
                Tag::new(vec![
                    "tracker".into(),
                    "http://1337-tracker.net/announce".into(),
                ]),
                Tag::new(vec!["i".into(), "tcat:video,movie,4k".into()]),
                Tag::new(vec!["i".into(), "newznab:2045".into()]),
                Tag::new(vec!["i".into(), "imdb:tt15239678".into()]),
                Tag::new(vec!["i".into(), "tmdb:movie:693134".into()]),
                Tag::new(vec!["i".into(), "ttvdb:movie:290272".into()]),
                Tag::new(vec!["i".into(), "mal:anime:9253".into()]),
                Tag::new(vec!["t".into(), "movie".into()]),
                Tag::new(vec!["t".into(), "4k".into()]),
            ],
            "A long description.",
        );
        torrent.validate_structure().unwrap();
        assert_eq!(torrent.class(), EventClass::Regular);
        let opened = open_torrent(&torrent).unwrap();
        assert_eq!(opened.info_hash, hash);
        assert_eq!(opened.title.as_deref(), Some("Dune"));
        assert_eq!(opened.files[0].path, "info/example.txt");
        assert_eq!(opened.files[0].bytes, 1024);
        assert_eq!(
            opened.catalogs[0],
            CatalogId::Category(vec!["video".into(), "movie".into(), "4k".into()])
        );
        assert!(matches!(opened.catalogs[2], CatalogId::Imdb(ref id) if id == "tt15239678"));
        assert!(matches!(
            opened.catalogs[3],
            CatalogId::Tmdb { ref media, id: 693134 } if media == "movie"
        ));
        let magnet = magnet_uri(&opened);
        assert!(magnet.starts_with(&format!("magnet:?xt=urn:btih:{hash}")));
        assert!(magnet.contains("&tr=udp%3A%2F%2Ftracker.example%3A1337"));
        assert!(magnet.contains("&tr=http%3A%2F%2F1337-tracker.net%2Fannounce"));

        let base32 = sign(
            TORRENT_KIND,
            vec![
                Tag::new(vec!["x".into(), "abcdefghijklmnopqrstuvwxyz234567".into()]),
                Tag::new(vec!["file".into(), "movie.mkv".into(), "1".into()]),
            ],
            "",
        );
        assert_eq!(
            open_torrent(&base32).unwrap().info_hash,
            "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"
        );

        let bad_hash = sign(
            TORRENT_KIND,
            vec![
                Tag::new(vec!["x".into(), "zz".into()]),
                Tag::new(vec!["file".into(), "movie.mkv".into(), "1".into()]),
            ],
            "",
        );
        assert!(bad_hash.validate_structure().is_err());
        let no_file = sign(
            TORRENT_KIND,
            vec![Tag::new(vec!["x".into(), hash.into()])],
            "",
        );
        assert!(no_file.validate_structure().is_err());

        let comment = sign(
            COMMENT_KIND,
            vec![
                Tag::new(vec![
                    "e".into(),
                    torrent.id.clone(),
                    "wss://relay.example".into(),
                    "root".into(),
                    signer().pubkey().into(),
                ]),
                Tag::new(vec!["p".into(), signer().pubkey().into()]),
            ],
            "Looks good.",
        );
        comment.validate_structure().unwrap();
        let comment = open_torrent_comment(&comment).unwrap();
        assert_eq!(comment.root_id.as_deref(), Some(torrent.id.as_str()));
        assert_eq!(comment.parent_id, torrent.id);
        assert_eq!(comment.participants, vec![signer().pubkey().to_owned()]);

        let positional = sign(
            COMMENT_KIND,
            vec![Tag::new(vec!["e".into(), torrent.id.clone()])],
            "positional reply",
        );
        let positional = open_torrent_comment(&positional).unwrap();
        assert_eq!(positional.parent_id, torrent.id);
        assert!(positional.root_id.is_none());

        let bare = sign(COMMENT_KIND, Vec::new(), "no target");
        assert!(bare.validate_structure().is_err());
    }
}
