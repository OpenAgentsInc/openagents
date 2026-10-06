//! Exact reviewed plugin versions and their retained workbench actions.
//!
//! The interview and draft remain [`crate::plugin_flow`]'s. This owner
//! projects that work and delegates approved actions to the existing plugin
//! commands. Reading, reopening, and reconnecting execute nothing.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use route_contract::digest::Digest;
use route_contract::snapshot::CapabilityPin;
use serde::{Deserialize, Serialize};
use workbench::pane::{Description, PaneAdapter, PaneKind, PaneState, Subject};

use crate::client::{Coder, Ran};
use crate::plugin_flow::{Flow, STEP_TIMEOUT, TEST_TIMEOUT, Test};

const VERSION: &str = "openagents.plugin-workbench.v1";
/// Existing package and command engines; test fixtures can keep execution local.
pub trait Engine: Send + Sync {
    fn tests(&self, dir: &Path) -> Result<Vec<Test>, String>;
    fn run(&self, argv: &[String], timeout: std::time::Duration) -> Result<Ran, String>;
}
impl<C: Coder> Engine for C {
    fn tests(&self, dir: &Path) -> Result<Vec<Test>, String> {
        self.plugin_tests(dir)
    }
    fn run(&self, argv: &[String], timeout: std::time::Duration) -> Result<Ran, String> {
        let mut args = vec!["--json".into()];
        args.extend_from_slice(argv);
        self.plugin_command(&args, timeout)
    }
}
const MAX_FILES: usize = 512;
const MAX_BYTES: u64 = 8 * 1024 * 1024;

/// Provenance from the routed interview and its draft task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub flow: String,
    pub thread: String,
    pub task: String,
}

/// Reviewed declarations; a fee is a request, not a settled payment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Declarations {
    pub author: String,
    pub fee_msat: Option<u64>,
    pub payout: Option<String>,
}

/// Separate owner choices, each bound to one reviewed release.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Compare,
    Publish,
    Install,
    Enable,
    /// A task and thread independent of the authoring task and thread.
    Reuse {
        /// Owner-declared provenance, distinct from the executed route ID.
        source_task: String,
        thread: String,
        request: String,
        workspace: PathBuf,
    },
}

/// An exact request. A repeated ID with different bytes is refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: String,
    pub source: Source,
    pub release: CapabilityPin,
    pub tree: Digest,
    pub action: Action,
}

/// An unknown action is never dispatched automatically a second time.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    Unknown,
    Finished { ok: bool, output: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    pub request: Request,
    pub outcome: Outcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison: Option<Comparison>,
    /// The existing use engine's actual route and artifact result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reuse: Option<serde_json::Value>,
}

/// A retained baseline/subject report, with explicit unknown costs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Comparison {
    pub report: Digest,
    pub path: String,
    pub verdict: String,
    pub measurements: serde_json::Value,
}

/// The retained record; test failures remain beside successful actions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub v: String,
    pub source: Source,
    pub interview: Flow,
    #[serde(default)]
    pub turns: Vec<crate::basic_coder::Turn>,
    pub tests: Vec<Test>,
    pub release: CapabilityPin,
    pub tree: Digest,
    pub declarations: Declarations,
    pub revision: u64,
    pub attempts: BTreeMap<String, Attempt>,
}

/// One private retained owner record over the existing command engines.
pub struct Owner<C> {
    root: PathBuf,
    coder: C,
    serial: Mutex<()>,
}

impl<C: Engine> Owner<C> {
    /// Open an existing record. This reads no commands and starts no work.
    pub fn open(root: PathBuf, coder: C) -> Self {
        Self {
            root,
            coder,
            serial: Mutex::new(()),
        }
    }

    /// Freeze the reviewed draft after the existing package resolver accepts
    /// it. The package must already name the selected signing author.
    pub fn freeze(
        &self,
        source: Source,
        interview: Flow,
        dir: &Path,
        declarations: Declarations,
    ) -> Result<Record, String> {
        self.freeze_with_turns(source, interview, dir, declarations, Vec::new())
    }

