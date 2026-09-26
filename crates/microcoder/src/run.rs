//! The loop.
//!
//! ```text
//! while next_action isn't finished:
//!     jev_results = jev(state, user_prompt)
//!     prompt      = state + user_prompt + jev_results
//!     next_action = generate(prompt)
//!     run next_action's commands
//! ```
//!
//! Every generation is built fresh from the current state. There's no
//! conversation: no earlier model reply is sent back as a message.

use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::json;

use crate::env::Env;
use crate::models::{Generate, Generated, Judge, Judgment, NextAction, QuestionSet};
use crate::state::{Action, CommandResult, State, Test, cut};

/// What every generation is told, before the prompt.
pub const SYSTEM: &str = "You work on a task by running shell commands in its working \
directory. Each reply is one step: the commands to run next and why. You see the task, the \
environment, what earlier steps ran and printed, and judgments from Jev, a decision model, \
about the state. Treat Jev's judgments as evidence, not orders. Each command is a bash script, \
run in order in the working directory and fed to bash as written, so never wrap it in sh -c or \
bash -c. Commands stop at the first one that fails; nobody answers questions, and there is no \
editor, so write files with heredocs. The Files section shows, in full, the current contents of \
every path in `view`: keep the files you need there instead of printing them with cat, and \
you'll see them after each step's commands run. A non-empty `view` replaces the list; an empty \
one keeps it. Set `finished` to true, with no commands, \
only when the task is complete. Every other step must run at least one command: the \
files in view are already current, so asking to see them again does nothing.";

/// Where the model writes its acceptance tests before they freeze.
pub const ACCEPT_DIR: &str = "/tmp/acceptance";

/// Files kept in view, at most.
pub const VIEW_FILES: usize = 12;

/// Characters of all files in view together, at most.
pub const VIEW_CHARS: usize = 120_000;

/// The default user prompt.
pub const USER_PROMPT: &str = "Solve this task.";

/// When the loop stops, besides a finished action.
#[derive(Clone, Debug, Serialize)]
pub struct Limits {
    /// Steps, at most; `None` means no step limit.
    pub max_steps: Option<usize>,
    pub max_seconds: u64,
    /// Dollars of model and Jev spend.
    pub max_usd: f64,
    /// Seconds one command may run.
    pub command_seconds: u64,
    /// Replies in a row that don't match the format before the loop stops.
    pub max_bad_replies: usize,
    /// Replies in a row that run nothing and change nothing before the
    /// loop stops.
    pub max_idle_replies: usize,
    /// Whether the model defines acceptance tests first and `finished`
    /// waits for them to pass.
    pub acceptance: bool,
    /// Refused `finished` replies before the loop stops anyway.
    pub max_refused_finishes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_steps: None,
            max_seconds: 3_600,
            max_usd: 1.0,
            command_seconds: 300,
            max_bad_replies: 3,
            max_idle_replies: 3,
            acceptance: true,
            max_refused_finishes: 3,
        }
    }
}

/// Why the loop stopped.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "reason", content = "detail")]
pub enum Ending {
    Finished,
    StepLimit,
    TimeLimit,
    SpendLimit,
    BadReplies(String),
    /// Replies in a row ran no commands and asked for nothing new.
    Idle,
    /// The model kept saying it was finished while acceptance tests failed
    /// or before any were frozen.
    Unaccepted,
}

/// What the loop reports as it runs.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    Judged {
        step: usize,
        judgment: Judgment,
    },
    Generated {
        step: usize,
        prompt_chars: usize,
        generated: Generated,
    },
    Ran {
        step: usize,
        result: CommandResult,
    },
    /// The acceptance tests ran; `froze` is true on the run that froze them.
    Tested {
        step: usize,
        froze: bool,
        results: Vec<CommandResult>,
    },
    Ended {
        outcome: Outcome,
    },
}

/// Where events go.
pub trait Observer {
    fn event(&mut self, seconds: f64, event: &Event);
}

/// The run's totals.
#[derive(Clone, Debug, Serialize)]
pub struct Outcome {
    pub ending: Ending,
    pub steps: usize,
    pub seconds: f64,
    pub model_usd: f64,
    pub jev_usd: f64,
}

/// Builds one step's prompt from the state, the user prompt, and Jev's
/// judgment.
#[must_use]
pub fn prompt(state: &State, user_prompt: &str, jev: &str, acceptance: bool) -> String {
    let tests = if acceptance {
        format!(
            "# Acceptance tests\n\n{}\n\n",
            state.render_tests(ACCEPT_DIR)
        )
    } else {
        String::new()
    };
    let mut out = format!(
        "# Task\n\n{}\n\n# Instruction\n\n{user_prompt}\n\n# Environment\n\n{}\n\n# Files in view (current: read after the last step's commands ran)\n\n{}\n\n{tests}# Jev's judgments of the current state\n\n{jev}\n\n# Steps so far\n\n{}",
        state.task,
        state.environment,
        state.render_files(),
        state.render_actions()
    );
    if !state.notes.is_empty() {
        out.push_str("\n\n# Notes from the host\n\n");
        for note in &state.notes {
            out.push_str(&format!("- {note}\n"));
        }
    }
    out
}

