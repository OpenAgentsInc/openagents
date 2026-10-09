//! The Gym knowledge source behind the chat router's Gym and eval routes
//! ([`crate::router::seams::GymKb`]): verified records only, and retrieval
//! over them by embedding similarity and a typed Jev judgment.
//!
//! # What reaches the corpus
//!
//! Every record is admitted by code that checks it; nothing a model or a
//! message says becomes a record:
//!
//! - **Published results and checks**: `3189` events with the
//!   `oa:ext-eval:v1` marker, fetched from the relay ([`fetch_results`])
//!   and admitted only when `nostr::eval_ext::parse_publication` accepts
//!   them: the signature, both markers, the inline report against its
//!   digest and the `x` tag, the report under the profile, and every tag
//!   against what the report names. A check counts toward a result only as
//!   `nostr::eval_ext::linkage` reads it (same suite, subject, and lock; a
//!   different trainer).
//! - **Published test sets** (`eval-suite` releases) through a
//!   [`ReleaseReader`], which needs each release's manifest, suite, and
//!   case manifest bytes. The worker reads the starter test sets: the
//!   `3184` releases [`STARTER_PUBLISHERS`] signed (the hosted runner's
//!   key, which `knowledge/quests/ext-eval.*.json` name as their
//!   publisher), with their bytes fetched from [`SUITE_BLOBS`] and checked
//!   against every digest ([`suite_record`]). So the first run can start a
//!   test before anyone has published a result. A tool's test set is also
//!   read from the verified results that ran it.
//! - **Adoptions** (#9960): the newest `coder-defaults` release, the
//!   `3184` the package's root key ([`defaults_root`], from
//!   `packages/coder-defaults/package.json`) signed, with its manifest and
//!   each `openagents.eval-admission.v1` admission the manifest's
//!   provenance cites fetched from [`DEFAULTS_DOCUMENTS`] by digest and
//!   checked ([`adoption_records`]): one [`AdoptionRecord`] per `admit`
//!   decision, its subject matched to a catalog tool by slug. They are the
//!   admitted-capability set's adoptions
//!   ([`crate::router::capability::Admitted::of`]).
//! - **The app's changelog**, `CHANGELOG` in
//!   `crates/openagents-mobile/src/account.rs`, compiled in and read by
//!   [`changelog`].
//! - **Our product notes**: the admitted entries of
//!   `knowledge/openagents/` tagged `gym` (notes) or `tool` (the tool
//!   catalog), which [`knowledge::product`] already checked for sources.
//!
//! # Retrieval
//!
//! For `gym.news`, candidates are the [`CANDIDATES`] items nearest the
//! latest message by embedding cosine similarity together with the
//! [`NEWEST`] newest dated records (recency is a record's field, not a
//! word), and one Jev request asks a `relevant_N` Noul for each. Items at
//! or above the router's relevance floor are kept, most relevant first,
//! at most [`crate::router::gym::MAX_NEWS`]. There is no word-matching
//! fallback: without embeddings or the judge, the lookup fails and the
//! router answers as if there were no records.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::future::BoxFuture;
use jev::{Answer, Noul, NoulCriteria, Questions, RetryPolicy};
use knowledge::product::Corpus;
use knowledge::search::{Embed, Embedder, cosine};
use nostr::domain::Event;
use nostr::eval_ext::{self, Linkage, Publication};
use serde_json::{Value, json};

use crate::generate::{Message, Role};
use crate::product_kb::Judge;
use crate::router::RELEVANCE_FLOOR;
use crate::router::gym::{
    AdoptionRecord, Checks, EventPointer, Grounding, Item, MAX_NEWS, Note, Records, Release,
    ResultRecord, SuiteRecord, Tool,
};
use crate::router::seams::{GymKb, GymLookup, SeamError};

/// The relevance question set's identity, for evidence.
pub const SET: &str = "gym-news-relevance-v1";

/// How many nearest items Jev judges.
pub const CANDIDATES: usize = 8;

/// How many of the newest dated records are judged besides the nearest.
pub const NEWEST: usize = 4;

/// How many of the newest app builds the corpus holds.
pub const BUILDS: usize = 3;

/// The `coder-defaults` package record, `packages/coder-defaults/package.json`,
/// which names the root key that signs its releases.
pub const DEFAULTS_PACKAGE_RECORD: &str =
    include_str!("../../../packages/coder-defaults/package.json");

/// Where a `coder-defaults` document (a manifest or an admission) is
/// fetched by its digest, as `<base>/<hex>.json`: the repository's
/// `packages/coder-defaults/documents/`, where the operator's command keeps
/// each published document.
pub const DEFAULTS_DOCUMENTS: &str = "https://raw.githubusercontent.com/OpenAgentsInc/openagents/main/packages/coder-defaults/documents";

/// How many `coder-defaults` releases the relay read asks for; only the
/// newest is admitted.
pub const MAX_DEFAULTS: usize = 8;

/// The most admissions one `coder-defaults` release's documents are
/// fetched for.
pub const MAX_DEFAULT_ADMISSIONS: usize = 32;

/// The `coder-defaults` root key, hex, from the package record.
///
/// # Panics
///
/// Never for the checked-in record; a test checks it.
#[must_use]
pub fn defaults_root() -> String {
    let record: Value =
        serde_json::from_str(DEFAULTS_PACKAGE_RECORD).expect("the package record is JSON");
    record["root"]
        .as_str()
        .expect("the package record names its root")
        .to_owned()
}

/// `<root>:coder-defaults`, the package a defaults release names.
#[must_use]
pub fn defaults_package(root: &str) -> String {
    format!("{root}:coder-defaults")
}

/// The earlier turns Jev reads besides the latest message.
pub const EARLIER_TURNS: usize = 4;

/// The longest earlier turn Jev reads, in characters.
pub const TURN_CHARS: usize = 400;

/// NIP-EVAL's publication kind.
pub const PUBLICATION_KIND: u16 = 3189;

/// The most results one relay read admits.
pub const MAX_RESULTS: usize = 500;

/// How long one relay read may take.
pub const FETCH_BUDGET: Duration = Duration::from_secs(15);

/// The keys whose `eval-suite` releases are the starter test sets: the
/// hosted runner's, which released Project map's, Code finder's, and Test
/// reader's (`docs/deployment/eval-runner.md`). The starter quests
/// (`knowledge/quests/ext-eval.*.json`) list the same keys as the suites'
/// publishers; a test holds them equal.
pub const STARTER_PUBLISHERS: &[&str] = &[nostr::eval_ext::hosted::RUNNER];

