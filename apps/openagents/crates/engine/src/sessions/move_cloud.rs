//! Move a chat to a cloud computer at a turn boundary (#11932).
//!
//! Each chat has one durable [`MoveRecord`]. Every step writes the identity of
//! what it is about to do (a git ref, a create request, a file write intent, a
//! command request) into the record before it acts, so running [`Mover::run`]
//! again after a crash or a lost reply reuses the same identities. A retry
//! never creates a second computer or a second copy of the chat.
//!
//! The steps, in order:
//!
//! 1. Capture: commit the working tree, including untracked files but not
//!    ignored ones, to `refs/openagents/move/<chat>` without touching the
//!    index or the working tree. Push that ref to the project's remote, or
//!    else write a git bundle. Copy the harness's own session file (the Claude
//!    Code JSONL or the Codex rollout).
//! 2. Computer: attach to the chosen Computers API computer, or create it from
//!    a saved create request, and wait until it is awake.
//! 3. Probe: ask the computer whether it can hold the project at the same
//!    absolute path, and decide where the project goes.
//! 4. Upload: the bundle, the session file (same path only), or the bounded
//!    seed prompt (elsewhere only), each in pieces of at most 4 MiB.
//! 5. Restore: fetch the ref into the project and reproduce the working tree,
//!    index, and branch; put the session file where the harness finds it.
//! 6. Resume: start the harness on the computer. A same-path restore uses the
//!    harness's native resume; otherwise the chat continues from a bounded
//!    summary plus recent turns (`chat_home::Seed`, as in #11894). The login
//!    goes by credential reference only (#11813).
//!
//! A chat whose harness login isn't available for cloud computers stays on
//! this computer, and the record says why in plain words.

pub mod plan;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chat_home::Seed;
use openagents_proto::HarnessId;
use openagents_sdk::CancellationToken;
use openagents_sdk::computers::{
    CommandOptions, CommandOutcome, CommandRef, CommandRequest, ComputerRef, Computers,
    CreateRequest, CredentialRequest, Error as ComputersError, WaitFor, WriteIntent,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The largest piece the Computers files API takes in one write.
pub const PIECE_BYTES: usize = 4 * 1024 * 1024;
/// The prompt a resumed chat gets when its turn was paused.
pub const CONTINUE_PROMPT: &str = "Continue where you left off.";
const RECORD_VERSION: &str = "openagents.move-cloud.v1";
const LOGIN_ENV: &str = "OA_PARAM_LOGIN_REF";
const GIT_ENV: &str = "OA_PARAM_GIT_REF";
const LOGIN_TTL_SECONDS: u64 = 3600;
const AWAKE_TIMEOUT: Duration = Duration::from_secs(600);
const PROBE_TIMEOUT: Duration = Duration::from_secs(120);
const RESTORE_TIMEOUT: Duration = Duration::from_secs(1800);
const PUSH_TIMEOUT: Duration = Duration::from_secs(120);
const CODEX_FULL_ACCESS: &str = "--dangerously-bypass-approvals-and-sandbox";

/// The harnesses whose chats can move today.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveHarness {
    ClaudeCode,
    Codex,
}

impl MoveHarness {
    pub fn from_id(id: HarnessId) -> Option<Self> {
        match id {
            HarnessId::ClaudeCode => Some(Self::ClaudeCode),
            HarnessId::Codex => Some(Self::Codex),
            _ => None,
        }
    }
    /// The name a person knows the agent by.
    pub fn name(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "Codex",
        }
    }
    /// The harness's data folder under the computer's home.
    fn remote_dir(self) -> &'static str {
        match self {
            Self::ClaudeCode => ".claude",
            Self::Codex => ".codex",
        }
    }
    /// The variable the harness reads its login from; it only ever holds the
    /// `oa-ref:` marker, which the computer's proxy swaps for the real login.
    fn login_variable(self) -> &'static str {
        match self {
            Self::ClaudeCode => "CLAUDE_CODE_OAUTH_TOKEN",
            Self::Codex => "CODEX_API_KEY",
        }
    }
}

/// A login the account has registered for cloud computers: a credential
/// source and the rule that lets a command use it. Never the login itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginSource {
    pub source: String,
    pub rule: String,
}

/// Which logins a cloud computer can use, by reference.
#[derive(Clone, Debug, Default)]
pub struct CloudLogins {
    pub claude_code: Option<LoginSource>,
    pub codex: Option<LoginSource>,
    /// For fetching a pushed ref from a private remote.
    pub git: Option<LoginSource>,
}

impl CloudLogins {
    fn harness(&self, harness: MoveHarness) -> Option<&LoginSource> {
        match harness {
            MoveHarness::ClaudeCode => self.claude_code.as_ref(),
            MoveHarness::Codex => self.codex.as_ref(),
        }
    }
}

/// Where this computer's harnesses keep their session files.
#[derive(Clone, Debug)]
pub struct LocalRoots {
    /// `CLAUDE_CONFIG_DIR`, or `~/.claude`.
    pub claude: PathBuf,
    /// `CODEX_HOME`, or `~/.codex`.
    pub codex: PathBuf,
}

