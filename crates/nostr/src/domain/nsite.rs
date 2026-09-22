//! NIP-5A static websites.
//!
//! Kind `15128` is one root site per pubkey and has no `d` tag. Kind
//! `35128` is a named site whose `d` tag is 1 to 13 lowercase letters,
//! digits, or hyphens and does not end in a hyphen. Kind `5128` is a
//! regular snapshot of one of those sites. Each `path` tag maps an
//! absolute file to a SHA-256. The aggregate is the SHA-256 of those
//! pairs sorted as `<hash> <path>` lines.
//!
//! The relay does not serve HTTP or fetch Blossom blobs. Kind `5128`
//! is inside the NIP-90 numeric range; admission reads it as a snapshot.
//! NIP-5A is a draft, so these kinds stay off the NIP-11 list.

use std::str::FromStr;

use sha2::{Digest, Sha256};

use super::hex::{decode_lower_hex, encode_lower_hex};
use super::{DomainError, Event, EventClass, ReplacementAddress};

const ROOT_KIND: u16 = 15_128;
const NAMED_KIND: u16 = 35_128;
const SNAPSHOT_KIND: u16 = 5_128;

/// One file a site serves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteFile {
    pub path: String,
    pub sha256: String,
}

/// A root site, a named site, or a snapshot of one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteManifest {
    pub kind: u16,
    pub identifier: Option<String>,
    pub files: Vec<SiteFile>,
    pub aggregate: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub source: Option<String>,
    pub servers: Vec<String>,
    pub apps: Vec<String>,
    pub parent: Option<String>,
    pub origin: Option<String>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn named_identifier(value: &str) -> bool {
    (1..=13).contains(&value.len())
        && !value.ends_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte.is_ascii_lowercase() || byte == b'-')
}

fn http_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
    else {
        return false;
    };
    !rest.is_empty()
        && value.len() <= 2_048
        && !value
            .chars()
            .any(|char| char.is_whitespace() || char.is_control())
}

fn relay_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("wss://")
        .or_else(|| value.strip_prefix("ws://"))
    else {
        return false;
    };
    !rest.is_empty()
        && value.len() <= 2_048
        && !value
            .chars()
            .any(|char| char.is_whitespace() || char.is_control())
}

fn source_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("nostr://"))
    else {
        return false;
    };
    !rest.is_empty()
        && value.len() <= 2_048
        && !value
            .chars()
            .any(|char| char.is_whitespace() || char.is_control())
}

fn plain(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}

fn absolute_file(path: &str) -> bool {
    if !(3..=1_024).contains(&path.len()) || !path.starts_with('/') {
        return false;
    }
    if path
        .chars()
        .any(|char| char.is_whitespace() || char.is_control() || matches!(char, '\\' | '?' | '#'))
    {
        return false;
    }
    let mut segments = path.split('/');
    if segments.next() != Some("") {
        return false;
    }
    let segments: Vec<&str> = segments.collect();
    if segments.is_empty()
        || segments
            .iter()
            .any(|segment| segment.is_empty() || *segment == "." || *segment == "..")
    {
        return false;
    }
    let Some((stem, extension)) = segments[segments.len() - 1].rsplit_once('.') else {
        return false;
    };
    !stem.is_empty()
        && !extension.is_empty()
        && extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn site_address(value: &str) -> Result<String, DomainError> {
    let address = ReplacementAddress::from_str(value)
        .map_err(|_| invalid("a site reference names a root or named site"))?;
    let recognized = match address.kind {
        ROOT_KIND => address.identifier.is_empty(),
        NAMED_KIND => named_identifier(&address.identifier),
        _ => false,
    };
    if recognized {
        Ok(value.to_owned())
    } else {
        Err(invalid("a site reference names a root or named site"))
    }
}

/// SHA-256 of the sorted `<hash> <path>` lines.
#[must_use]
pub fn site_aggregate(files: &[SiteFile]) -> String {
    let mut lines = files
        .iter()
        .map(|file| format!("{} {}\n", file.sha256, file.path))
        .collect::<Vec<_>>();
    lines.sort();
    let mut hasher = Sha256::new();
    for line in &lines {
        hasher.update(line.as_bytes());
    }
    encode_lower_hex(&hasher.finalize())
}

fn files(event: &Event) -> Result<Vec<SiteFile>, DomainError> {
    let mut files: Vec<SiteFile> = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("path")) {
        let parts = tag.as_slice();
        if parts.len() != 3
            || !absolute_file(&parts[1])
            || decode_lower_hex::<32>(&parts[2], "site file").is_err()
        {
            return Err(invalid("a site path maps an absolute file to a sha256"));
        }
        if files.iter().any(|file| file.path == parts[1]) {
            return Err(invalid("a site path is listed once"));
        }
        files.push(SiteFile {
            path: parts[1].clone(),
            sha256: parts[2].clone(),
        });
    }
    if files.is_empty() {
        return Err(invalid("a site lists one path"));
    }
    Ok(files)
}

