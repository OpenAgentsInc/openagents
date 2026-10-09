//! `openagents plugin publish|search` and `openagents plugin install
//! NAME|ID`: the plugin registry (#10182).
//!
//! A published plugin is a NIP-EXT package (`nips/openagents/NIP-EXT.md`):
//! its files go to a blob server by digest, a `3184` release signed by the
//! publisher pins its manifest, and a `30184` listing points at the
//! release. Its id is `<publisher key>:<slug>`. The release may carry the
//! author's per-call `fee_msat` and Lightning `payout` (API doc G9).
//!
//! Installing from the registry finds the listing, checks the release's
//! signature, publisher, and revocations, fetches the manifest and every
//! file, checks each one's digest and size against the signed release, and
//! then installs the plugin as a local directory install does: off until
//! `openagents plugin enable`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use coder::package::Package;
use ext_eval::artifact::{ArtifactRef, JSON, MARKDOWN, TOML};
use ext_eval::blob::{Blossom, MAX_BLOB};
use nostr::domain::{Event, RelaySigner, Tag};
use serde_json::{Value, json};

use crate::relay::{Client, relay_url, signer_for, unix_now};
use crate::{Args, Output};

/// The manifest schema of a NIP-EXT package.
const MANIFEST_SCHEMA: &str = "openagents.package.v1";
/// The license a package states when its record names none.
const LICENSE: &str = "NOASSERTION";
/// The most a package may hold in all.
const MAX_PACKAGE: u64 = 64 * 1024 * 1024;
/// How long a relay has to answer one publish.
const SEND_WAIT: Duration = Duration::from_secs(10);
/// The most listings a search reads.
const SEARCH_LIMIT: u64 = 500;

const SWITCHES: &[&str] = &[];

/// The hosted runner's sample plugins (`deploy/eval-runner/catalog`), their
/// package records compiled in. The runner publishes their releases under
/// its key. They are its test fixtures: search never lists them, and
/// install finds one only when named exactly (#10307).
const CATALOG: [&str; 6] = [
    include_str!("../../plugin-repo-map/package.json"),
    include_str!("../../plugin-code-search/package.json"),
    include_str!("../../plugin-test-report/package.json"),
    include_str!("../../plugin-explain-error/package.json"),
    include_str!("../../plugin-release-notes/package.json"),
    include_str!("../../plugin-dependency-check/package.json"),
];

/// The catalog's plugins as listings with no release named yet: `find`
/// resolves the newest release when one is installed.
pub(crate) fn catalog() -> Vec<Listing> {
    CATALOG
        .iter()
        .filter_map(|text| serde_json::from_str::<Value>(text).ok())
        .filter_map(|record| {
            let slug = record["slug"].as_str()?.to_owned();
            let publisher = record["publisher"].as_str()?.to_owned();
            Some(Listing {
                package: format!("{publisher}:{slug}"),
                slug,
                publisher,
                title: record["name"].as_str().unwrap_or_default().to_owned(),
                description: record["summary"].as_str().unwrap_or_default().to_owned(),
                release: Value::Null,
                // The hosted runner keeps its releases' files in the Gym's
                // blob bucket.
                blobs: vec![coder::gym_kb::SUITE_BLOBS.to_owned()],
                created_at: 0,
            })
        })
        .collect()
}

/// `listings` without the hosted runner's sample plugins, which are its
/// test fixtures and never shown.
pub(crate) fn without_samples(listings: Vec<Listing>) -> Vec<Listing> {
    let samples: Vec<String> = catalog().into_iter().map(|entry| entry.package).collect();
    listings
        .into_iter()
        .filter(|listing| !samples.contains(&listing.package))
        .collect()
}

/// The newest release of the catalog plugin `entry`, as the listing
/// install reads.
fn catalog_release(registry: &mut dyn Registry, entry: Listing) -> Result<Listing, String> {
    let newest = registry
        .query(json!({
            "kinds": [nostr::ext::RELEASE_KIND],
            "authors": [entry.publisher],
            "#t": ["oa:ext:release:v1"],
            "limit": SEARCH_LIMIT,
        }))?
        .into_iter()
        .filter(|event| {
            nostr::ext::parse_record(event)
                .is_ok_and(|body| body["package"] == entry.package.as_str())
        })
        .max_by_key(|event| (event.created_at, event.id.clone()))
        .ok_or_else(|| format!("{} has no published release yet", entry.title))?;
    Ok(Listing {
        release: json!({"id": newest.id}),
        created_at: newest.created_at,
        ..entry
    })
}

/// Where registry events go and come from: a relay, or a fake in tests.
pub(crate) trait Registry {
    /// The events matching one NIP-01 filter.
    fn query(&mut self, filter: Value) -> Result<Vec<Event>, String>;
    /// Publish one event; an error when the relay refuses it.
    fn send(&mut self, event: Event) -> Result<(), String>;
}

/// Where package bytes go and come from, by digest.
pub(crate) trait Blobs {
    /// Store `bytes` under their digest.
    fn put(&self, bytes: &[u8], media_type: &str) -> Result<(), String>;
    /// The bytes `digest` names, checked against it.
    fn get(&self, digest: &str) -> Result<Vec<u8>, String>;
    /// The base URL readers fetch these blobs from, recorded in the
    /// listing, when there is one.
    fn locator(&self) -> Option<String>;
}

impl Registry for Client {
    fn query(&mut self, filter: Value) -> Result<Vec<Event>, String> {
        crate::ext_eval::fetch(self, filter)
    }

    fn send(&mut self, event: Event) -> Result<(), String> {
        let published = self.publish(event, SEND_WAIT)?;
        if published.accepted {
            Ok(())
        } else {
            Err(format!("the relay refused it: {}", published.message))
        }
    }
}

/// A Blossom server: the same blob path `plugin test publish` uploads a
/// test set to (`crate::ext_eval::blossom`).
pub(crate) struct BlossomStore {
    store: Blossom,
    signer: Option<RelaySigner>,
}

impl BlossomStore {
    pub(crate) fn new(store: Blossom, signer: Option<RelaySigner>) -> Self {
        Self { store, signer }
    }
}

impl Blobs for BlossomStore {
    fn put(&self, bytes: &[u8], media_type: &str) -> Result<(), String> {
        let signer = self
            .signer
            .as_ref()
            .ok_or_else(|| "this blob server is read-only here".to_owned())?;
        self.store.upload(signer, bytes, media_type, unix_now())
    }

    fn get(&self, digest: &str) -> Result<Vec<u8>, String> {
        self.store.fetch(digest)
    }

    fn locator(&self) -> Option<String> {
        Some(self.store.base().to_owned())
    }
}

/// A directory of blobs named by their hex digest, for an operator to copy
/// to a public store (`--blobs-dir`), read back from `read_base`.
pub(crate) struct DirStore {
    dir: PathBuf,
    read_base: Option<String>,
}

