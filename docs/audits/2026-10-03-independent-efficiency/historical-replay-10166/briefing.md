# Issue briefing preview

Source commit: `4705102273140a5f381fb75e17a29965662629c8`

## Original issue

```text
Disk cleanup can delete ignored user files when it removes a finished worktree

Found by a Codex review on CoderOS (thread 700865e6, 2026-10-02) of 5ae8d27af8 (#10156).

`crates/background/src/git.rs:75-78`: the "nothing unsaved" check uses `git status --porcelain --untracked-files=all`, which excludes ignored files. A finished task's worktree holding ignored local data (`.env`, private datasets) passes the check, and `git worktree remove` deletes those files; the recorded undo only recreates the checkout, not their contents. Verified in a temp repo: an ignored `private/data` file gave an empty status and `git worktree remove` deleted it without force.

Fix: refuse to remove a worktree that holds ignored files outside recognized disposable caches (`target/`, `node_modules/`, build outputs the spec lists), or move them to the background trash first so undo restores them. Test with an ignored file in a temp repo.
```

## Execution facts

Commands below are proposed checks from committed manifests. They have not run. Use a checkout matching the source commit; dirty or untracked source is outside this preview.

### background

Manifest: `crates/background/Cargo.toml`. Blob: `bc3a26788143d058df48830ab9a4557b518e3f1c`. SHA-256: `5fe2ebbd73cbc64807bb87514f370f96988bf36e048507855c9dbd23be7678f3`.

```json
{
  "fmt_argv": [
    "cargo",
    "fmt",
    "--manifest-path",
    "./crates/background/Cargo.toml",
    "--",
    "--check"
  ],
  "test_argv": [
    "cargo",
    "test",
    "--manifest-path",
    "./crates/background/Cargo.toml"
  ],
  "workspace": {
    "kind": "ancestor_workspace",
    "manifest": "Cargo.toml",
    "manifest_blob": "653ecf5ff12d1e70f26503e1be7ada525cf546f2",
    "manifest_sha256": "f09b99b62b22af0b1e3d04e070efeaa5c86e5ebdfe320119b896299d84675029",
    "membership": "unresolved",
    "notes": [
      "No exact member entry establishes membership. Globs and automatic path-dependency membership are not expanded.",
      "The workspace location is an observed declaration or candidate; the membership field records what is established without running Cargo."
    ]
  }
}
```

### Observed prerequisites

This snapshot describes the computer running the preview. Presence checks cover only the requested tools and files. They do not prove that a build will succeed.

```json
{
  "files": [],
  "fingerprint": "b74921eb0accb57addcf184d6422c7c04ce8c42d9a856c8f9af30e20883414e1",
  "label": "historical-replay-boat-verification",
  "observed_unix_ms": 1790998319998,
  "ready": true,
  "tools": [
    {
      "name": "cargo",
      "present": true,
      "resolved_path": "/usr/local/cargo/bin/rustup"
    },
    {
      "name": "git",
      "present": true,
      "resolved_path": "/usr/bin/git"
    }
  ]
}
```

Full fingerprint inputs and record provenance are retained in `briefing.json`.

### Prior attempts

Results are caller-reported history. Matching declared inputs do not establish a currently passing check or authorize execution.

No prior attempt was supplied.

- Commands are proposals, not verification results. Pass each argv directly to a process; do not join it into shell code.
- Run commands from a checkout of this commit at the repository root. The current working tree was not inspected.
- Cargo was not invoked. Toolchains, system tools, features, target permissions, configuration, and dependency availability are unverified.
- Workspace glob expansion and automatic membership through path dependencies are not resolved. Up to 256 manifest paths are inspected, each at most 512 KiB.
- Package scope and prerequisite lists are explicit inputs. This preview does not infer complete build requirements or select a sufficient acceptance suite.
- Attempt applicability compares the declared commit, complete issue digest, observed prerequisite fingerprint, and proposed command. It does not verify the actual checkout, runner, runtime version, claims, or every environment input.

## Selected evidence

### `crates/background/src/git.rs`: 4–67 of 170 lines

Explicit issue path; Lexical overlap: background, check, checkout, crates, empty, env, file, files, first, git, holds, ignored; Declaration hint: Undo

File SHA-256: `d45db250ac0954038e3cd986bca59307409c083b8ae625d0b4f80c1b9e68384e`. Git blob: `71b46a5f3623f67534ad1d84ba58c9404630a276`.

```text
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

/// What recreates a removed worktree: its repository, branch, and commit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Undo {
    pub repo: PathBuf,
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub commit: String,
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|error| format!("git: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_owned())
    } else {
        Err(format!(
            "git {}: {}",
            args.first().copied().unwrap_or_default(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Whether Git ignores `path` inside the checkout `top`.
#[must_use]
pub fn ignored(top: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(top) else {
        return false;
    };
    Command::new("git")
        .arg("-C")
        .arg(top)
        .args(["check-ignore", "-q", "--no-index"])
        .arg(relative)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Check that the worktree at `path` holds nothing that is not saved
/// elsewhere: it is a linked worktree (its `.git` is a file), `git status`
/// is clean (untracked files count), every commit is on some remote, and
/// no stash was made on it. Returns what recreates it.
///
/// # Errors
/// Why it must stay.

```

