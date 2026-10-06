//! Explicit retained-source harvesting. Candidates never grant admission or publication.
use crate::{Entry, Status, digest, harvest, lint};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub task: String,
    pub run: String,
    pub group: String,
    pub artifact: String,
    pub citation: String,
    /// Only these explicitly disclosed bytes may reach the proposer.
    pub disclosed: String,
}

impl Source {
    /// Read only the explicitly selected retained run; disclose its bounded harvest view.
    pub fn retained_run(dir: &Path, group: String, citation: String) -> Result<Self, String> {
        let record = harvest::record(dir)?;
        let summary = std::fs::read(dir.join("summary.json")).map_err(|e| e.to_string())?;
        let events = std::fs::read(dir.join("events.jsonl")).map_err(|e| e.to_string())?;
        let identity =
            serde_json::to_vec(&(digest(&summary), digest(&events))).map_err(|e| e.to_string())?;
        Ok(Self {
            task: record.task,
            run: record.run,
            group,
            artifact: digest(&identity),
            citation,
            disclosed: record.text,
        })
    }
}

/// Disclosure policy is supplied by the retained artifact's owner.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub sources: Vec<Source>,
    pub forbidden: Vec<String>,
    /// Acquisition, setup, and independent checks remain separate costs.
    pub costs: BTreeMap<String, Option<f64>>,
}

impl Selection {
    pub fn check(&self) -> Result<(), String> {
        if self.sources.is_empty() || self.sources.len() > 16 {
            return Err("Select 1 to 16 retained sources".into());
        }
        for source in &self.sources {
            if [&source.task, &source.run, &source.group, &source.citation]
                .iter()
                .any(|s| s.trim().is_empty() || s.len() > 1024)
                || !source.artifact.starts_with("sha256:")
                || source.artifact.len() != 71
                || !source.artifact[7..]
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                || source.disclosed.len() > 60_000
            {
                return Err("Invalid retained source reference or disclosure bound".into());
            }
            self.disclosure(&source.disclosed)?;
        }
        if ["acquisition_usd", "setup_usd", "checks_usd"]
            .iter()
            .any(|key| !self.costs.contains_key(*key))
            || self.forbidden.iter().any(|s| s.is_empty())
            || self
                .costs
                .values()
                .flatten()
                .any(|n| !n.is_finite() || *n < 0.0)
        {
            return Err("Invalid disclosure policy or cost".into());
        }
        Ok(())
    }
    fn disclosure(&self, text: &str) -> Result<(), String> {
        if self.forbidden.iter().any(|secret| text.contains(secret)) {
            return Err("Candidate contains forbidden private material".into());
        }
        Ok(())
    }
    fn record(&self) -> harvest::Record {
        harvest::Record {
            task: self.sources[0].task.clone(),
            run: self.sources[0].run.clone(),
            trace: false,
            contrast: false,
            text: self
                .sources
                .iter()
                .map(|s| format!("Source citation: {}\n{}\n", s.citation, s.disclosed))
                .collect(),
        }
    }
}

/// Every attempt is retained, including failures and uncertain model costs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    pub model_basis: String,
    pub unknown_cost_reason: Option<String>,
    pub model_usd: Option<f64>,
    pub embedding_usd: Option<f64>,
    pub known_lower_bound_usd: f64,
    pub outcomes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub revision: u64,
    pub bytes: String,
    pub digest: String,
    pub problems: Vec<crate::Problem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub v: String,
    pub selection: Selection,
    /// Earlier edits remain inspectable by exact digest.
    pub candidates: Vec<Candidate>,
    pub attempts: Vec<Attempt>,
}