impl DirStore {
    pub(crate) fn new(dir: PathBuf, read_base: Option<String>) -> Self {
        Self { dir, read_base }
    }
}

fn hex(digest: &str) -> &str {
    digest.strip_prefix("sha256:").unwrap_or(digest)
}

impl Blobs for DirStore {
    fn put(&self, bytes: &[u8], _media_type: &str) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|error| error.to_string())?;
        let digest = nostr::contracts::digest_bytes(bytes);
        std::fs::write(self.dir.join(hex(&digest)), bytes)
            .map_err(|error| format!("{}: {error}", self.dir.display()))
    }

    fn get(&self, digest: &str) -> Result<Vec<u8>, String> {
        let bytes = std::fs::read(self.dir.join(hex(digest)))
            .map_err(|error| format!("blob {digest}: {error}"))?;
        if nostr::contracts::digest_bytes(&bytes) != format!("sha256:{}", hex(digest)) {
            return Err(format!("blob {digest} does not match its digest"));
        }
        Ok(bytes)
    }

    fn locator(&self) -> Option<String> {
        self.read_base.clone()
    }
}

/// One file a package lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Listed {
    pub path: String,
    pub bytes: Vec<u8>,
    pub media_type: String,
}

/// A plugin directory, packed for the registry.
#[derive(Clone, Debug)]
pub(crate) struct Packed {
    /// `<publisher>:<slug>`.
    pub package: String,
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub version: String,
    /// The manifest's exact bytes.
    pub manifest: Vec<u8>,
    pub files: Vec<Listed>,
}

/// The per-call fee a release charges and where it is paid (G9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Fee {
    pub msat: u64,
    pub payout: String,
}

fn media_for(path: &str) -> &'static str {
    match Path::new(path).extension().and_then(|ext| ext.to_str()) {
        Some("json") => JSON,
        Some("md") => MARKDOWN,
        Some("toml") => TOML,
        Some("wasm") => "application/wasm",
        Some("txt") => "text/plain",
        Some("html") => "text/html",
        _ => "application/octet-stream",
    }
}

/// Every regular file under `dir`, by relative POSIX path, leaving out
/// what a local install leaves out, hidden files, and links.
fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, PathBuf)>) -> Result<(), String> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|error| format!("{}: {error}", dir.display()))?
        .filter_map(Result::ok)
        .collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name();
        let text = name.to_string_lossy();
        if text.starts_with('.') || crate::plugin_local::SKIP.iter().any(|skip| text == *skip) {
            continue;
        }
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        let path = entry.path();
        if kind.is_dir() {
            walk(&path, base, out)?;
        } else if kind.is_file() {
            let relative = path
                .strip_prefix(base)
                .map_err(|error| error.to_string())?
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            out.push((relative, path));
        }
    }
    Ok(())
}

/// The record's bytes as published: its own, with `publisher` filled in
/// when it was left empty, so an install files it under that key.
fn published_record(text: &str, publisher: &str) -> Result<Vec<u8>, String> {
    let empty = "\"publisher\": \"\"";
    if text.matches(empty).count() == 1 {
        return Ok(text
            .replace(empty, &format!("\"publisher\": \"{publisher}\""))
            .into_bytes());
    }
    let mut value: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    if value["publisher"].as_str().unwrap_or_default().is_empty() {
        value["publisher"] = json!(publisher);
        let mut bytes = serde_json::to_vec_pretty(&value).map_err(|error| error.to_string())?;
        bytes.push(b'\n');
        return Ok(bytes);
    }
    Ok(text.as_bytes().to_vec())
}

/// Packs the plugin in `dir` for `publisher`: its record must resolve, and
/// a record that names a publisher must name this one.
pub(crate) fn pack(dir: &Path, publisher: &str) -> Result<Packed, String> {
    let record_path = dir.join("package.json");
    let package = Package::load(&record_path)
        .map_err(|why| format!("{} is not a plugin: {why}", dir.display()))?;
    let lock = Package::resolve(dir, &package)
        .map_err(|refusal| format!("{} does not resolve: {refusal}", dir.display()))?;
    if !package.publisher.is_empty() && package.publisher != publisher {
        return Err(format!(
            "package.json names the publisher {}; publish it with that key (--as PROFILE)",
            package.publisher
        ));
    }
    let text = std::fs::read_to_string(&record_path)
        .map_err(|error| format!("{}: {error}", record_path.display()))?;
    let record = published_record(&text, publisher)?;
    let mut found = Vec::new();
    walk(dir, dir, &mut found)?;
    let mut files = Vec::new();
    let mut total = 0_u64;
    for (path, on_disk) in found {
        let bytes = if path == "package.json" {
            record.clone()
        } else {
            std::fs::read(&on_disk).map_err(|error| format!("{path}: {error}"))?
        };
        let size = bytes.len() as u64;
        if size > MAX_BLOB {
            return Err(format!("{path} is over {MAX_BLOB} bytes"));
        }
        total += size;
        if total > MAX_PACKAGE {
            return Err(format!("the plugin is over {MAX_PACKAGE} bytes"));
        }
        files.push(Listed {
            media_type: media_for(&path).to_owned(),
            path,
            bytes,
        });
    }
    let record_ref = ArtifactRef::of(&record, JSON, Some(ext_eval::arms::PACKAGE_SCHEMA));
    let mut components = Vec::new();
    if let (Some(reference), Some(pin)) = (&package.program, &lock.program) {
        let relative = Path::new(&pin.found)
            .strip_prefix(dir)
            .unwrap_or(Path::new(&pin.found))
            .to_string_lossy()
            .into_owned();
        let file = files
            .iter()
            .find(|file| file.path == relative)
            .ok_or_else(|| format!("the program {} is not in the plugin", reference.name))?;
        components.push(json!({
            "slug": reference.name,
            "kind": "program",
            "definition": ArtifactRef::of(&file.bytes, JSON, None).value(),
            "descriptor": record_ref.value(),
        }));
    }
    for file in &files {
        if let Some(name) = file
            .path
            .strip_prefix("skills/")
            .and_then(|rest| rest.strip_suffix(".md"))
            .filter(|name| !name.contains('/'))
            && !components
                .iter()
                .any(|component| component["slug"] == *name)
        {
            components.push(json!({
                "slug": name,
                "kind": "guidance",
                "definition": ArtifactRef::of(&file.bytes, MARKDOWN, None).value(),
            }));
        }
    }
    let version = if package.version.is_empty() {
        "0.0.0".to_owned()
    } else {
        package.version.clone()
    };
    let id = format!("{publisher}:{}", package.slug);
    let manifest = json!({
        "v": MANIFEST_SCHEMA,
        "requires": [],
        "package": id,
        "version": version,
        "license": LICENSE,
        "provenance": {"source": "local", "receipts": [], "unknowns": []},
        "components": components,
        "files": files.iter().map(|file| json!({
            "path": file.path,
            "digest": nostr::contracts::digest_bytes(&file.bytes),
            "size": file.bytes.len(),
            "media_type": file.media_type,
        })).collect::<Vec<_>>(),
        "dependencies": [],
    });
    nostr::ext::parse_manifest(&manifest).map_err(|error| error.to_string())?;
    Ok(Packed {
        package: id,
        slug: package.slug.clone(),
        name: if package.name.trim().is_empty() {
            package.slug.clone()
        } else {
            package.name.clone()
        },
        summary: package.summary,
        version,
        manifest: ext_eval::artifact::json_bytes(&manifest),
        files,
    })
}

