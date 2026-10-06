//! Explicit retained-source readers shared by the standalone and Verse panes.
use crate::{Contribution, State};
use nostr::{domain::Event, eval_ext, xp};
use openagents_chat::{
    client::NoCoder,
    plugin_workbench::{Action, Outcome, Owner},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use workbench::{
    Host, Revision,
    pane::{Description, PaneAdapter, PaneKind, PaneState, Subject},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub plugins: Vec<PathBuf>,
    #[serde(default)]
    pub knowledge: Vec<PathBuf>,
    #[serde(default)]
    pub reviews: Vec<PathBuf>,
    #[serde(default)]
    pub events: Vec<PathBuf>,
    #[serde(default)]
    pub documents: Vec<PathBuf>,
    #[serde(default)]
    pub operators: BTreeSet<String>,
    #[serde(default)]
    pub evaluators: BTreeSet<String>,
    #[serde(default)]
    pub referees: BTreeSet<String>,
    pub ledger: Option<PathBuf>,
}
fn private_file(path: &Path, maximum: u64) -> Result<(), String> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| "Contribution source is unavailable")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > maximum {
        return Err("Contribution source must be a bounded regular file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("Contribution source must exclude group and other access".into());
        }
    }
    Ok(())
}
fn bytes(path: &Path, maximum: u64) -> Result<Vec<u8>, String> {
    private_file(path, maximum)?;
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| "Contribution source is unavailable")?
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read contribution source")?;
    if bytes.len() as u64 > maximum {
        return Err("Contribution source exceeds its bound".into());
    }
    Ok(bytes)
}
impl Config {
    pub fn load(path: &Path) -> Result<Self, String> {
        let config: Self = serde_json::from_slice(&bytes(path, 256 * 1024)?)
            .map_err(|_| "Invalid contribution configuration")?;
        if config.plugins.len() + config.knowledge.len() > 16
            || config.events.len() > 256
            || config.documents.len() > 256
            || config.reviews.len() > 16
        {
            return Err("Too many contribution sources".into());
        }
        Ok(config)
    }
    pub fn read(&self, now: u64) -> Result<Vec<Contribution>, String> {
        if self.plugins.len() + self.knowledge.len() > 16
            || self.events.len() > 256
            || self.documents.len() > 256
            || self.reviews.len() > 16
        {
            return Err("Too many contribution sources".into());
        }
        let mut source_bytes = 0usize;
        let mut events = Vec::new();
        let mut future_events = 0usize;
        for path in &self.events {
            let event_bytes = bytes(path, 1024 * 1024)?;
            source_bytes += event_bytes.len();
            if source_bytes > 8 * 1024 * 1024 {
                return Err("Contribution evidence exceeds 8 MiB".into());
            }
            let event: Event =
                serde_json::from_slice(&event_bytes).map_err(|_| "Invalid retained event")?;
            event
                .validate_crypto()
                .map_err(|_| "Retained contribution event has an invalid signature")?;
            if event.created_at > now {
                future_events += 1;
                continue;
            }
            events.push(event);
        }
        let mut documents = Vec::new();
        for path in &self.documents {
            let document = bytes(path, 1024 * 1024)?;
            source_bytes += document.len();
            if source_bytes > 8 * 1024 * 1024 {
                return Err("Contribution evidence exceeds 8 MiB".into());
            }
            documents.push(document);
        }
        let docs = xp_ledger::eval::documents(documents);
        let trust = xp_ledger::XpTrust {
            referees: self.referees.clone(),
            runners: self.evaluators.clone(),
        };
        let ledger = xp_ledger::derive_with(&events, &docs, &trust);
        let mut out = Vec::new();
        for root in &self.plugins {
            let owner = Owner::open(root.clone(), NoCoder);
            bytes(&root.join("record.json"), 8 * 1024 * 1024)?;
            let record = owner.read()?;
            // Recheck the immutable reviewed tree, not just the display record.
            let files = owner.reviewed_files()?;
            let package = files
                .iter()
                .find(|(name, _)| name == "package.json")
                .ok_or("Frozen package record is unavailable")?;
            let parsed: serde_json::Value =
                serde_json::from_slice(&package.1).map_err(|_| "Invalid frozen package record")?;
            if route_contract::digest::Digest::of_bytes(&package.1) != record.release.digest
                || parsed["version"] != record.release.version
                || parsed["publisher"] != record.declarations.author
                || parsed["slug"].as_str().is_none_or(|slug| {
                    record.release.id != format!("{}:{slug}", record.declarations.author)
                })
                || record.attempts.len() > 128
            {
                return Err("Frozen contribution identity or attempt bound changed".into());
            }
            let mut row = empty(
                record.release.id.clone(),
                record.release.version.clone(),
                record.release.digest.as_str().into(),
                record.declarations.author.clone(),
            );
            row.source_record = openagents_chat::plugin_workbench::local_instance(root);
            row.sources.push(format!(
                "flow {} thread {} task {}",
                record.source.flow, record.source.thread, record.source.task
            ));
            row.authored =
                State::observed(vec![record.source.flow.clone()], "Reviewed frozen tree");
            for (id, attempt) in &record.attempts {
                if id.is_empty()
                    || id.len() > 128
                    || !id
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
                    || &attempt.request.id != id
                    || attempt.request.source != record.source
                    || attempt.request.release != record.release
                    || attempt.request.tree != record.tree
                {
                    return Err("Contribution attempt names a different frozen release".into());
                }
                let success = matches!(attempt.outcome, Outcome::Finished { ok: true, .. });
                row.attempts.push(format!(
                    "{id}: {} {}",
                    action_name(&attempt.request.action),
                    if success {
                        "succeeded"
                    } else {
                        "failed or unknown"
                    }
                ));
                if let Some(compare) = &attempt.comparison {
                    match comparison(root,id,compare,&record.release) {
                        Ok((outcome,measurements))=>row.attempts.push(format!("comparison {} verdict {} measurements {}",compare.report.as_str(),outcome,measurements)),
                        Err(())=>row.limitations.push(format!("Comparison {} unavailable: retained report missing or exact binding changed",compare.report.as_str())),
                    }
                }
                match &attempt.request.action {
                    Action::Publish if success => row.published.references.push(id.clone()),
                    Action::Install if success => row.installed.references.push(id.clone()),
                    Action::Reuse {
                        source_task,
                        thread,
                        ..
                    } if success => {
                        if let Some(reuse) = &attempt.reuse {
                            let pin =
                                serde_json::to_value(&record.release).map_err(|e| e.to_string())?;
                            if reuse["v"] == "openagents.plugin-use.v1"
                                && reuse["pin"] == pin
                                && reuse["dispatched"] == "ran"
                                && reuse["thread"] == *thread
                                && source_task != &record.source.task
                                && thread != &record.source.thread
                                && reuse["outputs"].as_array().is_some_and(|v| !v.is_empty())
                            {
                                if let Some(request) = reuse["request"].as_str().filter(|request| {
                                    request.starts_with("use-")
                                        && request.len() <= 1024
                                        && !request.chars().any(char::is_control)
                                }) {
                                    row.invoked.references.push(request.into());
                                    row.limitations.push(format!("Reuse {request}: declared source task {source_task}; retained route invocation, distinct workload independently unverified"));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            row.published.available = !row.published.references.is_empty();
            row.installed.available = !row.installed.references.is_empty();
            row.invoked.available = !row.invoked.references.is_empty();

            let releases = xp_ledger::eval::defaults_releases(&events, &record.release.id, &docs);
            let release_ids: BTreeSet<String> = releases
                .into_iter()
                .filter(|(event, release)| {
                    event.pubkey == row.author
                        && release.manifest.version == row.version
                        && release.manifest.files.len() == files.len()
                        && files.iter().all(|(name, bytes)| {
                            release.manifest.files.iter().any(|f| {
                                &f.path == name
                                    && f.digest
                                        == route_contract::digest::Digest::of_bytes(bytes).as_str()
                                    && f.size == bytes.len() as u64
                            })
                        })
                })
                .map(|(e, _)| e.id)
                .collect();
            row.published.references.extend(release_ids.iter().cloned());
            row.published.available = !row.published.references.is_empty();
            signed_evidence(&mut row, &events, &docs, &release_ids, &self.operators, now);
            row.credited = State::observed(
                ledger
                    .credits
                    .iter()
                    .filter(|c| {
                        c.evidence.iter().any(|e| {
                            release_ids.contains(e)
                                || row.validated.references.contains(e)
                                || row.adopted.references.contains(e)
                                || related_results(&events, &release_ids).contains(e)
                        })
                    })
                    .map(|c| {
                        format!(
                            "{} recipient {} {} XP role {}",
                            c.award, c.pubkey, c.xp, c.role
                        )
                    })
                    .collect(),
                "Trusted referee awards verified against retained evidence",
            );
            row.limitations.extend(
                ledger
                    .revoked
                    .iter()
                    .filter(|a| {
                        events
                            .iter()
                            .find(|e| &e.id == *a)
                            .and_then(|e| xp::parse_award(e).ok())
                            .is_some_and(|award| {
                                award.evidence.iter().any(|e| {
                                    release_ids.contains(&e.id)
                                        || related_results(&events, &release_ids).contains(&e.id)
                                })
                            })
                    })
                    .map(|a| format!("Revoked award {a}")),
            );
            if let Some(path) = &self.ledger {
                private_file(path, 512 * 1024 * 1024)?;
                let payments = pay_ledger::Ledger::open_read_only(path)
                    .map_err(|_| "Contribution payment ledger is unavailable")?;
                for release in &release_ids {
                    let receipts = payments
                        .contribution_receipts(&row.identity, release, &row.author)
                        .map_err(|_| "Contribution payment receipts are unavailable")?;
                    for receipt in receipts {
                        row.costs.push(format!(
                            "settlement {} resource {} author share {} msat payout {:?} state {:?}",
                            receipt.settlement,
                            receipt.resource,
                            receipt.share_msat,
                            receipt.payout_id,
                            receipt.payout_state
                        ));
                        if receipt.is_settled() {
                            row.settled.references.push(format!(
                                "settlement {} payout {} wallet {}",
                                receipt.settlement,
                                receipt.payout_id.unwrap_or_default(),
                                receipt.wallet_reference.unwrap_or_default()
                            ));
                        }
                    }
                }
                row.settled.available = !row.settled.references.is_empty();
                row.settled.scope = "Local ledger claims: exact author share belongs to succeeded wallet-referenced payout; batch amount is not individual share amount".into();
            }
            out.push(row);
        }
        for path in &self.knowledge {
            bytes(path, 4 * 1024 * 1024)?;
            let session = knowledge::workbench::Session::read(path)?;
            for candidate in &session.candidates {
                let entry = knowledge::Entry::parse(&candidate.bytes)?;
                let mut row = empty(
                    entry.id,
                    entry.version.to_string(),
                    candidate.digest.clone(),
                    entry.author,
                );
                row.source_record = openagents_chat::plugin_workbench::local_instance(path);
                row.authored = State::observed(
                    vec![candidate.digest.clone()],
                    "Retained immutable candidate",
                );
                row.sources = session
                    .selection
                    .sources
                    .iter()
                    .map(|s| {
                        format!(
                            "task {} run {} group {} artifact {} citation {}",
                            s.task,
                            s.run,
                            s.group,
                            s.artifact,
                            route_contract::digest::Digest::of_bytes(s.citation.as_bytes())
                                .as_str()
                        )
                    })
                    .collect();
                row.costs = session
                    .selection
                    .costs
                    .iter()
                    .filter(|(key, _)| {
                        ["acquisition_usd", "setup_usd", "checks_usd", "runtime_usd"]
                            .contains(&key.as_str())
                    })
                    .map(|(k, v)| format!("{k}: {}", v.map_or("unknown".into(), |v| v.to_string())))
                    .collect();
                row.attempts = session.attempts.iter().enumerate().map(|(i,a)|format!("proposal {i} outcomes {} outcome digest {}; known lower bound {} USD; model cost {:?}; embedding cost {:?}; unknown cost {}",a.outcomes.len(),route_contract::digest_of(&a.outcomes).as_str(),a.known_lower_bound_usd,a.model_usd,a.embedding_usd,a.unknown_cost_reason.is_some())).collect();
                let entries: BTreeSet<_> = events
                    .iter()
                    .filter_map(|event| {
                        nostr::kb::parse_entry(event)
                            .ok()
                            .filter(|entry| {
                                entry.document == candidate.bytes
                                    && entry.id == row.identity
                                    && entry.version.to_string() == row.version
                            })
                            .map(|_| event.id.clone())
                    })
                    .collect();
                row.published = State::observed(
                    entries.iter().cloned().collect(),
                    "Signed NIP-KB publication of exact retained candidate bytes; publisher signature is not external validation",
                );
                row.credited = State::observed(
                    ledger
                        .credits
                        .iter()
                        .filter(|credit| {
                            events
                                .iter()
                                .find(|event| event.id == credit.award)
                                .and_then(|event| xp::parse_award(event).ok())
                                .and_then(|award| award.entry)
                                .is_some_and(|entry| entries.contains(&entry.id))
                        })
                        .map(|credit| {
                            format!(
                                "{} recipient {} {} XP role {}",
                                credit.award, credit.pubkey, credit.xp, credit.role
                            )
                        })
                        .collect(),
                    "Trusted referee award recomputed over exact published entry",
                );
                let mut retired_admissions = BTreeSet::new();
                for review in &self.reviews {
                    bytes(review, 4 * 1024 * 1024)?;
                    let bundle = knowledge::prospective::Bundle::read(review)?;
                    if bundle.plan.record.candidate_digest != candidate.digest {
                        continue;
                    }
                    if !self.operators.contains(&bundle.plan.record.operator)
                        || !self.evaluators.contains(&bundle.plan.record.evaluator)
                    {
                        row.limitations
                            .push("Review signer is outside reader trust".into());
                        continue;
                    }
                    let trust = knowledge::prospective::Trust {
                        operator: bundle.plan.record.operator.clone(),
                        evaluator: bundle.plan.record.evaluator.clone(),
                    };
                    if bundle.report.record.completed_at <= now
                        && bundle.plan.record.committed_at <= now
                        && bundle.check_report(&candidate.bytes, &trust).is_ok()
                    {
                        row.validated = State::observed(
                            vec![bundle.report.digest()?],
                            "Independent prospective paired check; operator admission assessed separately",
                        );
                    }
                    match bundle.state(&candidate.bytes, now, &trust) {
                        Ok(knowledge::prospective::State::Admitted) => {
                            row.validated = State::observed(
                                vec![bundle.report.digest()?],
                                "Independent prospective paired check",
                            );
                            row.adopted = State::observed(
                                vec![
                                    bundle
                                        .review
                                        .as_ref()
                                        .ok_or("Missing operator review")?
                                        .digest()?,
                                ],
                                "Current signed operator admission",
                            );
                        }
                        Ok(state) => {
                            if let Some(review) = &bundle.review {
                                retired_admissions.insert(review.digest()?);
                            }
                            row.validated = State::observed(
                                vec![bundle.report.digest()?],
                                "Verified historical independent paired check; admission no longer current",
                            );
                            row.limitations
                                .push(format!("Prospective state {state:?}; no current admission"));
                        }
                        Err(reason) => row
                            .limitations
                            .push(format!("Prospective evidence refused: {reason}")),
                    }
                }
                row.adopted
                    .references
                    .retain(|reference| !retired_admissions.contains(reference));
                row.adopted.available = !row.adopted.references.is_empty();
                out.push(row);
            }
        }
        if future_events > 0 {
            for row in &mut out {
                row.limitations.push(format!(
                    "{future_events} future-dated signed events excluded from current evidence"
                ));
            }
        }
        Ok(out)
    }
}
fn empty(identity: String, version: String, digest: String, author: String) -> Contribution {
    Contribution {
        source_record: String::new(),
        identity,
        version,
        digest,
        author,
        sources: vec![],
        authored: State::unavailable("No authoring record"),
        published: State::unavailable(
            "Local publish command outcome; signed release checked separately",
        ),
        installed: State::unavailable("Local installer outcome; no authority inferred"),
        invoked: State::unavailable("No exact retained route invocation"),
        validated: State::unavailable("No independent exact-version check"),
        adopted: State::unavailable("No current trusted operator admission"),
        credited: State::unavailable("No verified signed award"),
        settled: State::unavailable("No actual succeeded payout for an exact release share"),
        attempts: vec![],
        costs: vec![
            "acquisition_usd: unknown".into(),
            "setup_usd: unknown".into(),
            "checks_usd: unknown".into(),
        ],
        limitations: vec![],
    }
}
fn comparison(
    root: &Path,
    id: &str,
    compare: &openagents_chat::plugin_workbench::Comparison,
    pin: &route_contract::snapshot::CapabilityPin,
) -> Result<(&'static str, String), ()> {
    let path = Path::new(&compare.path);
    let expected = root
        .join("comparisons")
        .join(id)
        .canonicalize()
        .map_err(|_| ())?;
    let actual = path.canonicalize().map_err(|_| ())?;
    let metadata = std::fs::symlink_metadata(path).map_err(|_| ())?;
    if !actual.starts_with(expected)
        || !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > 1024 * 1024
    {
        return Err(());
    }
    let bytes = std::fs::read(path).map_err(|_| ())?;
    if bytes.len() > 1024 * 1024
        || route_contract::digest::Digest::of_bytes(&bytes) != compare.report
    {
        return Err(());
    }
    let report = eval_ext::parse_report(&bytes).map_err(|_| ())?;
    if report.baseline.is_none()
        || report.subject.definition.artifact.digest != pin.digest.as_str()
        || !report
            .subject
            .definition
            .id
            .starts_with(&format!("{}/", pin.id))
    {
        return Err(());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| ())?;
    Ok((
        verdict(value["verdict"].as_str().unwrap_or("unknown")),
        measurement_summary(&value["measurements"]),
    ))
}
fn action_name(action: &Action) -> &'static str {
    match action {
        Action::Compare => "compare",
        Action::Publish => "publish",
        Action::Install => "install",
        Action::Enable => "enable",
        Action::Reuse { .. } => "reuse",
    }
}
fn verdict(value: &str) -> &'static str {
    match value {
        "pass" => "pass",
        "fail" => "fail",
        "inconclusive" => "inconclusive",
        _ => "unknown",
    }
}
fn measurement_summary(value: &serde_json::Value) -> String {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let arm = m["arm"].as_str()?;
            let metric = m["metric"].as_str()?;
            if !["baseline", "subject"].contains(&arm)
                || !["cases_passed", "mean_score", "cost_usd", "seconds"].contains(&metric)
            {
                return None;
            }
            Some(format!(
                "{arm} {metric}: {} unknown_count {} denominator {}",
                m["value"]
                    .as_f64()
                    .map_or("unknown".into(), |v| v.to_string()),
                m["unknown_count"]
                    .as_u64()
                    .map_or("unknown".into(), |v| v.to_string()),
                m["denominator"]
                    .as_u64()
                    .map_or("unknown".into(), |v| v.to_string())
            ))
        })
        .take(16)
        .collect::<Vec<_>>()
        .join("; ")
}
fn related_results(events: &[Event], releases: &BTreeSet<String>) -> BTreeSet<String> {
    xp_ledger::eval::publications(events)
        .into_iter()
        .filter(|(_, (_, p))| {
            p.subject_release
                .as_ref()
                .is_some_and(|s| releases.contains(&s.id))
        })
        .map(|(id, _)| id)
        .collect()
}
fn signed_evidence(
    row: &mut Contribution,
    events: &[Event],
    docs: &xp_ledger::eval::Documents,
    releases: &BTreeSet<String>,
    operators: &BTreeSet<String>,
    now: u64,
) {
    let publications = xp_ledger::eval::publications(events);
    let validations = xp_ledger::eval::validations(events, &publications);
    for (id, (_, p)) in &publications {
        if p.subject_release
            .as_ref()
            .is_some_and(|s| releases.contains(&s.id))
        {
            row.attempts.push(format!(
                "signed result {id}: {:?}, evaluator {} suite {} report {} checks {:?}",
                p.verdict(),
                p.evaluator,
                p.suite_release.id,
                p.report_ref.digest,
                p.checks
            ));
            if let Some(checked) = validations.get(id) {
                row.validated.references.push(id.clone());
                row.validated.references.extend(checked.clone());
            }
        }
    }
    row.validated.available = !row.validated.references.is_empty();
    row.validated.scope =
        "NIP-EVAL independent second-suite validation of exact signed release".into();
    let mut current = std::collections::BTreeMap::new();
    for operator in operators {
        let package = xp_ledger::adopt::package_of(operator);
        if let Some((event, _)) = xp_ledger::eval::defaults_releases(events, &package, docs)
            .into_iter()
            .filter(|(e, _)| e.created_at <= now)
            .last()
        {
            current.insert(operator, event);
        }
    }
    for event in current.values() {
        let Ok((manifest, admissions)) = xp_ledger::eval::adoption_documents(event, docs) else {
            continue;
        };
        for admission in admissions {
            let Ok(parsed) = eval_ext::parse_admission(admission) else {
                continue;
            };
            if parsed
                .subject
                .event
                .as_ref()
                .is_none_or(|s| !releases.contains(&s.id))
            {
                continue;
            }
            if parsed.expires_at <= now
                || parsed.issuer != event.pubkey
                || parsed.decision != "admit"
            {
                row.limitations.push(format!(
                    "Operator admission {}: {} expired or untrusted",
                    event.id, parsed.decision
                ));
                continue;
            }
            let adoption = xp::eval_adopt::Adoption {
                release: event,
                manifest,
                admission,
                results: events,
                checks: events,
                requests: events,
            };
            for quest in events.iter().filter_map(|e| xp::parse_quest(e).ok()) {
                if let Ok(completion) = xp::eval_adopt::check_eval_adopt(&quest, &adoption) {
                    // The owning rule verifies reports, independent checks, admission, and defaults dependency.
                    let _ = completion;
                    if row.validated.available
                        && releases.iter().any(|id| {
                            parsed
                                .subject
                                .event
                                .as_ref()
                                .is_some_and(|release| &release.id == id)
                        })
                    {
                        row.adopted.references.push(event.id.clone());
                    }
                }
            }
        }
    }
    row.adopted.available = !row.adopted.references.is_empty();
    row.adopted.scope = "Trusted operator defaults release checked by NIP-XP adoption rule".into();
}

struct Adapter {
    config: Config,
    host: Host,
}
impl PaneAdapter for Adapter {
    fn kind(&self) -> PaneKind {
        PaneKind::Receipt
    }
    fn describe(&self, subject: &Subject) -> Description {
        if subject.host() != &self.host {
            return Description::only(PaneState::Missing, "Contribution");
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let Ok(rows) = self.config.read(now) else {
            return Description::only(PaneState::Unavailable, "Contribution evidence unavailable");
        };
        let Some(row) = rows.iter().find(|r| id(r) == subject.id()) else {
            return Description::only(PaneState::Missing, "Contribution");
        };
        let revision = Revision::Sha256(
            route_contract::digest_of(row)
                .as_str()
                .trim_start_matches("sha256:")
                .into(),
        );
        if subject.revision().is_some_and(|r| r != &revision) {
            return Description::only(
                PaneState::Stale {
                    current: Some(revision),
                },
                "Contribution evidence changed",
            );
        }
        let mut detail = row.lines();
        while detail.len() > 2048 {
            detail.pop();
        }
        Description {
            state: PaneState::Ready,
            title: "Contribution evidence".into(),
            detail,
            actions: vec![],
        }
    }
}
fn id(row: &Contribution) -> String {
    route_contract::digest_of(&(
        row.source_record.clone(),
        row.identity.clone(),
        row.version.clone(),
        row.digest.clone(),
    ))
    .as_str()
    .trim_start_matches("sha256:")
    .into()
}
pub fn mount(application: &mut terminal_core::Application, config: Config) -> Result<(), String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "Invalid clock")?
        .as_secs();
    let rows = config.read(now)?;
    let host = Host::Local {
        instance: route_contract::digest_of(&config)
            .as_str()
            .trim_start_matches("sha256:")
            .into(),
    };
    application.products.panes = std::mem::take(&mut application.products.panes).adapter_for_host(
        host.clone(),
        Box::new(Adapter {
            config,
            host: host.clone(),
        }),
    );
    for row in &rows {
        application.products.open(
            PaneKind::Receipt,
            &Subject::Record {
                host: host.clone(),
                id: id(row),
                revision: None,
            },
        )?;
    }
    application.paper.on = true;
    Ok(())
}

#[cfg(test)]
mod tests;