    fn freeze_with_turns(
        &self,
        source: Source,
        interview: Flow,
        dir: &Path,
        declarations: Declarations,
        turns: Vec<crate::basic_coder::Turn>,
    ) -> Result<Record, String> {
        source.check()?;
        declarations.check()?;
        if !matches!(
            interview.step(),
            Some(crate::plugin_flow::Step::Tests | crate::plugin_flow::Step::Publish)
        ) {
            return Err(
                "A review needs the existing flow's drafted tests or publication step.".into(),
            );
        }
        let tests = self.coder.tests(dir)?;
        let files = files(dir)?;
        let bytes = files
            .iter()
            .find(|(name, _)| name == "package.json")
            .ok_or("The draft has no package record.")?
            .1
            .clone();
        let package: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let slug = package["slug"]
            .as_str()
            .filter(|s| crate::plugin_flow::slug_ok(s))
            .ok_or("The draft has no valid slug.")?;
        let author = package["publisher"]
            .as_str()
            .ok_or("The draft has no publisher.")?;
        if author != declarations.author {
            return Err("The reviewed publisher and signing author differ.".into());
        }
        if interview.slug.as_deref() != Some(slug) {
            return Err("The interview and draft name different plugins.".into());
        }
        let version = package["version"]
            .as_str()
            .filter(|s| text(s, 64))
            .ok_or("The draft has no release version.")?;
        let record = Record {
            v: VERSION.into(),
            source,
            interview,
            turns,
            tests,
            release: CapabilityPin {
                id: format!("{author}:{slug}"),
                version: version.into(),
                digest: Digest::of_bytes(&bytes),
            },
            tree: tree(&files),
            declarations,
            revision: 1,
            attempts: BTreeMap::new(),
        };
        // A newly created directory is never substituted for a kept review.
        private_dir(&self.root)?;
        let _lock = self.lock()?;
        if self.root.join("record.json").exists() {
            return Err("This reviewed flow already exists.".into());
        }
        let draft = self.root.join("draft");
        fs::create_dir(&draft).map_err(|e| e.to_string())?;
        for (name, bytes) in files {
            let path = draft.join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            write_private(&path, &bytes)?;
        }
        self.save(&record)?;
        Ok(record)
    }

    /// Freeze the existing routed thread's draft, preserving its task binding
    /// and interview. The caller provides the drafted files from that task.
    pub fn freeze_routed(
        &self,
        flow: String,
        snapshot: &crate::service::Snapshot,
        dir: &Path,
        declarations: Declarations,
    ) -> Result<Record, String> {
        let thread = snapshot
            .chat
            .clone()
            .ok_or("The routed thread is missing.")?;
        let bound = snapshot
            .coder
            .as_ref()
            .ok_or("The routed draft has no Coder task.")?;
        if bound.host != crate::thread::LOCAL_HOST || snapshot.busy || snapshot.failure.is_some() {
            return Err("Review needs a completed draft on this computer.".into());
        }
        let interview = snapshot
            .turns
            .iter()
            .rev()
            .find(|turn| turn.role == crate::basic_coder::Role::Assistant)
            .and_then(|turn| turn.meta.as_ref())
            .and_then(|meta| meta.plugin.clone())
            .ok_or("The thread has no routed plugin interview.")?;
        let interview = if interview.step() == Some(crate::plugin_flow::Step::Draft) {
            let names = files(dir)?
                .into_iter()
                .map(|(name, _)| {
                    format!(
                        "plugins/{}/{}",
                        dir.file_name().unwrap_or_default().to_string_lossy(),
                        name
                    )
                })
                .collect::<Vec<_>>();
            let (slug, _) = crate::plugin_flow::drafted(names.iter().map(String::as_str))
                .ok_or("The original draft has no package.")?;
            interview.advanced(&crate::plugin_flow::Outcome::Drafted {
                slug,
                tests: self.coder.tests(dir)?,
            })
        } else {
            interview
        };
        self.freeze_with_turns(
            Source {
                flow,
                thread,
                task: bound.task.clone(),
            },
            interview,
            dir,
            declarations,
            snapshot.turns.clone(),
        )
    }

    pub fn read(&self) -> Result<Record, String> {
        let path = self.root.join("record.json");
        let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if !meta.is_file() || meta.len() > MAX_BYTES {
            return Err("The reviewed flow record is unsafe or too large.".into());
        }
        let record: Record = serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        if record.v != VERSION {
            return Err("This flow record version is unsupported.".into());
        }
        record.source.check()?;
        record.declarations.check()?;
        Ok(record)
    }

