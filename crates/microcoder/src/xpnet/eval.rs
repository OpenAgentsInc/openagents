//! The extension evaluation referee and adoption into Coder's defaults
//! (`docs/extensions/evaluation.md`, "Checks, adoption, and credit").
//!
//! `xp referee` is one pass of the automated referee job: it reads
//! `oa:ext-eval:v1` results and checks and the `coder-defaults` releases
//! from the relay, publishes the quest version each confirmed check or
//! adoption needs (from the templates in `knowledge/quests/ext-eval.*.json`),
//! and signs the `eval-check` and `eval-adopt` awards the rules accept,
//! with the referee key, which never leaves its host. A key that already
//! holds a live award is never signed again, so a rerun signs nothing
//! twice. Each refusal is logged once, with event IDs and the rule's
//! reason and never a report's or message's text. It also writes the
//! adoption queue: tools with a Better result that checks by at least
//! three distinct trainers confirmed.
//!
//! `xp adopt` is the operator's command. It lists the queue, and with
//! `--subject` it adopts one candidate: it writes the
//! `openagents.eval-admission.v1` decision, the next `coder-defaults`
//! manifest, which depends on the tool's release, and publishes the
//! release signed with the `coder-defaults` key. Adoption is never
//! automatic. Credit is XP; nothing here pays.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use coder::relay::Identity;
use knowledge::remote::npub;
use knowledge::xp::Trainers;
use knowledge::xp::adopt;
use knowledge::xp::eval::{self as credit, Candidate, Documents};
use nostr::contracts;
use nostr::domain::Event;
use nostr::eval_ext::{self, Publication};
use nostr::{ext, kb, kinds, xp};
use serde_json::{Value, json};

use super::{XpOptions, now, relay_url, short, sign};
use crate::kbnet::{LIMIT, Relay};

/// The quest templates the referee publishes quest versions from.
const TEMPLATES: &[(&str, &str)] = &[
    (
        "ext-eval.project-map.json",
        include_str!("../../../../knowledge/quests/ext-eval.project-map.json"),
    ),
    (
        "ext-eval.code-finder.json",
        include_str!("../../../../knowledge/quests/ext-eval.code-finder.json"),
    ),
    (
        "ext-eval.test-reader.json",
        include_str!("../../../../knowledge/quests/ext-eval.test-reader.json"),
    ),
    (
        "ext-eval.adopt.json",
        include_str!("../../../../knowledge/quests/ext-eval.adopt.json"),
    ),
];

/// An `eval-check` template: one starter suite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckTemplate {
    /// The quest ID its versions share, such as `ext-eval.project-map.check`.
    pub id: String,
    /// The tool's name in copy, such as `Project map`.
    pub name: String,
    /// The suite package's slug.
    pub suite_package: String,
    /// Root keys whose releases of that package are the starter suite.
    pub publishers: BTreeSet<String>,
    pub season: Value,
    pub title: String,
    pub objective: String,
    pub award: Value,
    pub max_awards: u64,
}

/// The `eval-adopt` template.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdoptTemplate {
    pub id: String,
    pub season: Value,
    pub title: String,
    pub objective: String,
    pub award: Value,
}

/// Every template the referee works from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Templates {
    pub checks: Vec<CheckTemplate>,
    pub adopt: Option<AdoptTemplate>,
}

fn field<'a>(value: &'a Value, key: &str, file: &str) -> Result<&'a Value, String> {
    value
        .get(key)
        .filter(|v| !v.is_null())
        .ok_or(format!("{file}: `{key}` is missing"))
}

fn text_of(value: &Value, key: &str, file: &str) -> Result<String, String> {
    field(value, key, file)?
        .as_str()
        .map(str::to_owned)
        .ok_or(format!("{file}: `{key}` must be text"))
}

