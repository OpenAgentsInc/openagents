//! NIP-KB v1: shared knowledge entries (`nips/openagents/NIP-KB.md`).
//!
//! A `3190` event is one immutable version of one entry, a `30190` head
//! points at the author's current version, a `3191` withdraws one exact
//! version, and evidence is a NIP-EVAL `3189` publication whose subject is
//! an entry version. This module builds the unsigned parts of each and
//! checks signed ones: signature, body, and tag agreement. Parsing the
//! entry document itself, and deciding whom to trust, belong to the reader.

use serde_json::{Map, Value, json};

use crate::contracts::{
    ArtifactRef, ContractError, DefinitionRef, RefusalCode, check_artifact_bytes, digest_bytes,
    parse_artifact, parse_definition, parse_strict,
};
use crate::domain::{Event, Tag};

/// One immutable entry version.
pub const ENTRY_KIND: u16 = 3_190;
/// The author's current version of one entry.
pub const HEAD_KIND: u16 = 30_190;
/// Irreversible withdrawal of one entry version.
pub const WITHDRAWAL_KIND: u16 = 3_191;
/// A NIP-EVAL public evaluation declaration.
pub const EVIDENCE_KIND: u16 = 3_189;

/// The kinds an entry can be.
pub const ENTRY_TYPES: &[&str] = &["method", "edge-case", "slip", "environment", "tool"];

/// The schema an entry document's ArtifactRef names.
pub const ENTRY_SCHEMA: &str = "openagents.kb-entry.v1";
/// The schema of the evidence report.
pub const REPORT_SCHEMA: &str = "openagents.eval-report.v1";
/// The body version of a NIP-EVAL publication.
pub const PUBLICATION_VERSION: &str = "openagents.eval-publication.v1";
/// The NIP-EVAL publication marker.
pub const EVAL_MARKER: &str = "oa:eval:v1";

/// Characters a withdrawal reason may have, at most.
pub const MAX_REASON_CHARS: usize = 1_000;

/// An event's kind, tags, and content, ready for a signer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsigned {
    pub kind: u16,
    pub tags: Vec<Tag>,
    pub content: String,
}

/// A verified `3190`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryVersion {
    /// The entry ID.
    pub id: String,
    pub version: u64,
    /// `method`, `edge-case`, `slip`, `environment`, or `tool`.
    pub kind: String,
    /// The whole entry file.
    pub document: String,
    /// Lowercase hex SHA-256 of the document, as the `x` tag carries it.
    pub digest: String,
    /// The topic `t` tags.
    pub topics: Vec<String>,
}

/// An exact `3190` a head or withdrawal names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pointer {
    /// The event ID.
    pub id: String,
    pub pubkey: String,
}

/// A verified `30190`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    pub id: String,
    pub version: u64,
    pub entry: Pointer,
}

/// A verified `3191`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Withdrawal {
    pub id: String,
    pub version: u64,
    pub entry: Pointer,
    pub reason: String,
}

/// A verified `3189` whose subject is an entry version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    pub report: ArtifactRef,
    /// The report's exact bytes, checked against `report`.
    pub report_bytes: String,
    pub subject: DefinitionRef,
    /// The `3190` event IDs the `e` tags name.
    pub entries: Vec<String>,
}

/// Whether `id` is an entry ID: a lowercase letter, then up to 127
/// lowercase letters, digits, dots, and hyphens.
#[must_use]
pub fn valid_entry_id(id: &str) -> bool {
    id.len() <= 128
        && id.starts_with(|c: char| c.is_ascii_lowercase())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
}

/// The qualified component ID shared contracts use for an entry:
/// `<pubkey>:kb/<slug>`, the slug being the ID with `.` replaced by `_`.
#[must_use]
pub fn qualified_id(pubkey: &str, entry_id: &str) -> String {
    format!("{pubkey}:kb/{}", entry_id.replace('.', "_"))
}

/// The ArtifactRef of an entry document's exact bytes.
#[must_use]
pub fn document_artifact(document: &str) -> Value {
    json!({
        "digest": digest_bytes(document.as_bytes()),
        "size": document.len(),
        "media_type": "text/markdown",
        "schema": ENTRY_SCHEMA,
    })
}