    /// Read the reviewed files without dispatching an action. File bytes and
    /// names remain bound to the review's tree digest.
    pub fn reviewed_files(&self) -> Result<Vec<(String, Vec<u8>)>, String> {
        let record = self.read()?;
        let files = files(&self.root.join("draft"))?;
        if tree(&files) != record.tree {
            return Err("The reviewed draft changed. Review a new version.".into());
        }
        Ok(files)
    }

    /// Execute an explicit owner choice. Persist uncertainty before handing
    /// the command to its existing engine; exact retries return that attempt.
    pub fn apply(&self, request: Request) -> Result<Outcome, String> {
        if !token(&request.id) {
            return Err("The action needs a bounded request ID.".into());
        }
        let _serial = self
            .serial
            .lock()
            .map_err(|_| "The flow owner is unavailable.")?;
        let _lock = self.lock()?;
        let mut record = self.read()?;
        if let Some(kept) = record.attempts.get(&request.id) {
            return if kept.request == request {
                Ok(kept.outcome.clone())
            } else {
                Err("This request ID already names another action.".into())
            };
        }
        if request.source != record.source
            || request.release != record.release
            || request.tree != record.tree
        {
            return Err("The action does not name this exact reviewed flow and release.".into());
        }
        if record
            .attempts
            .values()
            .any(|a| a.outcome == Outcome::Unknown)
        {
            return Err(
                "An earlier action is unknown. Inspect its original result before another action."
                    .into(),
            );
        }
        if record.attempts.len() >= 64 {
            return Err("This flow has reached its action limit.".into());
        }
        let draft_files = files(&self.root.join("draft"))?;
        if tree(&draft_files) != record.tree {
            return Err("The reviewed draft changed. Review a new version.".into());
        }
        let package_bytes = &draft_files
            .iter()
            .find(|(name, _)| name == "package.json")
            .ok_or("The kept package record is missing.")?
            .1;
        let package: serde_json::Value =
            serde_json::from_slice(package_bytes).map_err(|e| e.to_string())?;
        if Digest::of_bytes(package_bytes) != record.release.digest
            || package["version"].as_str() != Some(record.release.version.as_str())
            || package["publisher"].as_str() != Some(record.declarations.author.as_str())
            || record.release.id
                != format!(
                    "{}:{}",
                    record.declarations.author,
                    package["slug"].as_str().unwrap_or_default()
                )
        {
            return Err("The retained release identity differs from its reviewed package.".into());
        }
        let success = |action: &Action| {
            record.attempts.values().any(|a| {
                &a.request.action == action
                    && matches!(a.outcome, Outcome::Finished { ok: true, .. })
            })
        };
        match &request.action {
            Action::Publish | Action::Install
                if !record.attempts.values().any(|a| {
                    a.request.action == Action::Compare
                        && a.comparison.is_some()
                        && matches!(a.outcome, Outcome::Finished { .. })
                }) =>
            {
                return Err("Review the baseline and subject comparison first.".into());
            }
            Action::Enable if !success(&Action::Install) => {
                return Err("Install this exact release before enabling it.".into());
            }
            Action::Reuse {
                source_task,
                thread,
                request,
                workspace,
            } => {
                if !success(&Action::Enable)
                    || !text(source_task, 128)
                    || !text(thread, 128)
                    || !request_text(request)
                    || source_task == &record.source.task
                    || thread == &record.source.thread
                    || !workspace.is_absolute()
                {
                    return Err(
                        "Reuse needs an enabled exact release and an independent task and thread."
                            .into(),
                    );
                }
            }
            _ => {}
        }
        // Repeating the same successful choice under a new request cannot
        // publish or install again. Comparisons and independent reuse differ.
        if matches!(
            request.action,
            Action::Publish | Action::Install | Action::Enable
        ) && success(&request.action)
        {
            return Err("This release already completed that choice.".into());
        }
        let argv = self.argv(&record, &request);
        record.attempts.insert(
            request.id.clone(),
            Attempt {
                request: request.clone(),
                outcome: Outcome::Unknown,
                comparison: None,
                reuse: None,
            },
        );
        record.revision += 1;
        self.save(&record)?;
        let timeout = if request.action == Action::Compare {
            TEST_TIMEOUT
        } else {
            STEP_TIMEOUT
        };
        let mut outcome = match self.coder.run(&argv, timeout) {
            Ok(Ran { ok, output }) => {
                if matches!(request.action, Action::Reuse { .. }) {
                    record.attempts.get_mut(&request.id).unwrap().reuse =
                        serde_json::from_str(&output).ok();
                }
                Outcome::Finished {
                    ok,
                    output: bounded(&output, 16 * 1024),
                }
            }
            Err(_) => return Ok(Outcome::Unknown),
        };
        if let Action::Reuse { thread, .. } = &request.action {
            let result = record.attempts[&request.id].reuse.as_ref();
            let valid = result.is_some_and(|value| {
                value["v"] == "openagents.plugin-use.v1"
                    && value["thread"] == *thread
                    && matches!(value["dispatched"].as_str(), Some("ran" | "followed"))
                    && value["outputs"]
                        .as_array()
                        .is_some_and(|outputs| !outputs.is_empty())
                    && serde_json::from_value::<CapabilityPin>(value["pin"].clone())
                        .ok()
                        .as_ref()
                        == Some(&record.release)
                    && value["request"]
                        .as_str()
                        .is_some_and(|id| id.starts_with("use-"))
            });
            if !valid {
                let original = match &outcome {
                    Outcome::Finished { output, .. } => output.as_str(),
                    Outcome::Unknown => "",
                };
                outcome = Outcome::Finished {
                    ok: false,
                    output: bounded(
                        &format!(
                            "Reuse returned no attributable result for this exact release and thread.\n{original}"
                        ),
                        16 * 1024,
                    ),
                };
            }
        }
        if request.action == Action::Compare {
            let comparison = comparison(
                &self.root.join("comparisons").join(&request.id),
                &record.release,
            );
            match comparison {
                Ok(comparison) => {
                    record.attempts.get_mut(&request.id).unwrap().comparison = Some(comparison)
                }
                Err(why) => {
                    let original = match &outcome {
                        Outcome::Finished { output, .. } => output.as_str(),
                        Outcome::Unknown => "",
                    };
                    record.attempts.get_mut(&request.id).unwrap().outcome = Outcome::Finished {
                        ok: false,
                        output: bounded(
                            &format!("The comparison report was not retained: {why}\n{original}"),
                            16 * 1024,
                        ),
                    };
                    record.revision += 1;
                    self.save(&record)?;
                    return Ok(record.attempts[&request.id].outcome.clone());
                }
            }
        }
        record.attempts.get_mut(&request.id).unwrap().outcome = outcome.clone();
        record.revision += 1;
        self.save(&record)?;
        Ok(outcome)
    }

