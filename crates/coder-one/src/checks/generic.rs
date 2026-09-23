//! Generic scenarios for an arbitrary task, run against the live
//! workspace.
//!
//! The data, interactive, and cancellation scenarios each need a task of
//! their family. A Terminal-Bench task can be anything, so these scenarios
//! read only what any task states:
//!
//! | Scenario | Expected relation |
//! | --- | --- |
//! | `generic.output` | Each output file a requirement tells the executor to write exists and isn't empty. |
//! | `generic.parse` | Each output file with a structured format parses as that format, and a CSV output starts with the header the task shows. |
//! | `generic.public-command` | A test or check command the instruction names exits 0. |
//! | `generic.claimed-command` | A test command the executor ran and saw pass still passes on the final state. |
//! | `generic.self-report` | Neither the executor's final report nor the outputs it wrote say the result failed. Off unless [`Options::self_report`]. |
//!
//! [`Options::optional_outputs`] changes two output verdicts: an empty file
//! a requirement asks for only when something is needed ("write any
//! dependencies needed to ...") passes, and a missing or malformed output
//! of a requirement the extraction was unsure binds is inconclusive rather
//! than a failure.
//!
//! Commands run in the task's working directory, bounded, with the
//! episode's credentials removed from their environment. Only commands
//! that read as tests or checks are admitted, and only when they name no
//! placeholder, install nothing, reach no network, and write nothing
//! through a redirect. The protected verifier's tests are never looked
//! for: a command that names `/tests` is refused.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::place::Place;
use super::selfreport;
use super::{Bounds, Context, Ineligible, Relation, Scenario, Verdict};
use crate::requirements::{Binding, Kind, Requirement};

/// The scenario kinds this family builds.
pub const KINDS: &[&str] = &[
    "generic.output",
    "generic.parse",
    "generic.public-command",
    "generic.claimed-command",
    "generic.self-report",
];

/// The most claimed commands one check reruns.
const MAX_CLAIMED: usize = 3;

/// The most characters of a command's output an observation keeps.
const OUTPUT_CHARS: usize = 2_000;

/// One command the executor ran, as its session reported it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claimed {
    pub command: String,
    /// The exit code the session saw, when it reported one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
}

/// The live workspace a generic check runs in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    /// The task's working directory, absolute.
    pub dir: String,
    /// Commands the executor sessions ran, in order.
    #[serde(default)]
    pub claimed: Vec<Claimed>,
    /// Seconds each command scenario may run.
    pub command_sec: u64,
    /// The executor's final report, which `generic.self-report` reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<String>,
    /// Which optional behaviors the generic scenarios run.
    #[serde(default, skip_serializing_if = "Options::is_default")]
    pub options: Options,
    /// A replay root standing in for the task's filesystem: absolute paths
    /// resolve under it and commands run in a sandbox rooted at it. `None`
    /// in an episode, where the check runs in the task's own container.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    /// In a replay, the paths whose final state the trial retained: a
    /// missing output elsewhere is unknown, not missing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collected: Vec<String>,
}

impl Workspace {
    /// Where this workspace's files and commands are.
    #[must_use]
    pub fn place(&self) -> Place {
        Place::of(self.root.as_deref())
    }

    /// Whether the state of `path` is known: always in an episode, and in
    /// a replay when the trial retained it or a directory holding it.
    #[must_use]
    pub fn known(&self, path: &str) -> bool {
        if self.root.is_none() || self.collected.is_empty() {
            return true;
        }
        self.collected.iter().any(|c| {
            let c = c.trim_end_matches('/');
            path == c || path.starts_with(&format!("{c}/"))
        })
    }
}

/// The generic scenarios' optional behaviors; all off by default, so a
/// check without them runs as it always has.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    /// Build `generic.self-report`: a failure the executor reports in its
    /// own words, its outputs, or the exit of a command the task names is
    /// a failed check.
    #[serde(default)]
    pub self_report: bool,
    /// Let an optional output be empty, and don't count a missing output
    /// against a requirement the extraction was unsure binds.
    #[serde(default)]
    pub optional_outputs: bool,
    /// Admit only the outputs a requirement tells the executor to write,
    /// not files a program writes at run time or inputs it reads, and
    /// build the behavior scenarios in [`super::behavior`].
    #[serde(default)]
    pub behavior: bool,
}

impl Options {
    /// Whether every option is off.
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Options::default()
    }
}

/// Words that mark a requirement as asking for an output file.
const WRITE_WORDS: &[&str] = &[
    "write", "save", "create", "output", "produce", "generate", "store", "export", "place", "put ",
];

/// Structured formats `generic.parse` reads.
const PARSED: &[&str] = &["json", "jsonl", "csv", "py", "sh"];