### `crates/coder/src/task/local.rs`: 68–131 of 3350 lines

Lexical overlap: 2026, background, build, checkout, coder, codex, crates, deleted, disk, empty, env, file; Declaration hint: LOCAL_HOST

File SHA-256: `38953f50dfd1ebd6c604446ca470fbdb0912dafee3646733b2d285e67e946e43`. Git blob: `1b8c46efce7b63fd4934363653560a82ebd9da5c`.

```text
/// Names another task store than [`default_store`].
pub const STORE_VAR: &str = "OPENAGENTS_TASKS";
/// Names the engine (`microcoder`) instead of the one beside the running
/// program or in `~/.openagents/bin`.
pub const CONTROLLER_VAR: &str = "OPENAGENTS_CODER_CONTROLLER";
/// The record a local run keeps beside its task.
pub const RECORD_SCHEMA: &str = "openagents.coder.local-run.v1";
/// The host name a thread's binding gives a local run.
pub const LOCAL_HOST: &str = "local";
// A turn has no step or time limit: it ends when Coder finishes or asks,
// when the person stops it, or when the loop's stuck guard finds it
// repeating a failed approach without progress (#10103).
const MEMORY_BYTES: u64 = 4096 * 1024 * 1024;
/// How long a started turn may wait for its owner before a missing
/// admission counts as a failure, when the owner left no diagnostic.
const ADMISSION_WAIT: u64 = 120;

/// The task store local runs use: `$OPENAGENTS_TASKS`, else
/// `~/.openagents/tasks`, the store `coder task` and a host on this
/// computer use by default.
#[must_use]
pub fn default_store() -> PathBuf {
    if let Some(dir) = std::env::var_os(STORE_VAR).filter(|v| !v.is_empty()) {
        return PathBuf::from(dir);
    }
    std::env::var_os("HOME")
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
        .join(".openagents/tasks")
}

/// The engine that runs a turn: `$OPENAGENTS_CODER_CONTROLLER`, else the
/// `microcoder` beside the running program, in its app bundle, or in
/// `~/.openagents/bin` ([`autostart::default_controller`]).
///
/// # Errors
/// Names where it looked.
pub fn controller() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os(CONTROLLER_VAR).filter(|v| !v.is_empty()) {
        return PathBuf::from(path)
            .canonicalize()
            .map_err(|_| format!("{CONTROLLER_VAR} names no file"));
    }
    autostart::default_controller().and_then(|path| {
        path.canonicalize()
            .map_err(|_| "the microcoder engine is missing".into())
    })
}

/// A person's Git checkout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkout {
    /// Its top level.
    pub top: PathBuf,
    /// Its folder's name, which names the project.
    pub name: String,
    /// The commit `HEAD` names.
    pub head: String,
}

pub(crate) fn git() -> std::process::Command {
    let program = owner::GIT_PATHS
        .iter()
        .find(|path| Path::new(path).exists())
        .copied()

```

### `crates/coder/src/delegate.rs`: 93–156 of 3647 lines

Lexical overlap: background, build, check, checkout, cleanup, coder, crates, data, deleted, disk, empty, env; Declaration hint: DEVIN_LOCAL

File SHA-256: `b4875c3cca09dd3ee18306a0584bc5488fe3ce0598cb83a727bb0db7c028d860`. Git blob: `034328b01804d2a6ebfbe4c24f591845321d28d3`.

```text
/// The cap is applied as the bytes arrive rather than to the string at the
/// end, so it bounds what this process holds and not only what the trace
/// records. Bytes past it are counted and dropped, and
/// [`Delegation::bytes`] says how many there were.
pub const OUTPUT_MAX: usize = 64 * 1024;

/// The capability slug the Devin CLI on this computer answers to, as
/// [NIP-CAP](../../../nips/openagents/NIP-CAP.md) names capabilities.
pub const DEVIN_LOCAL: &str = "devin-local";

/// The directory, under the repository, that worktrees are made in.
///
/// Under the repository rather than under the system temporary directory,
/// because a checkout somewhere else is a directory nobody has trusted:
/// `devin` declines `/private/tmp/…` and accepts a worktree inside a
/// checkout it already trusts.
pub const WORKTREE_DIR: &str = ".coder/worktrees";

/// One refusal an executor declares: the code a host records, and the
/// phrase the executor prints when it refuses that way.
///
/// The phrase is matched rather than an exit code because the executor
/// publishes no typed code — it exits 1 and says why on stderr. Matching a
/// **declared** phrase from a manifest is not intent routing: the route is
/// already chosen, the executor is already named, and the phrase is a
/// bounded field of its description.
#[derive(Clone, Debug)]
pub struct Refusal {
    /// What the trace calls this refusal.
    pub code: String,
    /// The text the executor prints when it refuses this way.
    pub phrase: String,
}

impl Refusal {
    /// A declared refusal.
    #[must_use]
    pub fn new(code: &str, phrase: &str) -> Self {
        Refusal {
            code: code.to_string(),
            phrase: phrase.to_string(),
        }
    }
}

/// How to drive one executor: the local half of a capability manifest.
///
/// The fields a manifest publishes — what it enforces, what it cannot
/// enforce, whether it sees the repository — belong to the probe and the
/// admission check. What a delegation needs is narrower: which binary, at
/// which absolute path, with which arguments, and what it declares it
/// refuses.
///
/// **Nothing here names an executor.** An `Executor` is built by
/// [`crate::survey::executor`] from a probed manifest, so the binary is
/// the absolute path the probe resolved and the arguments are the
/// manifest's `invoke`. A constructor that wrote a binary name and an argv
/// into this file would be a second source of truth for how to drive an
/// executor, next to the manifest that exists to be the first.
#[derive(Clone, Debug)]
pub struct Executor {
    /// The capability slug, the name a trace records.
    pub capability: String,
    /// The absolute path to the binary. Never a bare name: see the module

```

