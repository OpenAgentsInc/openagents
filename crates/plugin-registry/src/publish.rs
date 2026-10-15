//! Curator publication: immutable objects first, mutable discovery last.
use crate::catalog::{self, Catalog, Id};
use crate::{Result, SCHEMA_BYTES};
use nostr::contracts::{digest_bytes, jcs};
use nostr::domain::{Event, RelaySigner, Tag};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::Path;

#[derive(Clone, Debug, Default)]
pub struct Previous {
    pub release: Option<Event>,
    pub listing: Option<Event>,
    pub checkpoint: Option<Event>,
    pub revocations: Vec<Event>,
}
#[derive(Clone, Debug)]
pub struct Publication {
    pub listing: Event,
    pub checkpoint: Event,
    pub events: BTreeMap<String, Event>,
    pub artifacts: BTreeMap<String, Vec<u8>>,
}

pub fn build(
    signer: &RelaySigner,
    catalog: &Catalog,
    license: &str,
    now: u64,
    previous: Previous,
) -> Result<Publication> {
    if signer.pubkey() != catalog.id().publisher() {
        return Err("curator key does not own this registry ID".into());
    }
    catalog.fresh(now, now, 9_007_199_254_740_991, 300)?;
    let bytes = jcs(catalog.body()).map_err(|e| e.to_string())?;
    let schema = SCHEMA_BYTES.to_vec();
    let definition = catalog::artifact(&schema, "application/schema+json", None);
    let manifest = json!({"v":"openagents.package.v1","requires":[],"package":catalog.id().as_str(),"version":format!("registry-{}",catalog.revision()),"license":license,"provenance":{"source":"local","receipts":[],"unknowns":["member runtime behavior"]},"components":[{"slug":"catalog","kind":"schema","definition":definition}],"files":[{"path":"registry.json","digest":digest_bytes(&bytes),"size":bytes.len(),"media_type":"application/json"},{"path":"registry.schema.json","digest":digest_bytes(&schema),"size":schema.len(),"media_type":"application/schema+json"}],"dependencies":[]});
    let manifest_bytes = jcs(&manifest).map_err(|e| e.to_string())?;
    let release_body = json!({"v":1,"requires":[],"type":"release","package":catalog.id().as_str(),"version":manifest["version"],"manifest":catalog::artifact(&manifest_bytes,"application/json",Some("openagents.package.v1"))});
    let release = match previous.release {
        Some(event) => {
            let old = nostr::ext::parse_record(&event).map_err(|e| e.to_string())?;
            if old["package"] != catalog.id().as_str() {
                return Err("previous catalog belongs to another registry".into());
            }
            if old["version"] == release_body["version"] {
                if jcs(&old).map_err(|e| e.to_string())?
                    != jcs(&release_body).map_err(|e| e.to_string())?
                {
                    return Err(
                        "registry version already has different bytes; increase its revision"
                            .into(),
                    );
                }
                event
            } else {
                sign(signer, 3184, None, &release_body, now)?
            }
        }
        None => sign(signer, 3184, None, &release_body, now)?,
    };
    let listing_body = json!({"v":1,"requires":[],"type":"listing","package":catalog.id().as_str(),"state":"published","release":catalog::reference(&release),"title":catalog.body()["title"],"description":catalog.body()["description"]});
    let listing = match previous.listing {
        Some(event) => {
            let old = nostr::ext::parse_record(&event).map_err(|e| e.to_string())?;
            if old["package"] != catalog.id().as_str() {
                return Err("previous listing belongs to another registry".into());
            }
            if jcs(&old).map_err(|e| e.to_string())?
                == jcs(&listing_body).map_err(|e| e.to_string())?
            {
                event
            } else {
                sign(
                    signer,
                    30184,
                    Some(catalog.id().slug()),
                    &listing_body,
                    now.max(event.created_at.saturating_add(1)),
                )?
            }
        }
        None => sign(signer, 30184, Some(catalog.id().slug()), &listing_body, now)?,
    };
    let mut revocations = BTreeMap::new();
    for event in previous.revocations {
        let body = nostr::ext::parse_record(&event).map_err(|e| e.to_string())?;
        if event.kind != 3185 || body["package"] != catalog.id().as_str() {
            return Err("registry revocation belongs to another package".into());
        }
        revocations.insert(event.id.clone(), event);
    }
    let (revision, at) = if let Some(old) = previous.checkpoint {
        let body = nostr::ext::parse_record(&old).map_err(|e| e.to_string())?;
        if old.kind != 30185 || body["package"] != catalog.id().as_str() {
            return Err("previous checkpoint belongs to another registry".into());
        }
        let ids = body["revocations"]
            .as_array()
            .ok_or("publisher needs the expanded revocation list")?;
        if ids
            .iter()
            .any(|v| v.as_str().is_none_or(|id| !revocations.contains_key(id)))
        {
            return Err("publication would omit a known registry revocation".into());
        }
        (
            catalog::number(&body, "revision")?
                .checked_add(1)
                .ok_or("checkpoint revision exhausted")?,
            now.max(old.created_at.saturating_add(1)),
        )
    } else {
        (1, now)
    };
    let until = catalog::number(catalog.body(), "valid_until")?;
    if until <= at {
        return Err("catalog expires before its checkpoint can be published".into());
    }
    let checkpoint = checkpoint(
        signer,
        catalog.id(),
        revision,
        at,
        until,
        &revocations.values().cloned().collect::<Vec<_>>(),
    )?;
    let mut artifacts = BTreeMap::from([
        (digest_bytes(&bytes), bytes),
        (digest_bytes(&schema), schema),
        (digest_bytes(&manifest_bytes), manifest_bytes.clone()),
    ]);
    // The same closure checks run before writing a mirror or publishing events.
    crate::catalog::from_package(&release, &manifest_bytes, &artifacts, 64 * 1024 * 1024)?;
    let mut events = revocations;
    events.insert(release.id.clone(), release);
    events.insert(listing.id.clone(), listing.clone());
    events.insert(checkpoint.id.clone(), checkpoint.clone());
    Ok(Publication {
        listing,
        checkpoint,
        events,
        artifacts: std::mem::take(&mut artifacts),
    })
}

