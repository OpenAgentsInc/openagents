//! The loop's state: the environment, the task, and a bounded record of the
//! actions so far.

use serde::Serialize;

/// Characters of a command's output kept from its start.
pub const OUTPUT_HEAD: usize = 1_500;
/// Characters of a command's output kept from its end.
pub const OUTPUT_TAIL: usize = 3_000;
/// Actions shown in full; older ones are one line each.
pub const RECENT: usize = 4;

/// One command's result.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct CommandResult {
    pub command: String,
    /// The exit code, or `None` when a signal or the deadline ended it.
    pub exit: Option<i32>,
    pub timed_out: bool,
    pub seconds: f64,
    /// Standard output and standard error together, cut to a head and a tail.
    pub output: String,
}

impl CommandResult {
    /// Whether the command succeeded.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.exit == Some(0) && !self.timed_out
    }
}

/// One step's action: why, and what ran.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Action {
    pub step: usize,
    pub rationale: String,
    pub results: Vec<CommandResult>,
    /// Commands the model gave that didn't run, because one before them
    /// failed.
    pub skipped: Vec<String>,
}

/// The state every prompt is built from.
#[derive(Clone, Debug, Default, Serialize)]
pub struct State {
    /// What the environment is: the working directory and what a first
    /// look found there.
    pub environment: String,
    pub task: String,
    pub actions: Vec<Action>,
    /// Notes from the host, such as a reply that didn't match the format.
    pub notes: Vec<String>,
    /// The files the model keeps in view, read after its last action:
    /// each path and its contents, or `None` when there's no such file.
    pub files: Vec<(String, Option<String>)>,
    /// The acceptance tests, once frozen.
    pub tests: Vec<Test>,
    /// The step the tests were frozen at.
    pub frozen_at: Option<usize>,
    /// The latest run of every test, in order; each result's `command` is
    /// the test's name.
    pub test_results: Vec<CommandResult>,
}

/// One frozen acceptance test: a bash script that exits 0 only when its
/// requirement is met.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Test {
    pub name: String,
    pub script: String,
    /// Whether it passed when it was frozen, before the fix.
    pub passed_at_freeze: Option<bool>,
}

/// The first `head` and last `tail` characters of `text`, with a line that
/// says how much was left out.
#[must_use]
pub fn cut(text: &str, head: usize, tail: usize) -> String {
    let count = text.chars().count();
    if count <= head + tail {
        return text.to_string();
    }
    let start: String = text.chars().take(head).collect();
    let end: String = text.chars().skip(count - tail).collect();
    format!(
        "{start}\n[… {} characters left out …]\n{end}",
        count - head - tail
    )
}

fn one_line(text: &str, max: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    cut(&flat, max, 0)
}