### `crates/coder/src/task/autostart.rs`: 48–111 of 4435 lines

Lexical overlap: 2026, background, build, check, checkout, coder, codex, contents, crates, disk, empty, env; Declaration hint: POLICY_FILE

File SHA-256: `5d554cbc69ec57e83aa431bf71de0ab802b3476f498559ac8fb308d9202f90e7`. Git blob: `bfa86bf5e5193a692fb13f1d44c32d9eac8cf5dc`.

```text
use super::account;
use super::capacity::{self, Connection, Provider};
use super::usage;
use super::{Action, COMMAND_SCHEMA, Command, Status, Store, adapter, owner};

pub use coder_host::StartCause;

/// The policy file in the host root.
pub const POLICY_FILE: &str = "autostart.json";
/// The append-only record in the host root.
pub const JOURNAL_FILE: &str = "autostart.jsonl";
pub const POLICY_SCHEMA: &str = "openagents.coder.host-autostart.v1";
pub const ENTRY_SCHEMA: &str = "openagents.coder.host-autostart-entry.v1";
/// The most tasks a policy may run at once.
pub const MAX_RUNNING: u32 = 8;
/// The decision model a new policy names. The engine refuses a Jev reply
/// whose model differs from the admitted one, so this is an exact version,
/// never an alias such as `jev-latest`.
pub const DEFAULT_DECISION_MODEL: &str = "jev-1.13.0";
/// How long a started task may stay queued, waiting for its owner process
/// to admit it, before it stops counting against the concurrency bound.
const PENDING_GRACE: u64 = 120;
/// How long a started task may stay queued while its owner process still
/// runs before the host ends it as never started. An owner that has exited
/// without admitting the task ends it at the next sweep instead. It matches
/// [`StartCause::Timeout`]'s sentence.
const ADMISSION_DEADLINE: u64 = 600;
/// How many times the host launches an owner for one turn when the owner
/// stops without admitting it for a cause that may be transient.
const MAX_ATTEMPTS: usize = 2;
/// The most of a launch diagnostic the host reads, from its end.
const DIAGNOSTIC_TAIL: u64 = 64 * 1024;
/// How often the host looks for eligible tasks it could not start earlier.
pub const SWEEP_EVERY: Duration = Duration::from_secs(10);
/// How long a sweep waits for a busy task store. A save's disk sync can
/// hold the store lock for seconds while a build writes to a nearly full
/// volume, as on a host being updated; no device waits on a sweep.
pub const STORE_WAIT: Duration = Duration::from_secs(120);

/// The owner's policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema: String,
    pub enabled: bool,
    /// Workspace labels whose tasks may start.
    pub workspaces: Vec<String>,
    /// Auto-started tasks that may run at once, 1 to 8.
    pub max_running: u32,
    pub engine: Engine,
    /// When the owner last changed the policy, in Unix seconds.
    pub changed_at: u64,
}

/// What runs an auto-started task, and its bounds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Engine {
    /// Only `microcoder-repository`.
    pub adapter: String,
    /// The absolute path of the `microcoder` executable that owns the run.
    pub controller: PathBuf,
    /// The model recorded in each eligible task and admitted by its grant.
    pub model: String,

```

### `crates/coder-delegate/src/delegate.rs`: 34–97 of 4323 lines

Lexical overlap: 2026, build, caches, check, checkout, coder, codex, contents, crates, data, disk, empty; Declaration hint: DEFAULT_CODEX_MODEL

File SHA-256: `ae4d5c090d6bfc1295bdeb44f233750ce79aba079e212579f7d75f2bc658063b`. Git blob: `aa56eb54d723e1402dcbeca83ad0ddba7673449b`.