/// Reads one template file.
///
/// # Errors
///
/// A field that's missing or of the wrong type, or a publisher that
/// isn't a hex key.
pub fn parse_template(file: &str, text: &str, into: &mut Templates) -> Result<(), String> {
    let value: Value = serde_json::from_str(text).map_err(|e| format!("{file}: {e}"))?;
    let rule = text_of(&value, "rule", file)?;
    let season = field(&value, "season", file)?.clone();
    let award = field(&value, "award", file)?.clone();
    match rule.as_str() {
        xp::EVAL_CHECK => {
            let suite = field(&value, "suite", file)?;
            let mut publishers = BTreeSet::new();
            for key in suite["publishers"].as_array().cloned().unwrap_or_default() {
                let key = key.as_str().and_then(super::parse_author).ok_or(format!(
                    "{file}: a publisher must be an npub or a hex public key"
                ))?;
                publishers.insert(key);
            }
            into.checks.push(CheckTemplate {
                id: text_of(&value, "id", file)?,
                name: text_of(&value, "name", file)?,
                suite_package: text_of(suite, "package", file)?,
                publishers,
                season,
                title: text_of(&value, "title", file)?,
                objective: text_of(&value, "objective", file)?,
                award,
                max_awards: field(&value, "max_awards", file)?
                    .as_u64()
                    .ok_or(format!("{file}: `max_awards` must be a number"))?,
            });
        }
        xp::EVAL_ADOPT => {
            into.adopt = Some(AdoptTemplate {
                id: text_of(&value, "id", file)?,
                season,
                title: text_of(&value, "title", file)?,
                objective: text_of(&value, "objective", file)?,
                award,
            });
        }
        other => return Err(format!("{file}: unknown rule {other}")),
    }
    Ok(())
}

/// The templates in `dir` (`ext-eval.*.json`), or the built-in ones.
///
/// # Errors
///
/// A template that doesn't parse.
pub fn templates(dir: Option<&Path>) -> Result<Templates, String> {
    let mut out = Templates::default();
    match dir {
        None => {
            for (file, text) in TEMPLATES {
                parse_template(file, text, &mut out)?;
            }
        }
        Some(dir) => {
            let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
                .map_err(|e| format!("{}: {e}", dir.display()))?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("ext-eval.") && n.ends_with(".json"))
                })
                .collect();
            files.sort();
            for path in files {
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                parse_template(&path.display().to_string(), &text, &mut out)?;
            }
        }
    }
    Ok(out)
}

fn home(rest: &str) -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(rest))
}

/// `~/.openagents/nostr/coder-defaults-key`, the key `coder-defaults`
/// releases and admissions are signed with. It's never created on first
/// use: `xp defaults-keygen` makes it, once, on the referee host.
#[must_use]
pub fn defaults_key_file() -> Option<PathBuf> {
    home(".openagents/nostr/coder-defaults-key")
}

/// `~/.openagents/coder-defaults/documents`, where manifests and
/// admissions are kept by digest.
#[must_use]
pub fn documents_dir() -> Option<PathBuf> {
    home(".openagents/coder-defaults/documents")
}

fn queue_file(o: &XpOptions) -> Option<PathBuf> {
    o.queue
        .clone()
        .or_else(|| home(".openagents/coder-defaults/candidates.json"))
}

fn state_file(o: &XpOptions) -> Option<PathBuf> {
    o.state
        .clone()
        .or_else(|| home(".openagents/referee/eval-refusals.json"))
}

fn documents_path(o: &XpOptions) -> Option<PathBuf> {
    o.documents.clone().or_else(documents_dir)
}

/// The `coder-defaults` package: `--defaults-root`, else the root the
/// package record names.
fn defaults_package(o: &XpOptions) -> Result<String, String> {
    match &o.defaults_root {
        Some(root) => super::parse_author(root)
            .map(|root| adopt::package_of(&root))
            .ok_or(format!("{root} isn't an npub or a hex public key")),
        None => Ok(adopt::package()),
    }
}

/// Every document in `dir`, by digest. A missing directory holds none.
fn read_documents(dir: Option<&Path>) -> Documents {
    let Some(dir) = dir else {
        return Documents::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Documents::new();
    };
    credit::documents(
        entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_file())
            .filter_map(|p| std::fs::read(p).ok()),
    )
}