/// The parts of a `3190` for one entry version.
///
/// # Errors
///
/// A bad ID, a version of zero, an unknown kind, an empty document, or a
/// topic that isn't a lowercase tag.
pub fn entry(
    id: &str,
    version: u64,
    kind: &str,
    topics: &[String],
    document: &str,
) -> Result<Unsigned, ContractError> {
    check_identity(id, version)?;
    if !ENTRY_TYPES.contains(&kind) {
        return Err(unsupported("kind"));
    }
    if document.is_empty() {
        return Err(malformed("document"));
    }
    let mut tags = vec![
        tag(&["d", id]),
        tag(&["x", &hex_digest(document)]),
        tag(&["t", "oa:kb:entry:v1"]),
        tag(&["t", &format!("oa:kb:kind:{kind}")]),
    ];
    for topic in topics {
        if !valid_topic(topic) {
            return Err(malformed(format!("topic {topic}")));
        }
        tags.push(tag(&["t", topic]));
    }
    let content = json!({
        "v": 1, "requires": [], "type": "entry",
        "id": id, "version": version, "kind": kind, "document": document,
    });
    Ok(Unsigned {
        kind: ENTRY_KIND,
        tags,
        content: content.to_string(),
    })
}

/// The parts of a `30190` pointing at the signed `3190` `entry`.
///
/// # Errors
///
/// When `entry` isn't a valid `3190`.
pub fn head(entry: &Event) -> Result<Unsigned, ContractError> {
    let version = parse_entry(entry)?;
    let content = json!({
        "v": 1, "requires": [], "type": "head",
        "id": version.id, "version": version.version,
        "entry": {"id": entry.id, "pubkey": entry.pubkey, "kind": ENTRY_KIND},
    });
    Ok(Unsigned {
        kind: HEAD_KIND,
        tags: vec![
            tag(&["d", &version.id]),
            tag(&["e", &entry.id]),
            tag(&["t", "oa:kb:head:v1"]),
        ],
        content: content.to_string(),
    })
}

/// The parts of a `3191` withdrawing the signed `3190` `entry`.
///
/// # Errors
///
/// When `entry` isn't a valid `3190` or the reason is too long.
pub fn withdrawal(entry: &Event, reason: &str) -> Result<Unsigned, ContractError> {
    let version = parse_entry(entry)?;
    if reason.chars().count() > MAX_REASON_CHARS {
        return Err(ContractError::new(RefusalCode::LimitExceeded, "reason"));
    }
    let content = json!({
        "v": 1, "requires": [], "type": "withdrawal",
        "id": version.id, "version": version.version,
        "entry": {"id": entry.id, "pubkey": entry.pubkey, "kind": ENTRY_KIND},
        "reason": reason,
    });
    Ok(Unsigned {
        kind: WITHDRAWAL_KIND,
        tags: vec![
            tag(&["d", &version.id]),
            tag(&["e", &entry.id]),
            tag(&["t", "oa:kb:withdrawal:v1"]),
        ],
        content: content.to_string(),
    })
}

/// The parts of a `3189` publishing the evaluation report `report` (its
/// exact bytes) about the entry versions `entries` (their `3190` event
/// IDs). The report's `subject.definition` is the publication's subject.
///
/// # Errors
///
/// When the report isn't JSON, has no subject definition, or names no
/// entry.
pub fn evidence(report: &str, entries: &[String]) -> Result<Unsigned, ContractError> {
    let parsed = parse_strict(report.as_bytes())?;
    let subject = parsed
        .get("subject")
        .and_then(|s| s.get("definition"))
        .cloned()
        .ok_or_else(|| malformed("report.subject.definition"))?;
    parse_definition(&subject)?;
    if entries.is_empty() {
        return Err(malformed("entries"));
    }
    let digest = digest_bytes(report.as_bytes());
    let content = json!({
        "v": PUBLICATION_VERSION,
        "requires": [],
        "report": {
            "digest": digest,
            "size": report.len(),
            "media_type": "application/json",
            "schema": REPORT_SCHEMA,
        },
        "subject": subject,
        "supersedes": [],
        "meta": {"kb_report": report},
    });
    let mut tags = vec![
        tag(&["t", EVAL_MARKER]),
        tag(&["x", digest.trim_start_matches("sha256:")]),
    ];
    for id in entries {
        tags.push(tag(&["e", id]));
    }
    Ok(Unsigned {
        kind: EVIDENCE_KIND,
        tags,
        content: content.to_string(),
    })
}

