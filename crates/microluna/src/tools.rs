//! The five native function tools, and the workspace they act in.
//!
//! The model sees exactly these tools, declared as Responses API function
//! tools with strict JSON schemas, and never writes an action as JSON in
//! text:
//!
//! - `run_command` runs a shell command in the workspace.
//! - `read_file` returns a numbered region of a file.
//! - `apply_patch` applies a patch in the [`crate::patch`] format.
//! - `write_file` replaces a file's whole contents.
//! - `finish` ends the session with a typed [`Finish`].
//!
//! # The boundary
//!
//! A command runs through `/bin/sh` inside a `coder-boundary` writing
//! boundary whose only writable checkout is the workspace root, plus a
//! scratch directory the boundary owns and removes. `supervise` owns its
//! process group, its deadline, and its output caps, and holds the
//! boundary until the child is reaped. A host with no enforced backend
//! refuses the command; it never runs unbounded.
//!
//! The three file tools run in this process, and apply the same policy by
//! path: every path resolves inside the workspace root, `..` can't climb
//! out of it, and a symbolic link that leads out is refused.
//!
//! A workspace given a [`crate::remote::Remote`] ([`Workspace::in_remote`])
//! runs all four tools there instead, such as in a fresh container, and
//! the host's root holds nothing the session sees.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::patch::{self, Hunk};

/// The default wall-time bound on one command.
pub const COMMAND_WALL: Duration = Duration::from_secs(120);

/// The longest bound the model may ask for.
pub const COMMAND_WALL_MAX: Duration = Duration::from_secs(600);

/// Bytes kept per stream of one command.
pub const COMMAND_KEEP: usize = 16 * 1024;

/// Lines `read_file` returns when the model doesn't say.
pub const READ_LINES: usize = 200;

/// The most lines one `read_file` returns.
pub const READ_LINES_MAX: usize = 2_000;

/// The most bytes one `read_file` returns.
pub const READ_BYTES_MAX: usize = 48 * 1024;

/// How a session ended, in the model's own typed words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishStatus {
    /// The task is done.
    Done,
    /// The task can't continue without something the session lacks.
    Blocked,
    /// The model tried and failed.
    Failed,
}

/// What `finish` carries.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, serde::Serialize)]
pub struct Finish {
    /// How the session ended.
    pub status: FinishStatus,
    /// What was done, in a sentence or two.
    pub summary: String,
    /// The answer to a question, or an empty string.
    pub answer: String,
    /// Why the session is blocked or failed, typed, so the host can act on
    /// it; `none` when it is done.
    #[serde(default)]
    pub cause: Cause,
}

/// Why a session couldn't finish the work, in the model's typed words.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Cause {
    /// Nothing blocked the session.
    #[default]
    None,
    /// The test harness itself fails: a missing file, a runner error, or
    /// a test that errors before it asserts anything.
    HarnessBroken,
    /// A test asserts something the task contradicts.
    TestContradictsTask,
    /// A program or library the work needs isn't installed.
    MissingTool,
    /// The task leaves open something only its author can settle.
    NeedsInformation,
    /// Anything else.
    Other,
}

impl Cause {
    /// The cause as the tool spells it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Cause::None => "none",
            Cause::HarnessBroken => "harness_broken",
            Cause::TestContradictsTask => "test_contradicts_task",
            Cause::MissingTool => "missing_tool",
            Cause::NeedsInformation => "needs_information",
            Cause::Other => "other",
        }
    }
}

/// Whether a call only reads: `read_file`, or a `run_command` whose every
/// piece is a reading program (`cat`, `ls`, `grep`, `sed -n`, and the
/// like) with no redirection. Reads can run at the same time, and a turn
/// made only of reads is orientation rather than work.
#[must_use]
pub fn reads_only(name: &str, arguments: &str) -> bool {
    match name {
        "read_file" => true,
        "run_command" => {
            let command = serde_json::from_str::<Value>(arguments)
                .ok()
                .and_then(|v| v["command"].as_str().map(str::to_string))
                .unwrap_or_default();
            if command.trim().is_empty()
                || command.contains('>')
                || command.contains("tee ")
                || command.contains("system(")
            {
                return false;
            }
            command
                .split(['|', ';', '&', '\n'])
                .map(str::trim)
                .filter(|piece| !piece.is_empty())
                .all(|piece| {
                    let mut words = piece.split_whitespace();
                    let first = words.next().unwrap_or_default();
                    let first = first.rsplit('/').next().unwrap_or(first);
                    match first {
                        "sed" => words.next() == Some("-n"),
                        "cat" | "ls" | "head" | "tail" | "grep" | "rg" | "find" | "wc" | "nl"
                        | "tree" | "pwd" | "stat" | "file" | "du" | "sort" | "uniq" | "cut"
                        | "awk" | "echo" | "printf" | "which" | "type" | "cd" | "true" => true,
                        _ => false,
                    }
                })
        }
        _ => false,
    }
}

