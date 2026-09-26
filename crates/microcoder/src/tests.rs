//! The loop with a fake model, a fake Jev, and a fake environment.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::Duration;

use serde_json::Value;

use crate::env::Env;
use crate::models::{
    Generate, Generated, Judge, Judgment, NextAction, QuestionSet, question_set, route_set,
};
use crate::run::{Ending, Event, Limits, Models, Observer, run};
use crate::state::{CommandResult, State};

struct Script {
    replies: RefCell<VecDeque<Result<NextAction, String>>>,
    prompts: RefCell<Vec<String>>,
}

impl Script {
    fn new(replies: Vec<Result<NextAction, String>>) -> Self {
        Script {
            replies: RefCell::new(replies.into()),
            prompts: RefCell::new(Vec::new()),
        }
    }
}

impl Generate for Script {
    async fn generate(&self, _system: &str, prompt: &str) -> Generated {
        self.prompts.borrow_mut().push(prompt.to_string());
        let action = self
            .replies
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| Ok(act("again", &["true"], false)));
        Generated {
            action,
            model: "fake".to_string(),
            prompt_tokens: 10,
            completion_tokens: 5,
            usd: 0.01,
            milliseconds: 1,
        }
    }
}

/// Answers `done` 0.3, and `hard` with `hard`.
struct Jev {
    hard: f64,
}

impl Judge for Jev {
    async fn judge(&self, set: &QuestionSet, _state: &Value) -> Judgment {
        let answers = if set.id == route_set().id {
            vec![("hard".to_string(), self.hard)]
        } else {
            vec![("done".to_string(), 0.3)]
        };
        Judgment {
            answers,
            usd: 0.001,
            milliseconds: 1,
            error: None,
        }
    }
}

/// Commands that start with `fail` exit 1; everything else echoes itself.
struct Fake {
    ran: RefCell<Vec<String>>,
}

impl Env for Fake {
    async fn read(&self, path: &str) -> Option<String> {
        if path == "/tmp/acceptance/a.sh" {
            return Some("check a".to_string());
        }
        if path == "/tmp/acceptance/b.sh" {
            return Some("check b".to_string());
        }
        (!path.starts_with("missing")).then(|| format!("contents of {path}"))
    }

    async fn run(&self, command: &str, _deadline: Duration) -> CommandResult {
        self.ran.borrow_mut().push(command.to_string());
        if command.starts_with("ls -1 /tmp/acceptance") {
            return CommandResult {
                command: command.to_string(),
                exit: Some(0),
                timed_out: false,
                seconds: 0.0,
                output: "/tmp/acceptance/a.sh\n/tmp/acceptance/b.sh\n".to_string(),
            };
        }
        // The test `check b` fails until the model has run `fix b`.
        let fails = command.starts_with("fail")
            || (command == "check b" && !self.ran.borrow().iter().any(|c| c == "fix b"));
        CommandResult {
            command: command.to_string(),
            exit: Some(i32::from(fails)),
            timed_out: false,
            seconds: 0.0,
            output: format!("output of {command}"),
        }
    }
}

#[derive(Default)]
struct Log(Vec<Event>);

impl Observer for Log {
    fn event(&mut self, _seconds: f64, event: &Event) {
        self.0.push(event.clone());
    }
}

fn act(rationale: &str, commands: &[&str], finished: bool) -> NextAction {
    NextAction {
        rationale: rationale.to_string(),
        commands: commands.iter().map(|c| (*c).to_string()).collect(),
        view: Vec::new(),
        freeze_tests: false,
        finished,
    }
}

/// Limits without acceptance tests, for the tests of the plain loop.
fn plain() -> Limits {
    Limits {
        acceptance: false,
        ..Limits::default()
    }
}

fn state() -> State {
    State {
        environment: "/app".to_string(),
        task: "Make the thing.".to_string(),
        ..State::default()
    }
}

async fn go(script: &Script, limits: &Limits) -> (State, crate::run::Outcome, Vec<String>, Log) {
    go_with(script, limits, &Jev { hard: 0.1 }, None).await
}

async fn go_with(
    script: &Script,
    limits: &Limits,
    jev: &Jev,
    strong: Option<&Script>,
) -> (State, crate::run::Outcome, Vec<String>, Log) {
    let env = Fake {
        ran: RefCell::new(Vec::new()),
    };
    let mut log = Log::default();
    let set = question_set();
    let route = route_set();
    let models = Models {
        generator: script,
        judge: jev,
        set: &set,
        route: &route,
        strong,
    };
    let (state, outcome) = run(state(), "Solve this task.", &env, &models, limits, &mut log).await;
    let ran = env.ran.into_inner();
    (state, outcome, ran, log)
}