/// Checks a signed `3190` and returns the version it holds.
///
/// # Errors
///
/// A bad signature, a body this version doesn't implement, or a tag that
/// disagrees with the body.
pub fn parse_entry(event: &Event) -> Result<EntryVersion, ContractError> {
    let object = open(event, ENTRY_KIND, "entry")?;
    reject(
        &object,
        &["v", "requires", "type", "id", "version", "kind", "document"],
    )?;
    let id = text(&object, "id")?;
    let version = number(&object, "version")?;
    check_identity(&id, version)?;
    let kind = text(&object, "kind")?;
    if !ENTRY_TYPES.contains(&kind.as_str()) {
        return Err(unsupported("kind"));
    }
    let document = text(&object, "document")?;
    if document.is_empty() {
        return Err(malformed("document"));
    }
    if one_tag(event, "d")? != id {
        return Err(mismatch("d tag"));
    }
    let digest = hex_digest(&document);
    if one_tag(event, "x")? != digest {
        return Err(mismatch("x tag"));
    }
    let kinds: Vec<&str> = t_values(event)
        .filter_map(|t| t.strip_prefix("oa:kb:kind:"))
        .collect();
    if kinds != [kind.as_str()] {
        return Err(mismatch("kind tag"));
    }
    let topics = t_values(event)
        .filter(|t| !t.starts_with("oa:"))
        .map(str::to_string)
        .collect();
    Ok(EntryVersion {
        id,
        version,
        kind,
        document,
        digest,
        topics,
    })
}

/// Checks a signed `30190`.
///
/// # Errors
///
/// A bad signature or body, a tag that disagrees with the body, or an
/// entry signed by someone else.
pub fn parse_head(event: &Event) -> Result<Head, ContractError> {
    let object = open(event, HEAD_KIND, "head")?;
    reject(
        &object,
        &["v", "requires", "type", "id", "version", "entry"],
    )?;
    let (id, version, entry) = pointed(event, &object)?;
    Ok(Head { id, version, entry })
}

/// Checks a signed `3191`.
///
/// # Errors
///
/// As [`parse_head`], and a reason that is too long.
pub fn parse_withdrawal(event: &Event) -> Result<Withdrawal, ContractError> {
    let object = open(event, WITHDRAWAL_KIND, "withdrawal")?;
    reject(
        &object,
        &["v", "requires", "type", "id", "version", "entry", "reason"],
    )?;
    let (id, version, entry) = pointed(event, &object)?;
    let reason = text(&object, "reason")?;
    if reason.chars().count() > MAX_REASON_CHARS {
        return Err(ContractError::new(RefusalCode::LimitExceeded, "reason"));
    }
    Ok(Withdrawal {
        id,
        version,
        entry,
        reason,
    })
}

/// Checks that a head or withdrawal's pointer names exactly this signed
/// `3190`: its event ID, its author, its entry ID, and its version.
///
/// # Errors
///
/// [`RefusalCode::IdentityMismatch`] on any difference.
pub fn bind(
    pointer: &Pointer,
    id: &str,
    version: u64,
    entry: &Event,
) -> Result<EntryVersion, ContractError> {
    let parsed = parse_entry(entry)?;
    if pointer.id != entry.id
        || pointer.pubkey != entry.pubkey
        || parsed.id != id
        || parsed.version != version
    {
        return Err(mismatch("entry pointer"));
    }
    Ok(parsed)
}

/// Refuses two versions from one author with the same ID and version but
/// different documents.
///
/// # Errors
///
/// [`RefusalCode::Conflict`] on equivocation.
pub fn equivocation(left: &EntryVersion, right: &EntryVersion) -> Result<(), ContractError> {
    if left.id == right.id && left.version == right.version && left.digest != right.digest {
        return Err(ContractError::new(
            RefusalCode::Conflict,
            format!("{} version {} has two documents", left.id, left.version),
        ));
    }
    Ok(())
}