/// The tool declarations the model is sent, in a fixed order so they stay
/// part of the cached prefix.
#[must_use]
pub fn declarations() -> Vec<Value> {
    vec![
        function(
            "run_command",
            "Run a shell command with /bin/sh in the workspace root. Returns the exit \
             code, standard output, and standard error, each capped. Writes outside the \
             workspace are denied.",
            json!({
                "command": { "type": "string", "description": "The shell command." },
                "timeout_seconds": {
                    "type": ["integer", "null"],
                    "description": "Wall-time bound in seconds; null for 120, at most 600."
                }
            }),
        ),
        function(
            "read_file",
            "Read a region of a text file, with 1-based line numbers. Line numbers are \
             not part of the file.",
            json!({
                "path": { "type": "string", "description": "Path relative to the workspace root." },
                "start_line": { "type": ["integer", "null"], "description": "First line, 1-based; null for 1." },
                "max_lines": { "type": ["integer", "null"], "description": "Lines to return; null for 200, at most 2000." }
            }),
        ),
        function(
            "apply_patch",
            "Edit files with a patch: '*** Begin Patch', then '*** Add File: <path>' with \
             '+' lines, '*** Delete File: <path>', or '*** Update File: <path>' with \
             optional '@@ <context line>' markers and ' ', '-', '+' lines, then \
             '*** End Patch'. Paths are relative to the workspace root.",
            json!({
                "patch": { "type": "string", "description": "The whole patch." }
            }),
        ),
        function(
            "write_file",
            "Create or replace a file with the given contents. Prefer apply_patch for \
             edits to existing files.",
            json!({
                "path": { "type": "string", "description": "Path relative to the workspace root." },
                "contents": { "type": "string", "description": "The whole new file." }
            }),
        ),
        function(
            "finish",
            "End the session. Call it once, when the task is done, blocked, or failed.",
            json!({
                "status": { "type": "string", "enum": ["done", "blocked", "failed"] },
                "summary": { "type": "string", "description": "What was done, briefly." },
                "answer": { "type": "string", "description": "The answer to a question, or an empty string." },
                "cause": {
                    "type": "string",
                    "enum": ["none", "harness_broken", "test_contradicts_task", "missing_tool", "needs_information", "other"],
                    "description": "Why the session is blocked or failed: the test harness itself fails, a test contradicts the task, a needed program is missing, the task needs information only its author has, or other. none when done."
                }
            }),
        ),
    ]
}

fn function(name: &str, description: &str, properties: Value) -> Value {
    let required: Vec<&String> = properties
        .as_object()
        .map(|map| map.keys().collect())
        .unwrap_or_default();
    json!({
        "type": "function",
        "name": name,
        "description": description,
        "strict": true,
        "parameters": {
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false,
        },
    })
}

/// What one tool call did.
#[derive(Clone, Debug)]
pub struct Outcome {
    /// The text the model reads back.
    pub output: String,
    /// Whether the call did its work, for the ATIF record.
    pub status: atif::Outcome,
    /// The finish, when the call was `finish`.
    pub finish: Option<Finish>,
    /// Host evidence for the record: exit codes, bytes, files touched.
    pub extra: Map<String, Value>,
    /// Wall time the call took.
    pub milliseconds: u64,
}

impl Outcome {
    fn done(output: String) -> Self {
        Outcome {
            output,
            status: atif::Outcome::Completed,
            finish: None,
            extra: Map::new(),
            milliseconds: 0,
        }
    }

    fn failed(output: String) -> Self {
        Outcome {
            status: atif::Outcome::Failed,
            ..Outcome::done(output)
        }
    }

    fn refused(output: String) -> Self {
        Outcome {
            status: atif::Outcome::Cancelled,
            ..Outcome::done(output)
        }
    }

    fn noting(mut self, key: &str, value: Value) -> Self {
        self.extra.insert(key.to_string(), value);
        self
    }
}

/// How a command is confined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Isolation {
    /// Inside a `coder-boundary` writing boundary whose only writable
    /// checkout is the workspace root. A host that can't enforce one
    /// refuses the command.
    Boundary,
    /// Directly, because the whole process already runs in a disposable
    /// task container that is the boundary, as a Terminal-Bench trial
    /// does. The caller asserts this; Microluna never assumes it.
    TaskContainer,
    /// Inside a `coder-boundary` read-only boundary that lets a command
    /// write only its own scratch directory, with `apply_patch` and
    /// `write_file` refused: for a host that permits a session only to
    /// read, such as a Coder Terminal turn that changes nothing.
    ReadOnly,
}

impl Isolation {
    /// The word a command's record carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Isolation::Boundary => "writing",
            Isolation::TaskContainer => "task-container",
            Isolation::ReadOnly => "read-only",
        }
    }
}

/// The directory a session works in.
#[derive(Clone, Debug)]
pub struct Workspace {
    root: PathBuf,
    isolation: Isolation,
    read_only: bool,
    /// An observational review may read files and finish, but run no commands.
    observe_only: bool,
    /// The longest a command may run, below [`COMMAND_WALL_MAX`].
    command_max: Duration,
    /// What an evaluation run cuts commands off from, when it does.
    seal: Option<crate::Seal>,
    /// When set, commands may read only these paths, the workspace root,
    /// their scratch, and the system's program directories.
    reads: Option<Vec<PathBuf>>,
    /// When set, every tool runs there instead of on the host.
    remote: Option<Arc<dyn crate::remote::Remote>>,
}

/// Where a file tool's path leads: the host, or a remote's own file.
enum Target {
    Host(PathBuf),
    Remote(String),
}

impl Workspace {
    /// A workspace at `root`, which must exist.
    ///
    /// # Errors
    ///
    /// The I/O error when `root` can't be resolved.
    pub fn new(root: &Path) -> std::io::Result<Self> {
        Ok(Workspace {
            root: root.canonicalize()?,
            isolation: Isolation::Boundary,
            read_only: false,
            observe_only: false,
            command_max: COMMAND_WALL_MAX,
            seal: None,
            reads: None,
            remote: None,
        })
    }

    /// The same workspace, with every command sealed off from GitHub and,
    /// when the seal is offline, from the network; see [`crate::seal`]. An
    /// offline seal needs an enforced boundary: under
    /// [`Isolation::TaskContainer`] a command is refused rather than run
    /// with the network open.
    #[must_use]
    pub fn sealed_by(mut self, seal: crate::Seal) -> Self {
        self.seal = Some(seal);
        self
    }

    /// The seal commands run under, when there is one.
    #[must_use]
    pub fn seal(&self) -> Option<&crate::Seal> {
        self.seal.as_ref()
    }

    /// The same workspace, with every command bounded by `max` (at most
    /// [`COMMAND_WALL_MAX`]), whatever bound the model asks for. The
    /// command's output says when the bound ended it.
    #[must_use]
    pub fn commands_within(mut self, max: Duration) -> Self {
        self.command_max = max.clamp(Duration::from_secs(1), COMMAND_WALL_MAX);
        self
    }