/// Where a starter test set's files are read: the hosted runner's
/// public-read bucket, read like a Blossom server (`GET <base>/<sha256>`).
/// `CODER_EVAL_BLOBS` names another; `off` reads no releases.
pub const SUITE_BLOBS: &str = "https://storage.googleapis.com/openagentsgemini-eval-blobs";

/// The most test set releases one relay read admits.
pub const MAX_SUITES: usize = 64;

/// The largest release file fetched: a manifest, a suite, or a case
/// manifest.
pub const MAX_BLOB: usize = 1024 * 1024;

/// How long one file fetch may take.
pub const BLOB_BUDGET: Duration = Duration::from_secs(10);

/// How often the worker reads the relay for new results.
pub const REFRESH: Duration = Duration::from_secs(600);

/// The product note tag for a Gym note.
pub const GYM_TAG: &str = "gym";

/// The product note tag for a tool in the catalog.
pub const TOOL_TAG: &str = "tool";

/// The catalog's default tool (`CHK-07`): first in [`Records::tools`]
/// when its note exists. The sample plugins' notes are no longer shown, so
/// no note carries this id today and the catalog is empty.
pub const DEFAULT_TOOL: &str = "openagents.tool-project-map";

/// The hosted runner's catalog, compiled in: the plugin directories the
/// runner tests, in its order (#10090). The same file
/// `deploy/eval-runner/install.sh` turns into the runner's catalog. Its
/// packages are the runner's test fixtures and are not shown to people;
/// the plugins the chat shows are [`crate::builtin_plugins`].
pub const CATALOG_SOURCE: &str = include_str!("../../../deploy/eval-runner/catalog");

/// The catalog file's repository path.
pub const CATALOG_PATH: &str = "deploy/eval-runner/catalog";

/// The plugin directories [`CATALOG_SOURCE`] lists, in its order
/// (`crates/plugin-repo-map`, …): its lines that are neither blank nor
/// comments.
#[must_use]
pub fn catalog_dirs() -> Vec<&'static str> {
    CATALOG_SOURCE
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

/// Whether `subject` is one of the hosted runner's sample plugins
/// (`deploy/eval-runner/catalog`): signed by a [`STARTER_PUBLISHERS`] key
/// or the starter catalog's key. They are the runner's test fixtures and
/// are never shown to people.
#[must_use]
pub fn is_sample(subject: &nostr::contracts::DefinitionRef) -> bool {
    subject.id.split_once(':').is_some_and(|(key, _)| {
        STARTER_PUBLISHERS.contains(&key) || key == ext_eval::author::catalog::STARTER_KEY
    })
}

/// A catalog directory's component slug, the tag its tool note carries:
/// `crates/plugin-repo-map` is `repo-map`.
#[must_use]
pub fn catalog_slug(dir: &str) -> &str {
    let name = dir.rsplit('/').next().unwrap_or(dir);
    name.strip_prefix("plugin-").unwrap_or(name)
}

/// The changelog's path, which a build item cites.
pub const CHANGELOG_PATH: &str = "crates/openagents-mobile/src/account.rs";

/// The changelog source, compiled in.
const CHANGELOG_SOURCE: &str = include_str!("../../openagents-mobile/src/account.rs");

/// How long the Jev request may take.
pub const JUDGE_BUDGET: Duration = Duration::from_millis(1_500);

// ---------------------------------------------------------------------------
// The changelog
// ---------------------------------------------------------------------------

/// The string literal starting at `text`'s first `"`, unescaped, and the
/// rest after it.
fn literal(text: &str) -> Option<(String, &str)> {
    let start = text.find('"')?;
    let mut out = String::new();
    let mut chars = text[start + 1..].char_indices();
    while let Some((at, c)) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some((_, 'n')) => out.push(' '),
                Some((_, escaped)) => out.push(escaped),
                None => return None,
            },
            '"' => return Some((out, &text[start + 1 + at + 1..])),
            c => out.push(c),
        }
    }
    None
}

/// The value of the first `field: "…"` in `block`.
fn field(block: &str, field: &str) -> Option<String> {
    let at = block.find(&format!("{field}: \""))?;
    literal(&block[at + field.len()..]).map(|(value, _)| value)
}

/// The app's builds, newest first, from `CHANGELOG` in `source` (the
/// OpenAgents app's `account.rs`): each release's version, build, title,
/// and its items' titles. This reads a file of this repository for its
/// fixed shape; it reads no message.
#[must_use]
pub fn changelog_of(source: &str) -> Vec<Release> {
    let Some(start) = source.find("pub const CHANGELOG: &[Release] = &[") else {
        return Vec::new();
    };
    let body = &source[start..];
    // The list ends at the first line that closes it at the top level.
    let end = body.find("\n];").unwrap_or(body.len());
    let body = &body[..end];
    body.split("Release {")
        .skip(1)
        .filter_map(|block| {
            let (head, items) = block.split_once("items:").unwrap_or((block, ""));
            Some(Release {
                version: field(head, "version")?,
                build: field(head, "build")?,
                title: field(head, "title")?,
                items: items
                    .split("Item {")
                    .skip(1)
                    .filter_map(|item| field(item, "title"))
                    .collect(),
                source: CHANGELOG_PATH.to_string(),
            })
        })
        .collect()
}

/// The compiled-in changelog's builds, newest first.
#[must_use]
pub fn changelog() -> Vec<Release> {
    changelog_of(CHANGELOG_SOURCE)
}

// ---------------------------------------------------------------------------
// Product notes: Gym notes and the tool catalog
// ---------------------------------------------------------------------------

fn path_of(entry: &knowledge::Entry) -> String {
    format!("knowledge/{}/{}.md", knowledge::product::DIR, entry.id)
}

/// The tool catalog: the corpus's entries tagged `tool`, in the order of
/// the hosted runner's catalog ([`catalog_dirs`], whose first is the
/// default tool), each matched by its component slug among the note's
/// tags; a note the catalog doesn't list comes after, by id.
#[must_use]
pub fn tools(corpus: &Corpus) -> Vec<Tool> {
    let mut tools: Vec<Tool> = corpus
        .base
        .entries
        .iter()
        .filter(|entry| entry.tags.iter().any(|tag| tag == TOOL_TAG))
        .map(|entry| Tool {
            id: entry.id.clone(),
            name: entry.title.clone(),
            line: entry.summary.clone(),
            source: path_of(entry),
            slugs: entry
                .tags
                .iter()
                .filter(|tag| !matches!(tag.as_str(), TOOL_TAG | GYM_TAG))
                .cloned()
                .collect(),
        })
        .collect();
    let dirs = catalog_dirs();
    let place = |tool: &Tool| {
        let listed = dirs.iter().position(|dir| {
            let slug = catalog_slug(dir);
            tool.slugs.iter().any(|tag| tag == slug)
        });
        (tool.id != DEFAULT_TOOL, listed.unwrap_or(usize::MAX))
    };
    tools.sort_by(|a, b| place(a).cmp(&place(b)).then(a.id.cmp(&b.id)));
    tools
}