/// Keeps `bytes` in `dir`, named by digest.
fn keep_document(dir: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let digest = contracts::digest_bytes(bytes);
    let path = dir.join(format!("{}.json", digest.trim_start_matches("sha256:")));
    std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// The digests a release pins: its manifest, and once that's held, the
/// admissions the manifest cites.
fn wanted_digests(release: &Event, documents: &Documents) -> Vec<String> {
    let Ok(body) = ext::parse_record(release) else {
        return Vec::new();
    };
    let Ok(manifest) = contracts::parse_artifact(&body["manifest"]) else {
        return Vec::new();
    };
    let mut out = vec![manifest.digest.clone()];
    if let Some(bytes) = documents.get(&manifest.digest) {
        out.extend(
            adopt::receipts_of(bytes)
                .unwrap_or_default()
                .iter()
                .filter_map(|r| contracts::parse_artifact(r).ok())
                .map(|a| a.digest),
        );
    }
    out
}

/// Fetches one document a NIP-94 locator names, checking its digest.
async fn fetch(url: &str, digest: &str) -> Option<Vec<u8>> {
    if !url.starts_with("https://") {
        return None;
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .ok()?;
    let response = client.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    (bytes.len() <= credit::MAX_DOCUMENT_BYTES && contracts::digest_bytes(&bytes) == digest)
        .then(|| bytes.to_vec())
}

/// The documents the `releases` pin: those kept in `dir`, then those the
/// relay's locators point to, each checked against its digest.
pub(super) async fn documents_for(
    relay: &mut Relay,
    releases: &[Event],
    dir: Option<&Path>,
) -> Documents {
    let mut documents = read_documents(dir);
    for _ in 0..2 {
        let missing: BTreeSet<String> = releases
            .iter()
            .flat_map(|r| wanted_digests(r, &documents))
            .filter(|d| !documents.contains_key(d))
            .collect();
        if missing.is_empty() {
            break;
        }
        let hexes: Vec<&str> = missing
            .iter()
            .map(|d| d.trim_start_matches("sha256:"))
            .collect();
        let locators = relay
            .query(json!({"kinds": [ext::LOCATOR_KIND], "#x": hexes, "limit": LIMIT}))
            .await
            .unwrap_or_default();
        for locator in locators {
            let (Some(url), Some(hex)) = (
                locator.tag_values("url").next(),
                locator.tag_values("x").next(),
            ) else {
                continue;
            };
            let digest = format!("sha256:{hex}");
            if documents.contains_key(&digest) || !missing.contains(&digest) {
                continue;
            }
            if let Some(bytes) = fetch(url, &digest).await {
                documents.insert(digest, bytes);
            }
        }
    }
    documents
}

/// What the relay holds for one pass.
struct Seen {
    publications: BTreeMap<String, (Event, Publication)>,
    /// Every event read, for the trainers and the queue.
    events: Vec<Event>,
    requests: Vec<Event>,
    trainers: Trainers,
    /// NIP-EXT releases by ID: suites, tools, and `coder-defaults`.
    releases: BTreeMap<String, Event>,
    defaults: Vec<Event>,
    documents: Documents,
}

async fn read(relay: &mut Relay, package: &str, dir: Option<&Path>) -> Result<Seen, String> {
    let root = package.split(':').next().unwrap_or_default().to_owned();
    let mut events = relay
        .query(
            json!({"kinds": [kb::EVIDENCE_KIND], "#t": [eval_ext::PROFILE_MARKER], "limit": LIMIT}),
        )
        .await?;
    events.extend(
        relay
            .query(json!({"kinds": [xp::PROFILE_KIND, xp::LINK_KIND], "limit": LIMIT}))
            .await?,
    );
    events.extend(
        relay
            .query(json!({"kinds": [kinds::CJ_EXECUTION_REQUEST], "limit": LIMIT}))
            .await?,
    );
    let defaults = relay
        .query(json!({"kinds": [ext::RELEASE_KIND], "authors": [root], "limit": LIMIT}))
        .await?;
    let publications = credit::publications(&events);
    let wanted: BTreeSet<String> = publications
        .values()
        .flat_map(|(_, p)| {
            std::iter::once(p.suite_release.id.clone())
                .chain(p.subject_release.as_ref().map(|s| s.id.clone()))
        })
        .collect();
    let wanted: Vec<String> = wanted.into_iter().collect();
    let mut releases: BTreeMap<String, Event> = BTreeMap::new();
    for chunk in wanted.chunks(LIMIT) {
        for event in relay
            .query(json!({"ids": chunk, "limit": chunk.len()}))
            .await?
        {
            if event.kind == ext::RELEASE_KIND && ext::parse_record(&event).is_ok() {
                releases.insert(event.id.clone(), event);
            }
        }
    }
    let documents = documents_for(relay, &defaults, dir).await;
    let requests = credit::requests(&events);
    let trainers = Trainers::read(&events);
    events.extend(defaults.iter().cloned());
    Ok(Seen {
        publications,
        events,
        requests,
        trainers,
        releases,
        defaults,
        documents,
    })
}

/// The package (`<root>:<slug>`) a release publishes.
fn package_of(release: &Event) -> Option<String> {
    ext::parse_record(release).ok()?["package"]
        .as_str()
        .map(str::to_owned)
}

/// The starter template a result's suite belongs to: a release of the
/// template's suite package by one of its publishers.
fn template_for<'a>(
    templates: &'a Templates,
    seen: &Seen,
    publication: &Publication,
) -> Option<&'a CheckTemplate> {
    let release = seen.releases.get(&publication.suite_release.id)?;
    let package = package_of(release)?;
    templates.checks.iter().find(|t| {
        t.publishers.contains(&release.pubkey)
            && package == format!("{}:{}", release.pubkey, t.suite_package)
    })
}

