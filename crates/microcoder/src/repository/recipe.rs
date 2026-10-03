//! The delegate recipe applied at dispatch (#10208).
//!
//! Before any engine takes a repository turn, the host runs the recipe's
//! groundwork ([`coder_delegate::recipe::prepare`]): Jev judges the task's
//! class, probes and surveys the workspace, chooses knowledge entries, and
//! picks checks. Then the host freezes the checks by running each once
//! here (one that already passes can't tell done from not done and is
//! dropped), and every engine gets what [`route_contract::recipe`] says it
//! can take:
//!
//! - **Microcoder loop** (Codex and Claude routes): the briefing is the
//!   loop's Task section, the class's effort goes on the route, and the
//!   frozen checks run after each step that ran a command; the loop ends
//!   once they pass ([`crate::run::Ending::ChecksPassed`]).
//! - **Whole agents** (Grok Build, Devin, OpenCode): the briefing goes in
//!   front of the prompt, Grok Build gets the class's
//!   `--reasoning-effort`, and while the agent works the host runs the
//!   frozen checks whenever the workspace changed and then held still for
//!   one look ([`Recipe::watch`]); once they pass, the host ends the turn.
//!
//! The groundwork's Jev cost joins the run's cost. Nothing here is a step,
//! time, or spend budget: the early stop is success.

use std::cell::Cell;
use std::time::Duration;

use atif::{Source, Step};
use coder::task::adapter::Host;
use coder_delegate::recipe::{Prepared, TaskClass};
use serde_json::{Value, json};

use crate::state::{CommandResult, Test, cut};

/// The variable that turns the recipe off for a run: `off` runs every
/// engine raw (the "before" arm of a with/without measurement). Unset, or
/// anything else, keeps it on.
pub const OFF_VAR: &str = "OPENAGENTS_DELEGATE_RECIPE";

/// Whether the recipe runs, given the environment.
pub(crate) fn enabled(env: &dyn Fn(&str) -> Option<String>) -> bool {
    env(OFF_VAR).is_none_or(|value| !value.trim().eq_ignore_ascii_case("off"))
}

/// The seconds one frozen check may run: the loop's command deadline.
pub(crate) const CHECK_SECONDS: u64 = 300;

/// How often the host looks at a whole agent's workspace while it works.
pub(crate) const WATCH: Duration = Duration::from_secs(15);

/// The recipe prepared for one turn, with its checks frozen.
pub(crate) struct Recipe {
    pub(crate) prepared: Prepared,
    /// The checks frozen for this turn: each failed before the engine
    /// started.
    pub(crate) frozen: Vec<Test>,
    /// The last run of the frozen checks, in order.
    pub(crate) results: Vec<CommandResult>,
}

impl Recipe {
    /// Prepares the turn's recipe and freezes its checks, and records both
    /// in the transcript.
    /// `survey` off skips the workspace survey, for an engine that reads
    /// the workspace itself ([`coder_delegate::recipe::Input::survey`]).
    pub(crate) async fn prepare(host: &Host, jev: Option<jev::Client>, survey: bool) -> Recipe {
        let (request, earlier) = split_prompt(host.prompt(), &host.engine_prompt());
        let workdir = host.workspace().to_path_buf();
        let prepared = coder_delegate::recipe::prepare(coder_delegate::recipe::Input {
            workdir: &workdir,
            request: &request,
            earlier: &earlier,
            jev,
            resumed: false,
            knowledge_dirs: coder_delegate::recipe::knowledge_dirs(&workdir),
            survey,
        })
        .await;
        let mut frozen = Vec::new();
        let mut results = Vec::new();
        let mut freezing = Vec::new();
        for (index, check) in prepared.checks.iter().enumerate() {
            let result = run(host, check).await;
            let kept = !result.ok() && !result.timed_out;
            freezing.push(json!({"command": check, "exit": result.exit,
                "timed_out": result.timed_out, "seconds": result.seconds,
                "frozen": kept, "why": if kept { "fails before the change" }
                    else if result.timed_out { "ran past its deadline before the change" }
                    else { "already passes, so it can't tell done from not done" },
                "output": cut(&result.output, 0, 2_000)}));
            if kept {
                frozen.push(Test {
                    name: format!("check-{}", index + 1),
                    script: check.clone(),
                    passed_at_freeze: Some(false),
                });
                results.push(result);
            }
        }
        let recipe = Recipe {
            prepared,
            frozen,
            results,
        };
        // The run's independent check runs them again when it ends (#10232).
        host.freeze_checks(
            &recipe
                .frozen
                .iter()
                .map(|test| test.script.clone())
                .collect::<Vec<_>>(),
        );
        let _ = host.append(&Step::said(Source::System, &recipe.summary()).noting(
            "delegate_recipe",
            json!({"run": recipe.prepared.record, "freezing": freezing,
                        "frozen": recipe.frozen.iter().map(|t| &t.script).collect::<Vec<_>>(),
                        "lines": recipe.prepared.lines}),
        ));
        recipe
    }