impl LocalRoots {
    pub fn from_env() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        let dir = |name: &str, fallback: &str| {
            std::env::var_os(name)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(fallback))
        };
        Self {
            claude: dir("CLAUDE_CONFIG_DIR", ".claude"),
            codex: dir("CODEX_HOME", ".codex"),
        }
    }
    fn of(&self, harness: MoveHarness) -> &Path {
        match harness {
            MoveHarness::ClaudeCode => &self.claude,
            MoveHarness::Codex => &self.codex,
        }
    }
}

/// The chat to move, read at a turn boundary.
#[derive(Clone, Debug)]
pub struct ChatToMove {
    /// The chat's ID (a lowercase UUID).
    pub chat: String,
    pub harness: HarnessId,
    /// The folder the harness runs in.
    pub project: PathBuf,
    /// The harness's own session ID (`--resume` for Claude Code, the rollout
    /// ID for Codex).
    pub session: String,
    /// Bounded context for the fallback when the path can't be reproduced.
    pub seed: Seed,
}

/// Which cloud computer receives the chat.
#[derive(Clone, Serialize, Deserialize)]
pub enum Destination {
    /// An existing Computers API computer.
    Existing { computer: String },
    /// A new computer from a saved create request. Several chats moving to
    /// the same new computer share one request, so they share one computer.
    New { request: CreateRequest },
}

/// How the captured commit reaches the computer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "via")]
pub enum Workspace {
    Pushed { remote: String },
    Bundle { sha256: String, size: u64 },
}

/// The captured project and agent state. Every value is fixed at capture.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Capture {
    /// The repository's top folder.
    pub root: PathBuf,
    pub commit: String,
    pub tree: String,
    pub parent: Option<String>,
    pub branch: Option<String>,
    pub workspace: Workspace,
    /// The session file's path under the harness's data folder.
    pub session_path: String,
    pub session_sha256: String,
    pub session_size: u64,
}

/// Where the project goes on the computer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "at")]
pub enum RestoreTarget {
    /// The same absolute path, so the harness's native resume works.
    SamePath,
    /// Another folder; the chat continues from its seed.
    Elsewhere { root: String },
}

/// What the probe saw on the computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathState {
    /// Empty or new, or already restored from this exact capture.
    Available,
    /// Something else is there.
    Occupied,
    /// The folder can't be made.
    Unavailable,
}

/// Decide where the project goes. Only a path the computer can hold as this
/// project's own folder keeps native resume; anything else continues from
/// the seed in a folder of its own.
pub fn decide_restore(chat: &str, root: &Path, state: &PathState, home: &str) -> RestoreTarget {
    match state {
        PathState::Available => RestoreTarget::SamePath,
        PathState::Occupied | PathState::Unavailable => {
            let name = root
                .file_name()
                .and_then(|n| n.to_str())
                .filter(|n| !n.is_empty())
                .unwrap_or("project");
            RestoreTarget::Elsewhere {
                root: format!(
                    "{}/openagents/moved/{chat}/{name}",
                    home.trim_end_matches('/')
                ),
            }
        }
    }
}

/// One uploaded piece, with the write intent saved before it is sent.
#[derive(Clone, Serialize, Deserialize)]
pub struct Piece {
    pub remote: String,
    pub offset: u64,
    pub len: u64,
    pub intent: Option<WriteIntent>,
    pub done: bool,
}

/// The finished move.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum Outcome {
    Resumed {
        target: RestoreTarget,
        command: String,
    },
    /// The chat stays on this computer, with the reason in plain words.
    StaysLocal { reason: String },
}

/// The durable record of one chat's move.
#[derive(Clone, Serialize, Deserialize)]
pub struct MoveRecord {
    v: String,
    pub chat: String,
    pub harness: MoveHarness,
    pub project: PathBuf,
    pub session: String,
    pub seed: Seed,
    pub destination: Destination,
    /// Fixes the capture commit's dates, so a retried capture of an
    /// unchanged tree makes the same commit.
    pub planned_unix: u64,
    pub capture: Option<Capture>,
    pub computer: Option<ComputerRef>,
    pub probe: KeyedCommand,
    pub target: Option<RestoreTarget>,
    pub pieces: BTreeMap<String, Vec<Piece>>,
    pub restore: KeyedCommand,
    pub restored: bool,
    pub resume: KeyedCommand,
    pub outcome: Option<Outcome>,
    /// The command that saves the cloud computer's work for Bring back
    /// (#11933).
    #[serde(default)]
    pub back: KeyedCommand,
}

/// One command's identity, saved in stages: the request (its key), then the
/// credential request when it needs a login, then the final request with the
/// login's `oa-ref:` marker. Each stage is saved before it is sent.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct KeyedCommand {
    pub base: Option<CommandRequest>,
    pub credential: Option<CredentialRequest>,
    pub ready: Option<CommandRequest>,
}

impl MoveRecord {
    /// The hidden ref the capture commit lives at.
    pub fn git_ref(&self) -> String {
        format!("refs/openagents/move/{}", self.chat)
    }
    pub(crate) fn remote_dir(&self) -> String {
        format!(".openagents/move/{}", self.chat)
    }
}