impl Session {
    /// Read a bounded retained session, checking every immutable candidate revision.
    pub fn read(path: &Path) -> Result<Self, String> {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err("Retained knowledge session exceeds 4 MiB".into());
        }
        let session: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if session.v != "openagents.knowledge-workbench.v1" {
            return Err("Unsupported knowledge workbench session".into());
        }
        session.selection.check()?;
        for (i, c) in session.candidates.iter().enumerate() {
            let entry = Entry::parse(&c.bytes)?;
            session.selection.disclosure(&c.bytes)?;
            if c.revision != i as u64 + 1
                || c.digest != digest(c.bytes.as_bytes())
                || entry.status != Status::Candidate
                || !entry.evidence.is_empty()
                || entry.cites.is_empty()
                || entry.cites.iter().any(|citation| {
                    !session
                        .selection
                        .sources
                        .iter()
                        .any(|s| &s.citation == citation)
                })
                || session.selection.sources.iter().any(|s| {
                    [&s.task, &s.run, &s.group]
                        .iter()
                        .any(|id| !entry.written_from.contains(id))
                })
            {
                return Err("Retained candidate identity or source provenance changed".into());
            }
        }
        Ok(session)
    }
    /// Save a new private retained record; an existing history is never overwritten.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        use std::io::Write;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err("Retained knowledge session exceeds 4 MiB".into());
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path).map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())
    }
    pub fn new(selection: Selection) -> Result<Self, String> {
        selection.check()?;
        Ok(Self {
            v: "openagents.knowledge-workbench.v1".into(),
            selection,
            candidates: Vec::new(),
            attempts: Vec::new(),
        })
    }
    /// Explicitly generate candidates using the existing harvest and lint machinery.
    /// `dir` is a caller-owned staging directory, never a serving corpus.
    pub async fn harvest<P: harvest::Propose, E: crate::search::Embed>(
        &mut self,
        dir: &Path,
        proposer: &P,
        corpus: &lint::Corpus,
    ) -> Result<(), String> {
        self.selection.check()?;
        let metadata = std::fs::symlink_metadata(dir).map_err(|e| e.to_string())?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err("Harvest staging must be a regular private directory".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        if dir.read_dir().map_err(|e| e.to_string())?.next().is_some() {
            return Err("Harvest staging directory must be empty".into());
        }
        let paid = std::cell::RefCell::new(None);
        if self.selection.record().text.len() > 60_000 {
            return Err("Selected disclosures exceed the harvest record bound".into());
        }
        let gate = Gate {
            proposer,
            selection: &self.selection,
            paid: &paid,
        };
        let result =
            harvest::harvest_record::<_, E>(self.selection.record(), dir, &gate, None, corpus)
                .await;
        match result {
            Err(error) => {
                self.attempts.push(Attempt {
                    model_basis: paid
                        .borrow()
                        .as_ref()
                        .map_or_else(|| "unknown".into(), |c| c.basis.to_string()),
                    unknown_cost_reason: paid
                        .borrow()
                        .as_ref()
                        .and_then(|c| c.unknown.clone())
                        .or_else(|| Some(error.clone())),
                    model_usd: paid.borrow().as_ref().and_then(|c: &harvest::Cost| c.usd),
                    embedding_usd: Some(0.0),
                    known_lower_bound_usd: paid.borrow().as_ref().map_or(0.0, |c| c.known_usd),
                    outcomes: vec![error.clone()],
                });
                Err(error)
            }
            Ok(result) => {
                let mut outcomes = Vec::new();
                for (id, written) in &result.proposals {
                    match written {
                        harvest::Written::New(path) | harvest::Written::Version { path, .. } => {
                            let bytes = match std::fs::read_to_string(path) {
                                Ok(bytes) => bytes,
                                Err(error) => {
                                    outcomes.push(format!("{id}: unknown: {error}"));
                                    continue;
                                }
                            };
                            match self.edit(&bytes, corpus) {
                                Ok(_) => outcomes.push(format!("{id}: candidate retained")),
                                Err(error) => outcomes.push(format!("{id}: refused: {error}")),
                            }
                        }
                        harvest::Written::Refused(error) => {
                            outcomes.push(format!("{id}: refused: {error}"))
                        }
                    }
                }
                self.attempts.push(Attempt {
                    model_basis: result.model_cost.basis.to_string(),
                    unknown_cost_reason: result.model_cost.unknown.clone(),
                    model_usd: result.model_cost.usd,
                    embedding_usd: result.embedding_usd,
                    known_lower_bound_usd: result.known_usd(),
                    outcomes,
                });
                Ok(())
            }
        }
    }
    /// Edit a candidate, preserving all earlier bytes. The caller must display lint problems.
    pub fn edit(&mut self, bytes: &str, corpus: &lint::Corpus) -> Result<&Candidate, String> {
        self.selection.check()?;
        if bytes.len() > 128 * 1024 {
            return Err("Candidate exceeds 128 KiB".into());
        }
        self.selection.disclosure(bytes)?;
        let mut entry = Entry::parse(bytes)?;
        if entry.status != Status::Candidate || !entry.evidence.is_empty() {
            return Err("Workbench edits require a candidate without admission evidence".into());
        }
        if entry.cites.is_empty()
            || entry
                .cites
                .iter()
                .any(|c| !self.selection.sources.iter().any(|s| &s.citation == c))
        {
            return Err("Candidate citation is not an explicitly selected source".into());
        }
        entry.written_from = self
            .selection
            .sources
            .iter()
            .flat_map(|s| [s.task.clone(), s.run.clone(), s.group.clone()])
            .collect();
        entry.written_from.sort();
        entry.written_from.dedup();
        let mut corpus = corpus.clone();
        corpus
            .names
            .extend(self.selection.sources.iter().map(|s| s.task.clone()));
        let bytes = entry.render();
        let parsed = Entry::parse(&bytes)?;
        let problems = lint::lint(std::slice::from_ref(&parsed), &corpus);
        self.candidates.push(Candidate {
            revision: self.candidates.len() as u64 + 1,
            digest: digest(bytes.as_bytes()),
            bytes,
            problems,
        });
        Ok(self.candidates.last().expect("candidate was appended"))
    }
    /// Recheck the exact current candidate against the caller's benchmark corpus.
    pub fn lint(&self, corpus: &lint::Corpus) -> Result<Vec<crate::Problem>, String> {
        let candidate = self.candidates.last().ok_or("No candidate selected")?;
        self.selection.disclosure(&candidate.bytes)?;
        let entry = Entry::parse(&candidate.bytes)?;
        let mut corpus = corpus.clone();
        corpus
            .names
            .extend(self.selection.sources.iter().map(|s| s.task.clone()));
        Ok(lint::lint(&[entry], &corpus))
    }

    /// Draft only: freeze/execute remains a separate Gym owner operation.
    pub fn draft_study(&self, mut plan: crate::study::Plan) -> Result<crate::study::Plan, String> {
        let candidate = self.candidates.last().ok_or("No candidate selected")?;
        if !candidate.problems.is_empty() || !self.lint(&lint::Corpus::default())?.is_empty() {
            return Err("Fix candidate lint problems before drafting a study".into());
        }
        if plan.candidate_digest != candidate.digest {
            return Err("Study draft must pin the exact candidate bytes".into());
        }
        let entry = Entry::parse(&candidate.bytes)?;
        plan.source_tasks.extend(entry.written_from.iter().cloned());
        let groups: Vec<_> = self.selection.sources.iter().map(|s| &s.group).collect();
        if plan.cases.iter().any(|c| {
            c.partition == crate::study::Partition::Confirmation && groups.contains(&&c.group)
        }) {
            return Err("Source group cannot count as confirmation".into());
        }
        plan.validate(&[entry])?;
        Ok(plan)
    }
}