fn extension(path: &str) -> Option<&str> {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.').map(|(_, ext)| ext)
}

/// Where a path the task names lives: absolute as written, or under the
/// working directory. `~` and directories are left out.
fn resolve(dir: &Path, path: &str) -> Option<PathBuf> {
    let path = path.trim().trim_start_matches("./");
    if path.is_empty() || path.starts_with('~') || path.ends_with('/') || path.contains('*') {
        return None;
    }
    if path.starts_with('/') {
        // The verifier's own directory is protected.
        if path == "/tests" || path.starts_with("/tests/") {
            return None;
        }
        return Some(PathBuf::from(path));
    }
    Some(dir.join(path))
}

/// Verbs that tell the executor to write a file, for [`asks_executor`].
const WRITE_VERBS: &[&str] = &[
    "write", "save", "create", "produce", "generate", "store", "export", "place", "put", "provide",
    "emit", "submit",
];

/// Words that may come before an imperative write verb without naming
/// someone or something else as its subject.
const LEAD_WORDS: &[&str] = &[
    "please",
    "then",
    "also",
    "and",
    "finally",
    "additionally",
    "first",
    "next",
    "lastly",
    "you",
    "should",
    "must",
    "need",
    "to",
    "will",
    "can",
    "then,",
    "additionally,",
    "finally,",
    "also,",
];

/// Whether a requirement tells the executor to write a file, rather than
/// describing a file a program writes at run time ("Accepted leads should
/// write ...") or naming an input ("`output_format.txt`: naming rules").
/// A requirement that opens with a condition describes run-time behavior.
#[must_use]
pub fn asks_executor(text: &str) -> bool {
    let lower = text
        .trim_start_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase();
    if ["if ", "when ", "whenever ", "unless ", "once "]
        .iter()
        .any(|w| lower.starts_with(w))
    {
        return false;
    }
    let words: Vec<&str> = lower
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric() && c != ','))
        .collect();
    let Some(at) = words.iter().position(|w| {
        WRITE_VERBS.iter().any(|verb| {
            w.strip_prefix(verb)
                .is_some_and(|rest| rest.is_empty() || rest == "s")
        })
    }) else {
        // "must be written to", "is saved as": the file is the subject.
        return [
            "be written",
            "be saved",
            "be stored",
            "be placed",
            "be created",
        ]
        .iter()
        .any(|p| lower.contains(p));
    };
    words[..at]
        .iter()
        .all(|w| w.is_empty() || LEAD_WORDS.contains(w))
        || words[..at]
            .windows(2)
            .any(|w| w == ["you", "should"] || w == ["you", "must"])
}

/// The directory a requirement puts its outputs in, when it names exactly
/// one: "save it inside `/results/`".
#[must_use]
pub fn named_directory(text: &str) -> Option<String> {
    let mut found: Vec<String> = Vec::new();
    let tokens: Vec<&str> = text.split_whitespace().collect();
    for (i, raw) in tokens.iter().enumerate() {
        let token = raw
            .trim_start_matches(['`', '"', '\'', '(', '*'])
            .trim_end_matches(|c: char| {
                matches!(c, '`' | '"' | '\'' | ',' | ';' | ':' | ')' | '*' | '.')
            });
        if !token.starts_with('/') || token.len() < 2 || token.contains('*') {
            continue;
        }
        let last = token.trim_end_matches('/').rsplit('/').next().unwrap_or("");
        let placed = i > 0
            && ["inside", "in", "into", "under", "to", "at"].contains(
                &tokens[i - 1]
                    .trim_matches(|c: char| !c.is_alphanumeric())
                    .to_lowercase()
                    .as_str(),
            );
        let directory = token.ends_with('/') || (placed && !last.contains('.'));
        let dir = token.trim_end_matches('/').to_string();
        if directory && !dir.is_empty() && !found.contains(&dir) {
            found.push(dir);
        }
    }
    (found.len() == 1).then(|| found.remove(0))
}

/// Where an output a requirement names lives: a bare name inside the one
/// directory the requirement names, else as [`resolve`] places it.
fn resolve_output(dir: &Path, requirement: &Requirement, path: &str) -> Option<PathBuf> {
    let bare = !path.trim().starts_with('/') && !path.contains('/');
    if bare
        && let Some(named) = named_directory(&requirement.text)
        && named != "/tests"
        && !named.starts_with("/tests/")
    {
        return Some(PathBuf::from(named).join(path.trim()));
    }
    resolve(dir, path)
}