/// The hosted runner's sample plugins' old tool notes, kept as test
/// fixtures (`crates/coder/fixtures/gym-tools/`) so the catalog matching
/// stays tested while no product note is tagged `tool`.
#[cfg(test)]
pub(crate) fn fixture_tools() -> Vec<Tool> {
    let root = knowledge::product::repository();
    let corpus = Corpus::load(&root.join("crates/coder/fixtures/gym-tools"), Some(&root))
        .expect("the fixture tool notes load");
    tools(&corpus)
}

/// The Gym notes: the corpus's entries tagged `gym` and not `tool`.
#[must_use]
pub fn notes(corpus: &Corpus) -> Vec<Note> {
    corpus
        .base
        .entries
        .iter()
        .filter(|entry| {
            entry.tags.iter().any(|tag| tag == GYM_TAG)
                && !entry.tags.iter().any(|tag| tag == TOOL_TAG)
        })
        .map(|entry| Note {
            id: format!("{}@{}", entry.id, entry.version),
            title: entry.title.clone(),
            summary: entry.summary.clone(),
            source: path_of(entry),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Published records
// ---------------------------------------------------------------------------

fn pointer(event: &Event) -> EventPointer {
    EventPointer {
        id: event.id.clone(),
        pubkey: event.pubkey.clone(),
        kind: event.kind,
    }
}

/// The catalog tool a subject is, by its DefinitionRef's package or
/// component slug among the tool notes' slugs, and the name to show: the
/// catalog's, else the package's.
#[must_use]
pub fn tool_of(
    tools: &[Tool],
    subject: &nostr::contracts::DefinitionRef,
) -> (Option<String>, String) {
    let (package, component) = subject
        .id
        .split_once(':')
        .and_then(|(_, rest)| rest.split_once('/'))
        .unwrap_or(("", ""));
    match tools.iter().find(|tool| {
        tool.slugs
            .iter()
            .any(|slug| slug == package || slug == component)
    }) {
        Some(tool) => (Some(tool.id.clone()), tool.name.clone()),
        None => (None, package.replace(['-', '_'], " ")),
    }
}

/// Reads published test sets and adoptions from their NIP-EXT releases,
/// which needs each release's manifest bytes.
pub trait ReleaseReader: Send + Sync {
    /// A published test set.
    ///
    /// # Errors
    ///
    /// Why it is not one.
    fn suite(&self, event: &Event, tools: &[Tool]) -> Result<SuiteRecord, String>;
    /// The adoptions a `coder-defaults` release carries: one per `admit`
    /// decision its manifest cites.
    ///
    /// # Errors
    ///
    /// Why the release is not read.
    fn adoptions(&self, event: &Event, tools: &[Tool]) -> Result<Vec<AdoptionRecord>, String>;
}

/// No artifact fetcher yet: releases are not read, and a test set is read
/// from the verified results that ran it.
#[derive(Clone, Copy, Debug, Default)]
pub struct PendingReleases;

const PENDING: &str =
    "reading a release needs its manifest's bytes, and no artifact fetcher is wired";

impl ReleaseReader for PendingReleases {
    fn suite(&self, _: &Event, _: &[Tool]) -> Result<SuiteRecord, String> {
        Err(PENDING.to_string())
    }
    fn adoptions(&self, _: &Event, _: &[Tool]) -> Result<Vec<AdoptionRecord>, String> {
        Err(PENDING.to_string())
    }
}

/// Why a release was not read as a test set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReleaseError {
    /// A file with this digest is needed and not at hand yet.
    Missing(String),
    /// The release is refused; why, naming a check, never content.
    Refused(String),
}

impl std::fmt::Display for ReleaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReleaseError::Missing(digest) => write!(f, "{digest} is not fetched"),
            ReleaseError::Refused(why) => f.write_str(why),
        }
    }
}

fn refused(why: impl std::fmt::Display) -> ReleaseError {
    ReleaseError::Refused(why.to_string())
}

/// The DefinitionRef the chat names `tool` by when it offers a run: the
/// starter catalog's Wasm guest (`ext_eval::author::catalog`) whose
/// component is one of the tool's slugs. The hosted runner admits a
/// catalog tool by this reference.
#[must_use]
pub fn catalog_definition(tool: &Tool) -> Option<nostr::contracts::DefinitionRef> {
    ext_eval::author::catalog::Catalog::starter()
        .tools
        .into_iter()
        .find_map(|known| match known.source {
            ext_eval::author::catalog::Source::Existing(definition)
                if definition
                    .id
                    .rsplit_once('/')
                    .is_some_and(|(_, component)| tool.slugs.iter().any(|s| s == component)) =>
            {
                Some(definition)
            }
            _ => None,
        })
}