fn published_aggregate(event: &Event, computed: &str, required: bool) -> Result<(), DomainError> {
    let tags: Vec<_> = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("x"))
        .collect();
    match (tags.as_slice(), required) {
        ([], false) => Ok(()),
        ([tag], _) => {
            let parts = tag.as_slice();
            if parts.len() == 3 && parts[1] == computed && parts[2] == "aggregate" {
                Ok(())
            } else {
                Err(invalid("a site aggregate matches the path tags"))
            }
        }
        _ => Err(invalid("a site aggregate matches the path tags")),
    }
}

fn coordinates(event: &Event, name: &str) -> Result<Vec<String>, DomainError> {
    let mut values = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some(name)) {
        if tag.as_slice().len() != 2 {
            return Err(invalid("a site reference is a coordinate"));
        }
        let Some(value) = tag.value() else {
            return Err(invalid("a site reference is a coordinate"));
        };
        values.push(site_address(value)?);
    }
    Ok(values)
}

fn lineage(event: &Event, snapshot: bool) -> Result<(Option<String>, Option<String>), DomainError> {
    let parents = coordinates(event, "a")?;
    let origins = coordinates(event, "A")?;
    if snapshot {
        let [parent] = parents.as_slice() else {
            return Err(invalid("a snapshot names the site it captures"));
        };
        let origin = match origins.as_slice() {
            [] => None,
            [origin] => Some(origin.clone()),
            _ => return Err(invalid("a snapshot names the site it captures")),
        };
        return Ok((Some(parent.clone()), origin));
    }
    match (parents.len(), origins.len()) {
        (0, 0) => Ok((None, None)),
        (1, 1) => Ok((parents.into_iter().next(), origins.into_iter().next())),
        _ => Err(invalid("a copied site names its parent and its origin")),
    }
}

fn identifier(event: &Event) -> Result<Option<String>, DomainError> {
    if event.kind == SNAPSHOT_KIND {
        if event.tags.iter().any(|tag| tag.name() == Some("d")) {
            return Err(invalid("a snapshot has no d tag"));
        }
        return Ok(None);
    }
    let tags: Vec<_> = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("d"))
        .collect();
    match (event.kind, tags.as_slice()) {
        (ROOT_KIND, []) => Ok(None),
        (NAMED_KIND, [tag]) if tag.as_slice().len() == 2 => {
            let Some(value) = tag.value().filter(|value| named_identifier(value)) else {
                return Err(invalid(
                    "a named site identifier is 1 to 13 lowercase characters",
                ));
            };
            Ok(Some(value.to_owned()))
        }
        (ROOT_KIND, _) => Err(invalid("a root site has no d tag")),
        _ => Err(invalid("a named site has one identifier")),
    }
}

fn optional_text(
    event: &Event,
    name: &str,
    max: usize,
    reason: &'static str,
) -> Result<Option<String>, DomainError> {
    let tags: Vec<_> = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect();
    match tags.as_slice() {
        [] => Ok(None),
        [tag]
            if tag.as_slice().len() == 2 && tag.value().is_some_and(|value| plain(value, max)) =>
        {
            Ok(tag.value().map(ToOwned::to_owned))
        }
        _ => Err(invalid(reason)),
    }
}

fn source(event: &Event) -> Result<Option<String>, DomainError> {
    let tags: Vec<_> = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("source"))
        .collect();
    match tags.as_slice() {
        [] => Ok(None),
        [tag] if tag.as_slice().len() == 2 && tag.value().is_some_and(source_url) => {
            Ok(tag.value().map(ToOwned::to_owned))
        }
        _ => Err(invalid("a site source is an https or nostr URL")),
    }
}

fn servers(event: &Event) -> Result<Vec<String>, DomainError> {
    let mut servers: Vec<String> = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("server")) {
        if tag.as_slice().len() != 2 {
            return Err(invalid("a blossom server is an http URL"));
        }
        let Some(value) = tag.value().filter(|value| http_url(value)) else {
            return Err(invalid("a blossom server is an http URL"));
        };
        if servers.iter().any(|server| server == value) {
            return Err(invalid("a blossom server is listed once"));
        }
        servers.push(value.to_owned());
    }
    Ok(servers)
}