/// The output paths a requirement asks for: the paths it names when it is a
/// deliverable or its words ask for a file to be written. With
/// [`Options::behavior`], only when it tells the executor to write them.
fn outputs(requirement: &Requirement, options: Options) -> Vec<String> {
    let lower = requirement.text.to_lowercase();
    let asks = requirement.kind == Kind::Deliverable
        || WRITE_WORDS.iter().any(|word| lower.contains(word));
    if !asks || requirement.kind == Kind::Context {
        return Vec::new();
    }
    if options.behavior && !asks_executor(&requirement.text) {
        return Vec::new();
    }
    requirement
        .extracted
        .paths
        .iter()
        .filter(|path| extension(path).is_some() && !path.contains('<') && !path.contains("YYYY"))
        .cloned()
        .collect()
}

/// A command with its harmless tail removed: `2>&1`, and a pipe into
/// `tail`, `head`, or `cat`.
fn core(command: &str) -> String {
    let mut text = command.trim().to_string();
    if let Some(at) = text.find('|') {
        let rest = text[at + 1..].trim_start();
        let viewer = ["tail", "head", "cat"]
            .iter()
            .any(|word| rest.split_whitespace().next() == Some(word));
        if viewer && !rest.contains('>') {
            text.truncate(at);
        }
    }
    text.replace("2>&1", "").trim().to_string()
}

/// Whether one command, without `cd` or chaining, reads as a test or check.
fn test_word(part: &str) -> bool {
    let words: Vec<&str> = part.split_whitespace().collect();
    let Some(first) = words.first() else {
        return false;
    };
    let name = |word: &str| {
        let base = word.rsplit('/').next().unwrap_or(word).to_lowercase();
        base.starts_with("test")
            || base.starts_with("check")
            || base.starts_with("verify")
            || base.starts_with("run_tests")
            || base.ends_with("_test.py")
            || base.ends_with("_test.sh")
            || base.ends_with("tests.py")
    };
    match *first {
        "pytest" | "ctest" => true,
        "python" | "python3" => {
            words
                .windows(2)
                .any(|w| w[0] == "-m" && matches!(w[1], "pytest" | "unittest"))
                || words.get(1).is_some_and(|script| name(script))
        }
        "bash" | "sh" => words.get(1).is_some_and(|script| name(script)),
        "make" => {
            // `make -C DIR test` names the directory first.
            let target = if words.get(1) == Some(&"-C") {
                words.get(3)
            } else {
                words.get(1)
            };
            target.is_some_and(|t| matches!(*t, "test" | "check"))
        }
        "cargo" | "go" => words.get(1) == Some(&"test"),
        "npm" | "yarn" | "pnpm" => {
            words.get(1) == Some(&"test")
                || (words.get(1) == Some(&"run")
                    && words.get(2).is_some_and(|t| t.starts_with("test")))
        }
        other => other.starts_with("./") && name(other),
    }
}

/// Why a command can't run as a check, or `None` when it can: it must read
/// as a test, name no placeholder, and change nothing outside what a test
/// run writes.
#[must_use]
pub fn refusal(command: &str) -> Option<&'static str> {
    let text = core(command);
    if text.is_empty() {
        return Some("it is empty");
    }
    if text.contains("/tests") && (text.contains(" /tests") || text.starts_with("/tests")) {
        return Some("it names /tests, the protected verifier's directory");
    }
    if text.contains('<') || text.contains("YYYY") || text.contains("...") {
        return Some("it names a placeholder");
    }
    if text.contains('>')
        || text.contains('`')
        || text.contains("$(")
        || text.contains(';')
        || text.contains('|')
    {
        return Some("it redirects, substitutes, or pipes");
    }
    let lower = text.to_lowercase();
    for word in [
        "rm ", "mv ", "sudo", "curl", "wget", "install", "apt", "git ", "dd ", "mkfs", "chmod",
        "chown", "kill",
    ] {
        if lower.contains(word) {
            return Some("it could change the environment");
        }
    }
    let mut tested = false;
    for part in text.split("&&") {
        let part = part.trim();
        if part.starts_with("cd ") && !part[3..].trim().contains(' ') {
            continue;
        }
        if !test_word(part) {
            return Some("it doesn't read as a test or check");
        }
        tested = true;
    }
    (!tested).then_some("it runs no test")
}

fn requirement_for<'a>(context: &'a Context<'_>, kinds: &[Kind]) -> Option<&'a Requirement> {
    kinds
        .iter()
        .find_map(|kind| context.map.requirements.iter().find(|r| r.kind == *kind))
}

