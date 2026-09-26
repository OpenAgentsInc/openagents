//! Immutable NIP-EXT knowledge snapshots. Loading verifies a complete, inert
//! guidance closure; the publisher's admission claim is not the reader's grant.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::Path;

use nostr::contracts::{self, ArtifactRef};
use nostr::domain::{Event, RelaySigner, Tag};
use nostr::ext;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{Base, Entry, Status};

/// The bounded file transport, not a new Nostr event kind.
pub const BUNDLE_SCHEMA: &str = "openagents.kb-snapshot-bundle.v1";
/// Maximum serialized bundle, checked before parsing.
pub const MAX_BUNDLE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_DOCUMENT_BYTES: usize = 1024 * 1024;
const MAX_DOCUMENTS: usize = 512;

/// A signed release and every exact document it lists. No external fetch occurs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub v: String,
    pub release: Event,
    pub manifest: Value,
    pub documents: BTreeMap<String, String>,
}

/// A checked snapshot. Entries remain candidates until independent host admission.
pub struct Verified {
    pub base: Base,
    pub release: Event,
    pub manifest: Value,
}

fn artifact(bytes: &[u8], media_type: &str, schema: &str) -> Value {
    json!({"digest":crate::digest(bytes),"size":bytes.len(),"media_type":media_type,"schema":schema})
}

/// Build a signed release from explicitly chosen admitted entry documents.
///
/// # Errors
/// Refuses invalid, duplicate, withdrawn/candidate or oversized entries and
/// manifests that do not satisfy NIP-EXT.
pub fn create(
    documents: &[String],
    signer: &RelaySigner,
    slug: &str,
    version: &str,
    license: &str,
    created_at: u64,
) -> Result<Bundle, String> {
    let mut files = Vec::new();
    let mut components = Vec::new();
    let mut contents = BTreeMap::new();
    for text in documents {
        if text.len() > MAX_DOCUMENT_BYTES {
            return Err("snapshot document too large".into());
        }
        let entry = Entry::parse(text)?;
        if entry.status != Status::Admitted || entry.version == 0 {
            return Err("snapshot creation requires positive-version admitted entries".into());
        }
        let path = format!("knowledge/{}.md", entry.id);
        if contents.insert(path.clone(), text.clone()).is_some() {
            return Err("snapshot contains duplicate entry IDs".into());
        }
        let reference = artifact(text.as_bytes(), "text/markdown", "openagents.kb-entry.v1");
        files.push(json!({"path":path,"digest":reference["digest"],"size":text.len(),"media_type":"text/markdown"}));
        components.push(
            json!({"slug":entry.id.replace('.', "-"),"kind":"guidance","definition":reference}),
        );
    }
    let package = format!("{}:{slug}", signer.pubkey());
    let manifest = json!({"v":"openagents.package.v1","requires":[],"package":package,"version":version,
        "license":license,"provenance":{"source":"local","receipts":[],
        "unknowns":["Publisher curation is not independent admission or measured transfer evidence."]},
        "components":components,"files":files,"dependencies":[]});
    let bytes = contracts::jcs(&manifest).map_err(|e| e.to_string())?;
    let body = json!({"v":1,"requires":[],"type":"release","package":package,"version":version,
        "manifest":artifact(&bytes,"application/json","openagents.package.v1")});
    let release = signer.sign(
        created_at,
        ext::RELEASE_KIND,
        vec![Tag::new(vec!["t".into(), "oa:ext:release:v1".into()])],
        serde_json::to_string(&body).map_err(|e| e.to_string())?,
    );
    let bundle = Bundle {
        v: BUNDLE_SCHEMA.into(),
        release,
        manifest,
        documents: contents,
    };
    verify(&bundle)?;
    Ok(bundle)
}

