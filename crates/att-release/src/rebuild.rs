//! Independent rebuild statements (kind `3203`): a builder, not the
//! publisher, rebuilt the release's source with its recipe and signs the
//! image digest it got.
use crate::Result;
use nostr::att::{Component, Release, Source};
use nostr::domain::{Event, RelaySigner, Tag};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const KIND: u16 = nostr::kinds::ATT_REBUILD;
pub const REBUILD_V: &str = "openagents.att-rebuild.v1";
pub const MARKER: &str = "oa:att-rebuild:v1";

/// The statement a builder signs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rebuild {
    pub v: String,
    pub requires: Vec<String>,
    pub workload: String,
    pub source: Source,
    /// `sha256:<64 hex>`, the image the builder produced.
    pub image_digest: String,
    #[serde(default)]
    pub components: Vec<Component>,
    pub built_at: u64,
}

fn digest_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Sign a rebuild statement as a `3203` event.
///
/// # Errors
///
/// When the statement does not encode.
pub fn event(builder: &RelaySigner, rebuild: &Rebuild) -> Result<Event> {
    let content = String::from_utf8(
        nostr::contracts::jcs(&serde_json::to_value(rebuild).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let tags = vec![
        Tag::new(vec!["t".into(), MARKER.into()]),
        Tag::new(vec!["w".into(), rebuild.workload.clone()]),
        Tag::new(vec!["x".into(), digest_hex(content.as_bytes())]),
    ];
    Ok(builder.sign(rebuild.built_at, KIND, tags, content))
}

/// Parse a `3203` event.
///
/// # Errors
///
/// When the signature, kind, marker, digest or body is wrong.
pub fn parse(event: &Event) -> Result<Rebuild> {
    event
        .validate_crypto()
        .map_err(|_| "the rebuild signature does not verify")?;
    if event.kind != KIND || event.tag_values("t").next() != Some(MARKER) {
        return Err("not a rebuild statement".into());
    }
    if event.tag_values("x").next() != Some(digest_hex(event.content.as_bytes()).as_str()) {
        return Err("the rebuild digest tag does not match".into());
    }
    let rebuild: Rebuild =
        serde_json::from_str(&event.content).map_err(|_| "the rebuild statement does not parse")?;
    if rebuild.v != REBUILD_V || !rebuild.requires.is_empty() {
        return Err("an unknown rebuild version or feature".into());
    }
    Ok(rebuild)
}

/// Check rebuild statements against a release and count the independent
/// builders whose image matches. Any listed builder whose statement shows
/// a different image refuses the release.
///
/// # Errors
///
/// On a changed build, a statement for other source, a builder not listed
/// in the release, the publisher posing as a builder, or too few matches.
pub fn check(release: &Release, statements: &[Event], minimum: usize) -> Result<usize> {
    let mut builders = std::collections::BTreeSet::new();
    for event in statements {
        let rebuild = parse(event)?;
        if event.pubkey == release.publisher {
            return Err("the publisher cannot be its own independent builder".into());
        }
        let listed = release
            .rebuilds
            .iter()
            .find(|r| r.builder == event.pubkey)
            .ok_or("the rebuild is from a builder the release does not list")?;
        if rebuild.workload != release.workload || rebuild.source != release.source {
            return Err("the rebuild is of other source than the release".into());
        }
        if rebuild.image_digest != release.image.digest || listed.digest != release.image.digest {
            return Err(
                "an independent rebuild produced a different image: the build changed".into(),
            );
        }
        if !rebuild.components.is_empty()
            && !release.components.is_empty()
            && rebuild.components != release.components
        {
            return Err("an independent rebuild produced different components".into());
        }
        builders.insert(event.pubkey.clone());
    }
    if builders.len() < minimum {
        return Err(format!(
            "{} independent rebuilds match; the policy needs {minimum}",
            builders.len()
        ));
    }
    Ok(builders.len())
}