/// Checks a signed `3189` under this profile: the NIP-EVAL envelope, the
/// inline report against its digest, the signer as the report's evaluator,
/// and a subject that is a `3190` the `e` tags name.
///
/// # Errors
///
/// A typed refusal naming the first check that failed.
pub fn parse_evidence(event: &Event) -> Result<Evidence, ContractError> {
    if event.kind != EVIDENCE_KIND {
        return Err(mismatch("kind"));
    }
    event
        .validate_crypto()
        .map_err(|_| mismatch("event signature"))?;
    let markers: Vec<&str> = t_values(event).filter(|t| t.starts_with("oa:")).collect();
    if markers != [EVAL_MARKER] {
        return Err(mismatch("eval tag"));
    }
    let value = parse_strict(event.content.as_bytes())?;
    let object = value
        .as_object()
        .ok_or_else(|| malformed("publication"))?
        .clone();
    reject(
        &object,
        &["v", "requires", "report", "subject", "supersedes", "meta"],
    )?;
    if object.get("v").and_then(Value::as_str) != Some(PUBLICATION_VERSION) {
        return Err(ContractError::new(RefusalCode::UnsupportedVersion, "v"));
    }
    requires_empty(&object)?;
    let report = parse_artifact(require(&object, "report")?)?;
    if one_tag(event, "x")? != report.digest.trim_start_matches("sha256:") {
        return Err(mismatch("x tag"));
    }
    let subject_value = require(&object, "subject")?;
    let subject = parse_definition(subject_value)?;
    if !require(&object, "supersedes")?.is_array() {
        return Err(malformed("supersedes"));
    }
    let report_bytes = object
        .get("meta")
        .and_then(|m| m.get("kb_report"))
        .and_then(Value::as_str)
        .ok_or_else(|| ContractError::new(RefusalCode::ContentUnavailable, "meta.kb_report"))?
        .to_string();
    check_artifact_bytes(&report, report_bytes.as_bytes())?;
    let parsed = parse_strict(report_bytes.as_bytes())?;
    if parsed.get("evaluator").and_then(Value::as_str) != Some(event.pubkey.as_str()) {
        return Err(mismatch("evaluator"));
    }
    if parsed.get("subject").and_then(|s| s.get("definition")) != Some(subject_value) {
        return Err(mismatch("report subject"));
    }
    let entries: Vec<String> = event
        .tags
        .iter()
        .filter(|t| t.name() == Some("e"))
        .filter_map(Tag::value)
        .map(str::to_string)
        .collect();
    let named = subject
        .event
        .as_ref()
        .ok_or_else(|| malformed("subject.event"))?;
    if named.kind != ENTRY_KIND || !entries.contains(&named.id) {
        return Err(mismatch("subject event"));
    }
    Ok(Evidence {
        report,
        report_bytes,
        subject,
        entries,
    })
}

fn open(event: &Event, kind: u16, record: &str) -> Result<Map<String, Value>, ContractError> {
    if event.kind != kind {
        return Err(mismatch("kind"));
    }
    event
        .validate_crypto()
        .map_err(|_| mismatch("event signature"))?;
    let markers: Vec<&str> = t_values(event)
        .filter(|t| t.starts_with("oa:kb:") && !t.starts_with("oa:kb:kind:"))
        .collect();
    if markers != [format!("oa:kb:{record}:v1").as_str()] {
        return Err(mismatch("kb tag"));
    }
    if t_values(event).any(|t| t.chars().any(char::is_uppercase)) {
        return Err(malformed("t tag"));
    }
    let value = parse_strict(event.content.as_bytes())?;
    let object = value.as_object().ok_or_else(|| malformed(record))?.clone();
    if object.get("v").and_then(Value::as_u64) != Some(1) {
        return Err(ContractError::new(RefusalCode::UnsupportedVersion, "v"));
    }
    requires_empty(&object)?;
    if object.get("type").and_then(Value::as_str) != Some(record) {
        return Err(mismatch("type"));
    }
    Ok(object)
}