#[tokio::test]
async fn the_loop_stops_when_the_model_finishes() {
    let script = Script::new(vec![
        Ok(act("look", &["ls"], false)),
        Ok(act("done", &[], true)),
    ]);
    let (state, outcome, ran, log) = go(&script, &plain()).await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert_eq!(outcome.steps, 2);
    assert_eq!(ran, ["ls"]);
    assert_eq!(state.actions.len(), 2);
    assert!((outcome.model_usd - 0.02).abs() < 1e-9);
    assert!((outcome.jev_usd - 0.002).abs() < 1e-9);
    assert!(matches!(log.0.last(), Some(Event::Ended { .. })));
}

#[tokio::test]
async fn each_prompt_is_rebuilt_from_state_not_a_conversation() {
    let script = Script::new(vec![
        Ok(act("first reason", &["echo one"], false)),
        Ok(act("done", &[], true)),
    ]);
    go(&script, &plain()).await;
    let prompts = script.prompts.into_inner();
    assert_eq!(prompts.len(), 2);
    assert!(prompts[0].contains("None yet."));
    // The second prompt carries the first step's result as state, and Jev's
    // judgment, not the model's earlier reply as a message.
    assert!(prompts[1].contains("## Step 1") && prompts[1].contains("output of echo one"));
    assert!(prompts[1].contains("- done: probability 0.30"));
    assert!(prompts[1].contains("Rationale: first reason"));
}

#[tokio::test]
async fn a_failing_command_skips_the_rest_and_reaches_the_next_state() {
    let script = Script::new(vec![
        Ok(act("try", &["fail now", "echo never"], false)),
        Ok(act("done", &[], true)),
    ]);
    let (state, _, ran, _) = go(&script, &plain()).await;
    assert_eq!(ran, ["fail now"]);
    assert_eq!(state.actions[0].skipped, ["echo never"]);
    assert!(
        state
            .render_actions()
            .contains("not run: an earlier command failed")
    );
}

#[tokio::test]
async fn the_step_limit_stops_the_loop() {
    let script = Script::new(Vec::new());
    let limits = Limits {
        max_steps: Some(3),
        ..plain()
    };
    let (_, outcome, ran, _) = go(&script, &limits).await;
    assert_eq!(outcome.ending, Ending::StepLimit);
    assert_eq!(ran.len(), 3);
}

#[tokio::test]
async fn the_spend_limit_stops_the_loop() {
    let script = Script::new(Vec::new());
    let limits = Limits {
        max_usd: 0.025,
        ..plain()
    };
    let (_, outcome, _, _) = go(&script, &limits).await;
    assert_eq!(outcome.ending, Ending::SpendLimit);
    assert_eq!(outcome.steps, 3);
}

#[tokio::test]
async fn replies_that_miss_the_format_are_noted_then_stop_the_loop() {
    let script = Script::new(vec![
        Err("not JSON".to_string()),
        Ok(act("ok", &["ls"], false)),
        Err("bad 1".to_string()),
        Err("bad 2".to_string()),
        Err("bad 3".to_string()),
    ]);
    let (state, outcome, _, _) = go(&script, &plain()).await;
    assert_eq!(outcome.ending, Ending::BadReplies("bad 3".to_string()));
    assert!(state.notes[0].contains("Step 3's reply couldn't be used"));
    let prompts = script.prompts.into_inner();
    assert!(prompts[1].contains("# Notes from the host"));
}

#[tokio::test]
async fn files_in_view_appear_in_full_in_the_next_prompt() {
    let mut first = act("read", &["ls"], false);
    first.view = vec![
        "a.py".to_string(),
        "missing.txt".to_string(),
        "a.py".to_string(),
    ];
    let script = Script::new(vec![Ok(first), Ok(act("done", &[], true))]);
    let (state, _, _, _) = go(&script, &plain()).await;
    let prompts = script.prompts.into_inner();
    assert!(
        prompts[0].contains(
            "# Files in view (current: read after the last step's commands ran)\n\nNone."
        )
    );
    assert!(prompts[1].contains("## a.py\n\n```\ncontents of a.py\n```"));
    assert!(prompts[1].contains("## missing.txt\n\n(no such file)"));
    // Duplicates are read once.
    assert_eq!(state.files.len(), 2);
}

#[tokio::test]
async fn an_empty_view_keeps_the_files_in_view() {
    let mut first = act("read", &["ls"], false);
    first.view = vec!["a.py".to_string()];
    let script = Script::new(vec![
        Ok(first),
        Ok(act("run", &["echo hi"], false)),
        Ok(act("done", &[], true)),
    ]);
    go(&script, &plain()).await;
    let prompts = script.prompts.into_inner();
    assert!(prompts[2].contains("## a.py"), "the file stays in view");
}