    /// The same workspace, with `apply_patch` and `write_file` refused, for
    /// a session that only reads and runs. Commands still run.
    #[must_use]
    pub fn reading_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    /// Refuse commands and edits, including inside task containers. The
    /// review reads source and host-recorded test evidence without changing it.
    #[must_use]
    pub fn observing_only(mut self) -> Self {
        self.read_only = true;
        self.observe_only = true;
        self
    }

    /// The same workspace, with every command's reads confined: a command
    /// may read only `readable`, the workspace root, its own scratch
    /// directory, the seal's stub directory, and the system's program
    /// directories (`coder_boundary::SYSTEM_READS`). The host's home,
    /// temporary files, and every other workspace are out of sight. Only
    /// an enforced boundary can confine reads: under
    /// [`Isolation::TaskContainer`] a command is refused rather than run
    /// with reads open.
    #[must_use]
    pub fn confining_reads(mut self, readable: Vec<PathBuf>) -> Self {
        self.reads = Some(readable);
        self
    }

    /// Whether commands run with their reads confined; see
    /// [`Workspace::confining_reads`].
    #[must_use]
    pub fn confines_reads(&self) -> bool {
        self.reads.is_some()
            || self
                .seal
                .as_ref()
                .is_some_and(|seal| seal.read_scope().is_some())
    }

    /// The same workspace, with every tool run in `remote` instead of on
    /// the host: commands, reads, writes, and patches. The host's root
    /// then only holds what the caller puts there. Isolation and read
    /// confinement don't apply; the remote is the boundary. A seal still
    /// applies in one way: an offline seal refuses a command when the
    /// remote has a network.
    #[must_use]
    pub fn in_remote(mut self, remote: Arc<dyn crate::remote::Remote>) -> Self {
        self.remote = Some(remote);
        self
    }

    /// The same workspace, with commands confined by `isolation`.
    #[must_use]
    pub fn isolated_by(mut self, isolation: Isolation) -> Self {
        self.isolation = isolation;
        self
    }

    /// How commands are confined.
    #[must_use]
    pub fn isolation(&self) -> Isolation {
        self.isolation
    }

    /// The resolved root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Runs one tool call by name, with the arguments as the model wrote
    /// them. An unknown tool or malformed arguments is a refused call the
    /// model reads about, not an error that ends the session.
    pub async fn call(&self, name: &str, arguments: &str) -> Outcome {
        let started = Instant::now();
        let mut outcome = match name {
            "run_command" if self.observe_only => Outcome::refused(
                "This review reads files and the host's test results. Commands cannot run; report any missing evidence with finish.".to_string(),
            ),
            "apply_patch" | "write_file" if self.isolation == Isolation::ReadOnly => {
                Outcome::refused(format!(
                    "{name} did not run: this session is read-only, and the host permits no \
                     file changes. Answer from what you can read."
                ))
            }
            "run_command" => match parse::<RunCommand>(arguments) {
                Ok(args) => self.run_command(args).await,
                Err(refusal) => *refusal,
            },
            "read_file" => match parse::<ReadFile>(arguments) {
                Ok(args) => self.read_file(&args),
                Err(refusal) => *refusal,
            },
            "apply_patch" | "write_file" if self.read_only => Outcome::refused(
                "This session only reads and runs: it can't edit files. Report what you found \
                 with finish."
                    .to_string(),
            ),
            "apply_patch" => match parse::<ApplyPatch>(arguments) {
                Ok(args) => self.apply_patch(&args.patch),
                Err(refusal) => *refusal,
            },
            "write_file" => match parse::<WriteFile>(arguments) {
                Ok(args) => self.write_file(&args),
                Err(refusal) => *refusal,
            },
            "finish" => match parse::<Finish>(arguments) {
                Ok(finish) => Outcome {
                    finish: Some(finish),
                    ..Outcome::done("Session finished.".to_string())
                },
                Err(refusal) => *refusal,
            },
            other => Outcome::refused(format!(
                "There is no tool named '{other}'. The tools are run_command, read_file, \
                 apply_patch, write_file, and finish."
            )),
        };
        outcome.milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        outcome
    }

    /// Resolves a path the model named to one inside the root.
    ///
    /// # Errors
    ///
    /// A sentence for the model when the path leaves the workspace.
    pub fn resolve(&self, path: &str) -> Result<PathBuf, String> {
        let named = Path::new(path.trim());
        let relative = if named.is_absolute() {
            named
                .strip_prefix(&self.root)
                .map_err(|_| format!("{path} is outside the workspace"))?
                .to_path_buf()
        } else {
            named.to_path_buf()
        };
        let mut resolved = self.root.clone();
        for component in relative.components() {
            match component {
                Component::Normal(part) => resolved.push(part),
                Component::CurDir => {}
                Component::ParentDir => {
                    if !resolved.pop() || !resolved.starts_with(&self.root) {
                        return Err(format!("{path} climbs out of the workspace"));
                    }
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(format!("{path} is not a workspace path"));
                }
            }
        }
        if !resolved.starts_with(&self.root) {
            return Err(format!("{path} climbs out of the workspace"));
        }
        // A symbolic link on the way may still lead out: resolve the
        // deepest part that exists and check where it really is.
        let mut existing = resolved.as_path();
        while !existing.exists() {
            match existing.parent() {
                Some(parent) => existing = parent,
                None => break,
            }
        }
        match existing.canonicalize() {
            Ok(real) if real.starts_with(&self.root) => Ok(resolved),
            Ok(_) => Err(format!("{path} leads out of the workspace through a link")),
            Err(error) => Err(format!("can't resolve {path}: {error}")),
        }
    }