/// The referee's own quests, awards, and revocations.
struct Mine {
    quests: Vec<(Event, xp::Quest)>,
    /// Keys with a live award.
    keys: BTreeSet<String>,
    /// Live awards per quest coordinate.
    per_quest: BTreeMap<String, u64>,
}

async fn mine(relay: &mut Relay, me: &str) -> Result<Mine, String> {
    let events = relay
        .query(json!({
            "kinds": [xp::QUEST_KIND, xp::AWARD_KIND, xp::REVOCATION_KIND],
            "authors": [me], "limit": LIMIT,
        }))
        .await?;
    let revoked: BTreeSet<String> = events
        .iter()
        .filter(|e| e.kind == xp::REVOCATION_KIND)
        .filter_map(|e| xp::parse_revocation(e).ok())
        .map(|r| r.award.id)
        .collect();
    let mut out = Mine {
        quests: Vec::new(),
        keys: BTreeSet::new(),
        per_quest: BTreeMap::new(),
    };
    for event in &events {
        if event.kind == xp::QUEST_KIND {
            if let Ok(quest) = xp::parse_quest(event) {
                out.quests.push((event.clone(), quest));
            }
        } else if event.kind == xp::AWARD_KIND
            && !revoked.contains(&event.id)
            && let Ok(award) = xp::parse_award(event)
        {
            out.keys.insert(award.key);
            *out.per_quest.entry(award.coordinate).or_default() += 1;
        }
    }
    Ok(out)
}

/// One pass's tally.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Tally {
    /// Awards signed and published.
    pub signed: usize,
    /// Quest versions published.
    pub quests: usize,
    /// Completions refused, each logged once.
    pub refused: usize,
    /// Awards not signed because the key already holds one.
    pub already: usize,
    /// The adoption queue.
    pub candidates: Vec<Candidate>,
}

/// Refusals already logged, so each is logged once.
fn read_state(path: Option<&Path>) -> BTreeSet<String> {
    path.and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn write_state(path: Option<&Path>, logged: &BTreeSet<String>) {
    if let Some(path) = path {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, json!(logged).to_string());
    }
}

