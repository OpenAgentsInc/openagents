//! Credit for extension evaluation (`docs/extensions/evaluation.md`,
//! "Checks, adoption, and credit"): the `eval-check` and `eval-adopt`
//! awards a reader re-checks, what a trainer made, and the queue of tools
//! that are candidates for adoption into Coder's defaults.
//!
//! "Used" means exactly two things: another trainer reran a result to
//! protocol (a check, confirming or disputing), or Coder adopted the tool
//! into its defaults. Credit is XP and a name. Nothing here spends,
//! transfers, or converts XP, and nothing pays.
//!
//! An `eval-check` award names the result and the check, which are signed
//! `3189` publications a reader holds. An `eval-adopt` award also names a
//! `coder-defaults` release, whose manifest and admission are documents
//! the release pins by digest: a reader passes the bytes it holds as
//! [`Documents`], and an adoption whose bytes it doesn't hold isn't
//! counted.

use std::collections::{BTreeMap, BTreeSet};

use nostr::contracts::{self, ArtifactRef};
use nostr::domain::Event;
use nostr::eval_ext::{self, Publication, Verdict};
use nostr::xp::{self, Adoption, Award};
use nostr::{ext, kb, kinds};
use serde::Serialize;
use serde_json::Value;

use crate::{Ledger, Trainers};

/// Document bytes a reader holds, by `sha256:` digest: `coder-defaults`
/// manifests and the admissions they cite.
pub type Documents = BTreeMap<String, Vec<u8>>;

/// How many distinct trainers' confirming checks make a **Better** result
/// a candidate for adoption.
pub const CONFIRMING_CHECKS: usize = 3;

/// How many externally validating results (a **Better** result on a second
/// suite by another author, released after the tool's release, on the
/// same task distribution) a candidate needs beside its confirming
/// checks. Reproduction on the author's own suite proves reproducibility;
/// this proves the delta wasn't fitted to that suite.
pub const VALIDATIONS: usize = 1;

/// The slug of Coder's defaults package.
pub const DEFAULTS_SLUG: &str = "coder-defaults";

/// Most bytes one document may have.
pub const MAX_DOCUMENT_BYTES: usize = 1024 * 1024;

/// `bytes` keyed by their digests.
#[must_use]
pub fn documents<I: IntoIterator<Item = Vec<u8>>>(items: I) -> Documents {
    items
        .into_iter()
        .filter(|bytes| bytes.len() <= MAX_DOCUMENT_BYTES)
        .map(|bytes| (contracts::digest_bytes(&bytes), bytes))
        .collect()
}

/// The valid extension evaluation publications (`3189` with the
/// `oa:ext-eval:v1` marker) among `events`, each once, by event ID.
#[must_use]
pub fn publications(events: &[Event]) -> BTreeMap<String, (Event, Publication)> {
    events
        .iter()
        .filter(|e| e.kind == kb::EVIDENCE_KIND)
        .filter_map(|e| {
            eval_ext::parse_publication(e)
                .ok()
                .map(|p| (e.id.clone(), (e.clone(), p)))
        })
        .collect()
}

/// The signed NIP-CJ execution requests among `events`, which hosted
/// results name.
#[must_use]
pub fn requests(events: &[Event]) -> Vec<Event> {
    let mut seen = BTreeSet::new();
    events
        .iter()
        .filter(|e| e.kind == kinds::CJ_EXECUTION_REQUEST && seen.insert(e.id.clone()))
        .cloned()
        .collect()
}

/// Checks one `eval-check` award completely: the award alone, its quest,
/// and the rule over the result and check it names.
///
/// # Errors
///
/// A message naming the first check that failed.
pub fn verify_eval_check(
    award_event: &Event,
    quest: &Event,
    result: &Event,
    check: &Event,
    requests: &[Event],
) -> Result<(Award, xp::Quest), String> {
    let award = xp::parse_award(award_event).map_err(|e| e.to_string())?;
    let parsed = xp::bind_quest(&award, quest).map_err(|e| e.to_string())?;
    xp::bind_eval_check(&award, &parsed, result, check, requests).map_err(|e| e.to_string())?;
    Ok((award, parsed))
}