    async fn run_command(&self, args: RunCommand) -> Outcome {
        let wall = args
            .timeout_seconds
            .map_or(COMMAND_WALL, |seconds| Duration::from_secs(seconds.max(1)))
            .min(self.command_max);
        if self.remote.is_some()
            && self
                .seal
                .as_ref()
                .is_some_and(|seal| seal.read_scope().is_some())
        {
            return Outcome::refused(
                "The command did not run: a remote cannot enforce this evaluation's host read scope."
                    .to_string(),
            );
        }
        if let Some(remote) = &self.remote
            && self.seal.as_ref().is_some_and(crate::Seal::offline)
            && !remote.offline()
        {
            return Outcome::refused(
                "The command did not run: this session is sealed offline, and the place \
                 its commands run has a network."
                    .to_string(),
            );
        }
        let ended = match (&self.remote, self.isolation) {
            (Some(remote), _) => remote.run(&args.command, wall).await,
            (None, Isolation::Boundary | Isolation::ReadOnly) => {
                let spec = if self.isolation == Isolation::ReadOnly {
                    coder_boundary::Boundary::readonly()
                } else {
                    coder_boundary::Boundary::writing(&self.root)
                };
                let spec = if self.seal.as_ref().is_some_and(crate::Seal::offline) {
                    spec.offline()
                } else {
                    spec
                };
                let spec = match &self.reads {
                    None => spec,
                    Some(readable) => {
                        let mut spec = spec.confining_reads();
                        for path in readable {
                            spec = spec.readable(path);
                        }
                        if let Some(seal) = &self.seal {
                            spec = spec.readable(seal.stubs());
                        }
                        spec
                    }
                };
                let spec = match &self.seal {
                    Some(seal) => seal.constrain_reads(spec),
                    None => spec,
                };
                let boundary = match spec.owned_scratch_under(std::env::temp_dir()).build() {
                    Ok(boundary) => boundary,
                    Err(error) => {
                        return Outcome::refused(format!(
                            "The command did not run: this host can't enforce the write \
                             boundary ({error})."
                        ));
                    }
                };
                let mut command = match boundary.command("/bin/sh", ["-c", args.command.as_str()]) {
                    Ok(command) => command,
                    Err(error) => {
                        return Outcome::refused(format!("The command did not run: {error}."));
                    }
                };
                command.current_dir(&self.root);
                if let Some(scratch) = boundary.scratch() {
                    command.env("TMPDIR", scratch);
                }
                withhold_credentials(&mut command);
                if let Some(seal) = &self.seal {
                    seal.apply(&mut command);
                }
                if boundary.confines_reads() {
                    // The host's search path names directories the command
                    // can't read, such as a profile in the home directory;
                    // keep the ones it can, under their resolved names.
                    let path = command
                        .get_envs()
                        .find(|(name, _)| *name == "PATH")
                        .and_then(|(_, value)| value.map(std::ffi::OsStr::to_os_string))
                        .or_else(|| std::env::var_os("PATH"))
                        .unwrap_or_default();
                    command.env("PATH", boundary.search_path(&path));
                    if let Some(scratch) = boundary.scratch() {
                        command.env("HOME", scratch);
                    }
                }
                supervise::Job::from_command(command)
                    .bounded(supervise::Limits::within(wall).keeping(COMMAND_KEEP))
                    .run_holding(boundary.hold())
                    .await
            }
            (None, Isolation::TaskContainer) => {
                if self.confines_reads() {
                    return Outcome::refused(
                        "The command did not run: this session may read only its own files, \
                         and a task container can't limit what a command reads."
                            .to_string(),
                    );
                }
                if self.seal.as_ref().is_some_and(crate::Seal::offline) {
                    return Outcome::refused(
                        "The command did not run: this session is sealed offline, and a task \
                         container can't take the network away."
                            .to_string(),
                    );
                }
                let mut command = std::process::Command::new("/bin/sh");
                command.args(["-c", args.command.as_str()]);
                command.current_dir(&self.root);
                withhold_credentials(&mut command);
                if let Some(seal) = &self.seal {
                    seal.apply(&mut command);
                }
                supervise::Job::from_command(command)
                    .bounded(supervise::Limits::within(wall).keeping(COMMAND_KEEP))
                    .run()
                    .await
            }
        };
        let mut output = match &ended.ending {
            supervise::Ending::TimedOut => {
                format!("[timed out after {} s]\n", wall.as_secs())
            }
            supervise::Ending::Failed(why) => format!("[could not run: {why}]\n"),
            ending => format!(
                "[exit {}]\n",
                ending
                    .code()
                    .map_or("signal".to_string(), |c| c.to_string())
            ),
        };
        if !ended.stdout.is_empty() {
            output.push_str(&ended.stdout.marked());
        }
        if !ended.stderr.is_empty() {
            if !output.ends_with('\n') {
                output.push('\n');
            }
            output.push_str("[stderr]\n");
            output.push_str(&ended.stderr.marked());
        }
        let status = if ended.ending.success() {
            atif::Outcome::Completed
        } else {
            atif::Outcome::Failed
        };
        Outcome {
            status,
            ..Outcome::done(output.trim_end().to_string())
        }
        .noting("exit", json!(ended.ending.code()))
        .noting("bytes", json!(ended.bytes()))
        .noting("truncated", json!(ended.truncated()))
        .noting(
            "boundary",
            json!(
                self.remote
                    .as_ref()
                    .map_or(self.isolation.word(), |remote| remote.word())
            ),
        )
        .noting(
            "reads",
            json!(if self.remote.is_some() {
                "remote"
            } else if self.confines_reads() {
                "confined"
            } else {
                "open"
            }),
        )
        .noting(
            "sealed",
            json!(self.seal.as_ref().map(|seal| if seal.offline() {
                "github-and-network"
            } else {
                "github"
            })),
        )
    }

    /// Resolves a path the model named to the file a tool acts on.
    fn target(&self, path: &str) -> Result<Target, String> {
        match &self.remote {
            Some(remote) => crate::remote::resolve(remote.root(), path).map(Target::Remote),
            None => self.resolve(path).map(Target::Host),
        }
    }