/// The state Jev reads.
fn jev_state(state: &State) -> serde_json::Value {
    json!({
        "task": cut(&state.task, 6_000, 0),
        "environment": cut(&state.environment, 1_500, 0),
        "actions": cut(&state.render_actions(), 4_000, crate::models::JEV_STATE_CHARS - 4_000),
        "files_in_view": state.files.iter().map(|(path, _)| path.clone()).collect::<Vec<_>>(),
        "acceptance_tests": state.tests_summary().unwrap_or_else(|| "none frozen yet".to_string()),
    })
}

/// The two model calls and the questions Jev answers.
pub struct Models<'a, G: Generate, J: Judge> {
    pub generator: &'a G,
    pub judge: &'a J,
    pub set: &'a QuestionSet,
}

/// Reads the files the model keeps in view: the first [`VIEW_FILES`]
/// distinct paths, within [`VIEW_CHARS`] together.
async fn read_view<E: Env>(env: &E, paths: &[String]) -> Vec<(String, Option<String>)> {
    let mut seen = Vec::new();
    let mut files = Vec::new();
    let mut total = 0usize;
    for path in paths {
        let path = path.trim().to_string();
        if path.is_empty() || seen.contains(&path) || seen.len() >= VIEW_FILES {
            continue;
        }
        seen.push(path.clone());
        let contents = env.read(&path).await.map(|text| {
            let room = VIEW_CHARS.saturating_sub(total);
            let kept = cut(&text, room, 0);
            total += kept.chars().count();
            kept
        });
        files.push((path, contents));
    }
    files
}

/// Reads the tests the model wrote under [`ACCEPT_DIR`].
async fn load_tests<E: Env>(env: &E, deadline: Duration) -> Vec<Test> {
    let listing = env
        .run(&format!("ls -1 {ACCEPT_DIR}/*.sh 2>/dev/null"), deadline)
        .await;
    let mut tests = Vec::new();
    for path in listing
        .output
        .lines()
        .map(str::trim)
        .filter(|l| l.ends_with(".sh"))
    {
        if let Some(script) = env.read(path).await {
            let name = path.rsplit('/').next().unwrap_or(path).to_string();
            tests.push(Test {
                name,
                script,
                passed_at_freeze: None,
            });
        }
    }
    tests
}

/// Runs every frozen test from the host's own copy.
async fn run_tests<E: Env>(env: &E, tests: &[Test], deadline: Duration) -> Vec<CommandResult> {
    let mut results = Vec::new();
    for test in tests {
        let mut result = env.run(&test.script, deadline).await;
        result.command = test.name.clone();
        results.push(result);
    }
    results
}