impl Packed {
    /// The `3184` release body.
    fn release_body(&self, fee: Option<&Fee>) -> Value {
        let mut body = json!({
            "v": 1,
            "requires": [],
            "type": "release",
            "package": self.package,
            "version": self.version,
            "manifest": ArtifactRef::of(&self.manifest, JSON, Some(MANIFEST_SCHEMA)).value(),
        });
        if let Some(fee) = fee {
            body["fee_msat"] = json!(fee.msat);
            body["payout"] = json!(fee.payout);
        }
        body
    }

    /// Every blob the package needs: the manifest and each file, once.
    fn blobs(&self) -> Vec<(&[u8], &str)> {
        let mut seen = BTreeSet::new();
        std::iter::once((self.manifest.as_slice(), JSON))
            .chain(
                self.files
                    .iter()
                    .map(|file| (file.bytes.as_slice(), file.media_type.as_str())),
            )
            .filter(|(bytes, _)| seen.insert(nostr::contracts::digest_bytes(bytes)))
            .collect()
    }
}

/// What a publish left on the registry.
#[derive(Clone, Debug)]
pub(crate) struct Published {
    pub release: Event,
    pub listing: Event,
    /// Whether the release was already there.
    pub reused: bool,
    pub blobs: usize,
}

fn event_ref(event: &Event) -> Value {
    json!({"id": event.id, "pubkey": event.pubkey, "kind": event.kind})
}

fn marker(kind: &str) -> Tag {
    Tag::new(vec!["t".into(), format!("oa:ext:{kind}:v1")])
}

/// Publishes `packed`: uploads its blobs, then signs and sends the release
/// (once per version) and the listing pointing at it.
pub(crate) fn publish(
    packed: &Packed,
    signer: &RelaySigner,
    registry: &mut dyn Registry,
    blobs: &dyn Blobs,
    fee: Option<&Fee>,
    now: u64,
) -> Result<Published, String> {
    if let Some(fee) = fee {
        nostr::ext::check_payout(&fee.payout).map_err(|_| {
            format!(
                "{} is not a mainnet Spark address, Lightning address, or node key",
                fee.payout
            )
        })?;
    }
    let pubkey = signer.pubkey().to_string();
    let body = packed.release_body(fee);
    let content = body.to_string();
    let releases = registry.query(json!({
        "kinds": [nostr::ext::RELEASE_KIND],
        "authors": [pubkey],
        "#t": ["oa:ext:release:v1"],
    }))?;
    let mut existing = None;
    for event in releases {
        let Ok(found) = nostr::ext::parse_record(&event) else {
            continue;
        };
        if found["package"] != body["package"] || found["version"] != body["version"] {
            continue;
        }
        if event.content == content {
            existing = Some(event);
            break;
        }
        return Err(format!(
            "version {} of {} is already published with other contents (release {}); raise the version in package.json",
            packed.version, packed.package, event.id
        ));
    }
    let uploads = packed.blobs();
    for (bytes, media) in &uploads {
        blobs.put(bytes, media)?;
    }
    let reused = existing.is_some();
    let release = match existing {
        Some(event) => event,
        None => {
            let event = signer.sign(
                now,
                nostr::ext::RELEASE_KIND,
                vec![marker("release")],
                content,
            );
            nostr::ext::parse_record(&event).map_err(|error| error.to_string())?;
            registry
                .send(event.clone())
                .map_err(|why| format!("the release: {why}"))?;
            event
        }
    };
    let mut listing_body = json!({
        "v": 1,
        "requires": [],
        "type": "listing",
        "package": packed.package,
        "state": "published",
        "release": event_ref(&release),
        "title": packed.name,
        "description": packed.summary,
    });
    if let Some(base) = blobs.locator() {
        listing_body["meta"] = json!({"blobs": [base]});
    }
    let listing_content = listing_body.to_string();
    let previous = registry
        .query(json!({
            "kinds": [nostr::ext::LISTING_KIND],
            "authors": [pubkey],
            "#d": [packed.slug],
        }))?
        .into_iter()
        .max_by_key(|event| event.created_at);
    let listing = match previous {
        Some(event) if event.content == listing_content => event,
        previous => {
            let at = previous.map_or(now, |event| now.max(event.created_at + 1));
            let event = signer.sign(
                at,
                nostr::ext::LISTING_KIND,
                vec![
                    Tag::new(vec!["d".into(), packed.slug.clone()]),
                    marker("listing"),
                ],
                listing_content,
            );
            nostr::ext::parse_record(&event).map_err(|error| error.to_string())?;
            registry
                .send(event.clone())
                .map_err(|why| format!("the listing: {why}"))?;
            event
        }
    };
    Ok(Published {
        release,
        listing,
        reused,
        blobs: uploads.len(),
    })
}

/// One published plugin, from its newest listing.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Listing {
    /// `<publisher>:<slug>`.
    pub package: String,
    pub slug: String,
    pub publisher: String,
    pub title: String,
    pub description: String,
    /// The release the listing points at, an EventRef.
    pub release: Value,
    /// Blob servers the publisher named.
    pub blobs: Vec<String>,
    pub created_at: u64,
}

impl Listing {
    fn row(&self) -> Value {
        json!({
            "id": self.package,
            "slug": self.slug,
            "publisher": self.publisher,
            "title": self.title,
            "description": self.description,
            "release": self.release["id"],
            "created_at": self.created_at,
        })
    }
}