impl State {
    /// The files in view as the prompt shows them.
    #[must_use]
    pub fn render_files(&self) -> String {
        if self.files.is_empty() {
            return "None. List paths in `view` to see files here.".to_string();
        }
        self.files
            .iter()
            .map(|(path, contents)| match contents {
                Some(text) => format!("## {path}\n\n```\n{}\n```", text.trim_end()),
                None => format!("## {path}\n\n(no such file)"),
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// A one-line count of the latest test run, or `None` before a freeze.
    #[must_use]
    pub fn tests_summary(&self) -> Option<String> {
        if self.tests.is_empty() {
            return None;
        }
        let passing = self.test_results.iter().filter(|r| r.ok()).count();
        let failing: Vec<&str> = self
            .test_results
            .iter()
            .filter(|r| !r.ok())
            .map(|r| r.command.as_str())
            .collect();
        Some(if failing.is_empty() {
            format!("{passing} of {} acceptance tests pass", self.tests.len())
        } else {
            format!(
                "{passing} of {} acceptance tests pass; failing: {}",
                self.tests.len(),
                failing.join(", ")
            )
        })
    }

    /// The acceptance tests as the prompt shows them.
    #[must_use]
    pub fn render_tests(&self, dir: &str) -> String {
        let Some(step) = self.frozen_at else {
            return format!(
                "None frozen yet. Before you change the task's files, define the end conditions: \
write one bash test per requirement as {dir}/<name>.sh. A test exits 0 only when its requirement \
is met, so most should fail now. Cover every requirement and symptom the task states. For each \
method, statistic, or algorithm the task names, test it against its standard definition and \
properties on inputs where you know the right answer, not only the end behavior. Then set \
`freeze_tests` to true. The host freezes the tests, runs its own copies after every step, and \
accepts `finished` only when all of them pass."
            );
        };
        let mut out = format!(
            "Frozen at step {step}. The host runs its own copies after every step, so editing \
{dir} changes nothing. `finished` is accepted only when every test passes.\n"
        );
        for (test, result) in self.tests.iter().zip(&self.test_results) {
            let before = match test.passed_at_freeze {
                Some(true) => " (it already passed when frozen)",
                _ => "",
            };
            let status = if result.ok() {
                "pass".to_string()
            } else if result.timed_out {
                format!("FAIL: timed out after {:.0} s", result.seconds)
            } else {
                "FAIL".to_string()
            };
            out.push_str(&format!("\n## {}: {status}{before}\n", test.name));
            if !result.ok() {
                out.push_str(&format!(
                    "\n```\n{}\n```\n",
                    cut(result.output.trim_end(), 600, 1_200)
                ));
            }
        }
        out
    }

    /// The actions as the prompt shows them: the last [`RECENT`] in full,
    /// earlier ones one line each.
    #[must_use]
    pub fn render_actions(&self) -> String {
        if self.actions.is_empty() {
            return "None yet.".to_string();
        }
        let older = self.actions.len().saturating_sub(RECENT);
        let mut out = String::new();
        for action in &self.actions[..older] {
            let summary: Vec<String> = action
                .results
                .iter()
                .map(|r| {
                    format!(
                        "`{}` → {}",
                        one_line(&r.command, 80),
                        match (r.exit, r.timed_out) {
                            (_, true) => "timed out".to_string(),
                            (Some(code), _) => format!("exit {code}"),
                            (None, _) => "killed".to_string(),
                        }
                    )
                })
                .collect();
            out.push_str(&format!(
                "- Step {}: {} — {}\n",
                action.step,
                one_line(&action.rationale, 120),
                if summary.is_empty() {
                    "no commands".to_string()
                } else {
                    summary.join("; ")
                }
            ));
        }
        for action in &self.actions[older..] {
            out.push_str(&format!(
                "\n## Step {}\n\nRationale: {}\n",
                action.step, action.rationale
            ));
            for result in &action.results {
                let status = match (result.exit, result.timed_out) {
                    (_, true) => format!("timed out after {:.0} s", result.seconds),
                    (Some(code), _) => format!("exit {code}, {:.1} s", result.seconds),
                    (None, _) => "killed".to_string(),
                };
                out.push_str(&format!(
                    "\n$ {}\n[{status}]\n{}\n",
                    result.command, result.output
                ));
            }
            for command in &action.skipped {
                out.push_str(&format!(
                    "\n$ {command}\n[not run: an earlier command failed]\n"
                ));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(step: usize, output: &str) -> Action {
        Action {
            step,
            rationale: format!("reason {step}"),
            results: vec![CommandResult {
                command: format!("cmd {step}"),
                exit: Some(0),
                timed_out: false,
                seconds: 0.1,
                output: output.to_string(),
            }],
            skipped: Vec::new(),
        }
    }

    #[test]
    fn long_output_keeps_its_head_and_tail() {
        let text = "a".repeat(100) + &"b".repeat(100);
        let out = cut(&text, 10, 10);
        assert!(out.starts_with("aaaaaaaaaa\n[… 180 characters left out …]\nbbbbbbbbbb"));
        assert_eq!(cut("short", 10, 10), "short");
    }

    #[test]
    fn older_actions_shrink_to_one_line_each() {
        let state = State {
            actions: (1..=7).map(|n| action(n, &"x".repeat(500))).collect(),
            ..State::default()
        };
        let text = state.render_actions();
        assert!(text.contains("- Step 1: reason 1 — `cmd 1` → exit 0"));
        assert!(text.contains("## Step 7"));
        assert!(!text.contains("## Step 3"));
        // Three one-line steps and four full ones, each full output 500 characters.
        assert!(text.len() < 7 * 500);
    }
}