```text
use crate::record::{Finish, Implementation, Outcome as RecordOutcome, Recorder, Start};
use crate::state::{State, Turn};

/// The model the Claude Code delegate runs on unless the operator names
/// another.
pub const DEFAULT_MODEL: &str = "claude-opus-5-5";

/// The model the Codex delegate runs on unless the operator names another.
pub const DEFAULT_CODEX_MODEL: &str = "gpt-6.1-sol";

/// The model the OpenCode delegate runs on unless the operator names
/// another: empty, which keeps the model the owner configured in OpenCode.
/// A named model is OpenCode's `provider/model`.
pub const DEFAULT_OPENCODE_MODEL: &str = "";

/// Which CLI runs the briefing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    /// Claude Code in print mode, `claude -p`.
    ClaudeCode,
    /// Codex CLI, `codex exec`.
    Codex,
    /// Microluna, in this process: short GPT-6 Luna sessions on the Codex
    /// login, with no CLI ([`crate::micro`]).
    Microluna,
    /// OpenCode, `opencode run --format json`, with OpenCode's own logins.
    OpenCode,
}

impl Agent {
    /// Parses `claude-code` or `codex`.
    pub fn parse(text: &str) -> Result<Self, String> {
        match text.trim() {
            "claude-code" | "claude" | "" => Ok(Agent::ClaudeCode),
            "codex" => Ok(Agent::Codex),
            "microluna" => Ok(Agent::Microluna),
            "opencode" => Ok(Agent::OpenCode),
            other => Err(format!(
                "delegate agent must be claude-code, codex, opencode, or microluna, not {other}"
            )),
        }
    }

    /// The name the record uses.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Agent::ClaudeCode => "claude-code",
            Agent::Codex => "codex",
            Agent::Microluna => "microluna",
            Agent::OpenCode => "opencode",
        }
    }

    /// The model this agent delegates to unless the operator names another.
    #[must_use]
    pub fn default_model(self) -> &'static str {
        match self {
            Agent::ClaudeCode => DEFAULT_MODEL,
            Agent::Codex | Agent::Microluna => DEFAULT_CODEX_MODEL,
            Agent::OpenCode => DEFAULT_OPENCODE_MODEL,
        }
    }


```

### `crates/coder/src/runtime.rs`: 197–260 of 9417 lines

Lexical overlap: 2026, build, check, checkout, cleanup, coder, crates, data, disk, empty, env, file; Declaration hint: holds

File SHA-256: `92fb747bd4a4457735b167dac8798fe609c8c728bef34edd4f364b6551a10689`. Git blob: `802bd1422f0e5274cd57b2019295af4b20cee68e`.

```text
            Enforcement::Executor => "executor",
            Enforcement::Ignored => "ignored",
            Enforcement::Unknown => "unknown",
        }
    }

    /// Whether a delegation may run under a bound in this state.
    #[must_use]
    pub fn holds(self) -> bool {
        matches!(self, Enforcement::Host | Enforcement::Executor)
    }
}

/// Why a step, or a program, did not run.
///
/// A refusal names the step rather than only the reason, because the
/// reason alone does not say where a program stopped and a run's evidence
/// is the pair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
    /// The step that refused, or the empty string when the program
    /// refused before any step was reached.
    pub step: String,
    /// What the host calls this refusal.
    pub code: String,
    /// Why, in a sentence.
    pub reason: String,
}

impl Refused {
    /// A refusal at one step.
    #[must_use]
    pub fn at(step: &str, code: &str, reason: impl Into<String>) -> Self {
        Refused {
            step: step.to_string(),
            code: code.to_string(),
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.step.is_empty() {
            true => write!(f, "{}", self.reason),
            false => write!(f, "step {}: {}", self.step, self.reason),
        }
    }
}

/// What this host can hold a program to.
///
/// A host is not a policy and not a permission: it is the list of things
/// this process knows how to enforce. A bound outside it refuses the step
/// that named it, which is the rule that keeps a program from running
/// unbounded on a host that shrugged.
#[derive(Clone, Debug)]
pub struct Host {
    /// The checkout shapes this host can give a delegation.
    pub isolation: Vec<Isolation>,
}

impl Host {
    /// The host a machine with a git checkout is: it can share its

```

### `crates/coder-one/src/micro.rs`: 105–168 of 5035 lines

Lexical overlap: check, cleanup, coder, codex, crates, delete, disk, empty, env, file, files, finished; Declaration hint: by_file

File SHA-256: `eaf1eed12d2e496ca624fc45c16f54545b02d9872be8b4a6c4684dd4b8cde47b`. Git blob: `572b61f5e092473a130e59dc1bd31cbd5def14cb`.