fn apps(event: &Event) -> Result<Vec<String>, DomainError> {
    let mut apps: Vec<String> = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("app")) {
        let parts = tag.as_slice();
        if parts.len() != 3 || !relay_url(&parts[2]) {
            return Err(invalid("an app tag names an address and a relay"));
        }
        let address = ReplacementAddress::from_str(&parts[1])
            .map_err(|_| invalid("an app tag names an addressable event"))?;
        if EventClass::from_kind(address.kind) != EventClass::Addressable
            || address.identifier.is_empty()
        {
            return Err(invalid("an app tag names an addressable event"));
        }
        if apps.iter().any(|known| known == &parts[1]) {
            return Err(invalid("an app tag is listed once"));
        }
        apps.push(parts[1].clone());
    }
    Ok(apps)
}

fn open(event: &Event, snapshot: bool) -> Result<SiteManifest, DomainError> {
    if snapshot {
        if event.kind != SNAPSHOT_KIND {
            return Err(invalid("a site snapshot has kind 5128"));
        }
    } else if !matches!(event.kind, ROOT_KIND | NAMED_KIND) {
        return Err(invalid("a site has kind 15128 or 35128"));
    }
    let identifier = identifier(event)?;
    let files = files(event)?;
    let aggregate = site_aggregate(&files);
    published_aggregate(event, &aggregate, snapshot)?;
    let (parent, origin) = lineage(event, snapshot)?;
    Ok(SiteManifest {
        kind: event.kind,
        identifier,
        files,
        aggregate,
        title: optional_text(event, "title", 256, "a site title is one short line")?,
        description: optional_text(
            event,
            "description",
            1_024,
            "a site description is one short paragraph",
        )?,
        source: source(event)?,
        servers: servers(event)?,
        apps: apps(event)?,
        parent,
        origin,
    })
}

/// Read a kind `15128` root site or a kind `35128` named site.
pub fn open_site(event: &Event) -> Result<SiteManifest, DomainError> {
    open(event, false)
}

/// Read a kind `5128` manifest snapshot.
pub fn open_site_snapshot(event: &Event) -> Result<SiteManifest, DomainError> {
    open(event, true)
}

fn canonical_request(request: &str) -> String {
    let prefixed = if request.starts_with('/') {
        request.to_owned()
    } else {
        format!("/{request}")
    };
    if absolute_file(&prefixed) {
        return prefixed;
    }
    if prefixed.ends_with('/') {
        format!("{prefixed}index.html")
    } else {
        format!("{prefixed}/index.html")
    }
}