/// The manifest bytes a `coder-defaults` release pins, and the bytes of
/// each admission its provenance cites that `documents` holds.
///
/// # Errors
///
/// When the release isn't a valid NIP-EXT release, or `documents` doesn't
/// hold its manifest.
pub fn adoption_documents<'a>(
    release: &Event,
    documents: &'a Documents,
) -> Result<(&'a [u8], Vec<&'a [u8]>), String> {
    let manifest_ref = manifest_ref(release)?;
    let manifest = documents
        .get(&manifest_ref.digest)
        .ok_or(format!(
            "the release {}'s manifest isn't available",
            short(&release.id)
        ))?
        .as_slice();
    let parsed = eval_ext::parse_release(release, manifest).map_err(|e| e.to_string())?;
    let admissions = parsed
        .admissions
        .iter()
        .filter_map(|a| documents.get(&a.digest).map(Vec::as_slice))
        .collect();
    Ok((manifest, admissions))
}

fn manifest_ref(release: &Event) -> Result<ArtifactRef, String> {
    if release.kind != ext::RELEASE_KIND {
        return Err(format!("{} isn't a release", short(&release.id)));
    }
    let body = ext::parse_record(release).map_err(|e| e.to_string())?;
    contracts::parse_artifact(&body["manifest"]).map_err(|e| e.to_string())
}

/// Checks one `eval-adopt` award completely: the award alone, its quest,
/// and the rule over the release it names, the release's manifest and
/// admissions from `documents`, and the results and checks among
/// `publications`.
///
/// # Errors
///
/// A message naming the first check that failed.
pub fn verify_eval_adopt(
    award_event: &Event,
    quest: &Event,
    release: &Event,
    publications: &[Event],
    requests: &[Event],
    documents: &Documents,
) -> Result<(Award, xp::Quest), String> {
    let award = xp::parse_award(award_event).map_err(|e| e.to_string())?;
    let parsed = xp::bind_quest(&award, quest).map_err(|e| e.to_string())?;
    let (manifest, admissions) = adoption_documents(release, documents)?;
    let mut last = format!(
        "the release {} cites no admission this reader holds",
        short(&release.id)
    );
    for admission in admissions {
        let adoption = Adoption {
            release,
            manifest,
            admission,
            results: publications,
            checks: publications,
            requests,
        };
        match xp::bind_eval_adopt(&award, &parsed, &adoption) {
            Ok(()) => return Ok((award, parsed)),
            Err(e) => last = e.to_string(),
        }
    }
    Err(last)
}

fn short(hex: &str) -> &str {
    &hex[..hex.len().min(12)]
}

/// The trainer a key's work counts for: its linked trainer, else itself.
fn trainer_of(trainers: &Trainers, key: &str) -> String {
    trainers.trainer_of(key).unwrap_or(key).to_owned()
}

/// Whether a check that the rule accepts is one the OpenAgents referee
/// also accepts: the checker's trainer is neither the evaluator's trainer
/// nor the suite author's, counting two-sided key links (NIP-XP `13195`).
/// The rule compares keys; this compares the trainers behind them, so one
/// person's linked keys can't check each other.
///
/// # Errors
///
/// Why the check is refused.
pub fn distinct_trainers(
    trainers: &Trainers,
    checker: &str,
    evaluator: &str,
    suite_author: &str,
) -> Result<(), String> {
    if checker == evaluator {
        return Err(
            "the checker is the result's evaluator: checking your own result earns nothing".into(),
        );
    }
    if checker == suite_author {
        return Err(
            "the checker wrote the suite: checking results on your own tests earns nothing".into(),
        );
    }
    let checker = trainer_of(trainers, checker);
    if checker == trainer_of(trainers, evaluator) {
        return Err(
            "the checker's key is linked to the evaluator's trainer: checking your own result \
earns nothing"
                .into(),
        );
    }
    if checker == trainer_of(trainers, suite_author) {
        return Err(
            "the checker's key is linked to the suite author's trainer: checking results on \
your own tests earns nothing"
                .into(),
        );
    }
    Ok(())
}

/// One result that makes its tool a candidate: **Better**, confirmed by
/// checks from at least [`CONFIRMING_CHECKS`] distinct trainers, and
/// externally validated by at least [`VALIDATIONS`] results on an
/// independent second suite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CandidateResult {
    /// The result's `3189` event ID.
    pub result: String,
    /// The result's trainer.
    pub trainer: String,
    /// The report's digest, which an admission cites.
    pub report: String,
    /// The distinct trainers whose checks confirmed it.
    pub confirmed_by: Vec<String>,
    /// The confirming checks' event IDs.
    pub checks: Vec<String>,
    /// The externally validating results' event IDs, which an admission
    /// cites in `validation`.
    pub validations: Vec<String>,
}