struct Gate<'a, P> {
    proposer: &'a P,
    selection: &'a Selection,
    paid: &'a std::cell::RefCell<Option<harvest::Cost>>,
}
impl<P: harvest::Propose> harvest::Propose for Gate<'_, P> {
    fn model(&self) -> &str {
        self.proposer.model()
    }
    fn provider(&self) -> &str {
        self.proposer.provider()
    }
    async fn propose(
        &self,
        system: &str,
        prompt: &str,
    ) -> Result<(harvest::Proposals, harvest::Cost), String> {
        self.selection.disclosure(prompt)?;
        let (proposals, cost) = self.proposer.propose(system, prompt).await?;
        *self.paid.borrow_mut() = Some(cost.clone());
        for p in &proposals.entries {
            for text in [
                &p.id,
                &p.kind,
                &p.title,
                &p.summary,
                &p.applies_when,
                &p.body,
                &p.updates,
            ]
            .into_iter()
            .chain(p.tags.iter())
            .chain(p.cites.iter())
            {
                self.selection.disclosure(text)?;
            }
            if p.cites.is_empty()
                || p.cites
                    .iter()
                    .any(|c| !self.selection.sources.iter().any(|s| &s.citation == c))
            {
                return Err("Proposer returned a citation outside the selected sources".into());
            }
        }
        Ok((proposals, cost))
    }
}