/// The file a host would serve for `request`, or `/404.html` when present.
#[must_use]
pub fn resolve_site_path<'a>(manifest: &'a SiteManifest, request: &str) -> Option<&'a SiteFile> {
    let path = canonical_request(request);
    manifest
        .files
        .iter()
        .find(|file| file.path == path)
        .or_else(|| manifest.files.iter().find(|file| file.path == "/404.html"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{RelaySigner, ReplacementDecision, Tag, compare_replacement};

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"5a".repeat(32)).unwrap()
    }

    fn sample_files() -> Vec<SiteFile> {
        vec![
            SiteFile {
                path: "/index.html".to_owned(),
                sha256: "186ea5fd14e88fd1ac49351759e7ab906fa94892002b60bf7f5a428f28ca1c99"
                    .to_owned(),
            },
            SiteFile {
                path: "/favicon.ico".to_owned(),
                sha256: "fedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321"
                    .to_owned(),
            },
            SiteFile {
                path: "/404.html".to_owned(),
                sha256: "40".repeat(32),
            },
        ]
    }

    fn path_tags(files: &[SiteFile], aggregate: &str) -> Vec<Tag> {
        let mut tags = files
            .iter()
            .map(|file| Tag::new(vec!["path".into(), file.path.clone(), file.sha256.clone()]))
            .collect::<Vec<_>>();
        tags.push(Tag::new(vec![
            "x".into(),
            aggregate.into(),
            "aggregate".into(),
        ]));
        tags
    }

    #[test]
    fn a_root_site_replaces_and_a_snapshot_keeps_the_aggregate() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/5A.md"
        ))
        .unwrap();
        assert!(text.contains("15128"));
        assert!(text.contains("title"));
        assert!(text.contains("35128"));
        assert!(text.contains("5128"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "5A.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "5A.md")
        );

        let author = signer();
        let files = sample_files();
        let aggregate = site_aggregate(&files);
        let mut tags = path_tags(&files, &aggregate);
        tags.push(Tag::new(vec!["title".into(), "My Nostr Site".into()]));
        tags.push(Tag::new(vec![
            "server".into(),
            "https://blossom.example.com".into(),
        ]));
        tags.push(Tag::new(vec![
            "app".into(),
            format!("31990:{}:my-app", author.pubkey()),
            "wss://relay.example.com".into(),
        ]));
        let root = author.sign(1_700_000_000, ROOT_KIND, tags, String::new());
        root.validate_structure().unwrap();
        assert_eq!(root.class(), EventClass::Replaceable);
        let opened = open_site(&root).unwrap();
        assert_eq!(opened.title.as_deref(), Some("My Nostr Site"));
        assert_eq!(opened.aggregate, aggregate);
        assert_eq!(opened.servers.len(), 1);
        assert_eq!(
            resolve_site_path(&opened, "/").map(|file| file.path.as_str()),
            Some("/index.html")
        );
        assert_eq!(
            resolve_site_path(&opened, "/missing.html").map(|file| file.path.as_str()),
            Some("/404.html")
        );

        let mut revised_tags = path_tags(&files, &aggregate);
        revised_tags.push(Tag::new(vec!["title".into(), "Renamed".into()]));
        let revised = author.sign(1_700_000_100, ROOT_KIND, revised_tags, String::new());
        revised.validate_structure().unwrap();
        assert_eq!(
            compare_replacement(&root, &revised),
            Ok(ReplacementDecision::ReplaceCurrent)
        );

        let mut rooted = path_tags(&files, &aggregate);
        rooted.push(Tag::new(vec!["d".into(), "blog".into()]));
        let with_identifier = author.sign(1_700_000_150, ROOT_KIND, rooted, String::new());
        assert!(with_identifier.validate_structure().is_err());

        let mut named_tags = path_tags(&files, &aggregate);
        named_tags.push(Tag::new(vec!["d".into(), "blog".into()]));
        named_tags.push(Tag::new(vec!["title".into(), "My Blog".into()]));
        let named = author.sign(1_700_000_200, NAMED_KIND, named_tags, String::new());
        named.validate_structure().unwrap();
        assert_eq!(named.class(), EventClass::Addressable);
        assert_eq!(
            open_site(&named).unwrap().identifier.as_deref(),
            Some("blog")
        );
        let trailing = author.sign(
            1_700_000_250,
            NAMED_KIND,
            vec![
                Tag::new(vec!["d".into(), "blog-".into()]),
                Tag::new(vec![
                    "path".into(),
                    "/index.html".into(),
                    files[0].sha256.clone(),
                ]),
            ],
            String::new(),
        );
        assert!(trailing.validate_structure().is_err());

        let parent = format!("35128:{}:blog", author.pubkey());
        let mut copied_tags = path_tags(&files, &aggregate);
        copied_tags.push(Tag::new(vec!["d".into(), "copy".into()]));
        copied_tags.push(Tag::new(vec!["a".into(), parent.clone()]));
        copied_tags.push(Tag::new(vec!["A".into(), parent.clone()]));
        let copied = author.sign(1_700_000_300, NAMED_KIND, copied_tags, String::new());
        copied.validate_structure().unwrap();
        let copy = open_site(&copied).unwrap();
        assert_eq!(copy.parent.as_deref(), Some(parent.as_str()));
        assert_eq!(copy.origin.as_deref(), Some(parent.as_str()));

        let mut snapshot_tags = path_tags(&files, &aggregate);
        snapshot_tags.push(Tag::new(vec!["a".into(), parent.clone()]));
        snapshot_tags.push(Tag::new(vec!["title".into(), "My Blog v1".into()]));
        let snapshot = author.sign(1_700_000_400, SNAPSHOT_KIND, snapshot_tags, String::new());
        snapshot.validate_structure().unwrap();
        assert_eq!(snapshot.class(), EventClass::Regular);
        let captured = open_site_snapshot(&snapshot).unwrap();
        assert_eq!(captured.aggregate, aggregate);
        assert_eq!(captured.parent.as_deref(), Some(parent.as_str()));
        assert!(matches!(
            compare_replacement(&snapshot, &snapshot),
            Err(DomainError::NotReplaceable)
        ));
        let empty = author.sign(1_700_000_450, SNAPSHOT_KIND, Vec::new(), String::new());
        assert!(empty.validate_structure().is_err());

        let zeros = "00".repeat(32);
        let mut wrong = path_tags(&files, &zeros);
        wrong.push(Tag::new(vec!["title".into(), "Broken".into()]));
        let mismatched = author.sign(1_700_000_500, ROOT_KIND, wrong, String::new());
        assert!(mismatched.validate_structure().is_err());
    }
}