#[tokio::test]
async fn replies_that_run_nothing_are_noted_then_stop_the_loop() {
    let script = Script::new(vec![
        Ok(act("look", &[], false)),
        Ok(act("work", &["echo hi"], false)),
        Ok(act("look", &[], false)),
        Ok(act("look", &[], false)),
        Ok(act("look", &[], false)),
    ]);
    let (_, outcome, ran, _) = go(&script, &plain()).await;
    assert_eq!(outcome.ending, Ending::Idle);
    assert_eq!(ran, ["echo hi"]);
    let prompts = script.prompts.into_inner();
    assert!(prompts[1].contains("Step 1 ran no commands"));
    // A step that ran something clears the note.
    assert!(!prompts[2].contains("ran no commands and asked"));
}

fn freeze(rationale: &str, commands: &[&str]) -> NextAction {
    NextAction {
        freeze_tests: true,
        ..act(rationale, commands, false)
    }
}

#[tokio::test]
async fn finished_waits_for_the_frozen_tests_to_pass() {
    let script = Script::new(vec![
        Ok(act("done already", &[], true)),
        Ok(freeze("write tests", &["cat > /tmp/acceptance/a.sh"])),
        Ok(act("done", &[], true)),
        Ok(act("fix", &["fix b"], false)),
        Ok(act("done", &[], true)),
    ]);
    let (state, outcome, ran, log) = go(&script, &Limits::default()).await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert_eq!(outcome.steps, 5);
    assert_eq!(state.frozen_at, Some(2));
    // The host ran b at the freeze and after the fix.
    assert_eq!(ran.iter().filter(|c| *c == "check b").count(), 2);
    let prompts = script.prompts.into_inner();
    assert!(prompts[0].contains("None frozen yet"));
    assert!(prompts[1].contains("no acceptance tests are frozen"));
    assert!(prompts[2].contains("## b.sh: FAIL"));
    assert!(prompts[2].contains("The frozen script:\n\n```\n  1  check b\n```"));
    assert!(prompts[3].contains("1 acceptance tests fail"));
    let tested = log
        .0
        .iter()
        .filter(|e| matches!(e, Event::Tested { .. }))
        .count();
    assert_eq!(tested, 2);
}

#[tokio::test]
async fn repeated_refused_finishes_stop_the_loop() {
    let script = Script::new(vec![
        Ok(act("done", &[], true)),
        Ok(act("done", &[], true)),
        Ok(act("done", &[], true)),
    ]);
    let (_, outcome, _, _) = go(&script, &Limits::default()).await;
    assert_eq!(outcome.ending, Ending::Unaccepted);
}

#[tokio::test]
async fn a_hard_task_has_the_stronger_model_write_the_tests() {
    let luna = Script::new(vec![
        Ok(act("fix", &["fix b"], false)),
        Ok(act("done", &[], true)),
    ]);
    let strong = Script::new(vec![
        Ok(act("look", &["ls"], false)),
        Ok(freeze("write tests", &["cat > /tmp/acceptance/a.sh"])),
    ]);
    let (state, outcome, _, log) =
        go_with(&luna, &Limits::default(), &Jev { hard: 0.8 }, Some(&strong)).await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert_eq!(state.frozen_at, Some(2));
    // The stronger model wrote the tests; the default model did the rest.
    assert_eq!(strong.prompts.into_inner().len(), 2);
    assert_eq!(luna.prompts.into_inner().len(), 2);
    assert!(
        log.0
            .iter()
            .any(|e| matches!(e, Event::Assessed { strong: true, .. }))
    );
}

#[tokio::test]
async fn an_easy_task_stays_on_the_default_model() {
    let luna = Script::new(vec![
        Ok(freeze("write tests", &["cat > /tmp/acceptance/a.sh"])),
        Ok(act("fix", &["fix b"], false)),
        Ok(act("done", &[], true)),
    ]);
    let strong = Script::new(Vec::new());
    let (_, outcome, _, _) =
        go_with(&luna, &Limits::default(), &Jev { hard: 0.2 }, Some(&strong)).await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert!(strong.prompts.into_inner().is_empty());
}

#[tokio::test]
async fn the_stronger_model_hands_over_after_its_step_budget() {
    let luna = Script::new(vec![Ok(act("done", &[], true))]);
    let strong = Script::new(Vec::new());
    let limits = Limits {
        strong_steps: 2,
        max_steps: Some(4),
        ..Limits::default()
    };
    let _ = go_with(&luna, &limits, &Jev { hard: 0.9 }, Some(&strong)).await;
    assert_eq!(strong.prompts.into_inner().len(), 2);
}