/// Which results are externally validated: each result's validating
/// publications, by result ID. A validation counts when it names the
/// result with the `validates` marker, is **Better**, ran a different suite
/// on the same task distribution, and both releases are among `events` and
/// show it independent: the suite's release signed by someone other than
/// the tool's release signer, and created after it
/// ([`eval_ext::validation`]). A validation whose releases a reader doesn't
/// hold doesn't count; independence isn't assumed.
#[must_use]
pub fn validations(
    events: &[Event],
    publications: &BTreeMap<String, (Event, Publication)>,
) -> BTreeMap<String, Vec<String>> {
    let releases: BTreeMap<&str, &Event> = events
        .iter()
        .filter(|e| eval_ext::SUBJECT_KINDS.contains(&e.kind))
        .map(|e| (e.id.as_str(), e))
        .collect();
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (id, (_, second)) in publications {
        let Some(original_id) = second.validates.as_deref() else {
            continue;
        };
        let Some((_, original)) = publications.get(original_id) else {
            continue;
        };
        let Some(suite) = releases.get(second.suite_release.id.as_str()) else {
            continue;
        };
        let Some(subject) = original
            .subject_release
            .as_ref()
            .and_then(|s| releases.get(s.id.as_str()))
        else {
            continue;
        };
        if second.verdict() != Verdict::Pass {
            continue;
        }
        if eval_ext::validation(original, second, suite, subject)
            .is_ok_and(eval_ext::Validation::externally_validates)
        {
            out.entry(original_id.to_owned())
                .or_default()
                .push(id.clone());
        }
    }
    out
}

/// A tool release that is a candidate for adoption into Coder's defaults.
/// The decision is always an operator's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Candidate {
    /// The extension's NIP-EXT release ID.
    pub subject: String,
    /// The release's root key.
    pub author: String,
    /// The tested component, `<pubkey>:<package>/<component>`.
    pub definition: String,
    pub results: Vec<CandidateResult>,
}

/// Which results confirm for credit: each result's confirming checks, by
/// result ID, as (check ID, checker key) pairs. A check counts when the
/// rule's check conditions hold ([`xp::eval_check::confirmed_check`]).
fn confirmations(
    publications: &BTreeMap<String, (Event, Publication)>,
    requests: &[Event],
) -> BTreeMap<String, Vec<(String, String)>> {
    let mut out: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for (check_id, (_, check)) in publications {
        let Some(result_id) = check.checks.as_deref() else {
            continue;
        };
        let Some((_, result)) = publications.get(result_id) else {
            continue;
        };
        if let Ok((checker, _)) = xp::eval_check::confirmed_check(result, check, requests) {
            out.entry(result_id.to_owned())
                .or_default()
                .push((check_id.clone(), checker));
        }
    }
    out
}

/// The adoption queue: tool releases with a **Better** result that at
/// least [`CONFIRMING_CHECKS`] distinct trainers' checks confirmed, less
/// those in `adopted` (subject release IDs a `coder-defaults` release
/// already depends on). Trainers are counted through two-sided key links,
/// and a checker whose trainer is the result's trainer or the suite
/// author's doesn't count.
#[must_use]
pub fn candidates(
    events: &[Event],
    trainers: &Trainers,
    adopted: &BTreeSet<String>,
) -> Vec<Candidate> {
    let publications = publications(events);
    let requests = requests(events);
    let confirmed = confirmations(&publications, &requests);
    let validated = validations(events, &publications);
    let mut by_subject: BTreeMap<String, Candidate> = BTreeMap::new();
    for (id, (_, result)) in &publications {
        if result.checks.is_some() || result.verdict() != Verdict::Pass {
            continue;
        }
        let Some(subject) = &result.subject_release else {
            continue;
        };
        if adopted.contains(&subject.id) {
            continue;
        }
        let Ok(evaluator) = eval_ext::verified_trainer(result, &requests) else {
            continue;
        };
        let mut confirmed_by = BTreeSet::new();
        let mut checks = Vec::new();
        for (check, checker) in confirmed.get(id).map(Vec::as_slice).unwrap_or_default() {
            if distinct_trainers(trainers, checker, evaluator, result.suite_author()).is_ok()
                && confirmed_by.insert(trainer_of(trainers, checker))
            {
                checks.push(check.clone());
            }
        }
        if confirmed_by.len() < CONFIRMING_CHECKS {
            continue;
        }
        let validations = validated.get(id).cloned().unwrap_or_default();
        if validations.len() < VALIDATIONS {
            continue;
        }
        by_subject
            .entry(subject.id.clone())
            .or_insert_with(|| Candidate {
                subject: subject.id.clone(),
                author: subject.pubkey.clone(),
                definition: result.report.subject.definition.id.clone(),
                results: Vec::new(),
            })
            .results
            .push(CandidateResult {
                result: id.clone(),
                trainer: evaluator.to_owned(),
                report: result.report_ref.digest.clone(),
                confirmed_by: confirmed_by.into_iter().collect(),
                checks,
                validations,
            });
    }
    by_subject.into_values().collect()
}