#[allow(clippy::too_many_arguments)]
fn scenario(
    context: &Context<'_>,
    id: String,
    kind: &str,
    requirement: &Requirement,
    applies: Vec<String>,
    interface: String,
    seconds: u64,
    expected: Relation,
    params: serde_json::Value,
) -> Scenario {
    Scenario {
        id,
        kind: kind.to_string(),
        requirements: vec![requirement.id.clone()],
        spans: context.spans_of(&[requirement]),
        applies,
        interface,
        bounds: Bounds {
            seconds,
            processes: 1,
        },
        effects: vec![if kind == "generic.output" || kind == "generic.parse" {
            "reads the file in the task's working directory".to_string()
        } else {
            "runs the command in the task's working directory, as the executor did, without the episode's credentials".to_string()
        }],
        candidate: context.candidate.digest(),
        input: atif::digest(&params),
        seed: None,
        expected,
        params,
    }
}

/// Build: the generic scenarios the requirements and the executor's
/// commands justify, or why none apply.
///
/// # Errors
///
/// Returns why no generic scenario applies.
pub fn build(context: &Context<'_>) -> Result<Vec<Scenario>, Vec<Ineligible>> {
    let Some(workspace) = context.workspace else {
        return Err(vec![Ineligible {
            kind: "generic".to_string(),
            why: "the check has no live workspace".to_string(),
        }]);
    };
    let dir = Path::new(&workspace.dir);
    let mut scenarios = Vec::new();
    let mut ineligible = Vec::new();
    let mut seen = Vec::new();
    for requirement in &context.map.requirements {
        for path in outputs(requirement, workspace.options) {
            let Some(resolved) = resolve_output(dir, requirement, &path) else {
                continue;
            };
            if seen.contains(&resolved) {
                continue;
            }
            seen.push(resolved.clone());
            let shown = resolved.to_string_lossy().into_owned();
            let options = workspace.options;
            let optional =
                options.optional_outputs && selfreport::optional_output(&requirement.text);
            let unsure = options.optional_outputs && requirement.binding == Binding::Uncertain;
            let mut params = json!({ "path": path, "resolved": shown });
            if optional {
                params["optional"] = json!(true);
            }
            if unsure {
                params["binding"] = json!("uncertain");
            }
            scenarios.push(scenario(
                context,
                format!("generic.output:{path}"),
                "generic.output",
                requirement,
                vec![format!("{} names the output {path}", requirement.id)],
                format!("the file {path}"),
                1,
                Relation {
                    statement: if optional {
                        format!("{path} exists; it may be empty when nothing is needed.")
                    } else {
                        format!("{path} exists and isn't empty.")
                    },
                    derivation: if optional {
                        format!(
                            "{} asks for {path} to hold only what is needed, so an empty file can meet it.",
                            requirement.id
                        )
                    } else {
                        format!("{} asks for {path} to be written.", requirement.id)
                    },
                },
                params.clone(),
            ));
            if let Some(ext) = extension(&path).filter(|ext| PARSED.contains(ext)) {
                let header = (ext == "csv")
                    .then(|| {
                        context
                            .map
                            .requirements
                            .iter()
                            .flat_map(|r| r.extracted.formats.iter())
                            .find(|f| f.contains(',') && !f.contains('<') && !f.contains('{'))
                            .cloned()
                    })
                    .flatten();
                scenarios.push(scenario(
                    context,
                    format!("generic.parse:{path}"),
                    "generic.parse",
                    requirement,
                    vec![format!("{path} has the structured format {ext}")],
                    format!("the file {path}"),
                    10,
                    Relation {
                        statement: match &header {
                            Some(header) => format!(
                                "{path} parses as {ext} and starts with the header {header}."
                            ),
                            None => format!("{path} parses as {ext}."),
                        },
                        derivation: format!(
                            "The task names {path}; its extension says its format."
                        ),
                    },
                    {
                        let mut parse = json!({ "path": path, "resolved": shown, "format": ext, "header": header });
                        for key in ["optional", "binding"] {
                            if let Some(value) = params.get(key) {
                                parse[key] = value.clone();
                            }
                        }
                        parse
                    },
                ));
            }
        }
    }
    let command_sec = workspace.command_sec.max(1);
    let mut commands_seen: Vec<String> = Vec::new();
    for requirement in &context.map.requirements {
        for command in &requirement.extracted.commands {
            let text = core(command);
            if commands_seen.contains(&text) {
                continue;
            }
            match refusal(command) {
                Some(why) => ineligible.push(Ineligible {
                    kind: "generic.public-command".to_string(),
                    why: format!("`{command}` isn't run: {why}"),
                }),
                None => {
                    commands_seen.push(text.clone());
                    scenarios.push(scenario(
                        context,
                        format!("generic.public-command:{}", commands_seen.len()),
                        "generic.public-command",
                        requirement,
                        vec![format!("{} names the command `{text}`", requirement.id)],
                        format!("the command `{text}`"),
                        command_sec,
                        Relation {
                            statement: format!("`{text}` exits 0 on the final state."),
                            derivation: format!(
                                "The instruction names `{text}` as a test or check."
                            ),
                        },
                        json!({ "command": text, "source": "instruction" }),
                    ));
                }
            }
        }
    }
    // The executor's own test commands, latest first: a test it saw pass
    // should still pass on what it left.
    let target = requirement_for(context, &[Kind::Check, Kind::Deliverable, Kind::Behavior]);
    let mut claimed: Vec<&Claimed> = Vec::new();
    for run in workspace.claimed.iter().rev() {
        let text = core(&run.command);
        if refusal(&run.command).is_some()
            || commands_seen.contains(&text)
            || claimed.iter().any(|c| core(&c.command) == text)
        {
            continue;
        }
        claimed.push(run);
        if claimed.len() >= MAX_CLAIMED {
            break;
        }
    }
    match (target, claimed.is_empty()) {
        (Some(requirement), false) => {
            for (i, run) in claimed.iter().enumerate() {
                let text = core(&run.command);
                scenarios.push(scenario(
                    context,
                    format!("generic.claimed-command:{}", i + 1),
                    "generic.claimed-command",
                    requirement,
                    vec![format!(
                        "the executor ran `{text}`{}",
                        run.exit_code.map_or(String::new(), |code| format!(" and saw exit {code}"))
                    )],
                    format!("the command `{text}`"),
                    command_sec,
                    Relation {
                        statement: format!("`{text}` still exits as the executor saw it, 0, on the final state."),
                        derivation: "The executor ran this test during its session; its result is the executor's own claim about the work.".to_string(),
                    },
                    json!({ "command": text, "source": "executor", "session_exit_code": run.exit_code }),
                ));
            }
        }
        (None, false) => ineligible.push(Ineligible {
            kind: "generic.claimed-command".to_string(),
            why: "no requirement for a claimed test to observe".to_string(),
        }),
        (_, true) => ineligible.push(Ineligible {
            kind: "generic.claimed-command".to_string(),
            why: "the executor ran no test command a check can rerun".to_string(),
        }),
    }
    if workspace.options.self_report {
        match self_report(context, workspace) {
            // First, so the selector keeps it within the scenario budget:
            // it costs nothing and reads what nothing else reads.
            Ok(built) => scenarios.insert(0, built),
            Err(why) => ineligible.push(why),
        }
    }
    if scenarios.is_empty() {
        if ineligible.is_empty() {
            ineligible.push(Ineligible {
                kind: "generic".to_string(),
                why: "no requirement names an output file or a test command".to_string(),
            });
        }
        Err(ineligible)
    } else {
        Ok(scenarios)
    }
}