pub(crate) fn bounded_detail(text: &str) -> String {
    let mut detail = String::new();
    for c in text.chars() {
        let c = if c.is_control() && c != '\n' { ' ' } else { c };
        if detail.len() + c.len_utf8() > 2048 {
            break;
        }
        detail.push(c);
    }
    detail
}

/// Read-only knowledge pane projection. Commands remain explicit session operations.
pub struct Adapter {
    pub id: String,
    pub host: ::workbench::Host,
    pub session: Session,
}
impl ::workbench::pane::PaneAdapter for Adapter {
    fn kind(&self) -> ::workbench::pane::PaneKind {
        ::workbench::pane::PaneKind::Knowledge
    }
    fn describe(&self, subject: &::workbench::pane::Subject) -> ::workbench::pane::Description {
        use ::workbench::pane::{Description, PaneState, Subject};
        let Subject::Record { host, id, revision } = subject else {
            return Description::only(PaneState::Missing, "Knowledge candidate");
        };
        if host != &self.host || id != &self.id {
            return Description::only(PaneState::Missing, "Knowledge candidate");
        }
        let current = self
            .session
            .candidates
            .last()
            .map(|c| ::workbench::Revision::Sha256(c.digest[7..].into()));
        if revision.is_some() && revision != &current {
            return Description::only(PaneState::Stale { current }, "Knowledge candidate");
        }
        Description {
            state: PaneState::Ready,
            title: "Knowledge candidate".into(),
            detail: {
                let summary = format!(
                    "{} selected sources; {} retained revisions; {} attempts. Candidate only; publication and admission require separate owner operations.",
                    self.session.selection.sources.len(),
                    self.session.candidates.len(),
                    self.session.attempts.len()
                );
                let sources = self
                    .session
                    .selection
                    .sources
                    .iter()
                    .take(3)
                    .map(|s| {
                        format!(
                            "{} / {} / {}",
                            s.task.chars().take(64).collect::<String>(),
                            s.run.chars().take(64).collect::<String>(),
                            s.group.chars().take(64).collect::<String>()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                let costs = self
                    .session
                    .selection
                    .costs
                    .iter()
                    .map(|(name, value)| {
                        format!(
                            "{name}: {}",
                            value.map_or_else(|| "unknown".into(), |n| format!("${n:.5}"))
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let candidate = self
                    .session
                    .candidates
                    .last()
                    .map(|c| {
                        format!(
                            "Revision {}: {} ({} lint problems)\n{}",
                            c.revision,
                            c.digest,
                            c.problems.len(),
                            Entry::parse(&c.bytes).map_or_else(
                                |_| "Candidate could not be parsed".into(),
                                |e| e.body
                            )
                        )
                    })
                    .unwrap_or_else(|| "No candidate generated".into());
                bounded_detail(&format!(
                    "{summary}\nSources (preview): {sources}\nCosts: {costs}\n{candidate}"
                ))
            },
            actions: vec![
                "harvest".into(),
                "inspect".into(),
                "edit".into(),
                "lint".into(),
                "draft-study".into(),
            ],
        }
    }
}

#[cfg(test)]
mod tests;