    fn load(&self, target: &Target) -> Result<String, String> {
        match (target, &self.remote) {
            (Target::Remote(path), Some(remote)) => remote
                .read(path, crate::remote::READ_MAX)
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned()),
            (Target::Host(path), _) => std::fs::read(path)
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                .map_err(|error| error.to_string()),
            (Target::Remote(_), None) => Err("no remote to read from".to_string()),
        }
    }

    fn store(&self, target: &Target, contents: Option<&str>) -> Result<(), String> {
        match (target, &self.remote) {
            (Target::Remote(path), Some(remote)) => remote.write(path, contents.map(str::as_bytes)),
            (Target::Host(path), _) => match contents {
                Some(contents) => write(path, contents),
                None => std::fs::remove_file(path),
            }
            .map_err(|error| error.to_string()),
            (Target::Remote(_), None) => Err("no remote to write to".to_string()),
        }
    }

    fn is_file(&self, target: &Target) -> bool {
        match target {
            Target::Host(path) => path.is_file(),
            Target::Remote(_) => self.load(target).is_ok(),
        }
    }

    fn named(target: &Target) -> String {
        match target {
            Target::Host(path) => path.display().to_string(),
            Target::Remote(path) => path.clone(),
        }
    }

    fn read_file(&self, args: &ReadFile) -> Outcome {
        let path = match self.target(&args.path) {
            Ok(path) => path,
            Err(why) => return Outcome::refused(why),
        };
        let text = match self.load(&path) {
            Ok(text) => text,
            Err(error) => return Outcome::failed(format!("can't read {}: {error}", args.path)),
        };
        let start = args.start_line.unwrap_or(1).max(1);
        let max = args
            .max_lines
            .unwrap_or(READ_LINES)
            .clamp(1, READ_LINES_MAX);
        let total = text.lines().count();
        let mut out = String::new();
        let mut shown = 0;
        for (index, line) in text.lines().enumerate().skip(start - 1).take(max) {
            let numbered = format!("{:>6}\t{line}\n", index + 1);
            if out.len() + numbered.len() > READ_BYTES_MAX {
                break;
            }
            out.push_str(&numbered);
            shown += 1;
        }
        let last = start + shown - 1;
        if shown == 0 {
            out = format!("[{} has {total} lines; none from line {start}]", args.path);
        } else if last < total {
            out.push_str(&format!("[lines {start}-{last} of {total}]"));
        } else {
            out.push_str(&format!("[end of file, {total} lines]"));
        }
        Outcome::done(out)
            .noting("lines", json!(total))
            .noting("shown", json!(shown))
    }

    fn write_file(&self, args: &WriteFile) -> Outcome {
        let path = match self.target(&args.path) {
            Ok(path) => path,
            Err(why) => return Outcome::refused(why),
        };
        match self.store(&path, Some(&args.contents)) {
            Ok(()) => Outcome::done(format!(
                "Wrote {} ({} bytes).",
                args.path,
                args.contents.len()
            ))
            .noting("files", json!([args.path])),
            Err(error) => Outcome::failed(format!("can't write {}: {error}", args.path)),
        }
    }

    fn apply_patch(&self, text: &str) -> Outcome {
        let hunks = match patch::parse(text) {
            Ok(hunks) => hunks,
            Err(error) => return Outcome::failed(format!("The patch doesn't parse: {error}")),
        };
        // Compute every new file before writing any, so a patch that fails
        // halfway leaves the workspace as it was.
        let mut writes: Vec<(Target, Option<String>)> = Vec::new();
        let mut touched = Vec::new();
        for hunk in &hunks {
            let planned = match hunk {
                Hunk::Add { path, contents } => self
                    .target(path)
                    .map(|target| vec![(target, Some(contents.clone()))]),
                Hunk::Delete { path } => self.target(path).and_then(|target| {
                    if self.is_file(&target) {
                        Ok(vec![(target, None)])
                    } else {
                        Err(format!("{path} is not a file"))
                    }
                }),
                Hunk::Update {
                    path,
                    move_to,
                    chunks,
                } => self.target(path).and_then(|source| {
                    let original = self
                        .load(&source)
                        .map_err(|error| format!("can't read {path}: {error}"))?;
                    let updated = patch::apply(&original, chunks)
                        .map_err(|error| format!("{path}: {error}"))?;
                    match move_to {
                        Some(to) => {
                            let target = self.target(to)?;
                            Ok(vec![(target, Some(updated)), (source, None)])
                        }
                        None => Ok(vec![(source, Some(updated))]),
                    }
                }),
            };
            match planned {
                Ok(planned) => writes.extend(planned),
                Err(why) => {
                    return Outcome::failed(format!(
                        "The patch was not applied, and nothing changed: {why}"
                    ));
                }
            }
            touched.push(match hunk {
                Hunk::Add { path, .. } => format!("A {path}"),
                Hunk::Delete { path } => format!("D {path}"),
                Hunk::Update {
                    path,
                    move_to: Some(to),
                    ..
                } => format!("M {path} -> {to}"),
                Hunk::Update { path, .. } => format!("M {path}"),
            });
        }
        for (path, contents) in &writes {
            if let Err(error) = self.store(path, contents.as_deref()) {
                return Outcome::failed(format!(
                    "The patch stopped partway at {}: {error}",
                    Self::named(path)
                ));
            }
        }
        Outcome::done(format!("Applied:\n{}", touched.join("\n"))).noting("files", json!(touched))
    }
}

fn write(path: &Path, contents: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)
}

fn parse<T: for<'de> Deserialize<'de>>(arguments: &str) -> Result<T, Box<Outcome>> {
    serde_json::from_str(arguments).map_err(|error| {
        Box::new(Outcome::refused(format!(
            "The arguments don't fit the tool: {error}"
        )))
    })
}

#[derive(Deserialize)]
struct RunCommand {
    command: String,
    timeout_seconds: Option<u64>,
}