/// The valid releases of `package` (`<root>:<slug>`) among `events` whose
/// manifests `documents` holds, oldest first.
#[must_use]
pub fn defaults_releases(
    events: &[Event],
    package: &str,
    documents: &Documents,
) -> Vec<(Event, eval_ext::Release)> {
    let mut seen = BTreeSet::new();
    let mut out: Vec<(Event, eval_ext::Release)> = events
        .iter()
        .filter(|e| e.kind == ext::RELEASE_KIND && seen.insert(e.id.clone()))
        .filter_map(|e| {
            let manifest = documents.get(&manifest_ref(e).ok()?.digest)?;
            let release = eval_ext::parse_release(e, manifest).ok()?;
            (release.package == package).then(|| (e.clone(), release))
        })
        .collect();
    out.sort_by(|a, b| (a.0.created_at, &a.0.id).cmp(&(b.0.created_at, &b.0.id)));
    out
}

/// Subject release IDs some release of `package` already depends on.
#[must_use]
pub fn adopted(events: &[Event], package: &str, documents: &Documents) -> BTreeSet<String> {
    defaults_releases(events, package, documents)
        .into_iter()
        .flat_map(|(_, release)| release.manifest.dependencies)
        .collect()
}

/// Where one piece of a trainer's work stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Standing {
    /// A trusted referee's award credits it.
    Awarded,
    /// It meets the rule, and no award credits it yet.
    Pending,
    /// A result nobody has checked yet.
    Waiting,
    /// A result whose only checks dispute it. It stays visible; the
    /// disputing checks themselves are pending or awarded, since a rerun
    /// to protocol earns credit whichever way it came out.
    Disputed,
    /// It earns nothing: a self-check, another suite or subject, an
    /// inconclusive verdict, or the same role on a test set already paid
    /// for another.
    NoCredit,
}

/// A suite the trainer published.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MadeSuite {
    /// The suite's NIP-EXT release ID.
    pub release: String,
    /// Results published on it, checks included.
    pub results: usize,
    /// XP it earned its author, as suite author.
    pub xp: u64,
}

/// A result or check the trainer published.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MadeResult {
    /// The `3189` event ID.
    pub id: String,
    /// The suite's release ID.
    pub suite: String,
    /// The tool's release ID, when it's published.
    pub subject: Option<String>,
    /// `pass` (Better), `fail` (Worse), or `inconclusive`.
    pub verdict: String,
    /// For a check: the result it checks.
    pub checks: Option<String>,
    /// For a result: distinct trainers whose checks confirmed it.
    pub confirmed_by: usize,
    /// For a result: checks that dispute it.
    pub disputed_by: usize,
    pub standing: Standing,
    /// XP awarded for it to the trainer's keys.
    pub xp: u64,
}

/// A trainer's extension evaluation work and the credit it earned: the
/// `CARD-07` card and the Profile's "what you made".
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Made {
    pub suites: Vec<MadeSuite>,
    pub results: Vec<MadeResult>,
    pub checks: Vec<MadeResult>,
    /// `eval-adopt` credits to the trainer's keys.
    pub adoptions: Vec<crate::Credit>,
    /// `eval-check` and `eval-adopt` XP credited to the trainer's keys.
    pub xp: u64,
    /// Results and checks that meet the rule and wait for an award.
    pub pending: usize,
}