    fn argv(&self, record: &Record, request: &Request) -> Vec<String> {
        let dir = self.root.join("draft").display().to_string();
        let mut args = match &request.action {
            Action::Compare => vec![
                "plugin".into(),
                "test".into(),
                "run".into(),
                dir,
                "--trust".into(),
                "--output-dir".into(),
                self.root
                    .join("comparisons")
                    .join(&request.id)
                    .display()
                    .to_string(),
            ],
            Action::Publish => vec!["plugin".into(), "publish".into(), dir],
            Action::Install => vec!["plugin".into(), "install".into(), dir],
            Action::Enable => vec![
                "plugin".into(),
                "enable".into(),
                record.release.id.clone(),
                "--version".into(),
                record.release.version.clone(),
                "--digest".into(),
                record.release.digest.to_string(),
            ],
            Action::Reuse {
                thread,
                request,
                workspace,
                ..
            } => vec![
                "plugin".into(),
                "use".into(),
                record.release.id.clone(),
                "--version".into(),
                record.release.version.clone(),
                "--digest".into(),
                record.release.digest.to_string(),
                "--request".into(),
                request.clone(),
                "--thread".into(),
                thread.clone(),
                "--in".into(),
                workspace.display().to_string(),
            ],
        };
        if request.action == Action::Publish {
            if let Some(fee) = record.declarations.fee_msat {
                args.extend(["--fee-msat".into(), fee.to_string()]);
            }
            if let Some(payout) = &record.declarations.payout {
                args.extend(["--payout".into(), payout.clone()]);
            }
        }
        args
    }