/// Reads a starter test set from its signed `3184` `release` and the
/// files `fetch` returns by digest: the release's signature and marker
/// (`nostr::ext::parse_record`), its manifest against the release's digest
/// (`nostr::eval_ext::parse_release`), and the suite and case manifest
/// against the manifest (`nostr::eval_ext::check_suite_package`). The
/// signer must be one of [`STARTER_PUBLISHERS`] and the package theirs;
/// the package `<slug>-tests` is the catalog tool with that slug, and the
/// test set's size is its case manifest's count.
///
/// # Errors
///
/// [`ReleaseError::Missing`] names a file `fetch` does not have yet;
/// [`ReleaseError::Refused`] names the check that failed.
pub fn suite_record(
    release: &Event,
    tools: &[Tool],
    fetch: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Result<SuiteRecord, ReleaseError> {
    if !STARTER_PUBLISHERS.contains(&release.pubkey.as_str()) {
        return Err(refused("the release is not a starter publisher's"));
    }
    let get = |digest: &str| fetch(digest).ok_or_else(|| ReleaseError::Missing(digest.to_string()));
    let body = nostr::ext::parse_record(release).map_err(refused)?;
    let manifest_ref =
        nostr::contracts::parse_artifact(body.get("manifest").unwrap_or(&Value::Null))
            .map_err(refused)?;
    let manifest_bytes = get(&manifest_ref.digest)?;
    let parsed = eval_ext::parse_release(release, &manifest_bytes).map_err(refused)?;
    let manifest: Value = serde_json::from_slice(&manifest_bytes).map_err(refused)?;
    let suite_ref = manifest
        .get("components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find(|component| {
                component.get("kind").and_then(Value::as_str) == Some(eval_ext::COMPONENT_KIND)
            })
        })
        .and_then(|component| component.get("definition"))
        .ok_or_else(|| refused("the release holds no eval-suite component"))?;
    let suite_ref = nostr::contracts::parse_artifact(suite_ref).map_err(refused)?;
    let suite_bytes = get(&suite_ref.digest)?;
    let suite = eval_ext::parse_suite(&suite_bytes).map_err(refused)?;
    let cases_bytes = get(&suite.cases.digest)?;
    let package =
        eval_ext::check_suite_package(&manifest, &suite_bytes, &cases_bytes).map_err(refused)?;
    let slug = parsed
        .package
        .strip_prefix(&format!("{}:", release.pubkey))
        .ok_or_else(|| refused("the package is not its signer's"))?;
    let tool_slug = slug
        .strip_suffix("-tests")
        .ok_or_else(|| refused("the package is not a tool's test set"))?;
    let tool = tools
        .iter()
        .find(|tool| tool.slugs.iter().any(|s| s == tool_slug))
        .ok_or_else(|| refused("the test set is for no tool in the catalog"))?;
    let subject =
        catalog_definition(tool).ok_or_else(|| refused("the tool has no catalog reference"))?;
    let at = pointer(release);
    Ok(SuiteRecord {
        release: at.clone(),
        tool: Some(tool.id.clone()),
        tool_name: tool.name.clone(),
        author: release.pubkey.clone(),
        subject,
        cases: package.cases.cases.len() as u64,
        at: release.created_at,
        source: at,
    })
}

/// Whether `release` names a test set's package (`<signer>:<slug>-tests`),
/// read from its content's bounded `package` field; [`suite_record`]
/// checks the rest.
#[must_use]
pub fn is_test_set_release(release: &Event) -> bool {
    serde_json::from_str::<Value>(&release.content)
        .ok()
        .and_then(|body| body.get("package")?.as_str().map(str::to_string))
        .is_some_and(|package| package.ends_with("-tests"))
}

/// Reads the adoptions of a `coder-defaults` release from its signed
/// `3184` `release` and the documents `fetch` returns by digest: the
/// release's signature and marker (`nostr::ext::parse_record`), its
/// manifest against the release's digest (`nostr::eval_ext::parse_release`),
/// and each admission the manifest's provenance cites against its digest
/// (`nostr::eval_ext::parse_admission`). The signer must be `root` and the
/// package `<root>:coder-defaults`; each `admit` decision is one record,
/// its subject matched to the catalog by slug ([`tool_of`]). A `reject` or
/// `inconclusive` decision adopts nothing.
///
/// # Errors
///
/// [`ReleaseError::Missing`] names a document `fetch` does not have yet;
/// [`ReleaseError::Refused`] names the check that failed.
pub fn adoption_records(
    release: &Event,
    tools: &[Tool],
    root: &str,
    fetch: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Result<Vec<AdoptionRecord>, ReleaseError> {
    if release.pubkey != root {
        return Err(refused("the release is not the coder-defaults root's"));
    }
    let get = |digest: &str| fetch(digest).ok_or_else(|| ReleaseError::Missing(digest.to_string()));
    let body = nostr::ext::parse_record(release).map_err(refused)?;
    let manifest_ref =
        nostr::contracts::parse_artifact(body.get("manifest").unwrap_or(&Value::Null))
            .map_err(refused)?;
    let manifest_bytes = get(&manifest_ref.digest)?;
    let parsed = eval_ext::parse_release(release, &manifest_bytes).map_err(refused)?;
    if parsed.package != defaults_package(root) {
        return Err(refused("the package is not coder-defaults"));
    }
    let at = pointer(release);
    let mut records = Vec::new();
    for artifact in &parsed.admissions {
        let bytes = get(&artifact.digest)?;
        if nostr::contracts::digest_bytes(&bytes) != artifact.digest
            || bytes.len() as u64 != artifact.size
        {
            return Err(refused("an admission's bytes do not match its digest"));
        }
        let admission = eval_ext::parse_admission(&bytes).map_err(refused)?;
        if admission.issuer != root {
            return Err(refused("an admission is not the root's decision"));
        }
        if admission.decision != "admit" || is_sample(&admission.subject) {
            continue;
        }
        let (tool, tool_name) = tool_of(tools, &admission.subject);
        records.push(AdoptionRecord {
            release: at.clone(),
            tool,
            tool_name,
            at: release.created_at,
        });
    }
    Ok(records)
}

/// Whether `release` is signed by the `coder-defaults` root: the newest
/// such release is read for adoptions ([`adoption_records`] checks the
/// rest).
#[must_use]
pub fn is_defaults_release(release: &Event, root: &str) -> bool {
    release.kind == nostr::ext::RELEASE_KIND && release.pubkey == root
}

/// Releases read from files already fetched, by digest.
#[derive(Clone, Debug)]
pub struct Fetched {
    pub files: HashMap<String, Arc<Vec<u8>>>,
    /// The `coder-defaults` root, whose releases carry adoptions.
    pub root: String,
}

impl Default for Fetched {
    fn default() -> Self {
        Self {
            files: HashMap::new(),
            root: defaults_root(),
        }
    }
}

impl ReleaseReader for Fetched {
    fn suite(&self, event: &Event, tools: &[Tool]) -> Result<SuiteRecord, String> {
        suite_record(event, tools, &|digest| {
            self.files.get(digest).map(|bytes| bytes.as_ref().clone())
        })
        .map_err(|error| error.to_string())
    }
    fn adoptions(&self, event: &Event, tools: &[Tool]) -> Result<Vec<AdoptionRecord>, String> {
        adoption_records(event, tools, &self.root, &|digest| {
            self.files.get(digest).map(|bytes| bytes.as_ref().clone())
        })
        .map_err(|error| error.to_string())
    }
}

/// What [`admit`] did with a batch of events.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Admitted {
    pub results: Vec<ResultRecord>,
    pub suites: Vec<SuiteRecord>,
    pub adoptions: Vec<AdoptionRecord>,
    /// Refused events: id and why, naming a check, never content.
    pub refused: Vec<(String, String)>,
}