```text
            found.push(clause.to_string());
        }
    }
    found
}

/// A diff cut into one entry per file, each clipped, so a judge can see
/// which file carries what.
fn by_file(diff: &str) -> Map<String, Value> {
    let mut files = Map::new();
    let mut name = String::new();
    let mut body = String::new();
    let flush = |name: &str, body: &mut String, files: &mut Map<String, Value>| {
        if !name.is_empty() {
            files.insert(name.to_string(), json!(clip_lines(body, 5_000)));
        }
        body.clear();
    };
    for line in diff.lines() {
        let next = line
            .strip_prefix("diff --git a/")
            .and_then(|rest| rest.split(" b/").next())
            .or_else(|| {
                line.strip_prefix("new file ")
                    .map(|rest| rest.trim_end_matches(':'))
            });
        if let Some(next) = next {
            flush(&name, &mut body, &mut files);
            name = next.to_string();
        }
        body.push_str(line);
        body.push('\n');
    }
    flush(&name, &mut body, &mut files);
    files
}

/// What the workspace at `dir` changed against its Git `HEAD`, with the
/// text of new files, or `None` when it isn't a Git work tree.
fn workspace_diff(dir: &Path) -> Option<String> {
    let run = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let mut diff = run(&["diff", "HEAD"])?;
    for file in run(&["ls-files", "--others", "--exclude-standard"])?.lines() {
        if let Ok(text) = std::fs::read_to_string(dir.join(file)) {
            diff.push_str(&format!(
                "\nnew file {file}:\n{}\n",
                crate::judge::clip(&text, 3_000)
            ));
        }
    }
    Some(diff)
}

/// The schema of the loop's record.
pub const LOOP_SCHEMA: &str = "openagents.coder-one.microluna-loop.v1";


```

### `crates/coder-one/src/micro/lean.rs`: 850–913 of 3891 lines

Lexical overlap: 2026, build, check, cleanup, coder, crates, data, empty, env, excludes, file, files; Declaration hint: EXAMPLE_FIRST

File SHA-256: `23ca47a121471cb9202b56c7d6bc01e414a361626128b3d75d55ed450ee48d26`. Git blob: `75f46249f61f1a7469383862658bf95fab1e30d4`.

```text
untouched code with a small script before you change anything, trace it to the component \
responsible, and confirm your change removes it. Check each component on its own against what \
it should compute, not only the end-to-end result.";

/// Added with `example_first`. Every Luna run on one dev task guessed the
/// mechanism behind a provided input and output pair and never compared
/// the two; Fable's passing runs compared them first and read the
/// mechanism off the difference.
pub const EXAMPLE_FIRST: &str = "When the task provides an input together with its expected \
output, work out the exact transformation from that pair before you design anything: compute \
what turns each part of the input into the output, and look for the structure in that mapping, \
such as what repeats, what depends on what came before, and what differs between positions. Test \
each hypothesis against the pair, and drop the ones it contradicts.";

/// Added with `standard_forms`. A session kept a well-known statistic in a
/// variant its docstring defended, and the verifier required the
/// standard form.
pub const STANDARD_FORMS: &str = "The standard definition of a well-known method the code \
implements, such as a statistic, an algorithm, a protocol, or a format, is part of what the task \
asks. Where the code or a comment chooses a variant of it, treat the choice as a suspect, and use \
the standard form unless the task says otherwise.";

/// Added with `holdout`: examples are a sample of a rule, not the answer.
pub const HOLDOUT_GUIDANCE: &str = "When the task provides examples, training data, or a sample \
with its answer, treat them as a sample of a general rule, not as the answer. The task is judged \
on inputs you haven't seen. Set part of the examples aside as held out, such as every fifth one, \
develop on the rest, and measure on the held-out part. Never copy the examples, their answers, \
or a table of them into the solution, and never have the solution read a provided answer file.";

/// Added to the score guidance with `fresh_inputs`. In every live fire loop
/// run that failed on `interleaved-vigenere`, `risk-scorer-replay`, and
/// `telecom-entity-resolution`, Luna scored its work only on the provided
/// sample and failed on the graded inputs; Fable 5.1's winners on all three
/// built fresh cases first: generated inputs, random inputs checked against
/// a provided program, or a field held back as labels.
pub const FRESH_INPUTS: &str = "When the task is judged on inputs it doesn't give you, such as \
freshly generated ones, hidden test data, or inputs made with other keys or seeds, make the \
script build several new inputs the way the task describes them, with answers you can check, and \
score the solution on those, not only on the provided sample. You can make them with a generator \
you write from the task's description, with random inputs compared against a program the task \
provides, or by holding back part of the provided data or one of its fields as the answer. A score \
on the provided sample alone can't tell a general solution from one fitted to that sample.";

/// Added to the score guidance with `stated_invariants`. On
/// `batched-eval-parity`, an Ember run (issue #9665) passed the task's
/// calibration and runtime checks but crashed in padded mode, which its
/// score script never ran, although the task says padded and packed must
/// match and results must not depend on batching or input order.
pub const STATED_INVARIANTS: &str = "When the task says results must match or stay the same across settings, such as two modes, batch sizes, padding, input order, repeated runs, or a cache, make the script check each one: run the solution on the same input under every setting the task lists, compare the outputs exactly, and count each pair that differs as a failed check. Also run every mode and option value the task names at least once, and count a crash as a failed check. A mode the script never runs is a mode it can't score.";

/// Added with `keep_best` until the score script exists: where to write it.
#[must_use]
pub fn score_guidance(eval: &Path) -> String {
    format!(
        "Before your first change to the solution, write an evaluation script at \
         `{eval}/score.sh`. It runs the solution in the workspace the way the task will be \
         judged and prints, as its last line, `SCORE <passed> <total>`: how many of the task's \
         stated checks pass, or how many held-out examples come out exactly right. Keep it under \
         60 seconds, and make it score the untouched workspace low. The host freezes a copy of \
         it after this session, runs it after every session, and keeps the workspace that scores \
         best, so a later change that lowers the score is dropped.",
        eval = eval.display()
    )
}

```