/// Runs the loop until the model finishes or a limit stops it.
pub async fn run<E: Env, G: Generate, J: Judge, O: Observer>(
    mut state: State,
    user_prompt: &str,
    env: &E,
    models: &Models<'_, G, J>,
    limits: &Limits,
    observer: &mut O,
) -> (State, Outcome) {
    let started = Instant::now();
    let mut model_usd = 0.0;
    let mut jev_usd = 0.0;
    let mut bad = 0usize;
    let mut idle = 0usize;
    let mut refused = 0usize;
    // Whether a freeze was already sent back for tests that passed before
    // any fix.
    let mut sent_back = false;
    let mut step = 0usize;
    let ending = loop {
        if limits.max_steps.is_some_and(|max| step >= max) {
            break Ending::StepLimit;
        }
        if started.elapsed() >= Duration::from_secs(limits.max_seconds) {
            break Ending::TimeLimit;
        }
        if model_usd + jev_usd >= limits.max_usd {
            break Ending::SpendLimit;
        }
        step += 1;
        let judgment = models.judge.judge(&jev_state(&state)).await;
        jev_usd += judgment.usd;
        let jev_text = judgment.render(models.set);
        observer.event(
            started.elapsed().as_secs_f64(),
            &Event::Judged { step, judgment },
        );
        let text = prompt(&state, user_prompt, &jev_text, limits.acceptance);
        let generated = models.generator.generate(SYSTEM, &text).await;
        model_usd += generated.usd;
        observer.event(
            started.elapsed().as_secs_f64(),
            &Event::Generated {
                step,
                prompt_chars: text.len(),
                generated: generated.clone(),
            },
        );
        let action: NextAction = match generated.action {
            Ok(action) => {
                bad = 0;
                state.notes.clear();
                action
            }
            Err(error) => {
                bad += 1;
                if bad >= limits.max_bad_replies {
                    break Ending::BadReplies(error);
                }
                state.notes.push(format!(
                    "Step {step}'s reply couldn't be used ({}); reply with the JSON object the format asks for.",
                    cut(&error, 300, 0)
                ));
                continue;
            }
        };
        if action.finished && action.commands.is_empty() && !limits.acceptance {
            state.actions.push(Action {
                step,
                rationale: action.rationale,
                results: Vec::new(),
                skipped: Vec::new(),
            });
            break Ending::Finished;
        }
        // A reply that runs nothing and asks for no new file wastes a step.
        let in_view: Vec<&String> = state.files.iter().map(|(path, _)| path).collect();
        let new_file = action
            .view
            .iter()
            .any(|path| !in_view.contains(&&path.trim().to_string()));
        if action.commands.is_empty() && !action.finished && !new_file {
            idle += 1;
            state.actions.push(Action {
                step,
                rationale: action.rationale,
                results: Vec::new(),
                skipped: Vec::new(),
            });
            if idle >= limits.max_idle_replies {
                break Ending::Idle;
            }
            state.notes.push(format!(
                "Step {step} ran no commands and asked for no file that wasn't already in view. \
The Files in view section already holds the current contents of those files, read after the last \
command ran; asking for them again shows nothing new. Run a command that moves the task forward, \
or set finished to true if the task is complete."
            ));
            continue;
        }
        idle = 0;
        let mut results = Vec::new();
        let mut skipped = Vec::new();
        let mut failed = false;
        for command in &action.commands {
            if failed {
                skipped.push(command.clone());
                continue;
            }
            let result = env
                .run(command, Duration::from_secs(limits.command_seconds))
                .await;
            failed = !result.ok();
            observer.event(
                started.elapsed().as_secs_f64(),
                &Event::Ran {
                    step,
                    result: result.clone(),
                },
            );
            results.push(result);
        }
        state.actions.push(Action {
            step,
            rationale: action.rationale,
            results,
            skipped,
        });
        // An empty list keeps the files already in view, read fresh.
        let paths: Vec<String> = if action.view.is_empty() {
            state.files.iter().map(|(path, _)| path.clone()).collect()
        } else {
            action.view.clone()
        };
        state.files = read_view(env, &paths).await;
        if !limits.acceptance {
            if action.finished && !failed {
                break Ending::Finished;
            }
            continue;
        }
        let deadline = Duration::from_secs(limits.command_seconds);
        let mut froze = false;
        if action.freeze_tests && state.frozen_at.is_none() && !failed {
            state.tests = load_tests(env, deadline).await;
            if state.tests.is_empty() {
                state.notes.push(format!(
                    "Step {step} asked to freeze the acceptance tests, but {ACCEPT_DIR} holds no .sh file."
                ));
            } else {
                let results = run_tests(env, &state.tests, deadline).await;
                let passing: Vec<String> = results
                    .iter()
                    .filter(|r| r.ok())
                    .map(|r| r.command.clone())
                    .collect();
                if !passing.is_empty() && !sent_back {
                    // A test that passes on the unchanged code can't show
                    // that a fix worked. Send the freeze back once.
                    sent_back = true;
                    observer.event(
                        started.elapsed().as_secs_f64(),
                        &Event::Tested {
                            step,
                            froze: false,
                            results,
                        },
                    );
                    state.notes.push(format!(
                        "The tests weren't frozen: {} already pass on the unchanged code. The task \
says the code is broken, so a test that passes now either checks something that isn't broken, or \
states the requirement the way the broken code already behaves. Check each one against the task and \
against the standard definition of what it tests. Rewrite it so it fails on the current code, or \
delete it if its requirement truly holds already. Then set freeze_tests to true again; the second \
freeze is final.",
                        passing.join(", ")
                    ));
                    state.tests.clear();
                } else {
                    for (test, result) in state.tests.iter_mut().zip(&results) {
                        test.passed_at_freeze = Some(result.ok());
                    }
                    state.test_results = results;
                    state.frozen_at = Some(step);
                    froze = true;
                }
            }
        }
        let ran_something = !state.actions.last().is_none_or(|a| a.results.is_empty());
        if state.frozen_at.is_some() && (froze || ran_something) {
            if !froze {
                state.test_results = run_tests(env, &state.tests, deadline).await;
            }
            observer.event(
                started.elapsed().as_secs_f64(),
                &Event::Tested {
                    step,
                    froze,
                    results: state.test_results.clone(),
                },
            );
        }
        if action.finished && !failed {
            let failing = state.test_results.iter().filter(|r| !r.ok()).count();
            if state.frozen_at.is_some() && failing == 0 {
                break Ending::Finished;
            }
            refused += 1;
            if refused >= limits.max_refused_finishes {
                break Ending::Unaccepted;
            }
            state.notes.push(if state.frozen_at.is_none() {
                format!(
                    "Step {step} said the task is finished, but no acceptance tests are frozen. \
Write them under {ACCEPT_DIR} and set freeze_tests to true."
                )
            } else {
                format!(
                    "Step {step} said the task is finished, but {failing} acceptance tests fail. \
The task isn't finished until they pass; see the Acceptance tests section."
                )
            });
        }
    };
    let outcome = Outcome {
        ending,
        steps: step,
        seconds: started.elapsed().as_secs_f64(),
        model_usd,
        jev_usd,
    };
    observer.event(
        outcome.seconds,
        &Event::Ended {
            outcome: outcome.clone(),
        },
    );
    (state, outcome)
}