/// Admits `results` (`3189` events) that `nostr::eval_ext` verifies,
/// counts each original result's checks as `nostr::eval_ext::linkage`
/// reads them, and admits `suites` and `adoptions` through `releases`.
#[must_use]
pub fn admit(
    tools: &[Tool],
    results: &[Event],
    releases: &dyn ReleaseReader,
    suites: &[Event],
    adoptions: &[Event],
) -> Admitted {
    let mut admitted = Admitted::default();
    let mut read: Vec<(Publication, EventPointer)> = Vec::new();
    for event in results.iter().take(MAX_RESULTS) {
        match eval_ext::parse_publication(event) {
            Ok(publication) => {
                if !read.iter().any(|(seen, _)| seen.id == publication.id) {
                    read.push((publication, pointer(event)));
                }
            }
            Err(why) => admitted.refused.push((event.id.clone(), why.to_string())),
        }
    }
    // The newest subject lock per test set and subject, over every result
    // and check: what the hosted runner runs now, as far as the relay
    // shows. A check confirms only a result with the same lock.
    let mut locks: std::collections::BTreeMap<(&str, &str), (u64, &str)> =
        std::collections::BTreeMap::new();
    for (publication, _) in &read {
        let key = (
            publication.suite_release.id.as_str(),
            publication.report.subject.definition.id.as_str(),
        );
        let lock = (
            publication.created_at,
            publication.report.subject.lock.digest.as_str(),
        );
        let newest = locks.entry(key).or_insert(lock);
        if lock.0 > newest.0 {
            *newest = lock;
        }
    }
    for (publication, event) in &read {
        // The hosted runner's sample plugins are its test fixtures, never
        // shown: their results stay out of the Gym's news and cards.
        if is_sample(&publication.report.subject.definition) {
            continue;
        }
        let current = locks
            .get(&(
                publication.suite_release.id.as_str(),
                publication.report.subject.definition.id.as_str(),
            ))
            .is_none_or(|(_, lock)| *lock == publication.report.subject.lock.digest);
        let mut checked = Checks::default();
        for (check, _) in &read {
            match eval_ext::linkage(publication, check) {
                Linkage::Confirm => checked.confirmed += 1,
                Linkage::Dispute => checked.disputed += 1,
                Linkage::NotACheck => {}
            }
        }
        let subject = publication.report.subject.definition.clone();
        let (tool, tool_name) = tool_of(tools, &subject);
        admitted.results.push(ResultRecord {
            publication: event.clone(),
            tool,
            tool_name,
            trainer: publication.trainer().to_string(),
            suite: publication.suite_release.clone(),
            cases: publication.report.profile.cases.len() as u64,
            subject,
            headline: publication.report.profile.headline,
            verdict: publication.verdict(),
            report: publication.report_ref.clone(),
            checks: publication.checks.clone(),
            checked,
            current,
            at: publication.created_at,
        });
    }
    for event in suites {
        match releases.suite(event, tools) {
            Ok(suite) => admitted.suites.push(suite),
            Err(why) => admitted.refused.push((event.id.clone(), why)),
        }
    }
    for event in adoptions {
        match releases.adoptions(event, tools) {
            Ok(records) => admitted.adoptions.extend(records),
            Err(why) => admitted.refused.push((event.id.clone(), why)),
        }
    }
    admitted
        .results
        .sort_by_key(|record| std::cmp::Reverse(record.at));
    admitted
        .suites
        .sort_by_key(|record| std::cmp::Reverse(record.at));
    admitted
        .adoptions
        .sort_by_key(|record| std::cmp::Reverse(record.at));
    admitted
}

/// The relay filter for published results.
#[must_use]
pub fn filter() -> Value {
    json!({
        "kinds": [PUBLICATION_KIND],
        "#t": [eval_ext::PROFILE_MARKER],
        "limit": MAX_RESULTS,
    })
}

/// The relay filter for the starter test sets: the `3184` releases
/// [`STARTER_PUBLISHERS`] signed.
#[must_use]
pub fn suite_filter() -> Value {
    json!({
        "kinds": [nostr::ext::RELEASE_KIND],
        "authors": STARTER_PUBLISHERS,
        "#t": ["oa:ext:release:v1"],
        "limit": MAX_SUITES,
    })
}

/// The relay filter for the `coder-defaults` releases: the `3184` releases
/// `root` signed, newest first.
#[must_use]
pub fn defaults_filter(root: &str) -> Value {
    json!({
        "kinds": [nostr::ext::RELEASE_KIND],
        "authors": [root],
        "#t": ["oa:ext:release:v1"],
        "limit": MAX_DEFAULTS,
    })
}

/// Reads the published results, the starter test sets, and the
/// `coder-defaults` releases from the relay at `url`, as `identity`: one
/// subscription with [`filter`], [`suite_filter`], and
/// [`defaults_filter`], until the relay's end of stored events or
/// [`FETCH_BUDGET`]. The events are unverified here; [`admit`] checks each
/// one.
///
/// # Errors
///
/// The connection or the subscription fails.
pub async fn fetch_results(
    url: &str,
    identity: &crate::relay::Identity,
) -> Result<Vec<Event>, String> {
    use futures_util::StreamExt;
    let mut socket = crate::relay::connect(url, identity)
        .await
        .map_err(|error| error.to_string())?;
    let id = "gym-results";
    crate::relay::send(
        &mut socket,
        json!([
            "REQ",
            id,
            filter(),
            suite_filter(),
            defaults_filter(&defaults_root())
        ]),
    )
    .await
    .map_err(|error| error.to_string())?;
    let mut events = Vec::new();
    let reading = async {
        while let Some(frame) = socket.next().await {
            let Ok(tokio_tungstenite::tungstenite::Message::Text(text)) = frame else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            match (value[0].as_str(), value[1].as_str()) {
                (Some("EVENT"), Some(sub)) if sub == id => {
                    if let Ok(event) = serde_json::from_value::<Event>(value[2].clone()) {
                        events.push(event);
                        if events.len() >= MAX_RESULTS + MAX_SUITES + MAX_DEFAULTS {
                            break;
                        }
                    }
                }
                (Some("EOSE" | "CLOSED"), Some(sub)) if sub == id => break,
                _ => {}
            }
        }
    };
    let finished = tokio::time::timeout(FETCH_BUDGET, reading).await;
    let _ = crate::relay::send(&mut socket, json!(["CLOSE", id])).await;
    let _ = socket.close(None).await;
    finished.map_err(|_| format!("no end of stored events in {} s", FETCH_BUDGET.as_secs()))?;
    Ok(events)
}

