//! Private retained owner proofs and an explicitly selected scratch practice path.
use super::{Fact, Lane, Objective, Row, Scope, evaluate};
use coder_access::{Operation, Outcome, review::TaskReview, studio::Snapshot};
use openagents_chat::studio::{ResultView, State};
use route_contract::{AdmissionSnapshot, binding::WorkbenchBinding, studio::StudioRoute};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use workbench::{
    Host, Revision,
    pane::{Description, PaneAdapter, PaneKind, PaneState, Subject},
};

pub mod practice;

static BOOK_WRITE: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub const SCHEMA: &str = "openagents.onboarding-book.v1";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub scope: Scope,
    pub starter: PathBuf,
    pub book: PathBuf,
    pub tasks: PathBuf,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudioProof {
    pub route: StudioRoute,
    pub admission: AdmissionSnapshot,
    pub binding: WorkbenchBinding,
    pub result: ResultView,
    pub snapshot: Snapshot,
    pub review: Option<TaskReview>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalProof {
    pub owner: TerminalOwner,
    pub open: coder_pty::wire::Open,
    pub result: coder_pty::wire::TerminalResult,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalOwner {
    pub studio_stream: String,
    pub host_key: String,
    pub host_generation: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inspection {
    pub config: PathBuf,
    pub source: String,
    pub revision: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    pub schema: String,
    pub scope: Scope,
    pub snapshot: Option<Snapshot>,
    pub terminal: Option<TerminalProof>,
    pub records: Vec<StudioProof>,
    pub inspection: Option<Inspection>,
    #[serde(default)]
    pub direct: Vec<OwnerProof>,
}

/// A direct client retains the typed owner acknowledgment alongside the displayed source.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerProof {
    pub request: String,
    pub operation: Operation,
    pub outcome: Outcome,
    pub snapshot: Snapshot,
    pub review: Option<TaskReview>,
}

fn read(path: &Path, max: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    if !path.is_absolute() {
        return Err("Onboarding sources require explicit absolute paths".into());
    }
    let m = std::fs::symlink_metadata(path).map_err(|_| "Onboarding source unavailable")?;
    if !m.is_file() || m.file_type().is_symlink() || m.len() > max {
        return Err("Onboarding source must be a bounded regular file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if m.permissions().mode() & 0o077 != 0 {
            return Err("Onboarding sources must exclude group and other access".into());
        }
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| "Onboarding source unavailable")?
        .take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Onboarding source unavailable")?;
    if bytes.len() as u64 > max {
        return Err("Onboarding source exceeds its bound".into());
    }
    Ok(bytes)
}
fn digest<T: Serialize>(value: &T) -> String {
    route_contract::digest_of(value)
        .as_str()
        .trim_start_matches("sha256:")
        .into()
}
fn instance(snapshot: &Snapshot) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(snapshot.stream.as_bytes()))
}
fn git(path: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = std::process::Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    let output = command
        .arg("-C")
        .arg(path)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .map_err(|_| "Scratch Git inspection unavailable")?;
    if !output.status.success() {
        return Err("Scratch Git inspection refused".into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().into())
}
impl Config {
    pub fn load(path: &Path) -> Result<Self, String> {
        serde_json::from_slice(&read(path, 64 * 1024)?)
            .map_err(|_| "Invalid onboarding configuration".into())
    }
    fn book(&self) -> Result<Book, String> {
        self.scope.validate()?;
        let starter = self
            .starter
            .canonicalize()
            .map_err(|_| "Private starter unavailable")?;
        let repo = self
            .starter
            .join("repo")
            .canonicalize()
            .map_err(|_| "Scratch repository unavailable")?;
        if repo == starter || !repo.starts_with(&starter) {
            return Err("Scratch repository must remain inside its starter".into());
        }
        let common = PathBuf::from(git(
            &repo,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?)
        .canonicalize()
        .map_err(|_| "Scratch Git directory unavailable")?;
        if !common.starts_with(&repo) || common == repo || !git(&repo, &["remote"])?.is_empty() {
            return Err("Scratch repository must own its Git directory and have no remotes".into());
        }
        let marker: Scope = serde_json::from_slice(&read(
            &self.starter.join("onboarding-starter.json"),
            64 * 1024,
        )?)
        .map_err(|_| "Starter scope unavailable")?;
        if marker != self.scope || !self.starter.is_absolute() || !self.tasks.is_absolute() {
            return Err("Onboarding sources belong to another scratch scope".into());
        }
        let book: Book = serde_json::from_slice(&read(&self.book, 1024 * 1024)?)
            .map_err(|_| "Invalid retained onboarding book")?;
        if book.schema != SCHEMA
            || book.scope != self.scope
            || book.records.len() + book.direct.len() > 32
        {
            return Err("Onboarding book belongs to another scope or exceeds its bound".into());
        }
        Ok(book)
    }
    /// Bind a new real starter to an explicitly selected owner snapshot and existing task store.
    /// This selects observation sources only and admits no execution.
    pub fn bind_owner(
        &mut self,
        snapshot: Snapshot,
        tasks: PathBuf,
        workspace: String,
    ) -> Result<(), String> {
        let _guard = BOOK_WRITE
            .lock()
            .map_err(|_| "Onboarding retention unavailable")?;
        if self.scope.lane != Lane::Real {
            return Err("Simulated starter cannot bind a real owner".into());
        }
        snapshot.validate().map_err(|e| e.to_string())?;
        let mut book = self.book()?;
        if !book.records.is_empty()
            || !book.direct.is_empty()
            || book.terminal.is_some()
            || book.inspection.is_some()
            || book
                .snapshot
                .as_ref()
                .is_some_and(|s| !s.view.tasks.is_empty())
        {
            return Err("Retained starter evidence cannot be rebound to another owner".into());
        }
        if !tasks.is_absolute() || !tasks.is_dir() {
            return Err("Select an existing absolute owner task store".into());
        }
        self.scope.host = instance(&snapshot);
        self.scope.workspace = workspace;
        self.scope.validate()?;
        self.tasks = tasks;
        book.scope = self.scope.clone();
        book.snapshot = Some(snapshot);
        practice::replace(
            &self.starter.join("onboarding-starter.json"),
            &serde_json::to_vec(&self.scope).map_err(|_| "Owner scope encoding unavailable")?,
        )?;
        practice::replace(
            &self.book,
            &serde_json::to_vec(&book).map_err(|_| "Owner proof encoding unavailable")?,
        )?;
        practice::replace(
            &self.starter.join("config.json"),
            &serde_json::to_vec(self).map_err(|_| "Owner selection encoding unavailable")?,
        )
    }
    /// Retain a fresh snapshot from the selected owner without inferring answers or merges.
    pub fn record_snapshot(&self, snapshot: Snapshot) -> Result<(), String> {
        let _guard = BOOK_WRITE
            .lock()
            .map_err(|_| "Onboarding retention unavailable")?;
        snapshot.validate().map_err(|e| e.to_string())?;
        if instance(&snapshot) != self.scope.host {
            return Err("Observed studio belongs to another selected owner stream".into());
        }
        let mut book = self.book()?;
        if let Some(current) = &book.snapshot {
            if snapshot.sequence < current.sequence {
                return Err("Observed snapshot predates retained owner state".into());
            }
            if snapshot.sequence == current.sequence {
                return if digest(current) == digest(&snapshot) {
                    Ok(())
                } else {
                    Err("Owner snapshot conflicts at the retained sequence".into())
                };
            }
        }
        book.snapshot = Some(snapshot);
        practice::replace(
            &self.book,
            &serde_json::to_vec(&book).map_err(|_| "Owner snapshot encoding unavailable")?,
        )
    }
    /// Retain a successful explicit client operation; this never sends or retries work.
    pub fn record_owner(&self, proof: OwnerProof) -> Result<(), String> {
        let _guard = BOOK_WRITE
            .lock()
            .map_err(|_| "Onboarding retention unavailable")?;
        let mut book = self.book()?;
        let mut facts = Vec::new();
        self.operation_facts(
            &proof.snapshot,
            &proof.operation,
            &proof.outcome,
            &proof.request,
            proof.review.as_ref(),
            &mut facts,
        )?;
        evaluate(&self.scope, &facts)?;
        if book.direct.iter().any(|r| r.request == proof.request) {
            let old = book
                .direct
                .iter()
                .find(|r| r.request == proof.request)
                .unwrap();
            return if digest(old) == digest(&proof) {
                Ok(())
            } else {
                Err("Owner receipt identity conflicts with retained evidence".into())
            };
        }
        if book.direct.len() + book.records.len() >= 32 {
            return Err("Onboarding owner receipt limit reached".into());
        }
        if !matches!(
            proof.operation,
            Operation::AnswerDecision { .. } | Operation::DecideMerge { .. }
        ) {
            return Ok(());
        }
        if book
            .snapshot
            .as_ref()
            .is_none_or(|s| s.sequence < proof.snapshot.sequence)
        {
            book.snapshot = Some(proof.snapshot.clone());
        }
        book.direct.push(proof);
        practice::replace(
            &self.book,
            &serde_json::to_vec(&book).map_err(|_| "Owner evidence encoding unavailable")?,
        )
    }
    /// Retain an explicitly authenticated terminal owner's successful open acknowledgment.
    pub fn record_terminal(&self, proof: TerminalProof) -> Result<(), String> {
        let _guard = BOOK_WRITE
            .lock()
            .map_err(|_| "Onboarding retention unavailable")?;
        let mut book = self.book()?;
        let snapshot = book
            .snapshot
            .as_ref()
            .ok_or("Selected studio source unavailable")?;
        proof.open.check().map_err(|e| e.to_string())?;
        if proof.owner.studio_stream != snapshot.stream
            || instance(snapshot) != self.scope.host
            || proof.owner.host_key.len() != 64
            || !proof
                .owner
                .host_key
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            || proof.open.workspace != coder_host::mailbox::workspace_id(&self.scope.workspace)
            || proof.result.request != proof.open.request
            || proof.result.v != coder_pty::wire::RESULT
            || !proof.result.requires.is_empty()
            || proof.result.reason.is_some()
            || !matches!(
                proof.result.status,
                coder_pty::wire::Status::Accepted | coder_pty::wire::Status::Duplicate
            )
        {
            return Err("Terminal acknowledgment does not bind the selected owner".into());
        }
        let Some(coder_pty::wire::Value::Opened { terminal, size }) = &proof.result.value else {
            return Err("Terminal open acknowledgment unavailable".into());
        };
        if terminal.generation
            != coder_host::mailbox::terminal_generation(
                &proof.owner.host_key,
                proof.owner.host_generation,
            )
            || *size != proof.open.size
        {
            return Err("Terminal acknowledgment names another generation or size".into());
        }
        coder_pty::wire::Attach::new(
            &"0".repeat(64),
            terminal.clone(),
            coder_pty::wire::Mode::Observe,
            0,
            65536,
        )
        .check()
        .map_err(|e| e.to_string())?;
        book.terminal = Some(proof);
        practice::replace(
            &self.book,
            &serde_json::to_vec(&book).map_err(|_| "Terminal evidence encoding unavailable")?,
        )
    }
    /// Record the exact metadata revision that the operator explicitly inspected.
    pub fn inspect_contribution(&self, path: &Path, source: &str) -> Result<(), String> {
        let _guard = BOOK_WRITE
            .lock()
            .map_err(|_| "Onboarding retention unavailable")?;
        let selected = contribution_workbench::host::Config::load(path)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let rows = selected.read(now)?;
        let row = rows
            .iter()
            .find(|r| r.source_record == source)
            .ok_or("Selected contribution metadata unavailable")?;
        if !row.authored.available {
            return Err("Selected contribution has no retained authoring fact".into());
        }
        let mut book = self.book()?;
        book.inspection = Some(Inspection {
            config: path.into(),
            source: source.into(),
            revision: digest(row),
        });
        practice::replace(
            &self.book,
            &serde_json::to_vec(&book).map_err(|_| "Inspection retention unavailable")?,
        )
    }
    pub fn rows(&self) -> Result<Vec<Row>, String> {
        let book = self.book()?;
        let mut facts = Vec::new();
        let fact = |objective, references| Fact {
            scope: self.scope.clone(),
            objective,
            references,
        };
        if let Some(terminal) = &book.terminal {
            terminal.open.check().map_err(|e| e.to_string())?;
            if terminal.open.workspace != coder_host::mailbox::workspace_id(&self.scope.workspace)
                || terminal.result.v != coder_pty::wire::RESULT
                || !terminal.result.requires.is_empty()
                || terminal.result.request != terminal.open.request
            {
                return Err("Terminal proof answers another scratch scope".into());
            }
            if matches!(
                terminal.result.status,
                coder_pty::wire::Status::Accepted | coder_pty::wire::Status::Duplicate
            ) && terminal.result.reason.is_none()
            {
                if let Some(coder_pty::wire::Value::Opened {
                    terminal: reference,
                    size,
                }) = &terminal.result.value
                {
                    coder_pty::wire::Attach::new(
                        &"0".repeat(64),
                        reference.clone(),
                        coder_pty::wire::Mode::Observe,
                        0,
                        65536,
                    )
                    .check()
                    .map_err(|e| e.to_string())?;
                    let Some(snapshot) = &book.snapshot else {
                        return Err("Terminal owner context is unavailable".into());
                    };
                    if instance(snapshot) != self.scope.host
                        || terminal.owner.studio_stream != snapshot.stream
                        || reference.generation
                            != coder_host::mailbox::terminal_generation(
                                &terminal.owner.host_key,
                                terminal.owner.host_generation,
                            )
                    {
                        return Err("Terminal belongs to another owner generation".into());
                    }
                    if *size != terminal.open.size {
                        return Err("Terminal proof has another admitted size".into());
                    }
                    facts.push(fact(
                        Objective::Terminal,
                        vec![format!(
                            "terminal:{}:{}",
                            reference.generation, reference.terminal
                        )],
                    ));
                }
            }
        }
        let mut snapshots: Vec<&Snapshot> = book.snapshot.iter().collect();
        snapshots.extend(book.records.iter().map(|r| &r.snapshot));
        snapshots.extend(book.direct.iter().map(|r| &r.snapshot));
        for snapshot in snapshots {
            snapshot.validate().map_err(|e| e.to_string())?;
            if instance(snapshot) != self.scope.host {
                return Err("Studio snapshot belongs to another host stream".into());
            }
            for task in &snapshot.view.tasks {
                let Some(goal) = snapshot
                    .view
                    .goals
                    .iter()
                    .find(|g| g.goal == task.goal && g.workspace == self.scope.workspace)
                else {
                    continue;
                };
                let Ok(retained) = coder::task::retained_task(&self.tasks, &task.task) else {
                    continue;
                };
                let root = self
                    .starter
                    .join("repo")
                    .canonicalize()
                    .map_err(|_| "Scratch repository unavailable")?;
                let retained_root = Path::new(&retained.intent.workspace.path)
                    .canonicalize()
                    .map_err(|_| "Retained task workspace unavailable")?;
                if git(&root, &["remote"])? != "" {
                    return Err("Starter repository must have no remote".into());
                }
                if retained_root != root {
                    return Err("Retained task belongs to another repository".into());
                }
                if self.scope.lane == Lane::Real
                    && retained.intent.configuration.model.as_deref()
                        == Some("onboarding-simulation")
                {
                    return Err("Simulated task cannot supply real completion".into());
                }
                facts.push(fact(
                    Objective::ScratchGoal,
                    vec![
                        format!("goal:{}", goal.goal),
                        format!(
                            "task:{}:{}",
                            retained.task_id,
                            retained.intent_digest.trim_start_matches("sha256:")
                        ),
                    ],
                ));
                break;
            }
        }
        for proof in &book.records {
            proof.route.check(&proof.admission, &proof.binding)?;
            if proof.binding.placement.computer != self.scope.host
                || proof.binding.placement.generation != proof.snapshot.stream
                || proof
                    .admission
                    .placement
                    .workspace
                    .as_ref()
                    .map(|w| w.project.as_str())
                    != Some(self.scope.workspace.as_str())
                || proof.result.schema != route_contract::studio::RESULT_SCHEMA
                || proof.result.request != proof.route.request
                || proof.result.route != proof.route.digest()
            {
                return Err("Studio proof does not bind the selected owner source".into());
            }
            let operation = openagents_chat::studio::operation(&proof.route)?;
            let State::Completed { outcome } = &proof.result.state else {
                continue;
            };
            outcome.validate().map_err(|e| e.to_string())?;
            if !outcome.answers(&operation) {
                return Err("Studio owner outcome answers another operation".into());
            }
            self.operation_facts(
                &proof.snapshot,
                &operation,
                outcome.as_ref(),
                &proof.result.request,
                proof.review.as_ref(),
                &mut facts,
            )?;
        }
        for proof in &book.direct {
            self.operation_facts(
                &proof.snapshot,
                &proof.operation,
                &proof.outcome,
                &proof.request,
                proof.review.as_ref(),
                &mut facts,
            )?;
        }
        if let Some(inspection) = &book.inspection {
            let config = contribution_workbench::host::Config::load(&inspection.config)?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let rows = config.read(now)?;
            let Some(row) = rows.iter().find(|r| {
                r.source_record == inspection.source && digest(*r) == inspection.revision
            }) else {
                return Err("Inspected contribution source changed or is unavailable".into());
            };
            if row.authored.available {
                facts.push(fact(
                    Objective::Contribution,
                    vec![
                        format!("contribution:{}", inspection.revision),
                        format!("source:{}", digest(&inspection.source)),
                    ],
                ));
            }
        }
        evaluate(&self.scope, &facts)
    }
    fn operation_facts(
        &self,
        snapshot: &Snapshot,
        operation: &Operation,
        outcome: &Outcome,
        request: &str,
        review: Option<&TaskReview>,
        facts: &mut Vec<Fact>,
    ) -> Result<(), String> {
        snapshot.validate().map_err(|e| e.to_string())?;
        operation.validate().map_err(|e| e.to_string())?;
        outcome.validate().map_err(|e| e.to_string())?;
        if instance(snapshot) != self.scope.host || !outcome.answers(operation) {
            return Err("Owner acknowledgment does not bind the selected source".into());
        }
        let fact = |objective, references| Fact {
            scope: self.scope.clone(),
            objective,
            references,
        };
        let check_task = |task: &str| -> Result<(), String> {
            let row = snapshot
                .view
                .tasks
                .iter()
                .find(|t| t.task == task)
                .ok_or("Owner task is absent from the retained snapshot")?;
            if !snapshot
                .view
                .goals
                .iter()
                .any(|g| g.goal == row.goal && g.workspace == self.scope.workspace)
            {
                return Err("Owner task belongs to another workspace".into());
            }
            let task = coder::task::retained_task(&self.tasks, task)
                .map_err(|_| "Retained scratch task unavailable")?;
            let root = self
                .starter
                .join("repo")
                .canonicalize()
                .map_err(|_| "Scratch repository unavailable")?;
            if Path::new(&task.intent.workspace.path)
                .canonicalize()
                .map_err(|_| "Retained task workspace unavailable")?
                != root
                || git(&root, &["remote"])? != ""
            {
                return Err("Owner task does not name an isolated scratch repository".into());
            }
            if self.scope.lane == Lane::Real
                && task.intent.configuration.model.as_deref() == Some("onboarding-simulation")
            {
                return Err("Simulated task cannot supply real completion".into());
            }
            Ok(())
        };
        match operation {
            Operation::AnswerDecision {
                decision, based_on, ..
            } => {
                let Some(row) = snapshot
                    .view
                    .decisions
                    .iter()
                    .find(|d| &d.decision == decision && d.based_on == *based_on)
                else {
                    return Err("Answer does not name the retained displayed decision".into());
                };
                let Outcome::Dispatched { receipt } = outcome else {
                    return Ok(());
                };
                if receipt.reference != *decision {
                    return Err("Decision receipt names another owner decision".into());
                }
                if let Some(task) = &row.task {
                    check_task(task)?;
                }
                if snapshot
                    .view
                    .goals
                    .iter()
                    .any(|g| g.goal == row.goal && g.workspace == self.scope.workspace)
                {
                    facts.push(fact(
                        Objective::AnsweredDecision,
                        vec![
                            format!("decision:{decision}:{based_on}"),
                            format!("request:{}", request),
                        ],
                    ));
                }
            }
            Operation::DecideMerge { decision } => {
                let Some(review) = review else { return Ok(()) };
                check_task(&decision.task)?;
                review.validate().map_err(|e| e.to_string())?;
                let Outcome::Merged { merged } = outcome else {
                    return Ok(());
                };
                if matches!(
                    review.completeness,
                    coder_access::review::Completeness::Unknown { .. }
                ) {
                    return Ok(());
                }
                if decision.verdict != coder_access::studio::Verdict::Merge
                    || review.task != decision.task
                    || review.base != decision.base
                    || review.head_commit != decision.head_commit
                    || review.head != decision.head
                {
                    return Err("Merge does not bind the retained reviewed revisions".into());
                }
                let Some(publication) = &merged.publication else {
                    return Ok(());
                };
                if publication.state != coder_access::review::PublishState::Published
                    || publication.url.is_some()
                {
                    return Ok(());
                }
                let Some(commit) = publication.commit.as_deref() else {
                    return Ok(());
                };
                let repo = self.starter.join("repo");
                if git(&repo, &["rev-parse", &format!("{commit}^{{tree}}")])? != review.head
                    || git(&repo, &["remote"])? != ""
                {
                    return Err(
                        "Reviewed merge must be a local scratch commit without remotes".into(),
                    );
                }
                facts.push(fact(
                    Objective::ReviewedMerge,
                    vec![
                        format!(
                            "review:{}",
                            digest(&(
                                &review.task,
                                &review.base,
                                &review.head_commit,
                                &review.head
                            ))
                        ),
                        format!("commit:{commit}"),
                        format!("request:{}", request),
                    ],
                ));
            }
            _ => {}
        }
        Ok(())
    }
}
struct Adapter {
    config: Config,
    host: Host,
}
impl PaneAdapter for Adapter {
    fn kind(&self) -> PaneKind {
        PaneKind::Background
    }
    fn describe(&self, subject: &Subject) -> Description {
        if subject.host() != &self.host || subject.id() != "onboarding" {
            return Description::only(PaneState::Missing, "Onboarding");
        }
        let Ok(rows) = self.config.rows() else {
            return Description::only(
                PaneState::Unavailable,
                "Onboarding evidence unavailable; check the explicit scratch sources",
            );
        };
        let revision = Revision::Sha256(digest(&rows));
        if subject.revision().is_some_and(|r| r != &revision) {
            return Description::only(
                PaneState::Stale {
                    current: Some(revision),
                },
                "Onboarding owner facts changed",
            );
        }
        let lane = if self.config.scope.lane == Lane::Simulated {
            "SIMULATED PRACTICE — $0; no real completion or XP"
        } else {
            "REAL SCRATCH — no execution or spending authority from this tracker"
        };
        let mut detail = format!(
            "{lane}\nCost: {}\nAuthority: {}\n",
            rows[0].expected_cost, rows[0].authority
        );
        for row in &rows {
            detail.push_str(&format!(
                "{} {}\n",
                if row.complete {
                    "COMPLETE — already true; skip"
                } else {
                    "NOT YET"
                },
                row.title
            ));
        }
        detail.push_str("Opening or refreshing dispatches nothing.\n");
        let mut truncated = false;
        for row in &rows {
            let more = format!(
                "{}\nPrerequisite: {}\nEvidence: {}\n",
                row.guide,
                row.prerequisites.join(", "),
                row.references.join(", ")
            );
            if detail.len() + more.len() > 1980 {
                truncated = true;
                break;
            }
            detail.push_str(&more);
        }
        if truncated {
            detail.push_str("Additional guide/evidence retained in the private progress book.\n");
        }
        Description {
            state: PaneState::Ready,
            title: "First workbench steps".into(),
            detail,
            actions: vec![],
        }
    }
}
pub fn mount(application: &mut terminal_core::Application, config: Config) -> Result<(), String> {
    let rows = config.rows()?;
    let host = Host::Local {
        instance: digest(&config),
    };
    preflight(&application.products, &host)?;
    application.products.panes = std::mem::take(&mut application.products.panes).adapter_for_host(
        host.clone(),
        Box::new(Adapter {
            config,
            host: host.clone(),
        }),
    );
    application.products.open(
        PaneKind::Background,
        &Subject::Record {
            host,
            id: "onboarding".into(),
            revision: Some(Revision::Sha256(digest(&rows))),
        },
    )?;
    application.paper.on = true;
    Ok(())
}
fn preflight(products: &terminal_core::resources::Products, host: &Host) -> Result<(), String> {
    if products.open.len() >= terminal_core::resources::PRODUCTS_MAX
        && !products.open.iter().any(|p| {
            p.pane == PaneKind::Background
                && p.subject.host() == host
                && p.subject.id() == "onboarding"
        })
    {
        return Err("Onboarding requires an available product pane".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;

/// Retains the exact displayed source across explicit confirmation and typed owner acknowledgment.
pub struct Capture {
    config: Config,
    pending: std::sync::Mutex<std::collections::BTreeMap<String, (Snapshot, Option<TaskReview>)>>,
}
impl Capture {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            pending: Default::default(),
        }
    }
    pub fn record_snapshot(&self, snapshot: Snapshot) -> Result<(), String> {
        self.config.record_snapshot(snapshot)
    }
    pub fn prepare(&self, request: &str, snapshot: Snapshot, review: Option<TaskReview>) {
        if let Ok(mut pending) = self.pending.lock() {
            if pending.len() < 32 {
                pending.insert(request.into(), (snapshot, review));
            }
        }
    }
    pub fn complete(
        &self,
        request: &str,
        operation: Operation,
        outcome: Outcome,
    ) -> Result<(), String> {
        let (snapshot, review) = self
            .pending
            .lock()
            .map_err(|_| "Onboarding capture unavailable")?
            .remove(request)
            .ok_or("Displayed onboarding source unavailable")?;
        self.config.record_owner(OwnerProof {
            request: request.into(),
            operation,
            outcome,
            snapshot,
            review,
        })
    }
}