/// Verify signature, manifest pin, all files, and every entry/component identity.
/// An explicit release pin does not prove that no unseen revocation exists.
///
/// # Errors
/// Refuses missing/unlisted bytes, tampering, unsupported executable components,
/// dependencies, malformed entries, ambiguous IDs, and oversized closures.
pub fn verify(bundle: &Bundle) -> Result<Verified, String> {
    if bundle.v != BUNDLE_SCHEMA
        || bundle.documents.is_empty()
        || bundle.documents.len() > MAX_DOCUMENTS
    {
        return Err("unsupported or empty snapshot bundle".into());
    }
    bundle
        .release
        .validate_nip01_structure()
        .map_err(|e| e.to_string())?;
    let record = ext::parse_record(&bundle.release).map_err(|e| e.to_string())?;
    if bundle.release.kind != ext::RELEASE_KIND {
        return Err("snapshot requires an immutable EXT release".into());
    }
    let reference = contracts::parse_artifact(&record["manifest"]).map_err(|e| e.to_string())?;
    if reference.schema.as_deref() != Some("openagents.package.v1")
        || reference.media_type != "application/json"
    {
        return Err("snapshot manifest has the wrong schema or media type".into());
    }
    let bytes = contracts::jcs(&bundle.manifest).map_err(|e| e.to_string())?;
    contracts::check_artifact_bytes(&reference, &bytes).map_err(|e| e.to_string())?;
    let manifest = ext::parse_manifest(&bundle.manifest).map_err(|e| e.to_string())?;
    if record["package"] != manifest.package
        || record["version"] != manifest.version
        || !manifest.dependencies.is_empty()
    {
        return Err("snapshot package/version mismatch or unsupported dependencies".into());
    }
    if manifest.files.len() != bundle.documents.len() {
        return Err("snapshot files and documents differ".into());
    }
    let components = bundle.manifest["components"]
        .as_array()
        .ok_or("missing components")?;
    if components.len() != manifest.files.len() {
        return Err("every snapshot entry needs one guidance component".into());
    }
    let mut supplied = BTreeMap::new();
    let mut definitions = BTreeMap::new();
    for component in components {
        if component["kind"] != "guidance" || component.get("descriptor").is_some() {
            return Err(
                "knowledge snapshots support only inert guidance without descriptors".into(),
            );
        }
        let reference =
            contracts::parse_artifact(&component["definition"]).map_err(|e| e.to_string())?;
        if reference.schema.as_deref() != Some("openagents.kb-entry.v1")
            || reference.media_type != "text/markdown"
            || definitions
                .insert(reference.digest.clone(), reference.clone())
                .is_some()
        {
            return Err("snapshot guidance identity is invalid or repeated".into());
        }
    }
    let mut base = Base {
        entries: Vec::new(),
    };
    let mut ids = BTreeSet::new();
    let mut total = 0usize;
    for file in &manifest.files {
        let text = bundle
            .documents
            .get(&file.path)
            .ok_or("missing snapshot document")?;
        total = total
            .checked_add(text.len())
            .ok_or("snapshot size overflow")?;
        if text.len() > MAX_DOCUMENT_BYTES
            || total > 8 * 1024 * 1024
            || file.media_type != "text/markdown"
        {
            return Err("snapshot document limit or media type".into());
        }
        let reference = ArtifactRef {
            digest: file.digest.clone(),
            size: file.size,
            media_type: file.media_type.clone(),
            schema: None,
            event: None,
            sources: Vec::new(),
        };
        contracts::check_artifact_bytes(&reference, text.as_bytes()).map_err(|e| e.to_string())?;
        let definition = definitions
            .get(&file.digest)
            .ok_or("snapshot file lacks guidance identity")?;
        contracts::check_artifact_bytes(definition, text.as_bytes()).map_err(|e| e.to_string())?;
        let mut entry = Entry::parse(text)?;
        if entry.version == 0
            || entry.status != Status::Admitted
            || !ids.insert(entry.id.clone())
            || file.path != format!("knowledge/{}.md", entry.id)
        {
            return Err("snapshot entry identity/status/path mismatch".into());
        }
        // Publisher status is retained in the document, but is not local admission.
        entry.status = Status::Candidate;
        base.entries.push(entry);
        supplied.insert(file.digest.clone(), text.as_bytes().to_vec());
    }
    let staged: Vec<String> = bundle.documents.keys().cloned().collect();
    let dependencies = BTreeMap::from([(bundle.release.id.clone(), manifest.clone())]);
    ext::verify_closure(&ext::Closure {
        manifest: &manifest,
        files: &supplied,
        staged: &staged,
        dependencies: &dependencies,
        root: &bundle.release.id,
        byte_limit: 8 * 1024 * 1024,
    })
    .map_err(|e| e.to_string())?;
    Ok(Verified {
        base,
        release: bundle.release.clone(),
        manifest: bundle.manifest.clone(),
    })
}

/// Read bounded strict JSON before checking a snapshot.
///
/// # Errors
/// Refuses unreadable, duplicate-key, malformed or unverified bundles.
pub fn read(path: &Path) -> Result<Verified, String> {
    let bundle: Bundle = read_json(path, MAX_BUNDLE_BYTES)?;
    verify(&bundle)
}

pub(crate) fn read_json<T: serde::de::DeserializeOwned>(
    path: &Path,
    limit: u64,
) -> Result<T, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("bundle exceeds its byte limit".into());
    }
    let value = contracts::parse_strict(&bytes).map_err(|e| e.to_string())?;
    serde_json::from_value(value).map_err(|e| e.to_string())
}

/// Write a bundle without replacing an existing pin or exposing partial bytes.
/// Files are private even for public snapshots; this grants no model disclosure.
///
/// # Errors
/// Refuses a conflicting existing file or an unavailable private filesystem.
pub fn write_new(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BUNDLE_BYTES {
        return Err("bundle exceeds its byte limit".into());
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let nonce = entropy()?;
    let tmp = parent.join(format!(".kb-{}", hex(&nonce)));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    #[cfg(not(unix))]
    {
        return Err("private bundle storage is unsupported on this host".into());
    }
    let result = (|| {
        let mut file = opts.open(&tmp).map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| e.to_string())?;
        std::fs::hard_link(&tmp, path)
            .map_err(|e| format!("cannot install immutable bundle: {e}"))?;
        std::fs::File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())
    })();
    let _ = std::fs::remove_file(&tmp);
    result
}

pub(crate) fn entropy() -> Result<[u8; 32], String> {
    let mut bytes = [0; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|e| e.to_string())?;
    Ok(bytes)
}
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests;