// ---------------------------------------------------------------------------
// The seam
// ---------------------------------------------------------------------------

/// The Gym's records and their retrieval.
pub struct GymKnowledge<E: Embed = Embedder> {
    /// Shared, with the fetched files and the vectors, with every copy lent
    /// to a job on the caller's keys ([`GymKnowledge::lent`]).
    records: Arc<Mutex<Records>>,
    embedder: E,
    recipient: String,
    judge: Arc<dyn Judge>,
    /// Where the starter test sets' files are fetched, or `None` to read
    /// no releases.
    blobs: Option<String>,
    /// Where the `coder-defaults` documents are fetched, or `None` to read
    /// no adoptions.
    documents: Option<String>,
    /// Release files fetched, by digest; content-addressed, so kept.
    files: Arc<Mutex<HashMap<String, Arc<Vec<u8>>>>>,
    http: reqwest::Client,
    /// Item vectors by item id and the model that made them.
    vectors: Arc<Mutex<HashMap<String, Arc<Vec<f32>>>>>,
}

impl GymKnowledge<Embedder> {
    /// The committed product corpus's tools and Gym notes, the compiled-in
    /// changelog, embeddings from the product KB's configuration, and
    /// `judge`; the starter test sets' files from `CODER_EVAL_BLOBS`, else
    /// [`SUITE_BLOBS`] (`off` reads no releases).
    ///
    /// # Errors
    ///
    /// The corpus does not load, or no embedder is set up.
    pub fn from_env(judge: Arc<dyn Judge>) -> Result<Self, String> {
        let dir = knowledge::product::default_dir();
        let root = knowledge::product::repository();
        let corpus = Corpus::load(&dir, root.join("knowledge").exists().then_some(&*root))?;
        let embedder = crate::product_kb::embedder_from_env()?;
        let recipient = crate::codebase::embedding_recipient(embedder.provider).to_string();
        let blobs = match std::env::var("CODER_EVAL_BLOBS") {
            Ok(value) if value.trim() == "off" => None,
            Ok(value) if !value.trim().is_empty() => Some(value.trim().to_string()),
            _ => Some(SUITE_BLOBS.to_string()),
        };
        let documents = match std::env::var("CODER_DEFAULTS_DOCUMENTS") {
            Ok(value) if value.trim() == "off" => None,
            Ok(value) if !value.trim().is_empty() => Some(value.trim().to_string()),
            _ => blobs.as_ref().map(|_| DEFAULTS_DOCUMENTS.to_string()),
        };
        Ok(GymKnowledge::new(&corpus, embedder, recipient, judge, blobs).with_documents(documents))
    }
}