#[derive(Deserialize)]
struct ReadFile {
    path: String,
    start_line: Option<usize>,
    max_lines: Option<usize>,
}

#[derive(Deserialize)]
struct ApplyPatch {
    patch: String,
}

#[derive(Deserialize)]
struct WriteFile {
    path: String,
    contents: String,
}

/// Removes from a model command's environment every credential and the
/// host's policy manifest, so a model that runs `env` sees neither. Coder
/// One's own shell does the same (`coder_one::shell::is_credential`).
pub fn withhold_credentials(command: &mut std::process::Command) {
    for (name, _) in std::env::vars_os() {
        if name.to_str().is_some_and(is_withheld) {
            command.env_remove(&name);
        }
    }
}

/// A variable a model's command must not see: a named credential, any
/// `*_API_KEY`, `*_TOKEN`, or `*_SECRET`, the Codex login's path, or the
/// host's policy manifest.
pub fn is_withheld(name: &str) -> bool {
    const NAMED: &[&str] = &[
        "OPENAGENTS_API_KEY",
        "TYPESAFE_API_KEY",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "OPENAI_API_KEY",
        "CODEX_AUTH_JSON_PATH",
        "CODER_ONE_POLICY",
    ];
    let upper = name.to_ascii_uppercase();
    NAMED.contains(&upper.as_str())
        || upper.ends_with("_API_KEY")
        || upper.ends_with("_TOKEN")
        || upper.ends_with("_SECRET")
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_model_command_never_sees_a_credential_or_the_policy() {
        for name in [
            "OPENAGENTS_API_KEY",
            "TYPESAFE_API_KEY",
            "GITHUB_TOKEN",
            "X_SECRET",
            "CODER_ONE_POLICY",
        ] {
            assert!(super::is_withheld(name), "{name}");
        }
        for name in ["PATH", "HOME", "LANG", "WORKSPACE"] {
            assert!(!super::is_withheld(name), "{name}");
        }
    }

    use super::*;

    fn workspace() -> (tempfile::TempDir, Workspace) {
        let dir = tempfile::tempdir().unwrap();
        let workspace = Workspace::new(dir.path()).unwrap();
        (dir, workspace)
    }

    #[test]
    fn every_declaration_is_a_strict_function() {
        let tools = declarations();
        let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
        assert_eq!(
            names,
            [
                "run_command",
                "read_file",
                "apply_patch",
                "write_file",
                "finish"
            ]
        );
        for tool in &tools {
            assert_eq!(tool["type"], "function");
            assert_eq!(tool["strict"], true);
            let properties = tool["parameters"]["properties"].as_object().unwrap();
            assert_eq!(
                tool["parameters"]["required"].as_array().unwrap().len(),
                properties.len()
            );
        }
    }

    #[test]
    fn paths_stay_inside_the_workspace() {
        let (dir, workspace) = workspace();
        assert!(workspace.resolve("a/b.txt").is_ok());
        assert!(workspace.resolve("a/../b.txt").is_ok());
        assert!(workspace.resolve("../escape").is_err());
        assert!(workspace.resolve("/etc/passwd").is_err());
        let inside = workspace.root().join("x.txt");
        assert!(workspace.resolve(inside.to_str().unwrap()).is_ok());
        std::os::unix::fs::symlink("/etc", dir.path().join("out")).unwrap();
        assert!(workspace.resolve("out/passwd").is_err());
    }

    #[tokio::test]
    async fn file_tools_read_write_and_patch() {
        let (_dir, workspace) = workspace();
        let wrote = workspace
            .call(
                "write_file",
                r#"{"path":"src/a.txt","contents":"one\ntwo\nthree\n"}"#,
            )
            .await;
        assert_eq!(wrote.status, atif::Outcome::Completed);
        let read = workspace
            .call(
                "read_file",
                r#"{"path":"src/a.txt","start_line":2,"max_lines":1}"#,
            )
            .await;
        assert!(read.output.starts_with("     2\ttwo\n"));
        assert!(read.output.contains("[lines 2-2 of 3]"));
        let patch =
            "*** Begin Patch\n*** Update File: src/a.txt\n two\n-three\n+THREE\n*** End Patch";
        let patched = workspace
            .call("apply_patch", &json!({ "patch": patch }).to_string())
            .await;
        assert_eq!(
            patched.status,
            atif::Outcome::Completed,
            "{}",
            patched.output
        );
        assert_eq!(
            std::fs::read_to_string(workspace.root().join("src/a.txt")).unwrap(),
            "one\ntwo\nTHREE\n"
        );
    }

    #[tokio::test]
    async fn a_failing_patch_changes_nothing() {
        let (_dir, workspace) = workspace();
        std::fs::write(workspace.root().join("a.txt"), "keep\n").unwrap();
        let patch = "*** Begin Patch\n*** Add File: new.txt\n+x\n\
                     *** Update File: a.txt\n-absent\n+y\n*** End Patch";
        let outcome = workspace
            .call("apply_patch", &json!({ "patch": patch }).to_string())
            .await;
        assert_eq!(outcome.status, atif::Outcome::Failed);
        assert!(!workspace.root().join("new.txt").exists());
    }

    #[tokio::test]
    async fn malformed_calls_are_refused_not_fatal() {
        let (_dir, workspace) = workspace();
        let unknown = workspace.call("delete_everything", "{}").await;
        assert_eq!(unknown.status, atif::Outcome::Cancelled);
        let bad = workspace.call("read_file", "{\"nope\":1}").await;
        assert_eq!(bad.status, atif::Outcome::Cancelled);
        let finish = workspace
            .call("finish", r#"{"status":"done","summary":"s","answer":"42"}"#)
            .await;
        assert_eq!(finish.finish.unwrap().answer, "42");
    }

    #[tokio::test]
    async fn a_command_runs_in_the_root_and_cannot_write_outside_it() {
        let (_dir, workspace) = workspace();
        let outside = tempfile::tempdir().unwrap();
        let probe = coder_boundary::Boundary::writing(workspace.root()).build();
        if let Err(error) = probe {
            eprintln!("skipped: no enforced boundary on this host ({error})");
            return;
        }
        let ran = workspace
            .call(
                "run_command",
                r#"{"command":"pwd && echo hi > made.txt","timeout_seconds":null}"#,
            )
            .await;
        assert_eq!(ran.status, atif::Outcome::Completed, "{}", ran.output);
        assert!(ran.output.contains(workspace.root().to_str().unwrap()));
        assert!(workspace.root().join("made.txt").exists());
        let target = outside.path().join("escaped.txt");
        let command = format!("echo x > {}", target.display());
        let denied = workspace
            .call(
                "run_command",
                &json!({ "command": command, "timeout_seconds": 10 }).to_string(),
            )
            .await;
        assert_eq!(denied.status, atif::Outcome::Failed, "{}", denied.output);
        assert!(!target.exists());
    }

    /// A sealed session's `gh` refuses, Git still works on the checkout,
    /// and an offline seal leaves the command only loopback.
    #[tokio::test]
    async fn a_sealed_command_has_no_github_and_no_network() {
        let (_dir, workspace) = workspace();
        if let Err(error) = coder_boundary::Boundary::writing(workspace.root())
            .offline()
            .build()
        {
            eprintln!("skipped: no enforced boundary on this host ({error})");
            return;
        }
        let seal_dir = tempfile::tempdir().unwrap();
        let workspace = workspace.sealed_by(crate::Seal::create(seal_dir.path(), true).unwrap());
        let gh = workspace
            .call(
                "run_command",
                r#"{"command":"gh issue view 9450","timeout_seconds":10}"#,
            )
            .await;
        assert_eq!(gh.status, atif::Outcome::Failed, "{}", gh.output);
        assert!(gh.output.contains(crate::seal::GH_REFUSAL), "{}", gh.output);
        let git = workspace
            .call(
                "run_command",
                r#"{"command":"git init -q . && git -c user.name=t -c user.email=t@t commit -q --allow-empty -m m && git log --oneline | wc -l","timeout_seconds":20}"#,
            )
            .await;
        assert_eq!(git.status, atif::Outcome::Completed, "{}", git.output);
        if cfg!(target_os = "linux") {
            let net = workspace
                .call(
                    "run_command",
                    r#"{"command":"awk 'NR > 2 { print $1 }' /proc/net/dev","timeout_seconds":10}"#,
                )
                .await;
            assert_eq!(net.status, atif::Outcome::Completed, "{}", net.output);
            assert_eq!(net.output.lines().skip(1).collect::<Vec<_>>(), ["lo:"]);
        }
    }

    /// A session with confined reads runs commands on its own files and
    /// the system's programs, and can't read a directory beside it.
    #[tokio::test]
    async fn a_read_confined_session_reads_only_its_own_files() {
        let (_dir, workspace) = workspace();
        let task = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        std::fs::write(task.path().join("instruction.md"), "the task\n").unwrap();
        std::fs::write(other.path().join("secret.txt"), "a candidate\n").unwrap();
        std::fs::write(workspace.root().join("spec.json"), "{}\n").unwrap();
        if let Err(error) = coder_boundary::Boundary::writing(workspace.root())
            .confining_reads()
            .build()
        {
            eprintln!("skipped: no enforced boundary on this host ({error})");
            return;
        }
        let seal_dir = tempfile::tempdir().unwrap();
        let workspace = workspace.sealed_by(
            crate::Seal::create(seal_dir.path(), true)
                .unwrap()
                .with_read_scope(crate::seal::ReadScope {
                    readable: vec![task.path().to_path_buf()],
                    writable: Vec::new(),
                    environment: Vec::new(),
                }),
        );
        assert!(workspace.confines_reads());
        let inside = workspace
            .call(
                "run_command",
                &json!({
                    "command": format!(
                        "pwd && cat spec.json && cat {}/instruction.md && echo x > made.txt",
                        task.path().display()
                    ),
                    "timeout_seconds": 10
                })
                .to_string(),
            )
            .await;
        assert_eq!(inside.status, atif::Outcome::Completed, "{}", inside.output);
        assert!(inside.output.contains("the task"), "{}", inside.output);
        assert!(
            inside.output.contains(workspace.root().to_str().unwrap()),
            "{}",
            inside.output
        );
        assert_eq!(inside.extra["reads"], json!("confined"));
        assert!(workspace.root().join("made.txt").exists());
        let outside = workspace
            .call(
                "run_command",
                &json!({
                    "command": format!("cat {}/secret.txt", other.path().display()),
                    "timeout_seconds": 10
                })
                .to_string(),
            )
            .await;
        assert_eq!(outside.status, atif::Outcome::Failed, "{}", outside.output);
        assert!(
            !outside.output.contains("a candidate"),
            "{}",
            outside.output
        );
        let gh = workspace
            .call(
                "run_command",
                r#"{"command":"gh issue view 1","timeout_seconds":10}"#,
            )
            .await;
        assert!(gh.output.contains(crate::seal::GH_REFUSAL), "{}", gh.output);
    }

    /// A task container can't confine reads, so a read-confined session
    /// there refuses every command instead of running it with reads open.
    #[tokio::test]
    async fn a_task_container_refuses_confined_reads() {
        let (_dir, workspace) = workspace();
        let workspace = workspace
            .isolated_by(Isolation::TaskContainer)
            .confining_reads(Vec::new());
        let ran = workspace
            .call(
                "run_command",
                r#"{"command":"echo hi > made.txt","timeout_seconds":10}"#,
            )
            .await;
        assert_eq!(ran.status, atif::Outcome::Cancelled, "{}", ran.output);
        assert!(ran.output.contains("can't limit what a command reads"));
        assert!(!workspace.root().join("made.txt").exists());
    }

    #[tokio::test]
    async fn a_read_only_session_reads_and_changes_nothing() {
        let (_dir, workspace) = workspace();
        let workspace = workspace.isolated_by(Isolation::ReadOnly);
        std::fs::write(workspace.root().join("a.txt"), "keep\n").unwrap();
        let read = workspace.call("read_file", r#"{"path":"a.txt"}"#).await;
        assert_eq!(read.status, atif::Outcome::Completed, "{}", read.output);
        let patch = "*** Begin Patch\n*** Add File: b.txt\n+x\n*** End Patch";
        for (name, arguments) in [
            (
                "write_file",
                r#"{"path":"a.txt","contents":"gone"}"#.to_string(),
            ),
            ("apply_patch", json!({ "patch": patch }).to_string()),
        ] {
            let refused = workspace.call(name, &arguments).await;
            assert_eq!(refused.status, atif::Outcome::Cancelled, "{name}");
            assert!(refused.output.contains("read-only"), "{}", refused.output);
        }
        assert_eq!(
            std::fs::read_to_string(workspace.root().join("a.txt")).unwrap(),
            "keep\n"
        );
        assert!(!workspace.root().join("b.txt").exists());
        if let Err(error) = coder_boundary::Boundary::readonly().build() {
            eprintln!("skipped the command half: no enforced boundary on this host ({error})");
            return;
        }
        let ran = workspace
            .call(
                "run_command",
                r#"{"command":"cat a.txt; echo x > made.txt","timeout_seconds":10}"#,
            )
            .await;
        assert!(ran.output.contains("keep"), "{}", ran.output);
        assert_eq!(ran.status, atif::Outcome::Failed, "{}", ran.output);
        assert!(!workspace.root().join("made.txt").exists());
    }

    /// A remote that holds files in memory and records the commands it
    /// was asked to run.
    #[derive(Debug, Default)]
    struct Held {
        files: std::sync::Mutex<std::collections::BTreeMap<String, Vec<u8>>>,
        ran: std::sync::Mutex<Vec<String>>,
        online: bool,
    }

    impl crate::remote::Remote for Held {
        fn word(&self) -> &'static str {
            "held"
        }
        fn root(&self) -> &str {
            "/w"
        }
        fn offline(&self) -> bool {
            !self.online
        }
        fn run(&self, command: &str, _wall: Duration) -> crate::remote::Running<'_> {
            self.ran.lock().unwrap().push(command.to_string());
            Box::pin(async {
                supervise::Ended {
                    ending: supervise::Ending::Exited(Some(0)),
                    stdout: supervise::Captured {
                        text: "ran there".to_string(),
                        bytes: 9,
                        truncated: false,
                    },
                    stderr: supervise::Captured::default(),
                    elapsed: Duration::ZERO,
                    memory: None,
                }
            })
        }
        fn read(&self, path: &str, _max: usize) -> Result<Vec<u8>, String> {
            self.files
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| format!("{path}: no such file"))
        }
        fn write(&self, path: &str, contents: Option<&[u8]>) -> Result<(), String> {
            let mut files = self.files.lock().unwrap();
            match contents {
                Some(bytes) => {
                    files.insert(path.to_string(), bytes.to_vec());
                }
                None => {
                    files.remove(path);
                }
            }
            Ok(())
        }
    }

    /// In a remote, every tool acts there: the host's root stays empty.
    #[tokio::test]
    async fn a_remote_session_runs_every_tool_there() {
        let (_dir, workspace) = workspace();
        let held = Arc::new(Held::default());
        let workspace = workspace
            .isolated_by(Isolation::TaskContainer)
            .confining_reads(Vec::new())
            .in_remote(held.clone());
        let wrote = workspace
            .call(
                "write_file",
                r#"{"path":"/w/a.py","contents":"one\ntwo\n"}"#,
            )
            .await;
        assert_eq!(wrote.status, atif::Outcome::Completed, "{}", wrote.output);
        let patch = "*** Begin Patch\n*** Update File: a.py\n@@\n one\n-two\n+three\n*** End Patch";
        let patched = workspace
            .call("apply_patch", &json!({ "patch": patch }).to_string())
            .await;
        assert_eq!(
            patched.status,
            atif::Outcome::Completed,
            "{}",
            patched.output
        );
        let read = workspace.call("read_file", r#"{"path":"a.py"}"#).await;
        assert!(read.output.contains("three"), "{}", read.output);
        let ran = workspace
            .call("run_command", r#"{"command":"ls","timeout_seconds":5}"#)
            .await;
        assert_eq!(ran.status, atif::Outcome::Completed, "{}", ran.output);
        assert_eq!(ran.extra["boundary"], json!("held"));
        assert_eq!(ran.extra["reads"], json!("remote"));
        assert_eq!(*held.ran.lock().unwrap(), vec!["ls".to_string()]);
        let outside = workspace
            .call("read_file", r#"{"path":"/app/out.txt"}"#)
            .await;
        assert_eq!(
            outside.status,
            atif::Outcome::Cancelled,
            "{}",
            outside.output
        );
        assert_eq!(std::fs::read_dir(workspace.root()).unwrap().count(), 0);
    }

    /// An offline seal refuses a command in a remote that has a network.
    #[tokio::test]
    async fn an_offline_seal_refuses_a_remote_with_a_network() {
        let (_dir, workspace) = workspace();
        let seal_dir = tempfile::tempdir().unwrap();
        let held = Arc::new(Held {
            online: true,
            ..Held::default()
        });
        let workspace = workspace
            .sealed_by(crate::Seal::create(seal_dir.path(), true).unwrap())
            .in_remote(held.clone());
        let ran = workspace
            .call("run_command", r#"{"command":"ls","timeout_seconds":5}"#)
            .await;
        assert_eq!(ran.status, atif::Outcome::Cancelled, "{}", ran.output);
        assert!(held.ran.lock().unwrap().is_empty());
    }
}