/// What the trainer holding `keys` made, and where its credit stands,
/// from `events` and the `ledger` derived from them.
#[must_use]
pub fn made(events: &[Event], ledger: &Ledger, keys: &[String]) -> Made {
    let mine = |key: &str| keys.iter().any(|k| k == key);
    let publications = publications(events);
    let requests = requests(events);
    let credits: Vec<&crate::Credit> = ledger
        .credits
        .iter()
        .filter(|c| (c.rule == xp::EVAL_CHECK || c.rule == xp::EVAL_ADOPT) && mine(&c.pubkey))
        .collect();
    let credited = |id: &str, role: &str| -> u64 {
        credits
            .iter()
            .filter(|c| {
                c.rule == xp::EVAL_CHECK && c.role == role && c.evidence.iter().any(|e| e == id)
            })
            .map(|c| c.xp)
            .sum()
    };
    let trainer = |p: &Publication| {
        eval_ext::verified_trainer(p, &requests)
            .ok()
            .map(str::to_owned)
    };
    let mut made = Made {
        xp: credits.iter().map(|c| c.xp).sum(),
        adoptions: credits
            .iter()
            .filter(|c| c.rule == xp::EVAL_ADOPT)
            .map(|c| (*c).clone())
            .collect(),
        ..Made::default()
    };
    let mut suites: BTreeMap<String, MadeSuite> = BTreeMap::new();
    let mut counted: BTreeSet<&str> = BTreeSet::new();
    for (id, (_, publication)) in &publications {
        if mine(publication.suite_author()) {
            let suite = suites
                .entry(publication.suite_release.id.clone())
                .or_insert_with(|| MadeSuite {
                    release: publication.suite_release.id.clone(),
                    results: 0,
                    xp: 0,
                });
            suite.results += 1;
            for credit in credits
                .iter()
                .filter(|c| c.role == "suite-author" && c.evidence.iter().any(|e| e == id))
            {
                if counted.insert(credit.award.as_str()) {
                    suite.xp += credit.xp;
                }
            }
        }
        let Some(by) = trainer(publication) else {
            continue;
        };
        if !mine(&by) {
            continue;
        }
        let mut row = MadeResult {
            id: id.clone(),
            suite: publication.suite_release.id.clone(),
            subject: publication.subject_release.as_ref().map(|s| s.id.clone()),
            verdict: publication.verdict().word().to_owned(),
            checks: publication.checks.clone(),
            confirmed_by: 0,
            disputed_by: 0,
            standing: Standing::NoCredit,
            xp: 0,
        };
        if let Some(original) = publication
            .checks
            .as_deref()
            .and_then(|r| publications.get(r))
        {
            row.xp = credited(id, "checker");
            // A rerun to protocol earns credit whether it confirms or
            // disputes; only a check that isn't one earns nothing.
            let credited_check =
                xp::eval_check::credited_check(&original.1, publication, &requests);
            row.standing = if row.xp > 0 {
                Standing::Awarded
            } else if credited_check.is_ok() {
                Standing::Pending
            } else {
                Standing::NoCredit
            };
            made.checks.push(row);
            continue;
        }
        let mut confirmers = BTreeSet::new();
        for (_, check) in publications.values() {
            if check.checks.as_deref() != Some(id.as_str()) {
                continue;
            }
            match xp::eval_check::confirmed_check(publication, check, &requests) {
                Ok((checker, _)) => {
                    confirmers.insert(checker);
                }
                Err(_) if eval_ext::linkage(publication, check) == eval_ext::Linkage::Dispute => {
                    row.disputed_by += 1;
                }
                Err(_) => {}
            }
        }
        row.confirmed_by = confirmers.len();
        row.xp = credited(id, "evaluator");
        row.standing = if row.xp > 0 {
            Standing::Awarded
        } else if row.confirmed_by > 0 {
            Standing::Pending
        } else if row.disputed_by > 0 {
            Standing::Disputed
        } else if publication.verdict() == Verdict::Inconclusive {
            Standing::NoCredit
        } else {
            Standing::Waiting
        };
        made.results.push(row);
    }
    // NIP-XP pays a key once per role per test set version
    // (`eval-check:<season>:<suite release>:<role>:<pubkey>`): once a
    // result or check on a test set is awarded, another of the same kind
    // on it waits for nothing.
    for rows in [&mut made.results, &mut made.checks] {
        let paid: BTreeSet<String> = rows
            .iter()
            .filter(|r| r.xp > 0)
            .map(|r| r.suite.clone())
            .collect();
        for row in rows.iter_mut() {
            if row.standing == Standing::Pending && paid.contains(&row.suite) {
                row.standing = Standing::NoCredit;
            }
        }
    }
    made.suites = suites.into_values().collect();
    made.pending = made
        .results
        .iter()
        .chain(&made.checks)
        .filter(|r| r.standing == Standing::Pending)
        .count();
    made
}

/// The raw report ArtifactRef and subject DefinitionRef a result
/// publication carries, as JSON, for an admission to cite.
///
/// # Errors
///
/// When the event isn't a valid result publication.
pub fn cited(result: &Event) -> Result<(Value, Value), String> {
    eval_ext::parse_publication(result).map_err(|e| e.to_string())?;
    let content: Value = serde_json::from_str(&result.content).map_err(|e| e.to_string())?;
    Ok((content["report"].clone(), content["subject"].clone()))
}

#[cfg(any(test, feature = "fixtures"))]
pub mod fixture;