    /// The class Jev judged, if it answered.
    pub(crate) fn class(&self) -> Option<TaskClass> {
        self.prepared.class
    }

    /// The effort a route on `engine` runs at for this turn.
    pub(crate) fn effort(&self, engine: &str, admitted: Option<&str>) -> Option<String> {
        route_contract::recipe::effort(engine, self.class(), admitted)
    }

    /// The text in front of the task: the briefing and the frozen checks.
    pub(crate) fn text(&self, resumed: bool) -> String {
        let frozen: Vec<String> = self.frozen.iter().map(|t| t.script.clone()).collect();
        self.prepared.text(resumed, &frozen)
    }

    /// The loop's early stop: after one step with every frozen check
    /// passing, when any were frozen.
    pub(crate) fn checks_stop(&self) -> Option<usize> {
        (!self.frozen.is_empty()).then_some(1)
    }

    /// What the groundwork cost: Jev's part only.
    pub(crate) fn cost(&self) -> coder::task::owner::Cost {
        coder::task::owner::Cost::from_usd(Some(0.0), self.prepared.jev_usd)
    }

    /// The transcript's line for the prepared recipe.
    fn summary(&self) -> String {
        let class = self.class().map_or("unjudged", TaskClass::word);
        format!(
            "Delegate recipe {}: a {class} task; a {}-character briefing with {} knowledge entries; {} of {} checks frozen.",
            route_contract::recipe::RECIPE_VERSION,
            self.prepared.briefing.chars(),
            self.prepared.knowledge.entries.len(),
            self.frozen.len(),
            self.prepared.checks.len()
        )
    }

    /// Runs every frozen check, records the results, and says whether they
    /// all pass.
    pub(crate) async fn check(&mut self, host: &Host, when: &str) -> bool {
        let mut results = Vec::new();
        for test in &self.frozen {
            results.push(run(host, &test.script).await);
        }
        let pass = results.iter().all(CommandResult::ok);
        let _ = host.append(
            &Step::said(
                Source::System,
                &format!(
                    "The host ran the frozen checks {when}: {}.",
                    if pass {
                        "they all pass"
                    } else {
                        "not all pass"
                    }
                ),
            )
            .noting(
                "recipe_checks",
                json!({"when": when, "pass": pass,
                "results": results.iter().map(record).collect::<Vec<_>>()}),
            ),
        );
        self.results = results;
        pass
    }

    /// While a whole agent works: every [`WATCH`], the host fingerprints the
    /// workspace, and when it changed since the checks last ran and then
    /// held still for one look, runs them. Returns once they all pass,
    /// after setting `passed`; never returns otherwise, so a caller selects
    /// it against the agent's turn. A workspace that isn't a Git work tree
    /// is never fingerprinted, and its checks run once after the turn.
    pub(crate) async fn watch(&mut self, host: &Host, passed: &Cell<bool>) {
        if self.frozen.is_empty() {
            return std::future::pending().await;
        }
        let mut seen: Option<u64> = None;
        let mut checked: Option<u64> = None;
        loop {
            tokio::time::sleep(WATCH).await;
            if host.cancelled() {
                return std::future::pending().await;
            }
            let Some(now) = fingerprint(host.workspace()).await else {
                return std::future::pending().await;
            };
            let settled = seen == Some(now);
            seen = Some(now);
            if !settled || checked == Some(now) {
                continue;
            }
            checked = Some(now);
            if self.check(host, "while the agent worked").await {
                passed.set(true);
                return;
            }
        }
    }
}

/// The prompt a whole agent's session gets: the recipe's text when the
/// turn has a recipe, otherwise the message (a continued session) or the
/// conversation with it (a new one).
pub(crate) fn agent_prompt(recipe: Option<&Recipe>, host: &Host, resumed: bool) -> String {
    match recipe {
        Some(recipe) => recipe.text(resumed),
        None if resumed => host.prompt().to_owned(),
        None => host.engine_prompt(),
    }
}

/// Prompts a whole agent's session, with the recipe's watcher beside it
/// ([`Recipe::watch`]). Returns the prompt's result and whether the
/// frozen checks passed while the agent worked, which ended the turn.
pub(crate) async fn prompt_watched(
    session: &mut acp_client::Session,
    prompt: &str,
    host: &Host,
    recipe: Option<&mut Recipe>,
    silence: Duration,
    grace: Duration,
    handler: &mut dyn acp_client::Handler,
) -> (
    Result<acp_client::wire::Prompted, acp_client::ClientError>,
    bool,
) {
    let passed = Cell::new(false);
    let stop = || host.cancelled() || passed.get();
    let prompting = session.prompt(prompt, silence, &stop, grace, handler);
    let result = match recipe {
        Some(recipe) => {
            tokio::pin!(prompting);
            let watching = recipe.watch(host, &passed);
            tokio::pin!(watching);
            tokio::select! {
                result = &mut prompting => result,
                () = &mut watching => prompting.await,
            }
        }
        None => prompting.await,
    };
    (result, passed.get())
}