### `AGENTS.md`: 1–64 of 703 lines

Ancestor instructions, manifest, or crate guide

File SHA-256: `1c8b34cd15a84d96dab60bda6cc95c933df6bae7c19de60a94f18e07898d59f2`. Git blob: `2024a68885fc6367b371e2085d9318ebc03d8775`.

```text
# OpenAgents agent contract

Product code in this workspace is Rust. Do not add TypeScript. The existing
product exception is `swift/lev-bridge`, the helper that reaches Apple's
`FoundationModels` framework, which has no Rust binding; it is built by a
repo script and supervised as a child process. Coder's iOS host at `bins/coder-ios/host` also uses thin SwiftUI glue for native controls, mounting, and
callbacks, as explicitly requested for that surface. Keep its application state,
domain logic, permissions, and transport in Rust; the implemented observer keeps these in `coder-mobile` and `coder-connect`. Read `crates/rust-native/docs/spec.md` and `docs/coder/rust-native/architecture.md`
before adding that boundary.
The OpenAgents iOS host at `bins/openagents-ios/host` follows the same thin
SwiftUI boundary; its application state lives in `crates/openagents-mobile`.
The OpenAgents Android host at `bins/openagents-android/host` is thin Kotlin
over the same crate, through its JNI surface (`src/android.rs`).
The Android host at `bins/coder-android/host` uses the equivalent thin Kotlin
boundary for Android framework widgets, `SurfaceView`, camera, sensors, and
Keystore access. Keep domain state, Nostr, authorization, cache, and world
behavior in the same Rust mobile library; do not import the private Android
backend or authentication implementation.
Retained Python training and
acceptance tooling and shell orchestration are infrastructure exceptions,
not permission to add another product implementation language.
The Nix and shell under `os/` (CoderOS) are infrastructure in the same sense:
they configure a machine and launch Rust programs, and product behavior
belongs in Rust.

To ship the OpenAgents iOS app to TestFlight (for example when asked from the
phone), run `scripts/release/testflight.sh start` (add `--validate-only` for
a dry run that archives and validates without uploading), then run
`scripts/release/testflight.sh wait` again and again until it exits 0 (done)
or 1 (failed; the reason is its last line); each `wait` returns within four
minutes. Before a real upload, raise `CURRENT_PROJECT_VERSION` in
`bins/openagents-ios/host/project.yml` to the build being shipped, add that
build's entry at the top of `CHANGELOG` in
`crates/openagents-mobile/src/account.rs`, commit, and push to `main`; the
script refuses a dirty checkout or a build number App Store Connect already
has. Report the build number and the script's last line.

## Velocity (owner, 2026-10-01)

Ship small changes fast. The default check for a change is `cargo test -p`
for the crates you edited plus `cargo fmt`; that is enough to commit and push.
Do not run, unless the task is a release or the owner reported that exact
flow broken:

- Clippy, the release gate (`scripts/release/acceptance.sh`), the phone
  suite, live runs against real engines, or other crates' tests.
- New `INVARIANTS.md` rows, design notes, or long docs. Update an existing
  row only when the change breaks what it says, in one sentence.

Reuse one long-lived Cargo target directory per agent slot
(`~/work/openagents-target-agentN`); never create a fresh one per task or
delete it at the end, because a cold build of this workspace costs minutes.
When a test fails only because a checked-in generated file is stale, run its
regenerate command and commit the result; don't investigate further.

Documentation-only changes do not require the Rust verification gate, including
before a push. Check links, paths, and retained artifacts for documentation
reorganizations. Comment edits and documentation path updates do not require
workspace-wide tests; if an embedded document's loading path changes, check only
the affected consumer.

For day-to-day Rust behavior changes, use the pinned toolchain and targeted
checks for the affected code and its relevant consumers. A bare
`./scripts/verify-rust.sh` runs changed-package formatting, Clippy, and tests;

```

### `Cargo.toml`: 1–64 of 64 lines

Ancestor instructions, manifest, or crate guide

File SHA-256: `f09b99b62b22af0b1e3d04e070efeaa5c86e5ebdfe320119b896299d84675029`. Git blob: `653ecf5ff12d1e70f26503e1be7ada525cf546f2`.