/// The published listings, newest per publisher and slug, newest first.
pub(crate) fn listings(
    registry: &mut dyn Registry,
    author: Option<&str>,
    slug: Option<&str>,
) -> Result<Vec<Listing>, String> {
    let mut filter = json!({
        "kinds": [nostr::ext::LISTING_KIND],
        "#t": ["oa:ext:listing:v1"],
        "limit": SEARCH_LIMIT,
    });
    if let Some(author) = author {
        filter["authors"] = json!([author]);
    }
    if let Some(slug) = slug {
        filter["#d"] = json!([slug]);
    }
    let mut newest: BTreeMap<(String, String), Event> = BTreeMap::new();
    for event in registry.query(filter)? {
        let Some(d) = event.tag_values("d").next().map(str::to_owned) else {
            continue;
        };
        let key = (event.pubkey.clone(), d);
        if newest
            .get(&key)
            .is_none_or(|kept| (event.created_at, &event.id) > (kept.created_at, &kept.id))
        {
            newest.insert(key, event);
        }
    }
    let mut out = Vec::new();
    for ((publisher, slug), event) in newest {
        let Ok(body) = nostr::ext::parse_record(&event) else {
            continue;
        };
        if body["state"] != "published" {
            continue;
        }
        out.push(Listing {
            package: body["package"].as_str().unwrap_or_default().to_owned(),
            slug,
            publisher,
            title: body["title"].as_str().unwrap_or_default().to_owned(),
            description: body["description"].as_str().unwrap_or_default().to_owned(),
            release: body["release"].clone(),
            blobs: body["meta"]["blobs"]
                .as_array()
                .map(|bases| {
                    bases
                        .iter()
                        .filter_map(Value::as_str)
                        .filter(|base| base.starts_with("https://") || base.starts_with("http://"))
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            created_at: event.created_at,
        });
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(out)
}

/// The listings matching `query`, best first: a plugin whose slug or id is
/// the query, then BM25 over each listing's slug, title, and description.
/// An empty query keeps them all, newest first.
pub(crate) fn search(listings: Vec<Listing>, query: &str) -> Vec<Listing> {
    let query = query.trim();
    if query.is_empty() {
        return listings;
    }
    let texts: Vec<String> = listings
        .iter()
        .map(|listing| {
            format!(
                "{} {} {} {}",
                listing.slug,
                listing.slug.replace('-', " "),
                listing.title,
                listing.description
            )
        })
        .collect();
    let scores = knowledge::search::bm25_texts(&texts, query);
    let mut ranked: Vec<(f64, Listing)> = listings
        .into_iter()
        .zip(scores)
        .filter_map(|(listing, score)| {
            let exact = listing.slug == query || listing.package == query;
            let score = if exact { f64::MAX } else { score };
            (score > 0.0).then_some((score, listing))
        })
        .collect();
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
    ranked.into_iter().map(|(_, listing)| listing).collect()
}

/// The one listing `name` names: an id (`<publisher>:<slug>`) or a slug
/// only one publisher has published.
pub(crate) fn find(registry: &mut dyn Registry, name: &str) -> Result<Listing, String> {
    let (author, slug) = match name.split_once(':') {
        Some((author, slug)) => (Some(author), slug),
        None => (None, name),
    };
    let found = listings(registry, author, Some(slug))?;
    match found.as_slice() {
        [] => match catalog().into_iter().find(|entry| {
            entry.slug == slug && author.is_none_or(|author| author == entry.publisher)
        }) {
            Some(entry) => catalog_release(registry, entry),
            None => Err(format!(
                "no published plugin is named {name}; `openagents plugin search` lists them"
            )),
        },
        [one] => Ok(one.clone()),
        many => Err(format!(
            "{} publishers have a plugin named {slug}; install one by its id: {}",
            many.len(),
            many.iter()
                .map(|listing| listing.package.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// What a download fetched and checked.
#[derive(Clone, Debug)]
pub(crate) struct Fetched {
    pub release: Event,
    pub version: String,
    pub manifest_digest: String,
    pub files: usize,
    pub fee: Option<Fee>,
}

fn get_any(stores: &[&dyn Blobs], digest: &str) -> Result<Vec<u8>, String> {
    let mut why = Vec::new();
    for store in stores {
        match store.get(digest) {
            Ok(bytes) => return Ok(bytes),
            Err(error) => why.push(error),
        }
    }
    Err(if why.is_empty() {
        format!("no blob server to fetch {digest} from")
    } else {
        why.join("; ")
    })
}

/// Fetches the release `listing` points at into `into`: the release must be
/// signed by the listing's publisher for the same package and not revoked,
/// and the manifest and every file must match the digests and sizes the
/// signed release pins.
pub(crate) fn download(
    registry: &mut dyn Registry,
    listing: &Listing,
    stores: &[&dyn Blobs],
    into: &Path,
) -> Result<Fetched, String> {
    let release_id = listing.release["id"]
        .as_str()
        .ok_or_else(|| format!("the listing of {} names no release", listing.package))?
        .to_owned();
    let release = registry
        .query(json!({"ids": [release_id], "kinds": [nostr::ext::RELEASE_KIND]}))?
        .into_iter()
        .find(|event| event.id == release_id)
        .ok_or_else(|| format!("the relay has no release {release_id}"))?;
    let body = nostr::ext::parse_record(&release)
        .map_err(|error| format!("the release {release_id} does not check: {error}"))?;
    if release.pubkey != listing.publisher || body["package"] != listing.package.as_str() {
        return Err(format!(
            "the release {release_id} is not {}'s",
            listing.package
        ));
    }
    let revoked = registry
        .query(json!({
            "kinds": [nostr::ext::REVOCATION_KIND],
            "authors": [listing.publisher],
            "#e": [release_id],
        }))?
        .into_iter()
        .any(|event| nostr::ext::parse_record(&event).is_ok());
    if revoked {
        return Err(format!(
            "its publisher revoked the release {release_id} of {}",
            listing.package
        ));
    }
    let manifest_ref =
        nostr::contracts::parse_artifact(&body["manifest"]).map_err(|error| error.to_string())?;
    let manifest_bytes = get_any(stores, &manifest_ref.digest)?;
    nostr::contracts::check_artifact_bytes(&manifest_ref, &manifest_bytes)
        .map_err(|error| format!("the manifest: {error}"))?;
    let manifest_value: Value =
        serde_json::from_slice(&manifest_bytes).map_err(|error| error.to_string())?;
    let manifest =
        nostr::ext::parse_manifest(&manifest_value).map_err(|error| error.to_string())?;
    if manifest.package != listing.package || body["version"] != manifest.version.as_str() {
        return Err("the manifest does not match its release".into());
    }
    if !manifest.dependencies.is_empty() {
        return Err(format!(
            "{} depends on other packages, which install cannot fetch yet",
            listing.package
        ));
    }
    let total: u64 = manifest.files.iter().map(|file| file.size).sum();
    if total > MAX_PACKAGE {
        return Err(format!("{} is over {MAX_PACKAGE} bytes", listing.package));
    }
    let mut bytes_by_digest = BTreeMap::new();
    for file in &manifest.files {
        if !bytes_by_digest.contains_key(&file.digest) {
            let bytes =
                get_any(stores, &file.digest).map_err(|why| format!("{}: {why}", file.path))?;
            bytes_by_digest.insert(file.digest.clone(), bytes);
        }
    }
    let staged: Vec<String> = manifest
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect();
    let mut dependencies = BTreeMap::new();
    dependencies.insert(release_id.clone(), manifest.clone());
    nostr::ext::verify_closure(&nostr::ext::Closure {
        manifest: &manifest,
        files: &bytes_by_digest,
        staged: &staged,
        dependencies: &dependencies,
        root: &release_id,
        byte_limit: MAX_PACKAGE,
    })
    .map_err(|error| format!("{} does not check: {error}", listing.package))?;
    for file in &manifest.files {
        let path = into.join(&file.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        std::fs::write(&path, &bytes_by_digest[&file.digest])
            .map_err(|error| format!("{}: {error}", path.display()))?;
    }
    let fee = body["payout"].as_str().map(|payout| Fee {
        msat: body["fee_msat"].as_u64().unwrap_or(0),
        payout: payout.to_owned(),
    });
    Ok(Fetched {
        release,
        version: manifest.version,
        manifest_digest: manifest_ref.digest,
        files: manifest.files.len(),
        fee,
    })
}

/// `openagents plugin publish|search|install NAME`; `None` for any other
/// command, and for `install DIR`, which `plugin_local` does.
pub fn run(output: &Output, words: &[String]) -> Option<u8> {
    let (command, rest) = words.split_first()?;
    let name = format!("plugin {command}");
    let args = match command.as_str() {
        "publish" | "search" | "install" => match Args::parse(rest, SWITCHES) {
            Ok(args) => args,
            Err(message) => {
                return Some(output.usage(&name, &message, crate::catalog::EXT_USAGE));
            }
        },
        _ => return None,
    };
    let result = match command.as_str() {
        "publish" => publish_command(&args),
        "search" => search_command(&args),
        _ => {
            let target = args.positional().first()?;
            if Path::new(target).is_dir() || target.starts_with('.') || target.contains('/') {
                return None;
            }
            install_command(&args, target)
        }
    };
    Some(match result {
        Ok(value) => {
            output.emit(&value, |value| {
                value["text"].as_str().unwrap_or_default().to_owned()
            });
            0
        }
        Err(message) => output.fail(&name, &message),
    })
}

fn publication_fee(
    msat: Option<&str>,
    sats: Option<&str>,
    payout: Option<&str>,
) -> Result<Option<Fee>, String> {
    if msat.is_some() && sats.is_some() {
        return Err("Use either --fee-msat or --fee-sats, not both".into());
    }
    let amount = match (msat, sats) {
        (Some(value), _) => value
            .parse::<u64>()
            .map_err(|_| "--fee-msat takes a whole number of millisatoshis")?,
        (_, Some(value)) => value
            .parse::<u64>()
            .ok()
            .and_then(|n| n.checked_mul(1000))
            .ok_or("--fee-sats takes a whole number that fits in u64 millisatoshis")?,
        _ => 0,
    };
    match payout {
        Some(payout) => {
            nostr::ext::check_payout(payout).map_err(|_| "Invalid payout destination")?;
            Ok(Some(Fee {
                msat: amount,
                payout: payout.to_owned(),
            }))
        }
        None if msat.is_some() || sats.is_some() => Err("A fee needs --payout".into()),
        None => Ok(None),
    }
}

fn publish_command(args: &Args) -> Result<Value, String> {
    let dir = PathBuf::from(args.positional().first().map_or(".", String::as_str));
    let dir = dir
        .canonicalize()
        .map_err(|error| format!("{}: {error}", dir.display()))?;
    let fee = publication_fee(
        args.option("fee-msat"),
        args.option("fee-sats"),
        args.option("payout"),
    )?;
    let signer = signer_for(args.option("as"))?;
    let repinned = crate::plugin_new::repinned_note(&dir)?;
    let packed = pack(&dir, signer.pubkey())?;
    let relay = relay_url(args.option("relay"));
    let blobs: Box<dyn Blobs> = match args.option("blobs-dir") {
        Some(blobs_dir) => Box::new(DirStore::new(
            PathBuf::from(blobs_dir),
            args.option("blossom").map(str::to_owned),
        )),
        None => Box::new(BlossomStore::new(
            crate::ext_eval::blossom(args, &relay)?,
            Some(signer.clone()),
        )),
    };
    let mut client = Client::connect(&relay, signer.clone());
    let published = publish(
        &packed,
        &signer,
        &mut client,
        blobs.as_ref(),
        fee.as_ref(),
        unix_now(),
    );
    client.close();
    let published = published?;
    let text = format!(
        "{}Published {} {} as {}\nrelease {}\nlisting {}\non {relay}{}\nInstall it with `openagents plugin install {}`.",
        repinned.map(|note| format!("{note}\n")).unwrap_or_default(),
        packed.name,
        packed.version,
        packed.package,
        published.release.id,
        published.listing.id,
        if published.reused {
            " (this version was already published)"
        } else {
            ""
        },
        packed.package,
    );
    Ok(json!({
        "text": text,
        "id": packed.package,
        "version": packed.version,
        "relay": relay,
        "release": published.release.id,
        "listing": published.listing.id,
        "reused": published.reused,
        "blobs": published.blobs,
        "blob_server": blobs.locator(),
        "fee": fee.map(|fee| json!({"fee_msat": fee.msat, "payout": fee.payout})),
    }))
}

pub(crate) use crate::catalog::published_list_text;

fn search_command(args: &Args) -> Result<Value, String> {
    let query = args.positional().join(" ");
    let author = args.option("author");
    let limit = args.number::<usize>("limit", 30)?;
    let relay = relay_url(args.option("relay"));
    let mut client = Client::connect(&relay, signer_for(args.option("as"))?);
    let found = listings(&mut client, author, None);
    let found: Vec<Listing> = match found {
        Ok(found) => search(without_samples(found), &query)
            .into_iter()
            .take(limit)
            .collect(),
        Err(error) => {
            client.close();
            return Err(error);
        }
    };
    let fees = fees(&mut client, &found);
    client.close();
    let text = if found.is_empty() {
        if query.trim().is_empty() {
            format!("No plugins are published on {relay}.")
        } else {
            format!("No published plugin matches \"{}\".", query.trim())
        }
    } else {
        found
            .iter()
            .map(|listing| {
                let price = fees
                    .get(&listing.package)
                    .map(|msat| format!(" · {} a call", sats(*msat)))
                    .unwrap_or_default();
                format!(
                    "{} · {}\n  {}{price}\n  openagents plugin install {}",
                    listing.title, listing.description, listing.package, listing.slug
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let text = published_list_text(&relay, &text);
    Ok(json!({
        "text": text,
        "relay": relay,
        "count": found.len(),
        "items": found
            .iter()
            .map(|listing| {
                let mut row = listing.row();
                row["fee_msat"] = json!(fees.get(&listing.package).copied().unwrap_or(0));
                row
            })
            .collect::<Vec<_>>(),
    }))
}

/// The per-call fee each listed plugin's release asks, by package; a plugin
/// that asks none, or whose release the relay doesn't answer, is absent.
fn fees(registry: &mut dyn Registry, found: &[Listing]) -> BTreeMap<String, u64> {
    let ids: Vec<&str> = found
        .iter()
        .filter_map(|listing| listing.release["id"].as_str())
        .collect();
    if ids.is_empty() {
        return BTreeMap::new();
    }
    registry
        .query(json!({"ids": ids, "kinds": [nostr::ext::RELEASE_KIND]}))
        .unwrap_or_default()
        .iter()
        .filter_map(|event| nostr::ext::parse_record(event).ok())
        .filter_map(|body| {
            let msat = body["fee_msat"].as_u64().filter(|msat| *msat > 0)?;
            Some((body["package"].as_str()?.to_owned(), msat))
        })
        .collect()
}

/// `msat` as people read it: `15 sats`, `1.5 sats`.
fn sats(msat: u64) -> String {
    if msat % 1000 == 0 {
        let whole = msat / 1000;
        format!("{whole} sat{}", if whole == 1 { "" } else { "s" })
    } else {
        format!("{} sats", msat as f64 / 1000.0)
    }
}

fn install_command(args: &Args, name: &str) -> Result<Value, String> {
    let relay = relay_url(args.option("relay"));
    let layout = background::Layout::from_env().map_err(|error| error.to_string())?;
    let mut client = Client::connect(&relay, signer_for(args.option("as"))?);
    let installed = install_from_registry(&layout, &mut client, name, args, &relay);
    client.close();
    installed
}

/// Fetches `name` from the registry and installs it, off.
fn install_from_registry(
    layout: &background::Layout,
    registry: &mut dyn Registry,
    name: &str,
    args: &Args,
    relay: &str,
) -> Result<Value, String> {
    let listing = find(registry, name)?;
    let mut owned: Vec<Box<dyn Blobs>> = Vec::new();
    if let Some(base) = args.option("blossom") {
        owned.push(Box::new(BlossomStore::new(Blossom::new(base)?, None)));
    }
    for base in &listing.blobs {
        owned.push(Box::new(BlossomStore::new(Blossom::new(base)?, None)));
    }
    owned.push(Box::new(BlossomStore::new(
        Blossom::for_relay(relay)?,
        None,
    )));
    let stores: Vec<&dyn Blobs> = owned.iter().map(AsRef::as_ref).collect();
    install_listing(layout, registry, &listing, &stores)
}

/// Downloads `listing` into a staging folder and installs it from there.
pub(crate) fn install_listing(
    layout: &background::Layout,
    registry: &mut dyn Registry,
    listing: &Listing,
    stores: &[&dyn Blobs],
) -> Result<Value, String> {
    let staging =
        layout
            .extensions()
            .join(format!(".fetching-{}-{}", listing.slug, std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|error| error.to_string())?;
    let result = download(registry, listing, stores, &staging).and_then(|fetched| {
        let record = Package::load(&staging.join("package.json"))?;
        if record.publisher != listing.publisher {
            return Err(format!(
                "its package.json names the publisher {:?}, not the key that signed it",
                record.publisher
            ));
        }
        let mut value = crate::plugin_local::install_into(layout, &staging)?;
        value["release"] = json!(fetched.release.id);
        value["manifest"] = json!(fetched.manifest_digest);
        value["id"] = json!(listing.package);
        if let Some(fee) = &fetched.fee {
            value["fee"] = json!({"fee_msat": fee.msat, "payout": fee.payout});
        }
        if let Some(dir) = value["plugin"]["dir"].as_str() {
            let _ = std::fs::write(
                Path::new(dir).join("installed-from.json"),
                ext_eval::artifact::json_bytes(&json!({
                    "id": listing.package,
                    "release": fetched.release.id,
                    "manifest": fetched.manifest_digest,
                    "version": fetched.version,
                })),
            );
        }
        let text = value["text"].as_str().unwrap_or_default().to_owned();
        value["text"] = json!(format!(
            "{text}\nFrom {} (release {}), {} files checked against the signed release.",
            listing.package, fetched.release.id, fetched.files
        ));
        Ok(value)
    });
    let _ = std::fs::remove_dir_all(&staging);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// A relay that keeps what it is sent and answers filters by kind,
    /// author, id, and `#d`/`#t`/`#e` tag.
    #[derive(Default)]
    struct FakeRelay {
        events: Vec<Event>,
    }

    impl Registry for FakeRelay {
        fn query(&mut self, filter: Value) -> Result<Vec<Event>, String> {
            let within = |key: &str, value: &str| {
                filter[key]
                    .as_array()
                    .is_none_or(|values| values.iter().any(|item| item == value))
            };
            Ok(self
                .events
                .iter()
                .filter(|event| {
                    within("ids", &event.id)
                        && within("authors", &event.pubkey)
                        && filter["kinds"]
                            .as_array()
                            .is_none_or(|kinds| kinds.iter().any(|kind| kind == event.kind))
                        && ["d", "t", "e"].iter().all(|tag| {
                            filter[format!("#{tag}")].as_array().is_none_or(|wanted| {
                                event
                                    .tag_values(tag)
                                    .any(|value| wanted.iter().any(|item| item == value))
                            })
                        })
                })
                .cloned()
                .collect())
        }

        fn send(&mut self, event: Event) -> Result<(), String> {
            event.validate_crypto().map_err(|_| "bad signature")?;
            self.events.push(event);
            Ok(())
        }
    }

    /// A blob store in memory.
    #[derive(Default)]
    struct FakeBlobs {
        blobs: RefCell<BTreeMap<String, Vec<u8>>>,
    }

    impl Blobs for FakeBlobs {
        fn put(&self, bytes: &[u8], _: &str) -> Result<(), String> {
            self.blobs
                .borrow_mut()
                .insert(nostr::contracts::digest_bytes(bytes), bytes.to_vec());
            Ok(())
        }

        fn get(&self, digest: &str) -> Result<Vec<u8>, String> {
            self.blobs
                .borrow()
                .get(digest)
                .cloned()
                .ok_or_else(|| format!("no blob {digest}"))
        }

        fn locator(&self) -> Option<String> {
            Some("https://blobs.example".into())
        }
    }

    fn signer(byte: &str) -> RelaySigner {
        RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
    }

    /// A background plugin like `plugins/disk-cleanup`.
    fn plugin(dir: &Path, slug: &str, summary: &str, version: &str) {
        let rule = "{\"id\": \"rule\"}\n";
        std::fs::create_dir_all(dir.join("background")).unwrap();
        std::fs::create_dir_all(dir.join("evals/results/old")).unwrap();
        std::fs::write(dir.join("background/rule.json"), rule).unwrap();
        std::fs::write(dir.join("README.md"), "# Read me\n").unwrap();
        std::fs::write(dir.join("evals/results/old/report.json"), "{}").unwrap();
        std::fs::write(dir.join(".DS_Store"), "x").unwrap();
        std::fs::write(
            dir.join("package.json"),
            format!(
                "{{\n  \"v\": 1,\n  \"slug\": \"{slug}\",\n  \"name\": \"{slug} plugin\",\n  \"summary\": \"{summary}\",\n  \"version\": \"{version}\",\n  \"publisher\": \"\",\n  \"background\": [{{\"name\": \"rule\", \"digest\": \"{}\"}}]\n}}\n",
                coder::package::digest(rule)
            ),
        )
        .unwrap();
    }

    #[test]
    fn published_list_identifies_its_source_before_the_rows() {
        let text = published_list_text("wss://relay.example", "Project map · Maps files");
        assert!(text.lines().next().unwrap().contains("Published plugins"));
        assert!(
            text.lines()
                .next()
                .unwrap()
                .contains("not your installed plugins")
        );
        assert!(text.contains("openagents plugin installed"));
        assert!(text.ends_with("Project map · Maps files"));
    }

    #[test]
    fn search_never_lists_the_sample_plugins() {
        assert_eq!(super::catalog().len(), 6);
        let stand_in = Listing {
            package: format!("{}:explain-error-check", "1".repeat(64)),
            slug: "explain-error-check".into(),
            publisher: "1".repeat(64),
            title: "Explain this error (payment check)".into(),
            description: "Finds the project file a failing command points at.".into(),
            release: json!({"id": "a"}),
            blobs: Vec::new(),
            created_at: 9,
        };
        // A sample's own listing on the relay is dropped; others stay.
        let listed = super::catalog()[0].clone();
        let all = without_samples(vec![stand_in, listed]);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].slug, "explain-error-check");
        assert!(
            search(all.clone(), "project map")
                .iter()
                .all(|listing| listing.slug != "project-map")
        );
        // Install by exact name resolves the newest release the runner's
        // key signed.
        let mut relay = FakeRelay::default();
        assert!(
            find(&mut relay, "project-map")
                .unwrap_err()
                .contains("no published release yet")
        );
        assert_eq!(sats(15_000), "15 sats");
        assert_eq!(sats(1_000), "1 sat");
        assert_eq!(sats(1_500), "1.5 sats");
    }

    #[test]
    fn a_published_plugin_is_found_and_installs_off_with_every_file_checked() {
        let work = tempfile::tempdir().unwrap();
        let source = work.path().join("disk-cleanup");
        plugin(
            &source,
            "disk-cleanup",
            "Keeps the disk from filling up.",
            "0.1.0",
        );
        let other = work.path().join("other");
        plugin(&other, "lint-fixer", "Fixes lint in a project.", "1.0.0");
        let (alice, bob) = (signer("11"), signer("22"));
        let mut relay = FakeRelay::default();
        let blobs = FakeBlobs::default();

        let packed = pack(&source, alice.pubkey()).unwrap();
        let paths: Vec<&str> = packed.files.iter().map(|file| file.path.as_str()).collect();
        // Test results and hidden files stay home.
        assert_eq!(paths, ["README.md", "background/rule.json", "package.json"]);
        let fee = Fee {
            msat: 2_000,
            payout: "alice@getalby.com".into(),
        };
        let published = publish(&packed, &alice, &mut relay, &blobs, Some(&fee), 1_000).unwrap();
        assert!(!published.reused);
        let body = nostr::ext::parse_record(&published.release).unwrap();
        assert_eq!(body["fee_msat"], 2_000);
        assert_eq!(body["payout"], "alice@getalby.com");
        nostr::ext::parse_record(&published.listing).unwrap();
        publish(
            &pack(&other, bob.pubkey()).unwrap(),
            &bob,
            &mut relay,
            &blobs,
            None,
            1_001,
        )
        .unwrap();

        // Publishing the same bytes again reuses the release; other bytes
        // under the same version refuse.
        let again = publish(&packed, &alice, &mut relay, &blobs, Some(&fee), 1_002).unwrap();
        assert!(again.reused);
        assert_eq!(again.release.id, published.release.id);
        std::fs::write(source.join("README.md"), "# Changed\n").unwrap();
        let changed = pack(&source, alice.pubkey()).unwrap();
        let refused = publish(&changed, &alice, &mut relay, &blobs, Some(&fee), 1_003);
        assert!(refused.unwrap_err().contains("raise the version"));

        let all = listings(&mut relay, None, None).unwrap();
        assert_eq!(all.len(), 2);
        let hits = search(all.clone(), "disk filling");
        assert_eq!(hits[0].package, packed.package);
        assert_eq!(hits.len(), 1);
        assert_eq!(search(all.clone(), "lint-fixer")[0].slug, "lint-fixer");
        assert!(search(all.clone(), "spaceships").is_empty());
        assert_eq!(search(all, "").len(), 2);

        let listing = find(&mut relay, "disk-cleanup").unwrap();
        assert_eq!(listing.blobs, vec!["https://blobs.example".to_owned()]);
        assert_eq!(
            find(&mut relay, &packed.package).unwrap().package,
            packed.package
        );
        assert!(find(&mut relay, "nothing").is_err());

        let home = work.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let layout = background::Layout::new(&home, None).unwrap();
        let value = install_listing(&layout, &mut relay, &listing, &[&blobs]).unwrap();
        assert_eq!(value["plugin"]["enabled"], false, "{value}");
        assert_eq!(value["release"], published.release.id.as_str());
        assert_eq!(value["fee"]["fee_msat"], 2_000);
        let dir = PathBuf::from(value["plugin"]["dir"].as_str().unwrap());
        assert!(dir.starts_with(layout.extensions().join(alice.pubkey())));
        assert_eq!(
            std::fs::read_to_string(dir.join("README.md")).unwrap(),
            "# Read me\n"
        );
        assert!(!dir.join(".DS_Store").exists());
        assert!(dir.join("installed-from.json").is_file());
        let installed = Package::load(&dir.join("package.json")).unwrap();
        assert_eq!(installed.publisher, alice.pubkey());
    }

    #[test]
    fn a_tampered_blob_or_a_revoked_release_does_not_install() {
        let work = tempfile::tempdir().unwrap();
        let source = work.path().join("p");
        plugin(&source, "disk-cleanup", "Keeps the disk clear.", "0.1.0");
        let alice = signer("11");
        let mut relay = FakeRelay::default();
        let blobs = FakeBlobs::default();
        let packed = pack(&source, alice.pubkey()).unwrap();
        let published = publish(&packed, &alice, &mut relay, &blobs, None, 1_000).unwrap();
        let home = work.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let layout = background::Layout::new(&home, None).unwrap();
        let listing = find(&mut relay, "disk-cleanup").unwrap();

        // A blob server that answers other bytes for the README.
        let readme = packed
            .files
            .iter()
            .find(|file| file.path == "README.md")
            .unwrap();
        let tampered = FakeBlobs::default();
        for (digest, bytes) in blobs.blobs.borrow().iter() {
            let bytes = if *digest == nostr::contracts::digest_bytes(&readme.bytes) {
                b"# Evil\n".to_vec()
            } else {
                bytes.clone()
            };
            tampered.blobs.borrow_mut().insert(digest.clone(), bytes);
        }
        let refused = install_listing(&layout, &mut relay, &listing, &[&tampered]).unwrap_err();
        assert!(refused.contains("does not check"), "{refused}");
        assert!(background::plugins::installed(&layout).is_empty());

        // A listing another key signed for the same package cannot point
        // at a release Alice did not sign.
        let mut forged = listing.clone();
        forged.publisher = signer("22").pubkey().to_owned();
        assert!(install_listing(&layout, &mut relay, &forged, &[&blobs]).is_err());

        let revocation = alice.sign(
            1_001,
            nostr::ext::REVOCATION_KIND,
            vec![
                marker("revocation"),
                Tag::new(vec!["e".into(), published.release.id.clone()]),
            ],
            json!({
                "v": 1, "requires": [], "type": "revocation", "package": packed.package,
                "release": event_ref(&published.release), "reason": "broken", "effective_at": 1_001,
            })
            .to_string(),
        );
        relay.send(revocation).unwrap();
        let refused = install_listing(&layout, &mut relay, &listing, &[&blobs]).unwrap_err();
        assert!(refused.contains("revoked"), "{refused}");
    }

    #[test]
    fn a_record_naming_another_publisher_does_not_pack() {
        let work = tempfile::tempdir().unwrap();
        plugin(work.path(), "p", "x", "0.1.0");
        let text = std::fs::read_to_string(work.path().join("package.json")).unwrap();
        std::fs::write(
            work.path().join("package.json"),
            text.replace(
                "\"publisher\": \"\"",
                &format!("\"publisher\": \"{}\"", signer("22").pubkey()),
            ),
        )
        .unwrap();
        let refused = pack(work.path(), signer("11").pubkey()).unwrap_err();
        assert!(refused.contains("--as PROFILE"), "{refused}");
    }

    #[test]
    fn brainstorm_guidance_pins_only_inert_files_and_installs_without_native_enablement() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/brainstorm");
        let alice = signer("11");
        let packed = pack(&source, alice.pubkey()).unwrap();
        let manifest: Value = serde_json::from_slice(&packed.manifest).unwrap();
        assert_eq!(manifest["components"].as_array().unwrap().len(), 1);
        assert_eq!(manifest["components"][0]["kind"], "guidance");
        assert_eq!(manifest["components"][0]["slug"], "brainstorm");
        let record = Package::load(&source.join("package.json")).unwrap();
        assert!(record.program.is_none());
        assert!(record.background.is_empty());
        assert!(record.capabilities.is_empty());
        assert!(record.compatibility.is_empty());
        let required: Value = serde_json::from_slice(
            &packed
                .files
                .iter()
                .find(|f| f.path == "native-host-requirement.json")
                .unwrap()
                .bytes,
        )
        .unwrap();
        assert_eq!(required["host"], "coder-new");
        assert_eq!(required["version"], "1.0.0-rc.4");
        assert_eq!(
            required["source_revision"],
            "d2d546b9836779e3ca463fd77b4610210fbedf7f"
        );
        assert_eq!(required["descriptive_only"], true);
        assert_eq!(
            required["operations"],
            json!(["brainstorm_search_people", "brainstorm_rank"])
        );
        assert!(packed.files.iter().all(|f| f.path.ends_with(".md")
            || f.path.ends_with(".json")
            || f.path.ends_with(".txt")));
        for file in &packed.files {
            let pin = manifest["files"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["path"] == file.path)
                .unwrap();
            assert_eq!(pin["digest"], nostr::contracts::digest_bytes(&file.bytes));
            assert_eq!(pin["size"], file.bytes.len());
        }

        let mut relay = FakeRelay::default();
        let blobs = FakeBlobs::default();
        let published = publish(&packed, &alice, &mut relay, &blobs, None, 1000).unwrap();
        let body = nostr::ext::parse_record(&published.release).unwrap();
        assert!(body.get("fee_msat").is_none());
        let listing = find(&mut relay, &packed.package).unwrap();
        let work = tempfile::tempdir().unwrap();
        let layout = background::Layout::new(work.path(), None).unwrap();
        let installed = install_listing(&layout, &mut relay, &listing, &[&blobs]).unwrap();
        assert_eq!(installed["plugin"]["enabled"], false);
        assert_eq!(
            installed["manifest"],
            nostr::contracts::digest_bytes(&packed.manifest)
        );
        let into = Path::new(installed["plugin"]["dir"].as_str().unwrap());
        for file in &packed.files {
            assert_eq!(std::fs::read(into.join(&file.path)).unwrap(), file.bytes);
        }
        assert!(
            !work
                .path()
                .join(".openagents/coder-new/bundled-plugins.json")
                .exists()
        );
        assert!(crate::plugin_local::enable_exact_in(&layout, &packed.package, true, None).is_ok());
        assert!(
            !work
                .path()
                .join(".openagents/coder-new/bundled-plugins.json")
                .exists()
        );
        assert!(
            relay.events.iter().all(|e| e.kind != 0),
            "guidance operations publish no profile"
        );

        let readme = packed.files.iter().find(|f| f.path == "README.md").unwrap();
        blobs.blobs.borrow_mut().insert(
            nostr::contracts::digest_bytes(&readme.bytes),
            b"changed".to_vec(),
        );
        assert!(
            install_listing(&layout, &mut relay, &listing, &[&blobs])
                .unwrap_err()
                .contains("does not check")
        );
        let mut changed = packed.clone();
        changed.manifest.push(b' ');
        assert!(
            publish(&changed, &alice, &mut relay, &blobs, None, 1001)
                .unwrap_err()
                .contains("raise the version")
        );
    }
}

#[cfg(test)]
mod fee_option_tests {
    use super::*;
    #[test]
    fn sats_convert_without_overflow_or_ambiguous_units() {
        assert_eq!(
            publication_fee(None, Some("2"), Some("alice@example.com"))
                .unwrap()
                .unwrap()
                .msat,
            2000
        );
        assert!(
            publication_fee(
                None,
                Some("18446744073709551615"),
                Some("alice@example.com")
            )
            .is_err()
        );
        assert!(publication_fee(Some("1"), Some("1"), Some("alice@example.com")).is_err());
        assert!(publication_fee(None, Some("2"), None).is_err());
        assert!(publication_fee(None, Some("-1"), Some("alice@example.com")).is_err());
    }
}