impl<E: Embed> GymKnowledge<E> {
    /// The Gym's records from `corpus` and the changelog, before any
    /// published record is admitted; release files are fetched from
    /// `blobs` (`None`: releases are not read).
    pub fn new(
        corpus: &Corpus,
        embedder: E,
        recipient: impl Into<String>,
        judge: Arc<dyn Judge>,
        blobs: Option<String>,
    ) -> Self {
        let records = Records {
            tools: tools(corpus),
            releases: changelog().into_iter().take(BUILDS).collect(),
            notes: notes(corpus),
            ..Records::default()
        };
        GymKnowledge {
            records: Arc::new(Mutex::new(records)),
            embedder,
            recipient: recipient.into(),
            judge,
            documents: blobs.as_ref().map(|_| DEFAULTS_DOCUMENTS.to_string()),
            blobs: blobs.map(|base| base.trim_end_matches('/').to_string()),
            files: Arc::new(Mutex::new(HashMap::new())),
            http: reqwest::Client::builder()
                .timeout(BLOB_BUDGET)
                .build()
                .unwrap_or_default(),
            vectors: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// The same records, files, and vectors for one job on the caller's
    /// own keys (BYOK): the message, and any record not yet embedded, is
    /// embedded with `embedder` and judged by `judge`, both on their keys.
    pub fn lent<F: Embed>(
        &self,
        embedder: F,
        recipient: impl Into<String>,
        judge: Arc<dyn Judge>,
    ) -> GymKnowledge<F> {
        GymKnowledge {
            records: self.records.clone(),
            embedder,
            recipient: recipient.into(),
            judge,
            blobs: self.blobs.clone(),
            documents: self.documents.clone(),
            files: self.files.clone(),
            http: self.http.clone(),
            vectors: self.vectors.clone(),
        }
    }

    /// The same seam fetching `coder-defaults` documents from `documents`
    /// (`None`: adoptions are not read).
    #[must_use]
    pub fn with_documents(mut self, documents: Option<String>) -> Self {
        self.documents = documents.map(|base| base.trim_end_matches('/').to_string());
        self
    }

    /// The records as of now.
    #[must_use]
    pub fn records(&self) -> Records {
        self.records
            .lock()
            .map(|records| records.clone())
            .unwrap_or_default()
    }

    /// Reads the relay at `url` for published results and the starter
    /// test sets, fetches the test sets' files, and admits the verified
    /// ones in place of the last read's.
    ///
    /// # Errors
    ///
    /// The relay read fails; the records keep the last read's.
    pub async fn refresh(
        &self,
        url: &str,
        identity: &crate::relay::Identity,
    ) -> Result<Admitted, String> {
        let events = fetch_results(url, identity).await?;
        let tools = self.records().tools;
        let root = defaults_root();
        let (releases, results): (Vec<Event>, Vec<Event>) = events
            .into_iter()
            .partition(|event| event.kind == nostr::ext::RELEASE_KIND);
        // The newest coder-defaults release carries every adoption.
        let defaults: Vec<Event> = releases
            .iter()
            .filter(|event| is_defaults_release(event, &root))
            .max_by_key(|event| (event.created_at, event.id.clone()))
            .filter(|_| self.documents.is_some())
            .cloned()
            .into_iter()
            .collect();
        // The runner also releases its catalog tools under the same
        // marker; a test set's package is `<slug>-tests`.
        let suites: Vec<Event> = releases
            .into_iter()
            .filter(|event| !is_defaults_release(event, &root))
            .filter(is_test_set_release)
            .take(MAX_SUITES)
            .collect();
        let admitted = if self.blobs.is_some() {
            for release in &suites {
                self.fetch_release(release, &tools).await;
            }
            for release in &defaults {
                self.fetch_defaults(release, &tools, &root).await;
            }
            let fetched = Fetched {
                files: self
                    .files
                    .lock()
                    .map(|files| files.clone())
                    .unwrap_or_default(),
                root,
            };
            admit(&tools, &results, &fetched, &suites, &defaults)
        } else {
            admit(&tools, &results, &PendingReleases, &suites, &[])
        };
        self.publish(&admitted);
        Ok(admitted)
    }

    /// Fetches the documents a `coder-defaults` `release` needs, one digest
    /// at a time as [`adoption_records`] asks for them, each checked
    /// against its digest and kept. A failure leaves the document missing,
    /// which [`admit`] reports.
    async fn fetch_defaults(&self, release: &Event, tools: &[Tool], root: &str) {
        let Some(base) = self.documents.clone() else {
            return;
        };
        // A manifest and at most a bounded number of admissions.
        for _ in 0..=MAX_DEFAULT_ADMISSIONS {
            let missing = {
                let Ok(files) = self.files.lock() else {
                    return;
                };
                match adoption_records(release, tools, root, &|digest| {
                    files.get(digest).map(|bytes| bytes.as_ref().clone())
                }) {
                    Err(ReleaseError::Missing(digest)) => digest,
                    _ => return,
                }
            };
            match self.fetch_file_from(&base, &missing, ".json").await {
                Ok(bytes) => {
                    if let Ok(mut files) = self.files.lock() {
                        files.insert(missing, Arc::new(bytes));
                    }
                }
                Err(why) => {
                    eprintln!("gym records: a coder-defaults document was not fetched: {why}");
                    return;
                }
            }
        }
    }

    /// Fetches the files `release` needs, one digest at a time as
    /// [`suite_record`] asks for them, each checked against its digest and
    /// kept. A failure leaves the file missing, which [`admit`] reports.
    async fn fetch_release(&self, release: &Event, tools: &[Tool]) {
        // A manifest, a suite, and a case manifest.
        for _ in 0..3 {
            let missing = {
                let Ok(files) = self.files.lock() else {
                    return;
                };
                match suite_record(release, tools, &|digest| {
                    files.get(digest).map(|bytes| bytes.as_ref().clone())
                }) {
                    Err(ReleaseError::Missing(digest)) => digest,
                    _ => return,
                }
            };
            match self.fetch_file(&missing).await {
                Ok(bytes) => {
                    if let Ok(mut files) = self.files.lock() {
                        files.insert(missing, Arc::new(bytes));
                    }
                }
                Err(why) => {
                    eprintln!("gym records: a test set's file was not fetched: {why}");
                    return;
                }
            }
        }
    }

    /// One file from the blob store, by its `sha256:` digest, checked.
    async fn fetch_file(&self, digest: &str) -> Result<Vec<u8>, String> {
        let base = self.blobs.clone().ok_or("no blob store")?;
        self.fetch_file_from(&base, digest, "").await
    }

    /// One file at `<base>/<hex><suffix>`, by its `sha256:` digest, checked
    /// against it.
    async fn fetch_file_from(
        &self,
        base: &str,
        digest: &str,
        suffix: &str,
    ) -> Result<Vec<u8>, String> {
        let hex = digest
            .strip_prefix("sha256:")
            .filter(|hex| hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()))
            .ok_or_else(|| format!("{digest} is not a sha256 digest"))?;
        let response = self
            .http
            .get(format!("{base}/{hex}{suffix}"))
            .send()
            .await
            .map_err(|error| format!("{hex}: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("{hex}: {}", response.status()));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_BLOB as u64)
        {
            return Err(format!("{hex}: larger than {MAX_BLOB} bytes"));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|error| format!("{hex}: {error}"))?;
        if bytes.len() > MAX_BLOB {
            return Err(format!("{hex}: larger than {MAX_BLOB} bytes"));
        }
        if nostr::contracts::digest_bytes(&bytes) != digest {
            return Err(format!("{hex}: the bytes do not match their digest"));
        }
        Ok(bytes.to_vec())
    }

    /// Replaces the published records with what `admitted` holds.
    pub fn publish(&self, admitted: &Admitted) {
        if let Ok(mut records) = self.records.lock() {
            records.results = admitted.results.clone();
            records.suites = admitted.suites.clone();
            records.adoptions = admitted.adoptions.clone();
        }
    }

    /// The embedding provider, named for a person.
    #[must_use]
    pub fn recipient(&self) -> &str {
        &self.recipient
    }

    /// Vectors for `items`, embedding only the ones not yet embedded.
    async fn vectors(&self, items: &[Item]) -> Result<Vec<Arc<Vec<f32>>>, SeamError> {
        let key = |item: &Item| format!("{}\n{}", self.embedder.model(), item.id());
        let missing: Vec<&Item> = {
            let known = self
                .vectors
                .lock()
                .map_err(|_| SeamError::Failed("the vector cache is poisoned".to_string()))?;
            items
                .iter()
                .filter(|item| !known.contains_key(&key(item)))
                .collect()
        };
        if !missing.is_empty() {
            let (vectors, _) = self
                .embedder
                .embed(missing.iter().map(|item| item.text()).collect())
                .await
                .map_err(|e| SeamError::Failed(format!("embedding the records failed: {e}")))?;
            if vectors.len() != missing.len() {
                return Err(SeamError::Failed(format!(
                    "{} vectors came back for {} records",
                    vectors.len(),
                    missing.len()
                )));
            }
            if let Ok(mut known) = self.vectors.lock() {
                for (item, vector) in missing.iter().zip(vectors) {
                    known.insert(key(item), Arc::new(vector));
                }
            }
        }
        let known = self
            .vectors
            .lock()
            .map_err(|_| SeamError::Failed("the vector cache is poisoned".to_string()))?;
        items
            .iter()
            .map(|item| {
                known
                    .get(&key(item))
                    .cloned()
                    .ok_or_else(|| SeamError::Failed("a record has no vector".to_string()))
            })
            .collect()
    }

    /// Embeds every record now, so the first question does not wait.
    ///
    /// # Errors
    ///
    /// The embedding call fails.
    pub async fn warm(&self) -> Result<(), SeamError> {
        self.vectors(&self.records().items()).await.map(|_| ())
    }

    /// The candidates for `message`: the [`CANDIDATES`] nearest items and
    /// the [`NEWEST`] newest dated records, nearest first, each once.
    ///
    /// # Errors
    ///
    /// An embedding call fails.
    pub async fn candidates(&self, message: &str, items: &[Item]) -> Result<Vec<Item>, SeamError> {
        let vectors = self.vectors(items).await?;
        let (mut query, _) = self
            .embedder
            .embed(vec![
                message
                    .chars()
                    .take(knowledge::search::QUERY_CHARS)
                    .collect(),
            ])
            .await
            .map_err(|e| SeamError::Failed(format!("embedding the message failed: {e}")))?;
        let query = query
            .pop()
            .ok_or_else(|| SeamError::Failed("no vector came back for the message".to_string()))?;
        let mut ranked: Vec<(usize, f64)> = vectors
            .iter()
            .enumerate()
            .map(|(n, vector)| (n, cosine(vector, &query)))
            .collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut chosen: Vec<usize> = ranked.iter().take(CANDIDATES).map(|(n, _)| *n).collect();
        // `items` lists dated records newest first.
        for (n, item) in items.iter().enumerate() {
            if item.at().is_some()
                && !chosen.contains(&n)
                && chosen.iter().filter(|m| items[**m].at().is_some()).count() < NEWEST
            {
                chosen.push(n);
            }
        }
        Ok(chosen.into_iter().map(|n| items[n].clone()).collect())
    }

    /// The items relevant to `lookup`, with Jev's relevance, most relevant
    /// first (newer first on a tie), at most [`MAX_NEWS`].
    ///
    /// # Errors
    ///
    /// An embedding call or the Jev request fails; the error names no
    /// message text.
    pub async fn news(&self, lookup: &GymLookup) -> Result<Vec<(Item, f64)>, SeamError> {
        let items = self.records().items();
        if items.is_empty() {
            return Ok(Vec::new());
        }
        let candidates = self.candidates(&lookup.message, &items).await?;
        let request =
            jev::SystemOneRequest::new(state(lookup, &candidates), questions(&candidates))
                .retry(RetryPolicy {
                    max_retries: 0,
                    budget: Some(JUDGE_BUDGET),
                    ..RetryPolicy::default()
                })
                .timeout(JUDGE_BUDGET);
        let response = self
            .judge
            .judge(request)
            .await
            .map_err(|e| SeamError::Failed(format!("the relevance judgment failed: {e}")))?;
        Ok(kept(&response, candidates))
    }
}

/// The key a candidate has in the state and its question id.
fn key(n: usize) -> String {
    format!("item_{}", n + 1)
}

/// What Jev reads: the latest message, the earlier turns, and the
/// candidates' texts.
#[must_use]
pub fn state(lookup: &GymLookup, items: &[Item]) -> Value {
    let earlier: Vec<Value> = lookup
        .transcript
        .iter()
        .rev()
        .skip_while(|m| m.role == Role::User && m.text.trim() == lookup.message.trim())
        .take(EARLIER_TURNS)
        .collect::<Vec<&Message>>()
        .into_iter()
        .rev()
        .map(|m| {
            json!({
                "role": match m.role { Role::User => "user", Role::Assistant => "assistant" },
                "text": m.text.chars().take(TURN_CHARS).collect::<String>(),
            })
        })
        .collect();
    let records: serde_json::Map<String, Value> = items
        .iter()
        .enumerate()
        .map(|(n, item)| {
            (
                key(n),
                json!({ "kind": item.kind(), "record": item.text() }),
            )
        })
        .collect();
    json!({
        "product": "The OpenAgents Gym, where people test tools on Coder, check each other's results, and earn XP",
        "latest_message": lookup.message,
        "earlier_turns": earlier,
        "records": records,
    })
}

/// One `relevant_N` Noul per candidate.
#[must_use]
pub fn questions(items: &[Item]) -> Questions {
    let mut questions = Questions::new();
    for n in 0..items.len() {
        let id = key(n);
        questions = questions.with(
            format!("relevant_{}", n + 1),
            Noul::with_criteria(
                format!(
                    "Would a good reply to the user's latest message, about what is new or in \
                     progress in the Gym, mention {id} in the state's records?"
                ),
                NoulCriteria::new()
                    .when_true(format!(
                        "Yes: {id} is news the user asked about, or part of what is new or in \
                         progress that they asked for"
                    ))
                    .when_false(format!(
                        "No: {id} is about something the user did not ask about, or only shares \
                         words with the message"
                    )),
            ),
        );
    }
    questions
}

/// The candidates at or above the relevance floor, most relevant first,
/// then newer first, at most [`MAX_NEWS`].
#[must_use]
pub fn kept(response: &jev::SystemOneResponse, candidates: Vec<Item>) -> Vec<(Item, f64)> {
    let relevance = |n: usize| match response.answers.get(&format!("relevant_{}", n + 1)) {
        Some(Answer::Noul(noul)) if noul.noul.is_finite() => noul.noul.clamp(0.0, 1.0),
        _ => 0.0,
    };
    let mut kept: Vec<(Item, f64)> = candidates
        .into_iter()
        .enumerate()
        .map(|(n, item)| (item, relevance(n)))
        .filter(|(_, p)| *p >= RELEVANCE_FLOOR)
        .collect();
    kept.sort_by(|(a, pa), (b, pb)| pb.total_cmp(pa).then(b.at().cmp(&a.at())));
    kept.truncate(MAX_NEWS);
    kept
}

impl GymKb for GymKnowledge<Embedder> {
    fn available(&self) -> bool {
        let records = self.records();
        !records.tools.is_empty() || !records.items().is_empty()
    }

    fn recipients(&self) -> Vec<String> {
        vec![self.recipient.clone()]
    }

    fn tools(&self) -> Vec<Tool> {
        self.records().tools
    }

    fn adoptions(&self) -> Vec<AdoptionRecord> {
        self.records().adoptions
    }

    fn ground<'a>(&'a self, lookup: &'a GymLookup) -> BoxFuture<'a, Result<Grounding, SeamError>> {
        Box::pin(async move {
            let news = if lookup.route == crate::router::RouteId::GymNews {
                self.news(lookup).await?
            } else {
                Vec::new()
            };
            Ok(Grounding {
                records: self.records(),
                news,
            })
        })
    }

    fn on_their_keys(&self, theirs: &crate::router::seams::TheirKeys) -> Option<Arc<dyn GymKb>> {
        let embedder = theirs.embedder()?;
        let recipient = crate::codebase::embedding_recipient(embedder.provider).to_string();
        Some(Arc::new(self.lent(
            embedder,
            recipient,
            theirs.judge.clone(),
        )))
    }
}

#[cfg(test)]
mod tests;