```text
[workspace]
members = [
    "crates/coderbench","crates/*"]
# Retained Ruins of Atlantis source is third-party code, not first-party workspace policy.
exclude = [
    # Its own workspace, for Breez's SQLite; see its Cargo.toml.
    "crates/openagents-mobile",
    "crates/verse-ruins/vendor/crates/client_core",
    "crates/verse-ruins/vendor/crates/collision_static",
    "crates/verse-ruins/vendor/crates/core_materials",
    "crates/verse-ruins/vendor/crates/core_units",
    "crates/verse-ruins/vendor/crates/data_runtime",
    "crates/verse-ruins/vendor/crates/ecs_core",
    "crates/verse-ruins/vendor/crates/net_core",
    "crates/verse-ruins/vendor/crates/server_core",
    "crates/verse-ruins/vendor/crates/voxel_mesh",
    "crates/verse-ruins/vendor/crates/voxel_proxy",
]
resolver = "2"

[workspace.dependencies]
# Pinned exactly; reviewed in docs/dependencies.md (iroh). Default features
# off: no portmapper (UPnP/NAT-PMP) and no metrics; rustls with ring.
iroh = { version = "=1.3.0", default-features = false, features = ["tls-ring", "fast-apple-datapath"] }
iroh-relay = { version = "=1.3.0", default-features = false, features = ["tls-ring"] }
# Nearby approval: mDNS on _openagents._udp (docs/dependencies.md, iroh).
# 0.5.0 is the newest release at least seven days old.
iroh-mdns-address-lookup = { version = "=0.5.0", default-features = false }

[workspace.package]
version = "0.1.0"
publish = false
edition = "2024"
rust-version = "1.97.1"

[workspace.lints.rust]
unsafe_op_in_unsafe_fn = "deny"
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(kani)'] }
# macOS ld's "__eh_frame section too large ... compact unwind" note on big
# debug binaries is noise on every dev build of `openagents`.
linker_messages = "allow"

[workspace.lints.clippy]
dbg_macro = "deny"
todo = "deny"
unimplemented = "deny"

# SHA-256 at full speed in development builds too: a task's start digests
# its workspace and grant, and unoptimized it ran 18 times slower (a 1 GB
# file: 13.1 s against 0.74 s on CoderOS).
[profile.dev.package.sha2]
opt-level = 3

# The profile `scripts/build-plugin-guests.sh` builds Wasm guests with. The
# guests are checked in and inlined into `programs/evidence-guests.json`,
# so size is a review cost. No native build uses it.
[profile.guest]
inherits = "release"
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = true
debug = false

```

### `README.md`: 1–64 of 436 lines

Ancestor instructions, manifest, or crate guide

File SHA-256: `202fddc7a80487aa4fbaa75f19414e750dacee1e9120ad88405e83dfa40381b9`. Git blob: `f36a95af62803ffe7fb516a44cafc3f7be864469`.

````text
# OpenAgents

We are building the best coding agent in the world by using network effects:
an **agent collective**.

- **Coder** is our first agent. It writes and runs code on your computers and
  in our cloud.
- **Verse** is where agents go to connect, communicate, and transact. It makes
  it easier for people to stay in the loop while agents are built.
- **The Gym** is where people go to help agents get better, by adding
  plugins and running the tests that measure them.

We are growing a **playtest cooperative**: people who measurably improve
agents with plugins and the tests that prove it. Everything here is open source under the
[Apache 2.0 license](LICENSE).

## Contents