/// One check run through the host, as the loop's commands run.
async fn run(host: &Host, script: &str) -> CommandResult {
    match host
        .command(script, Duration::from_secs(CHECK_SECONDS))
        .await
    {
        Ok(result) => CommandResult {
            command: script.to_owned(),
            exit: result.exit,
            timed_out: result.timed_out,
            seconds: result.seconds,
            output: cut(
                &result.output,
                crate::state::OUTPUT_HEAD,
                crate::state::OUTPUT_TAIL,
            ),
        },
        Err(error) => CommandResult {
            command: script.to_owned(),
            exit: None,
            timed_out: false,
            seconds: 0.0,
            output: format!("The repository host refused the check: {error}"),
        },
    }
}

fn record(result: &CommandResult) -> Value {
    json!({"command": result.command, "exit": result.exit, "timed_out": result.timed_out,
        "seconds": result.seconds, "output": cut(&result.output, 0, 2_000)})
}

/// This turn's message and the conversation before it, from the task's
/// prompt and its engine prompt (`coder::task::adapter::conversation_prompt`).
/// When the engine prompt isn't the conversation form, it is the message.
pub(crate) fn split_prompt(prompt: &str, engine: &str) -> (String, String) {
    if engine == prompt {
        return (prompt.to_owned(), String::new());
    }
    match engine
        .strip_suffix(prompt)
        .and_then(|before| before.strip_suffix("\nThe user's new message:\n"))
    {
        Some(earlier) => (prompt.to_owned(), earlier.trim().to_owned()),
        None => (engine.to_owned(), String::new()),
    }
}

/// A fingerprint of the workspace's uncommitted state: Git's status, its
/// diff, and the untracked files' names and sizes, hashed. `None` outside
/// a Git work tree or without Git.
async fn fingerprint(workspace: &std::path::Path) -> Option<u64> {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for args in [
        &["status", "--porcelain=v1", "-uall"][..],
        &["diff", "--no-ext-diff"][..],
    ] {
        let output = tokio::process::Command::new("git")
            .args(args)
            .current_dir(workspace)
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .await
            .ok()?;
        if !output.status.success() {
            return None;
        }
        output.stdout.hash(&mut hasher);
        if args[0] == "status" {
            // An untracked file's edits don't change the status line.
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                if let Some(path) = line.strip_prefix("?? ") {
                    let meta = std::fs::metadata(workspace.join(path.trim_matches('"')));
                    meta.map(|m| (m.len(), m.modified().ok()))
                        .ok()
                        .hash(&mut hasher);
                }
            }
        }
    }
    Some(hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_recipe_is_on_unless_the_variable_says_off() {
        assert!(enabled(&|_| None));
        assert!(enabled(&|_| Some("on".into())));
        assert!(!enabled(&|name| (name == OFF_VAR).then(|| " OFF ".into())));
    }

    #[test]
    fn the_prompt_splits_into_the_message_and_the_conversation_before_it() {
        assert_eq!(
            split_prompt("fix it", "fix it"),
            ("fix it".into(), String::new())
        );
        let engine = coder::task::adapter::conversation_prompt(
            &[coder::task::adapter::EarlierTurn {
                turn: 1,
                prompt: "look at x".into(),
                reply: Some("x is fine".into()),
            }],
            "now fix y",
        );
        let (request, earlier) = split_prompt("now fix y", &engine);
        assert_eq!(request, "now fix y");
        assert!(earlier.contains("User (turn 1):\nlook at x"));
        assert!(earlier.contains("Coder (turn 1):\nx is fine"));
        assert!(!earlier.contains("now fix y"));
        assert_eq!(split_prompt("a", "other"), ("other".into(), String::new()));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn the_fingerprint_moves_with_an_edit_and_is_none_outside_git() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(fingerprint(dir.path()).await, None);
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .args(args)
                .current_dir(dir.path())
                .output()
                .unwrap()
        };
        git(&["init", "-q"]);
        let empty = fingerprint(dir.path()).await.unwrap();
        assert_eq!(fingerprint(dir.path()).await, Some(empty));
        std::fs::write(dir.path().join("a.txt"), "one").unwrap();
        let one = fingerprint(dir.path()).await.unwrap();
        assert_ne!(one, empty);
        std::fs::write(dir.path().join("a.txt"), "one more").unwrap();
        assert_ne!(fingerprint(dir.path()).await.unwrap(), one);
    }
}