pub fn checkpoint(
    signer: &RelaySigner,
    package: &Id,
    revision: u64,
    as_of: u64,
    valid_until: u64,
    revocations: &[Event],
) -> Result<Event> {
    if signer.pubkey() != package.publisher() || revision == 0 {
        return Err("checkpoint authority or revision is invalid".into());
    }
    let mut ids = BTreeSet::new();
    for event in revocations {
        let body = nostr::ext::parse_record(event).map_err(|e| e.to_string())?;
        if event.kind != 3185
            || event.pubkey != package.publisher()
            || body["package"] != package.as_str()
        {
            return Err("revocation has another package or publisher".into());
        }
        catalog::event_ref(&body["release"], 3184, Some(package.publisher()))?;
        ids.insert(event.id.clone());
    }
    sign(
        signer,
        30185,
        Some(package.slug()),
        &json!({"v":1,"requires":[],"type":"checkpoint","package":package.as_str(),"revision":revision,"as_of":as_of,"valid_until":valid_until,"revocations":ids.into_iter().collect::<Vec<_>>()}),
        as_of,
    )
}
pub fn hidden(signer: &RelaySigner, package: &Id, now: u64) -> Result<Event> {
    if signer.pubkey() != package.publisher() {
        return Err("curator key does not own the registry".into());
    }
    sign(
        signer,
        30184,
        Some(package.slug()),
        &json!({"v":1,"requires":[],"type":"listing","package":package.as_str(),"state":"hidden","release":null,"title":"","description":""}),
        now,
    )
}
pub fn sign(
    signer: &RelaySigner,
    kind: u16,
    slug: Option<&str>,
    body: &Value,
    at: u64,
) -> Result<Event> {
    let mut tags = Vec::new();
    if let Some(slug) = slug {
        tags.push(Tag::new(vec!["d".into(), slug.into()]));
    }
    tags.push(Tag::new(vec![
        "t".into(),
        format!("oa:ext:{}:v1", catalog::text(body, "type")?),
    ]));
    if kind == 3185 {
        tags.push(Tag::new(vec![
            "e".into(),
            catalog::event_ref(&body["release"], 3184, Some(signer.pubkey()))?,
        ]));
    }
    let content =
        String::from_utf8(jcs(body).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let event = signer.sign(at, kind, tags, content);
    nostr::ext::parse_record(&event).map_err(|e| e.to_string())?;
    Ok(event)
}

/// Write immutable event/artifact files before switching discovery and heads.
pub fn mirror(root: &Path, publication: &Publication) -> Result<()> {
    let _lock = mirror_lock(root)?;
    let listing_body = nostr::ext::parse_record(&publication.listing).map_err(|e| e.to_string())?;
    let id = Id::parse(catalog::text(&listing_body, "package")?)?;
    let release_id = catalog::event_ref(&listing_body["release"], 3184, Some(id.publisher()))?;
    let release = publication
        .events
        .get(&release_id)
        .ok_or("publication omits the catalog release")?;
    catalog::matches_event(
        &listing_body["release"],
        release,
        3184,
        Some(id.publisher()),
    )?;
    let release_body = nostr::ext::parse_record(release).map_err(|e| e.to_string())?;
    let reference =
        nostr::contracts::parse_artifact(&release_body["manifest"]).map_err(|e| e.to_string())?;
    let manifest = publication
        .artifacts
        .get(&reference.digest)
        .ok_or("publication omits its manifest")?;
    let catalog =
        catalog::from_package(release, manifest, &publication.artifacts, 512 * 1024 * 1024)?;
    if catalog.id() != &id {
        return Err("publication has another catalog identity".into());
    }
    let checkpoint_body =
        nostr::ext::parse_record(&publication.checkpoint).map_err(|e| e.to_string())?;
    if publication.checkpoint.kind != 30185 || checkpoint_body["package"] != id.as_str() {
        return Err("publication checkpoint has another identity".into());
    }
    let old_path = root.join(format!("heads/30184/{}/{}.json", id.publisher(), id.slug()));
    if old_path.exists() {
        let old: Event = serde_json::from_slice(&read(&old_path, crate::CATALOG_LIMIT)?)
            .map_err(|e| e.to_string())?;
        let body = nostr::ext::parse_record(&old).map_err(|e| e.to_string())?;
        if body["state"] == "published" {
            let old_id = catalog::event_ref(&body["release"], 3184, Some(id.publisher()))?;
            let old_release: Event = serde_json::from_slice(&read(
                &root.join(format!("events/{old_id}.json")),
                crate::CATALOG_LIMIT,
            )?)
            .map_err(|e| e.to_string())?;
            let new_id = catalog::event_ref(&listing_body["release"], 3184, Some(id.publisher()))?;
            let new_release = publication
                .events
                .get(&new_id)
                .ok_or("publication omits its catalog release")?;
            let old_body = nostr::ext::parse_record(&old_release).map_err(|e| e.to_string())?;
            let new_body = nostr::ext::parse_record(new_release).map_err(|e| e.to_string())?;
            let revision = |b: &Value| -> Result<u64> {
                catalog::text(b, "version")?
                    .strip_prefix("registry-")
                    .ok_or("catalog version")?
                    .parse::<u64>()
                    .map_err(|e| e.to_string())
            };
            if revision(&new_body)? < revision(&old_body)?
                || revision(&new_body)? == revision(&old_body)? && new_id != old_id
            {
                return Err("mirror catalog revision would roll back or conflict".into());
            }
        }
    }
    for (digest, bytes) in &publication.artifacts {
        let hash = digest.strip_prefix("sha256:").ok_or("artifact digest")?;
        catalog::hex(hash)?;
        if digest_bytes(bytes) != *digest {
            return Err("publication artifact has another digest".into());
        }
        immutable(&root.join(format!("artifacts/sha256/{hash}")), bytes)?;
    }
    for (id, event) in &publication.events {
        event.validate_crypto().map_err(|e| e.to_string())?;
        if id != &event.id {
            return Err("publication event has another ID".into());
        }
        immutable(
            &root.join(format!("events/{id}.json")),
            &serde_json::to_vec(event).map_err(|e| e.to_string())?,
        )?;
    }
    for event in [&publication.checkpoint, &publication.listing] {
        write_head_inner(root, event)?;
    }
    atomic(
        &root.join("registry.json"),
        &serde_json::to_vec(&publication.listing).map_err(|e| e.to_string())?,
    )
}
fn mirror_lock(root: &Path) -> Result<std::fs::File> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root.join(".publish.lock"))
        .map_err(|e| e.to_string())?;
    fs2::FileExt::try_lock_exclusive(&file)
        .map_err(|_| "another mirror publication is in progress")?;
    Ok(file)
}
pub fn write_head(root: &Path, event: &Event) -> Result<()> {
    let _lock = mirror_lock(root)?;
    write_head_inner(root, event)
}
fn write_head_inner(root: &Path, event: &Event) -> Result<()> {
    let body = nostr::ext::parse_record(event).map_err(|e| e.to_string())?;
    let package = Id::parse(catalog::text(&body, "package")?)?;
    if !matches!(event.kind, 30184 | 30185) {
        return Err("event is not an EXT discovery/checkpoint head".into());
    }
    let path = root.join(format!(
        "heads/{}/{}/{}.json",
        event.kind,
        package.publisher(),
        package.slug()
    ));
    if path.exists() {
        let old: Event = serde_json::from_slice(&read(&path, crate::CATALOG_LIMIT)?)
            .map_err(|_| "old mirror head is malformed")?;
        let old_body = nostr::ext::parse_record(&old).map_err(|e| e.to_string())?;
        if old.kind != event.kind || old_body["package"] != body["package"] {
            return Err("mirror head has another identity".into());
        }
        if event.kind == 30185 {
            let old_revision = catalog::number(&old_body, "revision")?;
            let revision = catalog::number(&body, "revision")?;
            if revision < old_revision
                || revision == old_revision && old_body["revocations"] != body["revocations"]
            {
                return Err("mirror checkpoint revision would roll back or conflict".into());
            }
            if let (Some(old_ids), Some(ids)) = (
                old_body["revocations"].as_array(),
                body["revocations"].as_array(),
            ) {
                if old_ids.iter().any(|id| !ids.contains(id)) {
                    return Err("mirror checkpoint would omit a known revocation".into());
                }
            }
        }
        if newer(&old, event) {
            return Err("mirror head would roll back".into());
        }
    }
    atomic(
        &path,
        &serde_json::to_vec(event).map_err(|e| e.to_string())?,
    )
}
pub fn newer(left: &Event, right: &Event) -> bool {
    left.created_at > right.created_at || left.created_at == right.created_at && left.id < right.id
}
pub(crate) fn atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.error.to_string())?;
    #[cfg(unix)]
    std::fs::File::open(parent)
        .and_then(|d| d.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(())
}
pub(crate) fn read(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return Err("registry file is not a bounded regular file".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("registry file exceeds its byte bound".into());
    }
    Ok(bytes)
}
fn immutable(path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() {
        if read(path, bytes.len())? != bytes {
            return Err("immutable registry mirror object differs".into());
        }
        return Ok(());
    }
    atomic(path, bytes)
}
