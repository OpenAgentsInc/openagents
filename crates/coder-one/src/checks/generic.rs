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

use super::{Bounds, Context, Ineligible, Relation, Scenario, Verdict};
use crate::requirements::{Kind, Requirement};

/// The scenario kinds this family builds.
pub const KINDS: &[&str] = &[
    "generic.output",
    "generic.parse",
    "generic.public-command",
    "generic.claimed-command",
];

/// The most claimed commands one check reruns.
const MAX_CLAIMED: usize = 3;

/// The most characters of a command's output an observation keeps.
const OUTPUT_CHARS: usize = 2_000;

/// Variables a check never passes to a command it runs.
const CREDENTIALS: &[&str] = &[
    "OPENAGENTS_API_KEY",
    "TYPESAFE_API_KEY",
    "CLAUDE_CODE_OAUTH_TOKEN",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "OPENAI_API_KEY",
    "CODER_ONE_POLICY",
];

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

/// The output paths a requirement asks for: the paths it names when it is a
/// deliverable or its words ask for a file to be written.
fn outputs(requirement: &Requirement) -> Vec<String> {
    let lower = requirement.text.to_lowercase();
    let asks = requirement.kind == Kind::Deliverable
        || WRITE_WORDS.iter().any(|word| lower.contains(word));
    if !asks || requirement.kind == Kind::Context {
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
        "make" => words.get(1).is_some_and(|t| matches!(*t, "test" | "check")),
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
        for path in outputs(requirement) {
            let Some(resolved) = resolve(dir, &path) else {
                continue;
            };
            if seen.contains(&resolved) {
                continue;
            }
            seen.push(resolved.clone());
            let shown = resolved.to_string_lossy().into_owned();
            scenarios.push(scenario(
                context,
                format!("generic.output:{path}"),
                "generic.output",
                requirement,
                vec![format!("{} names the output {path}", requirement.id)],
                format!("the file {path}"),
                1,
                Relation {
                    statement: format!("{path} exists and isn't empty."),
                    derivation: format!("{} asks for {path} to be written.", requirement.id),
                },
                json!({ "path": path, "resolved": shown }),
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
                    json!({ "path": path, "resolved": shown, "format": ext, "header": header }),
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
pub async fn run(context: &Context<'_>, scenario: &Scenario) -> Verdict {
    let Some(workspace) = context.workspace else {
        return Verdict::unavailable(&scenario.id, "the check has no live workspace");
    };
    let mut verdict = Verdict::new(&scenario.id, "passed");
    match scenario.kind.as_str() {
        "generic.output" => {
            let path = PathBuf::from(scenario.params["resolved"].as_str().unwrap_or_default());
            let meta = std::fs::metadata(&path);
            let observed = match &meta {
                Ok(meta) if meta.is_file() => {
                    json!({ "path": scenario.params["path"], "exists": true, "bytes": meta.len() })
                }
                Ok(_) => json!({ "path": scenario.params["path"], "exists": true, "file": false }),
                Err(_) => json!({ "path": scenario.params["path"], "exists": false }),
            };
            verdict.observations.push(observed);
            let good = meta.as_ref().is_ok_and(|m| m.is_file() && m.len() > 0);
            if !good {
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
            let path = PathBuf::from(scenario.params["resolved"].as_str().unwrap_or_default());
            let format = scenario.params["format"].as_str().unwrap_or_default();
            let Ok(text) = std::fs::read_to_string(&path) else {
                return Verdict::unavailable(&scenario.id, "the file is missing or isn't text");
            };
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
            if problem.is_some() {
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
            let shell = crate::minitask::process::which("bash")
                .or_else(|| crate::minitask::process::which("sh"));
            let Some(shell) = shell else {
                return Verdict::unavailable(&scenario.id, "no shell on this host");
            };
            let mut command = std::process::Command::new(shell);
            command
                .arg("-c")
                .arg(command_text)
                .current_dir(&workspace.dir)
                .stdin(std::process::Stdio::null());
            for name in CREDENTIALS {
                command.env_remove(name);
            }
            let ran = crate::minitask::process::run(
                command,
                Duration::from_secs(scenario.bounds.seconds),
            )
            .await;
            let output = crate::support::scrub(&format!("{}{}", ran.stdout, ran.stderr));
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
        assert!(refusal("pytest /tests/test_outputs.py").is_some());
        assert!(refusal("python3 summarize.py LOG_DIR > out.csv").is_some());
        assert!(refusal("pip install pytest && pytest").is_some());
        assert!(refusal("python3 solve.py").is_some());
        assert!(refusal("rm -rf build && make test").is_some());
        assert!(refusal("python3 x.py <input>").is_some());
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