/// The requirement a self-reported failure contradicts: the first check or
/// behavior that names a command or an output, else the first check,
/// behavior, or deliverable.
fn self_report_target<'a>(context: &'a Context<'_>) -> Option<&'a Requirement> {
    context
        .map
        .requirements
        .iter()
        .find(|r| {
            matches!(r.kind, Kind::Check | Kind::Behavior)
                && (!r.extracted.commands.is_empty()
                    || !outputs(r, context.workspace.map(|w| w.options).unwrap_or_default())
                        .is_empty())
        })
        .or_else(|| requirement_for(context, &[Kind::Check, Kind::Behavior, Kind::Deliverable]))
}

/// `generic.self-report`: reads the final report, the JSON outputs the
/// requirements name, and the session's runs of the commands the
/// instruction names.
fn self_report(context: &Context<'_>, workspace: &Workspace) -> Result<Scenario, Ineligible> {
    let refuse = |why: &str| Ineligible {
        kind: "generic.self-report".to_string(),
        why: why.to_string(),
    };
    let requirement = self_report_target(context)
        .ok_or_else(|| refuse("no requirement for a self-report to contradict"))?;
    let dir = Path::new(&workspace.dir);
    let mut files: Vec<serde_json::Value> = Vec::new();
    for r in &context.map.requirements {
        for path in outputs(r, workspace.options) {
            if extension(&path) != Some("json") {
                continue;
            }
            if let Some(resolved) = resolve_output(dir, r, &path)
                && !files.iter().any(|f| f["path"] == path.as_str())
            {
                files.push(json!({ "path": path, "resolved": resolved.to_string_lossy() }));
            }
        }
    }
    let mut commands: Vec<String> = Vec::new();
    for command in context
        .map
        .requirements
        .iter()
        .flat_map(|r| r.extracted.commands.iter().map(|c| core(c)))
    {
        if !command.contains('<') && !command.contains("YYYY") && !commands.contains(&command) {
            commands.push(command);
        }
    }
    let report = workspace.report.as_deref().filter(|r| !r.trim().is_empty());
    if report.is_none() && files.is_empty() && commands.is_empty() {
        return Err(refuse(
            "no final report, JSON output, or named command to read",
        ));
    }
    let mut applies = Vec::new();
    if report.is_some() {
        applies.push("the executor wrote a final report".to_string());
    }
    if !files.is_empty() {
        applies.push(format!("the task names {} JSON output(s)", files.len()));
    }
    if !commands.is_empty() {
        applies.push(format!(
            "the instruction names {} command(s)",
            commands.len()
        ));
    }
    Ok(scenario(
        context,
        "generic.self-report".to_string(),
        "generic.self-report",
        requirement,
        applies,
        "the executor's final report and outputs".to_string(),
        5,
        Relation {
            statement: "Neither the executor's final report nor its outputs say the result failed, rests on a guess, or couldn't be done, and no command the task names last exited nonzero.".to_string(),
            derivation: format!(
                "The executor's own account is evidence about {}: a failure it reports is a failure the check doesn't need to rediscover.",
                requirement.id
            ),
        },
        json!({
            "report_chars": report.map(|r| r.chars().count()),
            "outputs": files,
            "commands": commands,
        }),
    ))
}