struct Pass<'a> {
    relay: &'a mut Relay,
    identity: &'a Identity,
    mine: Mine,
    tally: Tally,
    logged: BTreeSet<String>,
    at: u64,
}

impl Pass<'_> {
    /// Logs a refusal once: event IDs and the rule's reason, never text
    /// from the events themselves.
    fn refuse(&mut self, what: &str, id: &str, why: &str) {
        self.tally.refused += 1;
        let line = format!("refused {what} {}: {why}", short(id));
        if self.logged.insert(format!("{what}:{id}:{why}")) {
            println!("{line}");
        }
    }

    /// The referee's quest version for `spec`'s acceptance, publishing the
    /// next version of `id` when none pins it.
    async fn ensure_quest(
        &mut self,
        id: &str,
        pins: impl Fn(&xp::Quest) -> bool,
        spec: impl Fn(u64) -> Value,
    ) -> Result<Event, String> {
        if let Some((event, _)) = self
            .mine
            .quests
            .iter()
            .find(|(_, q)| q.address.starts_with(&format!("{id}@")) && pins(q))
        {
            return Ok(event.clone());
        }
        let version = self
            .mine
            .quests
            .iter()
            .filter(|(_, q)| q.address.starts_with(&format!("{id}@")))
            .filter_map(|(_, q)| q.address.rsplit_once('@')?.1.parse::<u64>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        let parts = xp::quest(&spec(version)).map_err(|e| format!("{id}: {e}"))?;
        let event = sign(self.identity, parts);
        let parsed = xp::parse_quest(&event).map_err(|e| e.to_string())?;
        self.relay.publish(&event).await?;
        println!(
            "{}: quest {} published ({} XP, season {})",
            parsed.address,
            short(&event.id),
            parsed.total(),
            parsed.season.id
        );
        self.tally.quests += 1;
        self.mine.quests.push((event.clone(), parsed));
        Ok(event)
    }

    /// Signs and publishes each award whose key holds none yet, within
    /// the quest's `max_awards`.
    async fn publish_awards(
        &mut self,
        quest: &Event,
        parts: Vec<kb::Unsigned>,
    ) -> Result<(), String> {
        let parsed = xp::parse_quest(quest).map_err(|e| e.to_string())?;
        let coordinate = xp::coordinate(&quest.pubkey, &parsed.address);
        for part in parts {
            let key = serde_json::from_str::<Value>(&part.content)
                .ok()
                .and_then(|v| v["key"].as_str().map(str::to_owned))
                .ok_or("an award without a key")?;
            if self.mine.keys.contains(&key) {
                self.tally.already += 1;
                continue;
            }
            let live = self.mine.per_quest.get(&coordinate).copied().unwrap_or(0);
            if parsed.award_limit().is_some_and(|max| live >= max) {
                self.refuse("award", &quest.id, "the quest version paid its max_awards");
                continue;
            }
            let event = self
                .identity
                .signer()
                .sign(self.at, part.kind, part.tags, part.content);
            let award = xp::parse_award(&event).map_err(|e| e.to_string())?;
            self.relay.publish(&event).await?;
            println!(
                "{}: award {} signed: {} {} XP to {}",
                parsed.address,
                short(&event.id),
                award.awardees[0].role,
                award.awardees[0].xp,
                npub(&award.awardees[0].pubkey)
            );
            self.mine.keys.insert(key);
            *self.mine.per_quest.entry(coordinate.clone()).or_default() += 1;
            self.tally.signed += 1;
        }
        Ok(())
    }
}

fn season_open(season: &Value, at: u64) -> bool {
    let (Some(opens), Some(closes)) = (season["opens_at"].as_u64(), season["closes_at"].as_u64())
    else {
        return false;
    };
    (opens..=closes).contains(&at)
}

fn pointer(event: &Event) -> Value {
    json!({"id": event.id, "pubkey": event.pubkey, "kind": event.kind})
}