fn pointed(
    event: &Event,
    object: &Map<String, Value>,
) -> Result<(String, u64, Pointer), ContractError> {
    let id = text(object, "id")?;
    let version = number(object, "version")?;
    check_identity(&id, version)?;
    let entry = require(object, "entry")?
        .as_object()
        .ok_or_else(|| malformed("entry"))?;
    reject(entry, &["id", "pubkey", "kind"])?;
    let pointer = Pointer {
        id: text(entry, "id")?,
        pubkey: text(entry, "pubkey")?,
    };
    if !is_hex(&pointer.id) || !is_hex(&pointer.pubkey) {
        return Err(malformed("entry"));
    }
    if entry.get("kind").and_then(Value::as_u64) != Some(u64::from(ENTRY_KIND)) {
        return Err(mismatch("entry.kind"));
    }
    if pointer.pubkey != event.pubkey {
        return Err(mismatch("entry author"));
    }
    if one_tag(event, "d")? != id {
        return Err(mismatch("d tag"));
    }
    if one_tag(event, "e")? != pointer.id {
        return Err(mismatch("e tag"));
    }
    Ok((id, version, pointer))
}

fn check_identity(id: &str, version: u64) -> Result<(), ContractError> {
    if !valid_entry_id(id) {
        return Err(malformed("id"));
    }
    if version == 0 {
        return Err(malformed("version"));
    }
    Ok(())
}

fn valid_topic(topic: &str) -> bool {
    !topic.is_empty()
        && !topic.starts_with("oa:")
        && !topic.chars().any(|c| c.is_uppercase() || c.is_whitespace())
}

fn hex_digest(document: &str) -> String {
    digest_bytes(document.as_bytes())
        .trim_start_matches("sha256:")
        .to_string()
}

pub(crate) fn tag(values: &[&str]) -> Tag {
    Tag::new(values.iter().map(|v| (*v).to_string()).collect())
}

pub(crate) fn t_values(event: &Event) -> impl Iterator<Item = &str> {
    event
        .tags
        .iter()
        .filter(|t| t.name() == Some("t"))
        .filter_map(Tag::value)
}

pub(crate) fn one_tag<'a>(event: &'a Event, name: &str) -> Result<&'a str, ContractError> {
    let values: Vec<&str> = event
        .tags
        .iter()
        .filter(|t| t.name() == Some(name))
        .filter_map(Tag::value)
        .collect();
    match values.as_slice() {
        [one] => Ok(one),
        _ => Err(malformed(format!("{name} tag"))),
    }
}

pub(crate) fn require<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a Value, ContractError> {
    object.get(key).ok_or_else(|| malformed(key))
}

pub(crate) fn text(object: &Map<String, Value>, key: &str) -> Result<String, ContractError> {
    require(object, key)?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| malformed(key))
}

pub(crate) fn number(object: &Map<String, Value>, key: &str) -> Result<u64, ContractError> {
    require(object, key)?.as_u64().ok_or_else(|| malformed(key))
}

pub(crate) fn requires_empty(object: &Map<String, Value>) -> Result<(), ContractError> {
    match require(object, "requires")?.as_array() {
        Some(items) if items.is_empty() => Ok(()),
        Some(_) => Err(unsupported("requires")),
        None => Err(malformed("requires")),
    }
}

pub(crate) fn reject(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), ContractError> {
    match object.keys().find(|k| !allowed.contains(&k.as_str())) {
        Some(key) => Err(unsupported(key.clone())),
        None => Ok(()),
    }
}

pub(crate) fn is_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub(crate) fn malformed(detail: impl Into<String>) -> ContractError {
    ContractError::new(RefusalCode::Malformed, detail)
}

pub(crate) fn unsupported(detail: impl Into<String>) -> ContractError {
    ContractError::new(RefusalCode::UnsupportedFeature, detail)
}

pub(crate) fn mismatch(detail: impl Into<String>) -> ContractError {
    ContractError::new(RefusalCode::IdentityMismatch, detail)
}

#[cfg(test)]
mod tests;