    fn lock(&self) -> Result<File, String> {
        if !cfg!(unix) {
            return Err("Private workflow storage is unsupported on this platform.".into());
        }
        let path = self.root.join("owner.lock");
        if fs::symlink_metadata(&path).is_ok_and(|m| !m.is_file()) {
            return Err("The flow lock is unsafe.".into());
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        file.try_lock()
            .map_err(|_| "Another owner is handling this flow.".to_owned())?;
        Ok(file)
    }

    fn save(&self, record: &Record) -> Result<(), String> {
        let bytes = serde_json::to_vec(record).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BYTES as usize {
            return Err("The retained flow exceeds its byte limit.".into());
        }
        let next = self.root.join("record.next");
        write_private(&next, &bytes)?;
        fs::rename(next, self.root.join("record.json")).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        File::open(&self.root)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl<C: Engine> PaneAdapter for Owner<C> {
    fn kind(&self) -> PaneKind {
        PaneKind::Evaluation
    }
    fn describe(&self, subject: &Subject) -> Description {
        let Ok(record) = self.read() else {
            return Description::only(PaneState::Unavailable, "Capability workflow unavailable");
        };
        if !matches!(subject, Subject::Resource { resource }
            if resource.kind == workbench::Kind::Evidence
                && resource.host == workbench::Host::Local { instance: local_instance(&self.root) }
                && resource.id == record.source.flow)
        {
            return Description::only(PaneState::Missing, "Capability workflow missing");
        }
        let current = workbench::Revision::Sha256(
            Digest::of_bytes(&serde_json::to_vec(&record).unwrap_or_default()).as_str()[7..]
                .to_owned(),
        );
        if subject.revision().is_some_and(|asked| asked != &current) {
            return Description::only(
                PaneState::Stale {
                    current: Some(current),
                },
                "Capability workflow changed",
            );
        }
        let mut detail = format!(
            "Flow {} · thread {} · task {}\n{} {}\n{}\nAuthor: {}\nFee: {:?} msat; payout: {:?} (unsettled)\n{} drafted tests\n",
            record.source.flow,
            record.source.thread,
            record.source.task,
            record.release.id,
            record.release.version,
            record.release.digest,
            record.declarations.author,
            record.declarations.fee_msat,
            record.declarations.payout,
            record.tests.len()
        );
        detail.push_str(
            "Interview/draft/publication/install/enable costs: unreported.
",
        );
        if let Ok(files) = self.reviewed_files() {
            let names = files
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>();
            detail.push_str(&format!(
                "Reviewed files: {}\n",
                bounded(&names.join(", "), 320)
            ));
        } else {
            return Description::only(PaneState::Unavailable, "Reviewed draft unavailable");
        }
        for attempt in record.attempts.values() {
            let status = match attempt.outcome {
                Outcome::Unknown => "unknown",
                Outcome::Finished { ok: true, .. } => "completed",
                Outcome::Finished { ok: false, .. } => "failed",
            };
            detail.push_str(&format!("{}: {status}\n", attempt.request.id));
        }
        // Keep compact costs and verdicts ahead of verbose command output.
        for attempt in record.attempts.values() {
            if let Some(comparison) = &attempt.comparison {
                detail.push_str(&format!(
                    "Comparison {}: {}\n{}\nReport: {}\n",
                    attempt.request.id,
                    comparison.verdict,
                    comparison.measurements,
                    comparison.report
                ));
            }
        }
        for turn in &record.turns {
            detail.push_str(&format!("{:?}: {}\n", turn.role, bounded(&turn.text, 240)));
        }
        for attempt in record.attempts.values() {
            detail.push_str(&format!(
                "{:?}: {:?}\n",
                attempt.request.action, attempt.outcome
            ));
        }
        Description {
            state: PaneState::Ready,
            title: "Capability workflow".into(),
            detail: bounded(&detail, workbench::SUMMARY_MAX),
            actions: vec!["inspect".into()],
        }
    }
}

impl Source {
    fn check(&self) -> Result<(), String> {
        if [&self.flow, &self.thread, &self.task]
            .into_iter()
            .all(|s| text(s, 128))
        {
            Ok(())
        } else {
            Err("The flow needs its original flow, thread, and task IDs.".into())
        }
    }
}
impl Declarations {
    fn check(&self) -> Result<(), String> {
        if self.author.len() != 64
            || !self
                .author
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || self.payout.as_ref().is_some_and(|s| !text(s, 256))
            || self.fee_msat.is_some_and(|n| n > 0) && self.payout.is_none()
        {
            return Err("The reviewed author, fee, or payout declaration is invalid.".into());
        }
        Ok(())
    }
}
fn token(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn text(s: &str, max: usize) -> bool {
    !s.is_empty() && s.len() <= max && !s.chars().any(char::is_control)
}
fn request_text(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 4096
        && s.chars()
            .all(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
}
fn bounded(s: &str, max: usize) -> String {
    let mut result = String::new();
    for c in s.chars().filter(|c| !c.is_control() || *c == '\n') {
        if result.len() + c.len_utf8() > max {
            break;
        }
        result.push(c);
    }
    result
}
fn private_dir(path: &Path) -> Result<(), String> {
    if !cfg!(unix) {
        return Err("Private workflow storage is unsupported on this platform.".into());
    }
    fs::create_dir_all(path).map_err(|e| e.to_string())?;
    if !fs::symlink_metadata(path)
        .map_err(|e| e.to_string())?
        .is_dir()
    {
        return Err("The flow directory is unsafe.".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if !cfg!(unix) {
        return Err("Private workflow storage is unsupported on this platform.".into());
    }
    use std::io::Write;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string())
}
fn files(root: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    fn visit(
        root: &Path,
        dir: &Path,
        out: &mut Vec<(String, Vec<u8>)>,
        total: &mut u64,
        visited: &mut usize,
        depth: usize,
    ) -> Result<(), String> {
        *visited += 1;
        if *visited > MAX_FILES || depth > 16 {
            return Err("The draft exceeds its directory or depth limit.".into());
        }
        for item in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let path = item.map_err(|e| e.to_string())?.path();
            let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if meta.is_dir() {
                visit(root, &path, out, total, visited, depth + 1)?;
            } else if meta.is_file() {
                *total = total.saturating_add(meta.len());
                if *total > MAX_BYTES || out.len() >= MAX_FILES {
                    return Err("The draft exceeds its file or byte limit.".into());
                }
                let name = path
                    .strip_prefix(root)
                    .map_err(|e| e.to_string())?
                    .to_str()
                    .ok_or("The draft path is not UTF-8.")?
                    .to_owned();
                out.push((name, fs::read(path).map_err(|e| e.to_string())?));
            } else {
                return Err("The draft contains a link or a special file.".into());
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    visit(root, root, &mut out, &mut 0, &mut 0, 0)?;
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}
fn tree(files: &[(String, Vec<u8>)]) -> Digest {
    let mut bytes = Vec::new();
    for (name, content) in files {
        bytes.extend_from_slice(&(name.len() as u64).to_le_bytes());
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(&(content.len() as u64).to_le_bytes());
        bytes.extend_from_slice(content);
    }
    Digest::of_bytes(&bytes)
}

#[cfg(test)]
mod tests;

/// Stable local mounting identity for a retained owner path.
#[must_use]
pub fn local_instance(root: &Path) -> String {
    Digest::of_bytes(root.to_string_lossy().as_bytes()).as_str()[7..].to_owned()
}

fn comparison(root: &Path, pin: &CapabilityPin) -> Result<Comparison, String> {
    let mut reports = Vec::new();
    // The existing local test runner writes one report in its run directory.
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if meta.is_dir() {
            reports.push(path.join("report.json"));
        } else if path.file_name().is_some_and(|n| n == "report.json") {
            reports.push(path);
        }
        if reports.len() > 1 {
            return Err("The comparison wrote several reports.".into());
        }
    }
    let path = reports.pop().ok_or("The comparison wrote no report.")?;
    let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > 1024 * 1024 {
        return Err("The report is unsafe or too large.".into());
    }
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let parsed = nostr::eval_ext::parse_report(&bytes).map_err(|e| format!("{e:?}"))?;
    if parsed.baseline.is_none()
        || parsed.subject.definition.artifact.digest != pin.digest.as_str()
        || !parsed
            .subject
            .definition
            .id
            .starts_with(&format!("{}/", pin.id))
    {
        return Err("The report does not compare this exact release with a baseline.".into());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let measurements = value["measurements"]
        .as_array()
        .ok_or("The report has no measurements.")?
        .iter()
        .filter(|m| {
            ["cases_passed", "mean_score", "cost_usd", "seconds"]
                .iter()
                .any(|name| m["metric"] == *name)
        })
        .map(|m| serde_json::json!({"arm": m["arm"], "metric": m["metric"], "value": m["value"], "unknown_count": m["unknown_count"], "denominator": m["denominator"]}))
        .collect::<Vec<_>>();
    for arm in ["baseline", "subject"] {
        if !measurements
            .iter()
            .any(|m| m["arm"] == arm && m["metric"] == "cost_usd")
        {
            return Err("The report does not declare both arms' costs or unknown costs.".into());
        }
    }
    Ok(Comparison {
        report: Digest::of_bytes(&bytes),
        path: path.display().to_string(),
        verdict: value["verdict"].as_str().unwrap_or("unknown").to_owned(),
        measurements: serde_json::Value::Array(measurements),
    })
}

/// Read-only reviewed files. Each reference binds the private owner and tree.
pub struct DraftPane {
    root: PathBuf,
}
impl DraftPane {
    pub fn open(root: PathBuf) -> Self {
        Self { root }
    }
    pub fn subjects(&self) -> Result<Vec<Subject>, String> {
        let owner = Owner::open(self.root.clone(), crate::client::NoCoder);
        let record = owner.read()?;
        let mut files = owner.reviewed_files()?;
        files.sort_by_key(|(name, _)| {
            (
                if name == "package.json" || name == "README.md" || name.starts_with("skills/") {
                    0
                } else {
                    1
                },
                name.clone(),
            )
        });
        Ok(files
            .into_iter()
            .take(16)
            .map(|(name, _)| Subject::Resource {
                resource: workbench::ResourceRef {
                    revision: Some(workbench::Revision::Sha256(
                        record.tree.as_str()[7..].into(),
                    )),
                    ..workbench::ResourceRef::new(
                        workbench::Kind::Artifact,
                        workbench::Host::Local {
                            instance: local_instance(&self.root),
                        },
                        file_id(&record, &name),
                    )
                },
            })
            .collect())
    }
}
fn file_id(record: &Record, name: &str) -> String {
    Digest::of_bytes(format!("{}:{name}", record.source.flow).as_bytes()).as_str()[7..].into()
}
impl PaneAdapter for DraftPane {
    fn kind(&self) -> PaneKind {
        PaneKind::Artifact
    }
    fn describe(&self, subject: &Subject) -> Description {
        let Subject::Resource { resource } = subject else {
            return Description::only(PaneState::Missing, "Reviewed file missing");
        };
        if resource.host
            != (workbench::Host::Local {
                instance: local_instance(&self.root),
            })
            || resource.kind != workbench::Kind::Artifact
        {
            return Description::only(PaneState::Missing, "Reviewed file missing");
        }
        let owner = Owner::open(self.root.clone(), crate::client::NoCoder);
        let Ok(record) = owner.read() else {
            return Description::only(PaneState::Unavailable, "Reviewed file unavailable");
        };
        let revision = workbench::Revision::Sha256(record.tree.as_str()[7..].into());
        if resource.revision.as_ref() != Some(&revision) {
            return Description::only(
                PaneState::Stale {
                    current: Some(revision),
                },
                "Reviewed file changed",
            );
        }
        let Ok(files) = owner.reviewed_files() else {
            return Description::only(PaneState::Unavailable, "Reviewed draft changed");
        };
        let Some((name, bytes)) = files
            .into_iter()
            .find(|(name, _)| file_id(&record, name) == resource.id)
        else {
            return Description::only(PaneState::Missing, "Reviewed file missing");
        };
        let content = String::from_utf8(bytes).unwrap_or_else(|_| {
            "Binary artifact; inspect exact bytes through plugin workbench show.".into()
        });
        Description {
            state: PaneState::Ready,
            title: bounded(&name, 128),
            detail: bounded(&content, workbench::SUMMARY_MAX),
            actions: vec!["inspect".into()],
        }
    }
}