/// One referee pass over what the relay holds.
///
/// # Errors
///
/// A relay that fails, or templates that don't parse.
pub async fn referee_pass(o: &XpOptions, identity: &Identity) -> Result<Tally, String> {
    let url = relay_url(o)?;
    let templates = templates(o.quests.as_deref())?;
    let package = defaults_package(o)?;
    let documents_dir = documents_path(o);
    let state = state_file(o);
    let mut relay = Relay::open(url, identity).await?;
    let seen = read(&mut relay, &package, documents_dir.as_deref()).await?;
    let mine = mine(&mut relay, identity.pubkey()).await?;
    let mut pass = Pass {
        relay: &mut relay,
        identity,
        mine,
        tally: Tally::default(),
        logged: read_state(state.as_deref()),
        at: now(),
    };

    for (check_id, (check_event, check)) in &seen.publications {
        let Some(result_id) = check.checks.as_deref() else {
            continue;
        };
        let Some((result_event, result)) = seen.publications.get(result_id) else {
            pass.refuse("check", check_id, "the result it checks isn't on the relay");
            continue;
        };
        let Some(template) = template_for(&templates, &seen, result) else {
            // Not a starter suite: no quest pays for it.
            continue;
        };
        if !season_open(&template.season, pass.at) {
            continue;
        }
        let Some(subject) = result
            .subject_release
            .as_ref()
            .and_then(|s| seen.releases.get(&s.id))
        else {
            pass.refuse("check", check_id, "the tool's release isn't on the relay");
            continue;
        };
        let trainers = (
            eval_ext::verified_trainer(check, &seen.requests),
            eval_ext::verified_trainer(result, &seen.requests),
        );
        let (Ok(checker), Ok(evaluator)) = trainers else {
            pass.refuse(
                "check",
                check_id,
                "a hosted run's signed request isn't available",
            );
            continue;
        };
        if let Err(why) =
            credit::distinct_trainers(&seen.trainers, checker, evaluator, result.suite_author())
        {
            pass.refuse("check", check_id, &why);
            continue;
        }
        let suite = seen.releases[&result.suite_release.id].clone();
        let t = template.clone();
        let quest = pass
            .ensure_quest(
                &t.id,
                |q| {
                    q.acceptance.eval.as_ref().is_some_and(|a| {
                        a.suite.as_ref().is_some_and(|s| s.id == suite.id)
                            && a.subject.id == subject.id
                    })
                },
                |version| {
                    json!({
                        "id": t.id, "version": version, "season": t.season,
                        "title": t.title, "objective": t.objective,
                        "acceptance": {
                            "rule": xp::EVAL_CHECK,
                            "suite": pointer(&suite), "subject": pointer(subject),
                            "max_awards": t.max_awards,
                        },
                        "reference": null, "award": t.award,
                    })
                },
            )
            .await?;
        match xp::eval_check_awards(&quest, result_event, check_event, &seen.requests, pass.at) {
            Ok(parts) => pass.publish_awards(&quest, parts).await?,
            Err(e) => pass.refuse("check", check_id, &format!("{:?}: {}", e.code, e.detail)),
        }
    }

    let publications: Vec<Event> = seen.publications.values().map(|(e, _)| e.clone()).collect();
    if let Some(template) = templates
        .adopt
        .clone()
        .filter(|t| season_open(&t.season, pass.at))
    {
        for (release, parsed) in
            credit::defaults_releases(&seen.defaults, &package, &seen.documents)
        {
            let Ok((manifest, admissions)) = credit::adoption_documents(&release, &seen.documents)
            else {
                continue;
            };
            for admission in admissions {
                let Ok(decision) = eval_ext::parse_admission(admission) else {
                    continue;
                };
                let Some(subject_ref) = decision.subject.event.as_ref() else {
                    continue;
                };
                if !parsed.manifest.dependencies.contains(&subject_ref.id) {
                    continue;
                }
                let subject = json!({
                    "id": subject_ref.id, "pubkey": subject_ref.pubkey, "kind": kinds::EXT_RELEASE,
                });
                let t = template.clone();
                let package = package.clone();
                let subject_id = subject_ref.id.clone();
                let quest = pass
                    .ensure_quest(
                        &t.id,
                        |q| {
                            q.acceptance
                                .eval
                                .as_ref()
                                .is_some_and(|a| a.subject.id == subject_id)
                        },
                        |version| {
                            json!({
                                "id": t.id, "version": version, "season": t.season,
                                "title": t.title, "objective": t.objective,
                                "acceptance": {
                                    "rule": xp::EVAL_ADOPT, "defaults": package, "subject": subject,
                                },
                                "reference": null, "award": t.award,
                            })
                        },
                    )
                    .await?;
                let adoption = xp::Adoption {
                    release: &release,
                    manifest,
                    admission,
                    results: &publications,
                    checks: &publications,
                    requests: &seen.requests,
                };
                match xp::eval_adopt_awards(&quest, &adoption, pass.at) {
                    Ok(parts) => pass.publish_awards(&quest, parts).await?,
                    Err(e) => {
                        pass.refuse(
                            "adoption",
                            &release.id,
                            &format!("{:?}: {}", e.code, e.detail),
                        );
                    }
                }
            }
        }
    }

    let adopted = credit::adopted(&seen.defaults, &package, &seen.documents);
    pass.tally.candidates = credit::candidates(&seen.events, &seen.trainers, &adopted);
    write_state(state.as_deref(), &pass.logged);
    if let Some(path) = queue_file(o) {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let queue =
            serde_json::to_string_pretty(&pass.tally.candidates).map_err(|e| e.to_string())?;
        std::fs::write(&path, queue).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(pass.tally)
}

/// `xp referee`: one pass of the automated referee job. Signs only with
/// an existing referee key; it never creates one.
///
/// # Errors
///
/// A bad usage, a missing key, or a relay that fails.
pub async fn referee(o: &XpOptions, key: &Path) -> Result<u8, String> {
    if !key.exists() {
        return Err(format!(
            "there's no referee key at {}; the referee job signs only with the existing key",
            key.display()
        ));
    }
    let identity = Identity::load_from(key)?;
    let tally = referee_pass(o, &identity).await?;
    for candidate in &tally.candidates {
        println!(
            "candidate for Coder's defaults: tool release {} ({}), {} confirmed Better result(s); \
an operator decides with `microcoder xp adopt --subject {}`",
            short(&candidate.subject),
            candidate.definition,
            candidate.results.len(),
            candidate.subject
        );
    }
    println!(
        "referee pass as {}: {} awards signed, {} quest versions published, {} already awarded, \
{} refused, {} candidates",
        npub(identity.pubkey()),
        tally.signed,
        tally.quests,
        tally.already,
        tally.refused,
        tally.candidates.len()
    );
    Ok(0)
}

/// `xp defaults-keygen`: creates the `coder-defaults` key, once, and
/// prints only its public key.
///
/// # Errors
///
/// When the file can't be written.
pub fn defaults_keygen(o: &XpOptions) -> Result<u8, String> {
    let key = o
        .key
        .clone()
        .or_else(defaults_key_file)
        .ok_or("HOME isn't set, so there's no key file: pass --key")?;
    if key.exists() {
        let pubkey = knowledge::remote::own_pubkey(&key).unwrap_or_default();
        println!(
            "{} already holds a key ({}); it is never replaced here",
            key.display(),
            npub(&pubkey)
        );
        return Ok(1);
    }
    if let Some(dir) = key.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let identity = Identity::load_from(&key)?;
    println!(
        "coder-defaults root {}\npublic key (hex) {}\nsecret key in {} (mode 0600); it never \
leaves this host.",
        npub(identity.pubkey()),
        identity.pubkey(),
        key.display()
    );
    Ok(0)
}

/// `xp adopt`: lists the adoption queue, or with `--subject RELEASE-ID`
/// adopts one candidate into Coder's defaults. Operator only: it signs
/// with the `coder-defaults` key, which only the referee host holds.
///
/// # Errors
///
/// A bad usage, a missing key, or a relay that fails.
pub async fn adopt_command(o: &XpOptions, key: &Path) -> Result<u8, String> {
    let url = relay_url(o)?;
    if !key.exists() {
        return Err(format!(
            "there's no coder-defaults key at {}; adoption is an operator's decision on the \
referee host",
            key.display()
        ));
    }
    let identity = Identity::load_from(key)?;
    let package = adopt::package_of(identity.pubkey());
    let mut relay = Relay::open(url, &identity).await?;
    let dir = documents_path(o);
    let seen = read(&mut relay, &package, dir.as_deref()).await?;
    let adopted = credit::adopted(&seen.defaults, &package, &seen.documents);
    let queue = credit::candidates(&seen.events, &seen.trainers, &adopted);
    let Some(subject) = o.subject.as_deref() else {
        println!("{} candidates for Coder's defaults", queue.len());
        for c in &queue {
            println!(
                "- tool release {} ({}): {} Better result(s), confirmed by {} trainers",
                c.subject,
                c.definition,
                c.results.len(),
                c.results
                    .iter()
                    .map(|r| r.confirmed_by.len())
                    .max()
                    .unwrap_or(0)
            );
        }
        return Ok(0);
    };
    if adopted.contains(subject) {
        println!(
            "tool release {} is already in Coder's defaults",
            short(subject)
        );
        return Ok(1);
    }
    let Some(candidate) = queue.iter().find(|c| c.subject == subject) else {
        println!(
            "tool release {} isn't a candidate: it needs a Better result that checks by at least \
{} distinct trainers confirmed",
            short(subject),
            credit::CONFIRMING_CHECKS
        );
        return Ok(1);
    };
    let results: Vec<&Event> = candidate
        .results
        .iter()
        .filter_map(|r| seen.publications.get(&r.result).map(|(e, _)| e))
        .collect();
    let at = now();
    let days = o.expires_days.unwrap_or(365);
    let admission = adopt::admission(identity.pubkey(), &results, at + days * 86_400)?;
    let previous = credit::defaults_releases(&seen.defaults, &package, &seen.documents);
    let (mut dependencies, mut receipts) = previous.last().map_or_else(
        || (Vec::new(), Vec::new()),
        |(release, parsed)| {
            let receipts = credit::adoption_documents(release, &seen.documents)
                .ok()
                .and_then(|(manifest, _)| adopt::receipts_of(manifest).ok())
                .unwrap_or_default();
            (parsed.manifest.dependencies.clone(), receipts)
        },
    );
    dependencies.push(subject.to_owned());
    receipts.push(adopt::receipt(&admission));
    let version = (previous.len() + 1).to_string();
    let manifest = adopt::manifest(&package, &version, &dependencies, &receipts)?;
    let release = sign(
        &identity,
        adopt::release(&manifest).map_err(|e| e.to_string())?,
    );
    for d in [
        dir.as_deref(),
        o.package_dir
            .as_deref()
            .map(|p| p.join("documents"))
            .as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        keep_document(d, &manifest)?;
        keep_document(d, &admission)?;
    }
    for bytes in [&manifest, &admission] {
        let locator = sign(
            &identity,
            adopt::locator(bytes, &adopt::document_url(bytes)),
        );
        relay.publish(&locator).await?;
    }
    relay.publish(&release).await?;
    println!(
        "coder-defaults {version} published as release {}: it depends on tool release {} and \
cites the admission {}. The referee signs the eval-adopt awards on its next pass. Commit {} and {} \
so readers elsewhere can fetch them.",
        short(&release.id),
        short(subject),
        contracts::digest_bytes(&admission),
        adopt::document_path(&manifest),
        adopt::document_path(&admission),
    );
    Ok(0)
}

#[cfg(test)]
mod tests;
