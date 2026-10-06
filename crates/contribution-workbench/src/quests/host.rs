//! Explicit private sources and read-only native quest navigation.
use super::{Quest, project};
use nostr::domain::Event;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use workbench::{
    Host, Kind, ResourceRef, Revision,
    pane::{Description, PaneAdapter, PaneKind, PaneState, Subject},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub events: Vec<PathBuf>,
    #[serde(default)]
    pub documents: Vec<PathBuf>,
    pub referees: BTreeSet<String>,
    #[serde(default)]
    pub runners: BTreeSet<String>,
    pub trainer: String,
}
impl Config {
    pub fn load(path: &Path) -> Result<Self, String> {
        serde_json::from_slice(&crate::host::bytes(path, 1024 * 1024)?)
            .map_err(|_| "Invalid private quest configuration".into())
    }
    fn read(&self, now: u64) -> Result<(Vec<Quest>, Vec<Event>), String> {
        if self.events.len() + self.documents.len() > 256
            || !self
                .events
                .iter()
                .chain(&self.documents)
                .all(|p| p.is_absolute())
            || !self
                .referees
                .iter()
                .chain(&self.runners)
                .chain(std::iter::once(&self.trainer))
                .all(|p| {
                    p.len() == 64
                        && p.bytes()
                            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                })
        {
            return Err(
                "Quest sources require bounded absolute paths and exact public keys".into(),
            );
        }
        let mut total = 0;
        let mut events = Vec::new();
        for path in &self.events {
            let bytes = crate::host::bytes(path, 1024 * 1024)?;
            total += bytes.len();
            if total > 8 * 1024 * 1024 {
                return Err("Quest sources exceed the retained evidence limit".into());
            }
            let event: Event =
                serde_json::from_slice(&bytes).map_err(|_| "Invalid retained quest event")?;
            event
                .validate_crypto()
                .map_err(|_| "Invalid retained quest signature")?;
            if event.created_at <= now {
                events.push(event);
            }
        }
        let mut documents = Vec::new();
        for path in &self.documents {
            let bytes = crate::host::bytes(path, 1024 * 1024)?;
            total += bytes.len();
            if total > 8 * 1024 * 1024 {
                return Err("Quest sources exceed the retained evidence limit".into());
            }
            documents.push(bytes);
        }
        if total > 8 * 1024 * 1024 {
            return Err("Quest sources exceed the retained evidence limit".into());
        }
        let trust = xp_ledger::XpTrust {
            referees: self.referees.clone(),
            runners: self.runners.clone(),
        };
        let rows = project(
            &events,
            &xp_ledger::eval::documents(documents),
            &trust,
            &self.trainer,
            now,
        );
        Ok((rows, events))
    }
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
struct Adapter {
    config: Config,
    host: Host,
    kind: PaneKind,
}
impl PaneAdapter for Adapter {
    fn kind(&self) -> PaneKind {
        self.kind
    }
    fn describe(&self, subject: &Subject) -> Description {
        if subject.host() != &self.host {
            return Description::only(PaneState::Missing, "Quest evidence");
        }
        let Ok((rows, events)) = self.config.read(now()) else {
            return Description::only(PaneState::Unavailable, "Quest evidence unavailable");
        };
        let id = subject.id();
        let (title, detail, revision) = if let Some(row) = rows.iter().find(|q| q.event == id) {
            (
                "Quest eligibility and XP",
                row.lines(),
                Revision::Sha256(
                    route_contract::digest_of(row)
                        .as_str()
                        .trim_start_matches("sha256:")
                        .into(),
                ),
            )
        } else if let Some(event) = events.iter().find(|e| {
            e.id == id
                && rows
                    .iter()
                    .any(|q| q.evidence.contains(&e.id) || q.awards.contains(&e.id))
        }) {
            let status = if self.kind == PaneKind::Receipt {
                rows.iter()
                    .filter(|q| q.awards.contains(&event.id))
                    .flat_map(|q| q.award_decisions.get(&event.id).into_iter().flatten())
                    .take(8)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                metadata(event)
            };
            (
                "Exact signed quest evidence",
                format!(
                    "Event: {}\nPublisher: {}\nKind: {}\nCreated: {}\n{status}",
                    event.id, event.pubkey, event.kind, event.created_at
                ),
                Revision::Sha256(
                    route_contract::digest_of(&(event.id.clone(), status.clone()))
                        .as_str()
                        .trim_start_matches("sha256:")
                        .into(),
                ),
            )
        } else {
            return Description::only(PaneState::Missing, "Exact quest evidence not retained");
        };
        if subject.revision().is_some_and(|r| r != &revision) {
            return Description::only(
                PaneState::Stale {
                    current: Some(revision),
                },
                "Quest evidence changed",
            );
        }
        let mut detail: String = detail
            .chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .collect();
        while detail.len() > 2048 {
            detail.pop();
        }
        Description {
            state: PaneState::Ready,
            title: title.into(),
            detail,
            actions: vec![],
        }
    }
}
fn metadata(event: &Event) -> String {
    if let Ok(entry) = nostr::kb::parse_entry(event) {
        return format!(
            "Knowledge entry: {}\nVersion: {}\nDocument digest: {}\nPublisher: {}",
            entry.id, entry.version, entry.digest, event.pubkey
        );
    }
    if let Ok(evidence) = nostr::kb::parse_evidence(event) {
        return format!(
            "Knowledge evidence report digest: {}\nExact entry references: {}\nA signed report still requires the owning transfer rule; historical screening is inconclusive.",
            evidence.report.digest,
            evidence.entries.join(", ")
        );
    }
    if let Ok(revocation) = nostr::xp::parse_revocation(event) {
        return format!(
            "Signed award revocation\nAward: {}\nReferee: {}\nReason digest: {}\nThe private reason text is retained in the configured source.",
            revocation.award.id,
            revocation.award.pubkey,
            route_contract::digest_of(&revocation.reason).as_str()
        );
    }
    if let Ok(publication) = nostr::eval_ext::parse_publication(event) {
        return format!(
            "Evaluation verdict: {:?}\nSuite release: {}\nSubject release: {}\nChecks: {}\nExact report digest: {}",
            publication.report.verdict,
            publication.suite_release.id,
            publication
                .subject_release
                .as_ref()
                .map_or("unpublished", |p| p.id.as_str()),
            publication.checks.as_deref().unwrap_or("none"),
            publication.report_ref.digest
        );
    }
    if let Ok(run) = nostr::xp::parse_run_evidence(event) {
        return format!(
            "Run verdict: {}\nRecipe digest: {}\nRecord digest: {}\nCost USD: {:?}\nSeconds: {:?}",
            run.verdict,
            run.recipe_digest,
            run.record.summary.digest,
            run.record.usd,
            run.record.seconds
        );
    }
    if let Ok(record) = nostr::ext::parse_record(event) {
        return format!(
            "Component release: {}\nVersion: {}\nManifest digest: {}",
            record
                .get("package")
                .and_then(|v| v.as_str())
                .unwrap_or("unavailable"),
            record
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("unavailable"),
            record
                .get("manifest")
                .and_then(|v| v.get("digest"))
                .and_then(|v| v.as_str())
                .unwrap_or("unavailable")
        );
    }
    "Typed evidence metadata unavailable; signed identity alone does not establish completion. Raw source content remains private.".into()
}
pub fn mount(application: &mut terminal_core::Application, config: Config) -> Result<(), String> {
    mount_products(&mut application.products, config)?;
    application.paper.on = true;
    Ok(())
}
fn mount_products(
    products: &mut terminal_core::resources::Products,
    config: Config,
) -> Result<(), String> {
    let (rows, events) = config.read(now())?;
    let host = Host::Local {
        instance: route_contract::digest_of(&config)
            .as_str()
            .trim_start_matches("sha256:")
            .into(),
    };
    let targets: BTreeSet<(String, u8)> = rows
        .iter()
        .flat_map(|q| {
            std::iter::once((q.event.clone(), 0)).chain(
                q.evidence
                    .iter()
                    .chain(&q.awards)
                    .filter(|id| events.iter().any(|e| &e.id == *id))
                    .map(|id| (id.clone(), if q.awards.contains(id) { 2 } else { 1 })),
            )
        })
        .collect();
    let additions = targets
        .iter()
        .filter(|(id, kind)| {
            !products.open.iter().any(|p| {
                p.subject.host() == &host
                    && p.subject.id() == id.as_str()
                    && p.pane
                        == match kind {
                            0 => PaneKind::Evaluation,
                            2 => PaneKind::Receipt,
                            _ => PaneKind::Artifact,
                        }
            })
        })
        .count();
    if products.open.len() + additions > terminal_core::resources::PRODUCTS_MAX {
        return Err(
            "Quest selection exceeds available product panes; select fewer retained quest records"
                .into(),
        );
    }
    for kind in [PaneKind::Evaluation, PaneKind::Receipt, PaneKind::Artifact] {
        products.panes = std::mem::take(&mut products.panes).adapter_for_host(
            host.clone(),
            Box::new(Adapter {
                config: config.clone(),
                host: host.clone(),
                kind,
            }),
        );
    }
    for row in &rows {
        for id in row.evidence.iter().chain(&row.awards) {
            if events.iter().any(|e| &e.id == id) {
                let kind = if row.awards.contains(id) {
                    PaneKind::Receipt
                } else {
                    PaneKind::Artifact
                };
                let subject = if kind == PaneKind::Receipt {
                    Subject::Record {
                        host: host.clone(),
                        id: id.clone(),
                        revision: Some(Revision::Sha256(
                            route_contract::digest_of(&(
                                id.clone(),
                                row.award_decisions
                                    .get(id)
                                    .into_iter()
                                    .flatten()
                                    .take(8)
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                            ))
                            .as_str()
                            .trim_start_matches("sha256:")
                            .into(),
                        )),
                    }
                } else {
                    Subject::Resource {
                        resource: ResourceRef::new(Kind::Artifact, host.clone(), id.clone()),
                    }
                };
                products.open(kind, &subject)?;
            }
        }
        let mut resource = ResourceRef::new(Kind::Evidence, host.clone(), row.event.clone());
        resource.revision = Some(Revision::Sha256(
            route_contract::digest_of(row)
                .as_str()
                .trim_start_matches("sha256:")
                .into(),
        ));
        products.open(PaneKind::Evaluation, &Subject::Resource { resource })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::tests::fixture;
    use super::*;
    #[test]
    fn shared_native_mount_opens_exact_evidence_and_refuses_stale_credit() {
        let (events, referee) = fixture();
        let root = tempfile::tempdir().unwrap();
        let mut config = Config {
            events: vec![],
            documents: vec![],
            referees: BTreeSet::from([referee]),
            runners: BTreeSet::new(),
            trainer: xp_ledger::eval::fixture::pubkey("quest-checker"),
        };
        for event in &events {
            let path = root.path().join(&event.id);
            std::fs::write(&path, serde_json::to_vec(event).unwrap()).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
            }
            config.events.push(path);
        }
        let mut products = terminal_core::resources::Products::default();
        mount_products(&mut products, config.clone()).unwrap();
        assert_eq!(products.open.len(), 8);
        assert!(products.open.iter().all(|p| p.actions.is_empty()));
        let award = events
            .iter()
            .find(|e| {
                nostr::xp::parse_award(e)
                    .is_ok_and(|a| a.awardees.iter().any(|r| r.pubkey == config.trainer))
            })
            .unwrap();
        let host = Host::Local {
            instance: route_contract::digest_of(&config)
                .as_str()
                .trim_start_matches("sha256:")
                .into(),
        };
        let (rows, _) = config.read(now()).unwrap();
        let decisions = rows[0].award_decisions[&award.id]
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        let subject = Subject::Record {
            host: host.clone(),
            id: award.id.clone(),
            revision: Some(Revision::Sha256(
                route_contract::digest_of(&(award.id.clone(), decisions))
                    .as_str()
                    .trim_start_matches("sha256:")
                    .into(),
            )),
        };
        let adapter = Adapter {
            config: config.clone(),
            host,
            kind: PaneKind::Receipt,
        };
        assert!(matches!(adapter.describe(&subject).state, PaneState::Ready));
        config.referees.clear();
        let changed = Adapter {
            config,
            host: adapter.host.clone(),
            kind: PaneKind::Receipt,
        };
        assert!(matches!(
            changed.describe(&subject).state,
            PaneState::Stale { .. }
        ));
        let report = events
            .iter()
            .find(|e| nostr::eval_ext::parse_publication(e).is_ok())
            .unwrap();
        let detail = metadata(report);
        assert!(detail.contains("Evaluation verdict"));
        assert!(detail.contains("Suite release"));
        assert!(!detail.contains("ext_eval_report"));
    }
    #[test]
    fn capacity_refusal_preserves_existing_panes() {
        let (events, referee) = fixture();
        let root = tempfile::tempdir().unwrap();
        let mut config = Config {
            events: vec![],
            documents: vec![],
            referees: BTreeSet::from([referee]),
            runners: BTreeSet::new(),
            trainer: xp_ledger::eval::fixture::pubkey("quest-checker"),
        };
        for event in events {
            let path = root.path().join(&event.id);
            std::fs::write(&path, serde_json::to_vec(&event).unwrap()).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
            }
            config.events.push(path);
        }
        let mut products = terminal_core::resources::Products::default();
        mount_products(&mut products, config.clone()).unwrap();
        let descriptor = products.open[0].clone();
        while products.open.len() < 32 {
            products.open.push(descriptor.clone());
        }
        let receipt = products
            .open
            .iter_mut()
            .find(|p| p.pane == PaneKind::Receipt)
            .unwrap();
        let host = receipt.subject.host().clone();
        let id = receipt.subject.id().to_owned();
        receipt.pane = PaneKind::Artifact;
        receipt.subject = Subject::Resource {
            resource: ResourceRef::new(Kind::Artifact, host, id),
        };
        let before = products.open.clone();
        assert!(
            mount_products(&mut products, config)
                .unwrap_err()
                .contains("select fewer")
        );
        assert_eq!(products.open, before);
    }
}