/// Why a CSV isn't well formed, or `None`: each row has the header's
/// number of fields, quotes respected.
fn csv_problem(text: &str, header: Option<&str>) -> Option<String> {
    let fields = |line: &str| {
        let mut n = 1;
        let mut quoted = false;
        for c in line.chars() {
            match c {
                '"' => quoted = !quoted,
                ',' if !quoted => n += 1,
                _ => {}
            }
        }
        n
    };
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let first = lines.next()?;
    if let Some(header) = header
        && first.trim() != header.trim()
    {
        return Some(format!(
            "the first line is {:?}, not the header {header:?}",
            crate::judge::clip(first, 200)
        ));
    }
    let width = fields(first);
    for (i, line) in lines.enumerate() {
        if fields(line) != width {
            return Some(format!(
                "row {} has {} fields where the header has {width}",
                i + 2,
                fields(line)
            ));
        }
    }
    None
}

/// Runs one generic scenario.
pub async fn run(context: &Context<'_>, scenario: &Scenario, scratch: &Path) -> Verdict {
    let Some(workspace) = context.workspace else {
        return Verdict::unavailable(&scenario.id, "the check has no live workspace");
    };
    let place = workspace.place();
    let mut verdict = Verdict::new(&scenario.id, "passed");
    match scenario.kind.as_str() {
        "generic.output" => {
            let resolved = scenario.params["resolved"].as_str().unwrap_or_default();
            let path = place.host(Path::new(resolved));
            let meta = std::fs::metadata(&path);
            if meta.is_err() && !workspace.known(resolved) {
                return Verdict::unavailable(
                    &scenario.id,
                    "the trial didn't retain this path, so a replay can't tell whether it was written",
                );
            }
            let observed = match &meta {
                Ok(meta) if meta.is_file() => {
                    json!({ "path": scenario.params["path"], "exists": true, "bytes": meta.len() })
                }
                Ok(_) => json!({ "path": scenario.params["path"], "exists": true, "file": false }),
                Err(_) => json!({ "path": scenario.params["path"], "exists": false }),
            };
            verdict.observations.push(observed);
            let optional = scenario.params["optional"] == true;
            let good = meta
                .as_ref()
                .is_ok_and(|m| m.is_file() && (optional || m.len() > 0));
            if optional && good {
                verdict.coverage.push(
                    "the requirement asks only for what is needed, so an empty file passes"
                        .to_string(),
                );
            }
            if !good && scenario.params["binding"] == "uncertain" {
                verdict.verdict = "inconclusive".to_string();
                verdict.coverage.push(
                    "the extraction was unsure this requirement binds, so a missing output doesn't count against it"
                        .to_string(),
                );
            } else if !good {
                verdict.verdict = "failed".to_string();
                verdict.hypotheses = vec![
                    "the executor wrote the output somewhere else, or under another name"
                        .to_string(),
                    "the step that writes the output never ran, or failed".to_string(),
                ];
            }
            verdict
                .coverage
                .push("checks that the file is there, not what it holds".to_string());
        }
        "generic.parse" => {
            let path = place.host(Path::new(
                scenario.params["resolved"].as_str().unwrap_or_default(),
            ));
            let format = scenario.params["format"].as_str().unwrap_or_default();
            let Ok(text) = std::fs::read_to_string(&path) else {
                return Verdict::unavailable(&scenario.id, "the file is missing or isn't text");
            };
            if scenario.params["optional"] == true && text.trim().is_empty() {
                verdict.observations.push(
                    json!({ "path": scenario.params["path"], "format": format, "problem": null, "empty": true }),
                );
                verdict
                    .coverage
                    .push("the file is empty, which an optional output may be".to_string());
                return verdict;
            }
            let problem = match format {
                "json" => serde_json::from_str::<serde_json::Value>(&text)
                    .err()
                    .map(|e| e.to_string()),
                "jsonl" => text
                    .lines()
                    .enumerate()
                    .filter(|(_, l)| !l.trim().is_empty())
                    .find_map(|(i, l)| {
                        serde_json::from_str::<serde_json::Value>(l)
                            .err()
                            .map(|e| format!("line {}: {e}", i + 1))
                    }),
                "csv" => csv_problem(&text, scenario.params["header"].as_str()),
                "py" | "sh" => {
                    let program = if format == "py" { "python3" } else { "bash" };
                    let Some(binary) = crate::minitask::process::which(program) else {
                        return Verdict::unavailable(
                            &scenario.id,
                            &format!("no {program} on this host"),
                        );
                    };
                    let mut command = std::process::Command::new(binary);
                    if format == "py" {
                        command.args(["-c", "import ast,sys; ast.parse(open(sys.argv[1], encoding='utf-8').read(), sys.argv[1])"]);
                    } else {
                        command.arg("-n");
                    }
                    command.arg(&path);
                    let ran = crate::minitask::process::run(command, Duration::from_secs(10)).await;
                    (ran.code != Some(0)).then(|| crate::judge::clip_tail(ran.stderr.trim(), 600))
                }
                _ => None,
            };
            verdict.observations.push(
                json!({ "path": scenario.params["path"], "format": format, "problem": problem }),
            );
            if problem.is_some() && scenario.params["binding"] == "uncertain" {
                verdict.verdict = "inconclusive".to_string();
                verdict.coverage.push(
                    "the extraction was unsure this requirement binds, so a malformed output doesn't count against it"
                        .to_string(),
                );
            } else if problem.is_some() {
                verdict.verdict = "failed".to_string();
                verdict.hypotheses = vec![format!(
                    "the file isn't valid {format}, or not in the shape the task shows"
                )];
            }
            verdict
                .coverage
                .push("checks the format, not the values".to_string());
        }
        "generic.public-command" | "generic.claimed-command" => {
            let command_text = scenario.params["command"].as_str().unwrap_or_default();
            let command = match place.shell(command_text, &workspace.dir, &[], scratch) {
                Ok(command) => command,
                Err(why) => return Verdict::unavailable(&scenario.id, &why),
            };
            let ran = crate::minitask::process::run(
                command,
                Duration::from_secs(scenario.bounds.seconds),
            )
            .await;
            let output = crate::support::scrub(&format!("{}{}", ran.stdout, ran.stderr));
            if ran.code != Some(0)
                && place.is_replay()
                && let Some(missing) = super::place::missing_environment(&output)
            {
                return Verdict::unavailable(
                    &scenario.id,
                    &format!("the replay lacks what the task's image provides: {missing}"),
                );
            }
            verdict.observations.push(json!({
                "command": command_text,
                "exit_code": ran.code,
                "killed": ran.killed,
                "milliseconds": ran.milliseconds,
                "output_tail": crate::judge::clip_tail(output.trim(), OUTPUT_CHARS),
            }));
            let session = scenario.params["session_exit_code"].as_i64();
            verdict.verdict = if ran.killed {
                verdict.coverage.push(format!(
                    "the command ran past its {}-second bound",
                    scenario.bounds.seconds
                ));
                "inconclusive".to_string()
            } else if ran.code == Some(0) {
                "passed".to_string()
            } else if scenario.kind == "generic.claimed-command" && session.is_some_and(|c| c != 0)
            {
                verdict
                    .coverage
                    .push("the executor saw this command fail too".to_string());
                "inconclusive".to_string()
            } else {
                verdict.hypotheses = vec![
                    "a later change broke what this test covers".to_string(),
                    "the test depends on state the session had and the final workspace doesn't"
                        .to_string(),
                ];
                "failed".to_string()
            };
            verdict
                .coverage
                .push("a passing test shows only what the test covers".to_string());
        }
        "generic.self-report" => {
            let mut findings = Vec::new();
            if let Some(report) = workspace.report.as_deref() {
                findings.extend(selfreport::admissions(report));
            }
            for file in scenario.params["outputs"].as_array().into_iter().flatten() {
                let resolved = place.host(Path::new(file["resolved"].as_str().unwrap_or_default()));
                let small = std::fs::metadata(&resolved).is_ok_and(|m| m.len() <= 4 * 1024 * 1024);
                if small && let Ok(text) = std::fs::read_to_string(&resolved) {
                    findings.extend(selfreport::output_flags(
                        file["path"].as_str().unwrap_or_default(),
                        &text,
                    ));
                }
            }
            let named: Vec<String> = scenario.params["commands"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c.as_str().map(str::to_string))
                .collect();
            findings.extend(selfreport::command_failures(&workspace.claimed, &named));
            verdict.coverage.push(
                "reads what the executor said and wrote about its own result, not the result itself"
                    .to_string(),
            );
            if findings.is_empty() {
                verdict.verdict = "inconclusive".to_string();
                verdict.coverage.push(
                    "no self-reported failure, which alone doesn't show the requirement met"
                        .to_string(),
                );
            } else {
                verdict.verdict = "failed".to_string();
                verdict.hypotheses = vec![
                    "the executor stopped at a result it knew was wrong or incomplete".to_string(),
                    "the executor resolved an ambiguity by assumption, and the assumption may be wrong".to_string(),
                    "the task's intended reading makes the reported obstacle go away".to_string(),
                ];
                verdict.observations = findings
                    .iter()
                    .map(|f| serde_json::to_value(f).unwrap_or_default())
                    .collect();
            }
        }
        other => return Verdict::unavailable(&scenario.id, &format!("no runner for {other}")),
    }
    verdict
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_test_commands_that_change_nothing_run() {
        assert_eq!(refusal("pytest -q"), None);
        assert_eq!(
            refusal("cd app && python3 -m pytest -x 2>&1 | tail -20"),
            None
        );
        assert_eq!(refusal("python3 test_solution.py"), None);
        assert_eq!(refusal("bash run_tests.sh"), None);
        assert_eq!(refusal("cargo test --release"), None);
        assert_eq!(refusal("make -C /app test"), None);
        assert!(refusal("make -C /app repro").is_some());
        assert!(refusal("pytest /tests/test_outputs.py").is_some());
        assert!(refusal("python3 summarize.py LOG_DIR > out.csv").is_some());
        assert!(refusal("pip install pytest && pytest").is_some());
        assert!(refusal("python3 solve.py").is_some());
        assert!(refusal("rm -rf build && make test").is_some());
        assert!(refusal("python3 x.py <input>").is_some());
    }

    #[test]
    fn a_bare_output_lives_in_the_directory_its_requirement_names() {
        assert_eq!(
            named_directory(
                "Provide your answers as a CSV file named `TB3_Conf_Answers.csv` and save it inside `/results/`."
            )
            .as_deref(),
            Some("/results")
        );
        assert_eq!(
            named_directory("Save it inside /results."),
            Some("/results".to_string())
        );
        assert_eq!(named_directory("Write `answer.json`."), None);
        assert_eq!(named_directory("Copy /a/ into /b/."), None);
    }

    #[test]
    fn only_an_instruction_to_the_executor_asks_for_an_output() {
        assert!(asks_executor(
            "Provide your answers as a CSV file named `x.csv` and save it inside `/results/`."
        ));
        assert!(asks_executor(
            "Write any Python dependencies to `/app/requirements.txt`."
        ));
        assert!(asks_executor("Then save the report to /app/report.json."));
        assert!(asks_executor(
            "You should write the plan to `/app/plan.json`."
        ));
        assert!(asks_executor(
            "The final answer must be written to `/app/answer.txt`."
        ));
        assert!(!asks_executor(
            "Accepted leads should write `/app/output/submission.json`."
        ));
        assert!(!asks_executor(
            "If `crm_leads.json` is malformed, then record it in `/app/output/rejected.json`."
        ));
        assert!(!asks_executor(
            "- **`output_format.txt`**: naming rules and formatting requirements for your answer."
        ));
        assert!(!asks_executor(
            "References to the same entity must produce the same token across `subject_links.csv`."
        ));
    }

    #[test]
    fn a_csv_needs_its_header_and_even_rows() {
        assert_eq!(csv_problem("a,b\n1,2\n", Some("a,b")), None);
        assert!(csv_problem("a,b\n1,2,3\n", None).unwrap().contains("row 2"));
        assert!(
            csv_problem("x,y\n1,2\n", Some("a,b"))
                .unwrap()
                .contains("header")
        );
        assert_eq!(csv_problem("a,b\n\"1,5\",2\n", None), None);
    }

    #[test]
    fn protected_and_home_paths_never_resolve() {
        let dir = Path::new("/app");
        assert_eq!(resolve(dir, "/tests/test.py"), None);
        assert_eq!(resolve(dir, "~/x.txt"), None);
        assert_eq!(resolve(dir, "out/"), None);
        assert_eq!(
            resolve(dir, "./answer.json"),
            Some(PathBuf::from("/app/answer.json"))
        );
        assert_eq!(
            resolve(dir, "/tmp/a.json"),
            Some(PathBuf::from("/tmp/a.json"))
        );
    }
}