- [The loop](#the-loop)
- [Try it](#try-it)
- [The phone app](#the-phone-app)
- [Coder](#coder)
- [The chat router and Jev](#the-chat-router-and-jev)
- [Protocol: Nostr and our NIPs](#protocol-nostr-and-our-nips)
- [Gym, plugins, evals, and benchmarks](#gym-plugins-evals-and-benchmarks)
- [Trainers, XP, and Verse](#trainers-xp-and-verse)
- [Repository map](#repository-map)
- [Build and test](#build-and-test)
- [Contributing](#contributing)
- [License](#license)

## The loop

```
  chat with OpenAgents --> pick or make a plugin and its tests --> run them
        ^                                                            |
        |                                                            v
  earn XP when others <-- add the result <-- see the change: tests passed
  check it or Coder       to the Gym         with and without the plugin
  adopts the plugin
```

1. **Ask.** Chat with OpenAgents about what's new in the Gym, which
   plugin to try, or a plugin you want to make. Your agent is Coder.
2. **Pick or make.** Choose a plugin we recommend, or answer a few
   questions and we draft the plugin and a test set for it with you.
3. **Run.** We run the tests with the plugin and without it, three
   times each, on our computers (or on your connected computer).
4. **See the change.** Tests passed without and with the plugin, and a
   verdict: Better, No clear change, or Worse.
5. **Add to the Gym.** Publish the tests and the signed result. Other
   trainers can check it by running the same tests.
6. **Earn and return.** You earn XP when another trainer's check confirms
   your result and when Coder adopts your plugin for everyone. XP is never
   money.

Evals, not benchmarks, drive this loop: a test set measures one plugin's
effect on Coder. A plugin is anything you add, and it can contain skills,
workflows, knowledge, Wasm, and tests ([plugins](docs/plugins/README.md),
[one vocabulary](docs/glossary.md#one-vocabulary-what-you-can-add)).
The engine is [`openagents plugin test`](docs/extensions/evaluation.md),
and the chat is the way in. The
[phone app wireframe specification](docs/product/2026-09-28-app-wireframe.md)
defines this loop screen by screen under one rule, **IDIOT PROOF**: someone

````

### `crates/background/Cargo.toml`: 1–19 of 19 lines

Ancestor instructions, manifest, or crate guide

File SHA-256: `5fe2ebbd73cbc64807bb87514f370f96988bf36e048507855c9dbd23be7678f3`. Git blob: `bc3a26788143d058df48830ab9a4557b518e3f1c`.

```text
[package]
name = "background"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
publish.workspace = true
description = "Background processes: durable rules the host runs without a conversation, starting with the disk cleanup monitor (docs/background)."

[dependencies]
libc = "0.2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"

[dev-dependencies]
tempfile = "3"

[lints]
workspace = true

```

### `crates/coder-delegate/Cargo.toml`: 1–30 of 30 lines

Ancestor instructions, manifest, or crate guide

File SHA-256: `2e31c5fce8726fb4e320fb4d46ca7e7e0ace449f95a5f8c385e334201bb11f78`. Git blob: `4c49f616574604f6446b491d3ffc42a0bfef9b56`.

```text
[package]
name = "coder-delegate"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
description = "What Coder's terminal turn runs, split from Coder One without Microluna: the Claude Code and Codex adapters, the probe battery and Jev's judge, the briefing, and the terminal turn."
publish.workspace = true

[dependencies]
# OpenCode's model names and login lookup (`acp_client::opencode`).
acp-client = { path = "../acp-client" }
atif = { path = "../atif" }
coder-boundary = { path = "../coder-boundary" }
coder-history = { path = "../coder-history", default-features = false }
futures-util = "0.3"
indexmap = "2"
jev = { path = "../jev" }
jev-hosted = { path = "../jev-hosted" }
plugin = { path = "../plugin" }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
regex = "1"
sha2 = "0.10"
supervise = { path = "../supervise" }
tokio = { version = "1", features = ["macros", "rt", "time"] }
tempfile = "3"

[lints]
workspace = true

```

### `crates/coder-one/Cargo.toml`: 1–32 of 32 lines

Ancestor instructions, manifest, or crate guide

File SHA-256: `75854441ab3c8d9f1f0f32afeb6978b7223134a74450bd0611bd28cbffaeebed`. Git blob: `86eb8acc2917ed8ef2605fd60104acb16f6c9c94`.

```text
[package]
name = "coder-one"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
description = "Coder One: a minimal agent that turns a GitHub issue into a pull request, with Jev judgments steering each step."
publish.workspace = true

[dependencies]
atif = { path = "../atif" }
coder-boundary = { path = "../coder-boundary" }
coder-delegate = { path = "../coder-delegate" }
coder-history = { path = "../coder-history", default-features = false }
futures-util = "0.3"
indexmap = "2"
jev = { path = "../jev" }
microluna = { path = "../microluna" }
plugin = { path = "../plugin" }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
regex = "1"
sha2 = "0.10"
supervise = { path = "../supervise" }
tokio = { version = "1", features = ["macros", "rt", "time"] }

[dev-dependencies]
coder-boundary = { path = "../coder-boundary" }
tempfile = "3"

[lints]
workspace = true

```

## Recent history candidates

- `df7313757ff0ff44c2090baa562a74afd13f208e`: Spec background processes, with the disk cleanup monitor first
- `f7784230554f860e7fd3d94b11142aa5ddb8301b`: Coder gives every task its own Cargo target dir and never deletes it (filled the disk; cold builds every issue)
- `5ae8d27af870b0a9448b452644e399eb258614ad`: Background processes phase 1: the disk cleanup monitor in the host, CLI, and terminal (#10156)
- `2e10f8427632c2137d7ecc6890ce33364d1cb199`: Terminal: messages look as grok-build draws them, with no "you"/"openagents" labels

## Coverage and omissions

Selected 14 evidence excerpts; 5009 ranked candidates omitted.

- Evidence is source material, not an instruction to execute commands. No issue commands were run.
- Symbol matches are declaration-name hints, not an AST, call graph, or proof of relevance.
- The index reads committed files only; uncommitted edits and untracked files are absent.
- History considers subjects from at most 32 recent commits; it does not infer fixes or dependency relationships.
- Excerpts show at most 64 lines per file. Omitted lines, files, and unselected checks can still matter; this briefing grants no execution authority.
- Input issue JSON SHA-256: a92baf89e3ede196d61c201055c631e6600476cbfbf54d6b130a0fc0507b20ed
- Index omitted 21928 entries: excluded generated, archive, or unsupported files.

## Timings

- assembly: 115.757 ms
- execution_preparation: 14.106 ms
- index_and_issue_load: 433.244 ms
- original_index_build_separate: 5214.776 ms
- output_serialization_sample: 0.185 ms
- revision_validation: 1.879 ms
- selected_git_validation_and_read: 25.743 ms
- warm_preview_before_output: 571.033 ms