/// A failure that leaves the chat where it is. Run the move again to retry;
/// it reuses every saved identity.
#[derive(Debug, thiserror::Error)]
pub enum MoveError {
    #[error("{0}")]
    Computers(#[from] ComputersError),
    #[error("Couldn't save the move: {0}")]
    Io(#[from] std::io::Error),
    #[error("Git couldn't capture the project: {0}")]
    Git(String),
    #[error("This move isn't planned. Plan it first.")]
    NotPlanned,
    #[error("The chat ID isn't valid.")]
    InvalidChat,
}

fn stays(reason: impl Into<String>) -> Outcome {
    Outcome::StaysLocal {
        reason: reason.into(),
    }
}

fn unsupported_harness() -> String {
    "This agent can't move to a cloud computer yet, so this chat stays on this computer.".into()
}
fn login_missing(harness: MoveHarness) -> String {
    format!(
        "{} isn't signed in for cloud computers, so this chat stays on this computer. Sign in to {} for cloud computers, then move it again.",
        harness.name(),
        harness.name()
    )
}
fn not_git() -> String {
    "This project isn't a Git repository, so this chat stays on this computer.".into()
}
fn no_session(harness: MoveHarness) -> String {
    format!(
        "We couldn't find this chat's {} conversation on this computer, so it stays here.",
        harness.name()
    )
}
fn restore_failed() -> String {
    "The cloud computer couldn't set up the project, so this chat stays on this computer.".into()
}
fn resume_failed(harness: MoveHarness) -> String {
    format!(
        "The cloud computer couldn't start {}, so this chat stays on this computer.",
        harness.name()
    )
}

/// Every plain-words reason a move can give, for the copy guard.
#[cfg(test)]
pub(crate) fn all_reasons() -> Vec<String> {
    let mut all = vec![unsupported_harness(), not_git(), restore_failed()];
    for h in [MoveHarness::ClaudeCode, MoveHarness::Codex] {
        all.extend([login_missing(h), no_session(h), resume_failed(h)]);
    }
    all
}

/// The move records, one JSON file per chat plus a folder for its captured
/// bundle, session file, and prompt.
pub struct MoveStore {
    dir: PathBuf,
}

impl MoveStore {
    pub fn open(dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    fn record_path(&self, chat: &str) -> PathBuf {
        self.dir.join(format!("{chat}.json"))
    }
    pub(crate) fn blobs(&self, chat: &str) -> PathBuf {
        self.dir.join(chat)
    }

    pub fn load(&self, chat: &str) -> Result<Option<MoveRecord>, MoveError> {
        if !chat_home::valid_id(chat) {
            return Err(MoveError::InvalidChat);
        }
        match std::fs::read(self.record_path(chat)) {
            Ok(bytes) => {
                let record: MoveRecord = serde_json::from_slice(&bytes)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                if record.v != RECORD_VERSION || record.chat != chat {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "unknown move record",
                    )
                    .into());
                }
                Ok(Some(record))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Set a finished record and its saved files aside, so the chat can
    /// move again later. A chat with no record is left alone.
    pub fn retire(&self, chat: &str, now_unix: u64) -> Result<(), MoveError> {
        if !chat_home::valid_id(chat) {
            return Err(MoveError::InvalidChat);
        }
        let record = self.record_path(chat);
        if record.is_file() {
            std::fs::rename(
                &record,
                self.dir.join(format!("{chat}.{now_unix}.done.json")),
            )?;
        }
        let blobs = self.blobs(chat);
        if blobs.is_dir() {
            std::fs::rename(&blobs, self.dir.join(format!("{chat}.{now_unix}.done")))?;
        }
        Ok(())
    }

    pub fn save(&self, record: &MoveRecord) -> Result<(), MoveError> {
        let bytes = serde_json::to_vec_pretty(record)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        write_atomic(&self.record_path(&record.chat), &bytes)?;
        Ok(())
    }

    /// Save a new record, or return the one already saved for this chat. An
    /// unsupported harness is recorded as staying local.
    pub fn plan(
        &self,
        chat: ChatToMove,
        destination: Destination,
        now_unix: u64,
    ) -> Result<MoveRecord, MoveError> {
        if let Some(existing) = self.load(&chat.chat)? {
            return Ok(existing);
        }
        let harness = MoveHarness::from_id(chat.harness);
        let mut record = MoveRecord {
            v: RECORD_VERSION.into(),
            chat: chat.chat,
            harness: harness.unwrap_or(MoveHarness::ClaudeCode),
            project: chat.project,
            session: chat.session,
            seed: chat.seed,
            destination,
            planned_unix: now_unix,
            capture: None,
            computer: None,
            probe: KeyedCommand::default(),
            target: None,
            pieces: BTreeMap::new(),
            restore: KeyedCommand::default(),
            restored: false,
            resume: KeyedCommand::default(),
            outcome: None,
            back: KeyedCommand::default(),
        };
        if harness.is_none() {
            record.outcome = Some(stays(unsupported_harness()));
        }
        self.save(&record)?;
        Ok(record)
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let tmp = path.with_extension("tmp");
    {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

fn sha256_file(path: &Path) -> std::io::Result<(String, u64)> {
    use std::io::Read as _;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut size = 0u64;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        size += n as u64;
        hasher.update(&buf[..n]);
    }
    Ok((format!("{:x}", hasher.finalize()), size))
}

/// The pieces a file of `size` bytes uploads as. An empty file is one piece.
pub fn piece_ranges(size: u64) -> Vec<(u64, u64)> {
    let step = PIECE_BYTES as u64;
    if size == 0 {
        return vec![(0, 0)];
    }
    (0..size.div_ceil(step))
        .map(|i| {
            let offset = i * step;
            (offset, step.min(size - offset))
        })
        .collect()
}

/// Quote a value for POSIX `sh`.
pub(crate) fn q(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

pub(crate) async fn git(
    dir: &Path,
    args: &[&str],
    env: &[(&str, String)],
) -> Result<String, MoveError> {
    let mut cmd = tokio::process::Command::new("git");
    cmd.arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd
        .output()
        .await
        .map_err(|e| MoveError::Git(e.to_string()))?;
    if !out.status.success() {
        return Err(MoveError::Git(
            String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Find the harness's own session file for this session ID.
pub fn find_session_file(
    roots: &LocalRoots,
    harness: MoveHarness,
    session: &str,
) -> Option<PathBuf> {
    if session.is_empty()
        || session.len() > 128
        || !session
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return None;
    }
    let root = roots.of(harness);
    match harness {
        MoveHarness::ClaudeCode => std::fs::read_dir(root.join("projects"))
            .ok()?
            .flatten()
            .map(|dir| dir.path().join(format!("{session}.jsonl")))
            .find(|path| path.is_file()),
        MoveHarness::Codex => {
            let suffix = format!("-{session}.jsonl");
            let mut stack = vec![(root.join("sessions"), 0)];
            while let Some((dir, depth)) = stack.pop() {
                for entry in std::fs::read_dir(&dir).ok()?.flatten() {
                    let path = entry.path();
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    if path.is_dir() && depth < 4 {
                        stack.push((path, depth + 1));
                    } else if name.starts_with("rollout-") && name.ends_with(&suffix) {
                        return Some(path);
                    }
                }
            }
            None
        }
    }
}

/// Commit the working tree, including untracked files but not ignored ones,
/// to `git_ref`, through a private index so the person's index and working
/// tree stay as they were. The commit's dates come from `planned_unix`, so
/// capturing an unchanged tree again makes the same commit.
pub async fn capture_tree(
    project: &Path,
    git_ref: &str,
    chat: &str,
    planned_unix: u64,
    index: &Path,
) -> Result<(PathBuf, String, String, Option<String>, Option<String>), MoveError> {
    let root = PathBuf::from(git(project, &["rev-parse", "--show-toplevel"], &[]).await?);
    let parent = git(
        &root,
        &["rev-parse", "-q", "--verify", "HEAD^{commit}"],
        &[],
    )
    .await
    .ok()
    .filter(|s| !s.is_empty());
    let branch = git(&root, &["symbolic-ref", "-q", "--short", "HEAD"], &[])
        .await
        .ok()
        .filter(|s| !s.is_empty());
    let _ = std::fs::remove_file(index);
    let real = PathBuf::from(git(&root, &["rev-parse", "--git-path", "index"], &[]).await?);
    let real = if real.is_absolute() {
        real
    } else {
        root.join(real)
    };
    if real.is_file() {
        std::fs::copy(&real, index)?;
    }
    let env = [("GIT_INDEX_FILE", index.to_string_lossy().into_owned())];
    git(&root, &["add", "-A", "--", "."], &env).await?;
    let tree = git(&root, &["write-tree"], &env).await?;
    let _ = std::fs::remove_file(index);
    let date = format!("{planned_unix} +0000");
    let who = [
        ("GIT_AUTHOR_NAME", "OpenAgents".to_owned()),
        ("GIT_AUTHOR_EMAIL", "move@openagents.com".to_owned()),
        ("GIT_COMMITTER_NAME", "OpenAgents".to_owned()),
        ("GIT_COMMITTER_EMAIL", "move@openagents.com".to_owned()),
        ("GIT_AUTHOR_DATE", date.clone()),
        ("GIT_COMMITTER_DATE", date),
    ];
    let message = format!("Move chat {chat} to a cloud computer");
    let mut args = vec![
        "commit-tree",
        "--no-gpg-sign",
        tree.as_str(),
        "-m",
        message.as_str(),
    ];
    if let Some(parent) = &parent {
        args.extend(["-p", parent.as_str()]);
    }
    let commit = git(&root, &args, &who).await?;
    git(&root, &["update-ref", git_ref, &commit], &[]).await?;
    Ok((root, commit, tree, parent, branch))
}

/// The remote a capture can be pushed to: the branch's upstream remote, else
/// `origin`, else the only remote.
async fn push_remote(root: &Path, branch: Option<&str>) -> Option<(String, String)> {
    let names = git(root, &["remote"], &[]).await.ok()?;
    let names: Vec<&str> = names.lines().filter(|l| !l.is_empty()).collect();
    let upstream = match branch {
        Some(b) => git(
            root,
            &["config", "--get", &format!("branch.{b}.remote")],
            &[],
        )
        .await
        .ok(),
        None => None,
    };
    let name = upstream
        .filter(|u| names.contains(&u.as_str()))
        .or_else(|| names.contains(&"origin").then(|| "origin".to_owned()))
        .or_else(|| (names.len() == 1).then(|| names[0].to_owned()))?;
    let url = git(root, &["remote", "get-url", &name], &[]).await.ok()?;
    remote_reachable(&url).then_some((name, url))
}

/// A cloud computer can't fetch from this computer's own folders.
fn remote_reachable(url: &str) -> bool {
    let local = url.starts_with('/') || url.starts_with("file:") || url.starts_with('.');
    !local || cfg!(test)
}

/// Runs a chat's move against one account's Computers client.
pub struct Mover<'a> {
    pub store: &'a MoveStore,
    pub computers: &'a Computers<'a>,
    pub logins: &'a CloudLogins,
    pub roots: &'a LocalRoots,
    pub cancel: CancellationToken,
}

impl Mover<'_> {
    fn load(&self, chat: &str) -> Result<MoveRecord, MoveError> {
        self.store.load(chat)?.ok_or(MoveError::NotPlanned)
    }

    fn finish(&self, record: &mut MoveRecord, outcome: Outcome) -> Result<Outcome, MoveError> {
        record.outcome = Some(outcome.clone());
        self.store.save(record)?;
        Ok(outcome)
    }

    /// Check the login, then capture the project and the session file on
    /// this computer. Safe to call again; a finished capture is kept.
    pub async fn capture(&self, chat: &str) -> Result<Option<Outcome>, MoveError> {
        let mut record = self.load(chat)?;
        if let Some(outcome) = &record.outcome {
            return Ok(Some(outcome.clone()));
        }
        if self.logins.harness(record.harness).is_none() {
            let reason = login_missing(record.harness);
            return self.finish(&mut record, stays(reason)).map(Some);
        }
        if record.capture.is_some() {
            return Ok(None);
        }
        let blobs = self.store.blobs(chat);
        std::fs::create_dir_all(&blobs)?;
        let Some(session_file) = find_session_file(self.roots, record.harness, &record.session)
        else {
            let reason = no_session(record.harness);
            return self.finish(&mut record, stays(reason)).map(Some);
        };
        if git(
            &record.project,
            &["rev-parse", "--is-inside-work-tree"],
            &[],
        )
        .await
        .is_err()
        {
            return self.finish(&mut record, stays(not_git())).map(Some);
        }
        let git_ref = record.git_ref();
        let (root, commit, tree, parent, branch) = capture_tree(
            &record.project,
            &git_ref,
            chat,
            record.planned_unix,
            &blobs.join("index"),
        )
        .await?;
        let session_copy = blobs.join("session.jsonl");
        std::fs::copy(&session_file, &session_copy)?;
        let (session_sha256, session_size) = sha256_file(&session_copy)?;
        let session_root = self.roots.of(record.harness);
        let session_path = session_file
            .strip_prefix(session_root)
            .map_err(|_| MoveError::Git("session file outside its folder".into()))?
            .to_string_lossy()
            .into_owned();
        let mut workspace = None;
        if let Some((name, url)) = push_remote(&root, branch.as_deref()).await {
            let spec = format!("+{commit}:{git_ref}");
            let args = ["push", "-q", "--no-verify", name.as_str(), spec.as_str()];
            let push = git(&root, &args, &[]);
            match tokio::time::timeout(PUSH_TIMEOUT, push).await {
                Ok(Ok(_)) => workspace = Some(Workspace::Pushed { remote: url }),
                Ok(Err(e)) => tracing::info!(chat, error = %e, "move push refused; using a bundle"),
                Err(_) => tracing::info!(chat, "move push timed out; using a bundle"),
            }
        }
        let workspace = match workspace {
            Some(w) => w,
            None => {
                let bundle = blobs.join("workspace.bundle");
                let path = bundle.to_string_lossy().into_owned();
                git(&root, &["bundle", "create", "-q", &path, &git_ref], &[]).await?;
                let (sha256, size) = sha256_file(&bundle)?;
                Workspace::Bundle { sha256, size }
            }
        };
        record.capture = Some(Capture {
            root,
            commit,
            tree,
            parent,
            branch,
            workspace,
            session_path,
            session_sha256,
            session_size,
        });
        self.store.save(&record)?;
        Ok(None)
    }

    /// Run the whole move, or continue it from where it stopped.
    pub async fn run(&self, chat: &str) -> Result<Outcome, MoveError> {
        if let Some(outcome) = self.capture(chat).await? {
            return Ok(outcome);
        }
        let mut record = self.load(chat)?;
        if let Some(outcome) = &record.outcome {
            return Ok(outcome.clone());
        }
        let computer = self.computer(&mut record).await?;
        let target = match record.target.clone() {
            Some(target) => target,
            None => match self.probe(&mut record, &computer).await? {
                Ok(target) => target,
                Err(outcome) => return self.finish(&mut record, outcome),
            },
        };
        self.upload(&mut record, &computer, &target).await?;
        if !record.restored
            && let Err(outcome) = self.restore(&mut record, &computer, &target).await?
        {
            return self.finish(&mut record, outcome);
        }
        let outcome = self.resume(&mut record, &computer, &target).await?;
        self.finish(&mut record, outcome)
    }

    async fn computer(&self, record: &mut MoveRecord) -> Result<ComputerRef, MoveError> {
        let reference = match &record.computer {
            Some(reference) => reference.clone(),
            None => {
                let reference = match &record.destination {
                    Destination::Existing { computer } => self.computers.reference(computer)?,
                    Destination::New { request } => self
                        .computers
                        .create(request, &self.cancel)
                        .await?
                        .reference()
                        .clone(),
                };
                record.computer = Some(reference.clone());
                self.store.save(record)?;
                reference
            }
        };
        let now = self.computers.get(&reference, &self.cancel).await?;
        if !now.awake() {
            if now.stopped() {
                self.computers.wake(&reference, &self.cancel).await?;
            }
            self.computers
                .wait(&reference, WaitFor::Awake, AWAKE_TIMEOUT, &self.cancel)
                .await?;
        }
        Ok(reference)
    }

    /// Send a saved command once (a retry reuses its key), wait for it, and
    /// read its output.
    pub(crate) async fn run_command(
        &self,
        request: &CommandRequest,
        timeout: Duration,
    ) -> Result<(CommandOutcome, Vec<u8>), MoveError> {
        let command = self.computers.execute(request, &self.cancel).await?;
        let done = match command.outcome() {
            Some(_) => command,
            None => {
                self.computers
                    .wait_command(command.reference(), timeout, &self.cancel)
                    .await?
            }
        };
        let outcome = done.outcome().unwrap_or(CommandOutcome::Unknown);
        let mut reader = self.computers.output(done.reference())?;
        let mut stdout = Vec::new();
        while let Some(page) = reader.next_page(self.computers, &self.cancel).await? {
            stdout.extend(page.stdout);
            if stdout.len() > 65_536 {
                break;
            }
        }
        Ok((outcome, stdout))
    }

    /// Build, or reload, a command's saved identity. Every stage is saved
    /// before anything is sent, so a retry sends the same key and body.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn prepare_command(
        &self,
        record: &mut MoveRecord,
        pick: fn(&mut MoveRecord) -> &mut KeyedCommand,
        computer: &ComputerRef,
        script: String,
        timeout: Duration,
        login: Option<&LoginSource>,
        env: &str,
    ) -> Result<CommandRequest, MoveError> {
        if let Some(ready) = pick(record).ready.clone() {
            return Ok(ready);
        }
        if pick(record).base.is_none() {
            let base = self.computers.command_request(
                computer,
                CommandOptions {
                    command: script,
                    timeout_seconds: Some(timeout.as_secs()),
                    ..Default::default()
                },
            )?;
            pick(record).base = Some(base);
            self.store.save(record)?;
        }
        let base = pick(record).base.clone().ok_or(MoveError::NotPlanned)?;
        let ready = match login {
            None => base,
            Some(login) => {
                if pick(record).credential.is_none() {
                    let request = self.computers.credential_request(
                        &base,
                        &login.source,
                        &login.rule,
                        LOGIN_TTL_SECONDS,
                    )?;
                    pick(record).credential = Some(request);
                    self.store.save(record)?;
                }
                let request = pick(record)
                    .credential
                    .clone()
                    .ok_or(MoveError::NotPlanned)?;
                let prepared = self
                    .computers
                    .prepare_credential(&request, &self.cancel)
                    .await?;
                self.computers
                    .command_with_credential(&base, &prepared, env)?
            }
        };
        pick(record).ready = Some(ready.clone());
        self.store.save(record)?;
        Ok(ready)
    }

    async fn probe(
        &self,
        record: &mut MoveRecord,
        computer: &ComputerRef,
    ) -> Result<Result<RestoreTarget, Outcome>, MoveError> {
        let capture = record.capture.clone().ok_or(MoveError::NotPlanned)?;
        let root = capture.root.to_string_lossy().into_owned();
        let script = format!(
            r#"P={p}; T={t}
if [ -d "$P" ] && [ -n "$(ls -A "$P" 2>/dev/null)" ]; then
  if [ "$(cat "$P/.git/openagents-move-tree" 2>/dev/null)" = "$T" ]; then echo path=available; else echo path=occupied; fi
elif mkdir -p "$P" 2>/dev/null && [ -w "$P" ]; then echo path=available
else echo path=unavailable; fi
printf 'home=%s\n' "$HOME""#,
            p = q(&root),
            t = q(&capture.tree),
        );
        let request = self
            .prepare_command(
                record,
                |r| &mut r.probe,
                computer,
                script,
                PROBE_TIMEOUT,
                None,
                "",
            )
            .await?;
        let (outcome, stdout) = self.run_command(&request, PROBE_TIMEOUT).await?;
        let text = String::from_utf8_lossy(&stdout);
        let field = |name: &str| {
            text.lines()
                .find_map(|l| l.strip_prefix(name).map(str::to_owned))
        };
        let (Some(state), Some(home)) = (field("path="), field("home=")) else {
            return Ok(Err(stays(restore_failed())));
        };
        if outcome != CommandOutcome::Succeeded || !home.starts_with('/') {
            return Ok(Err(stays(restore_failed())));
        }
        let state = match state.as_str() {
            "available" => PathState::Available,
            "occupied" => PathState::Occupied,
            _ => PathState::Unavailable,
        };
        let target = decide_restore(&record.chat, &capture.root, &state, &home);
        record.target = Some(target.clone());
        self.store.save(record)?;
        Ok(Ok(target))
    }

    /// The local files this target uploads, by name.
    fn uploads(
        &self,
        record: &MoveRecord,
        target: &RestoreTarget,
    ) -> Result<Vec<(String, PathBuf)>, MoveError> {
        let capture = record.capture.as_ref().ok_or(MoveError::NotPlanned)?;
        let blobs = self.store.blobs(&record.chat);
        let mut files = Vec::new();
        if matches!(capture.workspace, Workspace::Bundle { .. }) {
            files.push((
                "workspace.bundle".to_owned(),
                blobs.join("workspace.bundle"),
            ));
        }
        match target {
            RestoreTarget::SamePath => {
                files.push(("session.jsonl".to_owned(), blobs.join("session.jsonl")));
            }
            RestoreTarget::Elsewhere { .. } => {
                let prompt = blobs.join("prompt.txt");
                if !prompt.is_file() {
                    let text = record
                        .seed
                        .prompt(CONTINUE_PROMPT)
                        .unwrap_or_else(|| CONTINUE_PROMPT.to_owned());
                    write_atomic(&prompt, text.as_bytes())?;
                }
                files.push(("prompt.txt".to_owned(), prompt));
            }
        }
        Ok(files)
    }

    async fn upload(
        &self,
        record: &mut MoveRecord,
        computer: &ComputerRef,
        target: &RestoreTarget,
    ) -> Result<(), MoveError> {
        if record.restored {
            return Ok(());
        }
        let remote_dir = record.remote_dir();
        for (name, local) in self.uploads(record, target)? {
            let (_, size) = sha256_file(&local)?;
            if !record.pieces.contains_key(&name) {
                let pieces = piece_ranges(size)
                    .into_iter()
                    .enumerate()
                    .map(|(i, (offset, len))| Piece {
                        remote: format!("{remote_dir}/{name}.{i:03}"),
                        offset,
                        len,
                        intent: None,
                        done: false,
                    })
                    .collect();
                record.pieces.insert(name.clone(), pieces);
                self.store.save(record)?;
            }
            let count = record.pieces[&name].len();
            for i in 0..count {
                let piece = record.pieces[&name][i].clone();
                if piece.done {
                    continue;
                }
                let bytes = read_range(&local, piece.offset, piece.len)?;
                let intent = match piece.intent {
                    Some(intent) => {
                        // A write may have landed before the reply was lost;
                        // look before sending it again.
                        match self.computers.reconcile_write(&intent, &self.cancel).await {
                            Ok(Some(_)) => {
                                record.pieces.get_mut(&name).unwrap()[i].done = true;
                                self.store.save(record)?;
                                continue;
                            }
                            Ok(None) => intent,
                            Err(e) if e.status() == Some(404) => intent,
                            Err(e) => return Err(e.into()),
                        }
                    }
                    None => {
                        let intent = self
                            .computers
                            .write_intent(computer, &piece.remote, &bytes)?
                            .allow_overwrite();
                        record.pieces.get_mut(&name).unwrap()[i].intent = Some(intent.clone());
                        self.store.save(record)?;
                        intent
                    }
                };
                self.computers
                    .write_file(&intent, &bytes, &self.cancel)
                    .await?;
                record.pieces.get_mut(&name).unwrap()[i].done = true;
                self.store.save(record)?;
            }
        }
        Ok(())
    }

    fn joined(&self, record: &MoveRecord, name: &str, out: &str, sha256: &str) -> String {
        let pieces: Vec<String> = record.pieces[name]
            .iter()
            .map(|p| format!("\"$HOME\"/{}", q(&p.remote)))
            .collect();
        format!("join {} {} {}\n", out, q(sha256), pieces.join(" "))
    }

    async fn restore(
        &self,
        record: &mut MoveRecord,
        computer: &ComputerRef,
        target: &RestoreTarget,
    ) -> Result<Result<(), Outcome>, MoveError> {
        let capture = record.capture.clone().ok_or(MoveError::NotPlanned)?;
        let root = match target {
            RestoreTarget::SamePath => capture.root.to_string_lossy().into_owned(),
            RestoreTarget::Elsewhere { root } => root.clone(),
        };
        let git_ref = record.git_ref();
        let mut script = format!(
            r#"set -eu
P={p}; REF={r}; C={c}; T={t}; PARENT={parent}; BRANCH={branch}
sum() {{ if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1; }}
join() {{ out=$1; want=$2; shift 2; mkdir -p "$(dirname "$out")"; cat "$@" > "$out.part"; [ "$(sum "$out.part")" = "$want" ] || {{ echo "checksum mismatch" >&2; exit 3; }}; mv "$out.part" "$out"; }}
"#,
            p = q(&root),
            r = q(&git_ref),
            c = q(&capture.commit),
            t = q(&capture.tree),
            parent = q(capture.parent.as_deref().unwrap_or("")),
            branch = q(capture.branch.as_deref().unwrap_or("")),
        );
        let fetch = match &capture.workspace {
            Workspace::Bundle { sha256, .. } => {
                let bundle = format!(
                    "\"$HOME\"/{}",
                    q(&format!("{}/workspace.bundle", record.remote_dir()))
                );
                script.push_str(&self.joined(record, "workspace.bundle", &bundle, sha256));
                format!(r#"git fetch -q {bundle} "$REF:$REF""#)
            }
            Workspace::Pushed { remote } => {
                let auth = if self.logins.git.is_some() {
                    format!(r#"-c http.extraHeader="Authorization: Bearer ${GIT_ENV}" "#)
                } else {
                    String::new()
                };
                format!(r#"git {auth}fetch -q {} "+$REF:$REF""#, q(remote))
            }
        };
        script.push_str(&format!(
            r#"if [ "$(cat "$P/.git/openagents-move-tree" 2>/dev/null)" != "$T" ]; then
  mkdir -p "$P"; cd "$P"
  [ -e .git ] || git init -q
  {fetch}
  git checkout -q -f --detach "$C"
  if [ -n "$PARENT" ]; then
    git reset -q --mixed "$PARENT"
    if [ -n "$BRANCH" ]; then git checkout -q -B "$BRANCH"; fi
  else
    git read-tree --empty; git symbolic-ref HEAD "refs/heads/${{BRANCH:-main}}"
  fi
  printf '%s' "$T" > .git/openagents-move-tree
fi
"#
        ));
        if *target == RestoreTarget::SamePath {
            let out = format!(
                "\"$HOME\"/{}",
                q(&format!(
                    "{}/{}",
                    record.harness.remote_dir(),
                    capture.session_path
                ))
            );
            script.push_str(&self.joined(record, "session.jsonl", &out, &capture.session_sha256));
        }
        if let RestoreTarget::Elsewhere { .. } = target {
            let prompt = self.store.blobs(&record.chat).join("prompt.txt");
            let (sha, _) = sha256_file(&prompt)?;
            let out = format!(
                "\"$HOME\"/{}",
                q(&format!("{}/prompt.txt", record.remote_dir()))
            );
            script.push_str(&self.joined(record, "prompt.txt", &out, &sha));
        }
        let git_login = match capture.workspace {
            Workspace::Pushed { .. } => self.logins.git.clone(),
            Workspace::Bundle { .. } => None,
        };
        let request = self
            .prepare_command(
                record,
                |r| &mut r.restore,
                computer,
                script,
                RESTORE_TIMEOUT,
                git_login.as_ref(),
                GIT_ENV,
            )
            .await?;
        let (outcome, _) = self.run_command(&request, RESTORE_TIMEOUT).await?;
        if outcome != CommandOutcome::Succeeded {
            return Ok(Err(stays(restore_failed())));
        }
        record.restored = true;
        self.store.save(record)?;
        Ok(Ok(()))
    }

    /// The command that starts the harness on the computer.
    fn resume_script(&self, record: &MoveRecord, target: &RestoreTarget) -> String {
        let capture = record.capture.as_ref().expect("captured");
        let rel = record
            .project
            .strip_prefix(&capture.root)
            .unwrap_or(Path::new(""))
            .to_string_lossy()
            .into_owned();
        let root = match target {
            RestoreTarget::SamePath => capture.root.to_string_lossy().into_owned(),
            RestoreTarget::Elsewhere { root } => root.clone(),
        };
        let dir = if rel.is_empty() {
            q(&root)
        } else {
            q(&format!("{root}/{rel}"))
        };
        let login = format!(
            r#"export {}="${LOGIN_ENV}";"#,
            record.harness.login_variable()
        );
        let id = q(&record.session);
        let prompt_file = format!(
            "\"$HOME\"/{}",
            q(&format!("{}/prompt.txt", record.remote_dir()))
        );
        let run = match (record.harness, target) {
            (MoveHarness::ClaudeCode, RestoreTarget::SamePath) => format!(
                "claude --resume {id} -p {} --output-format stream-json --verbose",
                q(CONTINUE_PROMPT)
            ),
            (MoveHarness::ClaudeCode, RestoreTarget::Elsewhere { .. }) => {
                format!(r#"claude -p "$(cat {prompt_file})" --output-format stream-json --verbose"#)
            }
            // `codex exec` defaults to a read-only sandbox; the local driver
            // runs Codex with full access, and the cloud computer is the
            // sandbox (checked against codex-cli 0.163.0).
            (MoveHarness::Codex, RestoreTarget::SamePath) => format!(
                "codex exec --json {CODEX_FULL_ACCESS} resume {id} {}",
                q(CONTINUE_PROMPT)
            ),
            (MoveHarness::Codex, RestoreTarget::Elsewhere { .. }) => {
                format!(r#"codex exec --json {CODEX_FULL_ACCESS} "$(cat {prompt_file})""#)
            }
        };
        format!("cd {dir} && {login} exec {run}")
    }

    async fn resume(
        &self,
        record: &mut MoveRecord,
        computer: &ComputerRef,
        target: &RestoreTarget,
    ) -> Result<Outcome, MoveError> {
        let login = self
            .logins
            .harness(record.harness)
            .ok_or(MoveError::NotPlanned)?
            .clone();
        let script = self.resume_script(record, target);
        let request = self
            .prepare_command(
                record,
                |r| &mut r.resume,
                computer,
                script,
                Duration::from_secs(LOGIN_TTL_SECONDS),
                Some(&login),
                LOGIN_ENV,
            )
            .await?;
        let command = self.computers.execute(&request, &self.cancel).await?;
        if matches!(
            command.outcome(),
            Some(CommandOutcome::Failed | CommandOutcome::Cancelled)
        ) && command.data().exit_code != Some(0)
        {
            return Ok(stays(resume_failed(record.harness)));
        }
        Ok(Outcome::Resumed {
            target: target.clone(),
            command: command.reference().id().to_owned(),
        })
    }
}

fn read_range(path: &Path, offset: u64, len: u64) -> std::io::Result<Vec<u8>> {
    use std::io::{Read as _, Seek as _, SeekFrom};
    let mut file = std::fs::File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = vec![0u8; len as usize];
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

/// The command reference a finished move started, for following its output.
pub fn resumed_command(
    computers: &Computers<'_>,
    record: &MoveRecord,
) -> Option<Result<CommandRef, ComputersError>> {
    match (&record.outcome, &record.computer) {
        (Some(Outcome::Resumed { command, .. }), Some(computer)) => {
            Some(computers.command_reference(computer, command))
        }
        _ => None,
    }
}

#[cfg(test)]
pub(crate) mod tests;
