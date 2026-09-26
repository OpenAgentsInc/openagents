//! The loop with a fake model, a fake Jev, and a fake environment.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::Duration;

use serde_json::Value;

use crate::env::Env;
use knowledge::search::Retriever;
use knowledge::{Base, Entry};

use crate::models::{
    Generate, Generated, Judge, Judgment, NextAction, QuestionSet, conform_set, dispute_set,
    knowledge_set, question_set, route_set,
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

/// Answers `done` 0.3, `hard` with `hard`, and each knowledge candidate
/// with its entry's relevance in `relevance`, or 0.1.
struct Jev {
    hard: f64,
    /// Every dispute answer.
    wrong: f64,
    /// Every conformance answer.
    contradicts: f64,
    relevance: Vec<(&'static str, f64)>,
    /// The id of every question set asked.
    asked: RefCell<Vec<String>>,
}

fn jev(hard: f64) -> Jev {
    Jev {
        hard,
        wrong: 0.1,
        contradicts: 0.1,
        relevance: Vec::new(),
        asked: RefCell::new(Vec::new()),
    }
}

impl Judge for Jev {
    async fn judge(&self, set: &QuestionSet, state: &Value) -> Judgment {
        self.asked.borrow_mut().push(set.id.clone());
        let answers = if set.id == route_set().id {
            vec![("hard".to_string(), self.hard)]
        } else if set.id == conform_set().id {
            set.questions
                .iter()
                .map(|q| (q.id.clone(), self.contradicts))
                .collect()
        } else if set.id == dispute_set().id {
            set.questions
                .iter()
                .map(|q| (q.id.clone(), self.wrong))
                .collect()
        } else if set.id == knowledge_set().id {
            set.questions
                .iter()
                .map(|q| {
                    let id = state[&q.id]["id"].as_str().unwrap_or_default();
                    let p = self
                        .relevance
                        .iter()
                        .find(|(entry, _)| *entry == id)
                        .map_or(0.1, |(_, p)| *p);
                    (q.id.clone(), p)
                })
                .collect()
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
        expand: Vec::new(),
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
    go_with(script, limits, &jev(0.1), None).await
}

async fn go_with(
    script: &Script,
    limits: &Limits,
    jev: &Jev,
    strong: Option<&Script>,
) -> (State, crate::run::Outcome, Vec<String>, Log) {
    go_kb(script, limits, jev, strong, None).await
}

async fn go_kb(
    script: &Script,
    limits: &Limits,
    jev: &Jev,
    strong: Option<&Script>,
    knowledge: Option<&Retriever>,
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
        knowledge,
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
    let (state, outcome, _, log) = go_with(&luna, &routed(), &jev(0.8), Some(&strong)).await;
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
    let (_, outcome, _, _) = go_with(&luna, &routed(), &jev(0.2), Some(&strong)).await;
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
        ..routed()
    };
    let _ = go_with(&luna, &limits, &jev(0.9), Some(&strong)).await;
    assert_eq!(strong.prompts.into_inner().len(), 2);
}

fn entry(id: &str, kind: &str, summary: &str) -> Entry {
    Entry::parse(&format!(
        "---\nid: {id}\nversion: 1\nkind: {kind}\ntitle: Title of {id}\nsummary: {summary}\n\
applies_when: Always.\nstatus: admitted\nauthor: openagents\nprovenance:\n  cites: [A Book]\n---\n\n\
The body of {id}.\n"
    ))
    .unwrap()
}

/// A base of three entries, searched by words alone.
fn base() -> Retriever {
    Retriever::lexical(
        Base {
            entries: vec![
                entry("stats.thing", "method", "How to make the thing correctly."),
                entry("slip.trap", "slip", "A common mistake when making things."),
                entry("shell.other", "tool", "Unrelated shell advice."),
            ],
        },
        "no key in tests",
    )
}

fn relevant(pairs: &[(&'static str, f64)]) -> Jev {
    Jev {
        relevance: pairs.to_vec(),
        ..jev(0.1)
    }
}

#[tokio::test]
async fn kept_entries_appear_in_the_prompt_and_the_system_text() {
    let script = Script::new(vec![Ok(act("done", &[], true))]);
    let kb = base();
    let jev = relevant(&[("stats.thing", 0.7), ("slip.trap", 0.6)]);
    let (state, _, _, log) = go_kb(&script, &plain(), &jev, None, Some(&kb)).await;
    let prompts = script.prompts.into_inner();
    assert!(prompts[0].contains("# Knowledge base"));
    assert!(prompts[0].contains(
        "- stats.thing (method, relevance 0.70; by openagents, admitted): Title of stats.thing. \
How to make the thing correctly."
    ));
    assert!(prompts[0].contains("- slip.trap (slip, relevance 0.60"));
    assert!(!prompts[0].contains("shell.other"), "0.1 is below the bar");
    // Nothing at 0.8 or more, so no body is shown without being asked for.
    assert!(!prompts[0].contains("The body of"));
    assert_eq!(state.knowledge.len(), 2);
    // Jev judged the candidates, then the state.
    assert_eq!(
        jev.asked.into_inner(),
        [knowledge_set().id, question_set().id]
    );
    let Some(Event::Retrieved { retrieval, .. }) =
        log.0.iter().find(|e| matches!(e, Event::Retrieved { .. }))
    else {
        panic!("no retrieval event");
    };
    assert_eq!(retrieval.candidates.len(), 3);
    assert_eq!(retrieval.lexical_only.as_deref(), Some("no key in tests"));
    assert_eq!(retrieval.kept[0].id, "stats.thing");
}

#[tokio::test]
async fn an_expanded_entry_shows_its_body_next_step_and_stays() {
    let mut first = act("read the entry", &["ls"], false);
    first.expand = vec!["stats.thing".to_string(), "no.such".to_string()];
    let script = Script::new(vec![
        Ok(first),
        Ok(act("work", &["echo hi"], false)),
        Ok(act("done", &[], true)),
    ]);
    let kb = base();
    let jev = relevant(&[("stats.thing", 0.7)]);
    go_kb(&script, &plain(), &jev, None, Some(&kb)).await;
    let prompts = script.prompts.into_inner();
    assert!(!prompts[0].contains("The body of stats.thing"));
    assert!(prompts[1].contains("## stats.thing (in full): Title of stats.thing"));
    assert!(prompts[1].contains("The body of stats.thing."));
    assert!(
        prompts[1].contains("Step 1 asked to expand no.such, but the knowledge base has no entry")
    );
    assert!(
        prompts[2].contains("The body of stats.thing."),
        "an empty expand keeps it"
    );
}

#[tokio::test]
async fn a_highly_relevant_slip_is_shown_in_full_unasked() {
    let script = Script::new(vec![Ok(act("done", &[], true))]);
    let kb = base();
    let jev = relevant(&[("slip.trap", 0.85)]);
    go_kb(&script, &plain(), &jev, None, Some(&kb)).await;
    let prompts = script.prompts.into_inner();
    assert!(prompts[0].contains("## slip.trap (in full)"));
    assert!(prompts[0].contains("The body of slip.trap."));
}

#[tokio::test]
async fn with_the_knowledge_base_off_the_prompt_has_no_section() {
    let script = Script::new(vec![
        Ok(act("look", &["ls"], false)),
        Ok(act("done", &[], true)),
    ]);
    let jev = relevant(&[("stats.thing", 0.9)]);
    let (_, outcome, _, log) = go_kb(&script, &plain(), &jev, None, None).await;
    for prompt in script.prompts.into_inner() {
        assert!(!prompt.contains("# Knowledge base"));
    }
    assert!(!log.0.iter().any(|e| matches!(e, Event::Retrieved { .. })));
    assert!(outcome.knowledge.is_empty());
    assert!(!outcome.knowledge_assisted);
    assert!(!jev.asked.into_inner().contains(&knowledge_set().id));
}

#[tokio::test]
async fn the_record_lists_the_entries_used_with_their_digests() {
    let mut first = act("read", &["ls"], false);
    first.expand = vec!["stats.thing".to_string()];
    let script = Script::new(vec![Ok(first), Ok(act("done", &[], true))]);
    let kb = base();
    let jev = relevant(&[("stats.thing", 0.7)]);
    let (_, outcome, _, log) = go_kb(&script, &plain(), &jev, None, Some(&kb)).await;
    let used = &outcome.knowledge;
    assert!(outcome.knowledge_assisted);
    assert_eq!(used.len(), 1);
    assert_eq!(used[0].id, "stats.thing");
    assert_eq!(used[0].digest, kb.base.get("stats.thing").unwrap().digest);
    assert_eq!((used[0].kept_steps, used[0].expanded_steps), (2, 1));
    // The query didn't change, so the second step reused the first's
    // retrieval and asked Jev about candidates once.
    let retrievals: Vec<bool> = log
        .0
        .iter()
        .filter_map(|e| match e {
            Event::Retrieved { retrieval, .. } => Some(retrieval.cached),
            _ => None,
        })
        .collect();
    assert_eq!(retrievals, [false, true]);
    let asked = jev.asked.into_inner();
    assert_eq!(
        asked.iter().filter(|id| **id == knowledge_set().id).count(),
        1
    );
    // Jev's relevance cost is in the run's Jev total: two state judgments
    // and one relevance judgment.
    assert!((outcome.jev_usd - 0.003).abs() < 1e-9);
    let record = serde_json::to_value(&outcome).unwrap();
    assert_eq!(record["knowledge"][0]["id"], "stats.thing");
}

#[tokio::test]
async fn a_reply_that_only_expands_an_entry_is_not_idle() {
    let mut first = act("read the entry", &[], false);
    first.expand = vec!["stats.thing".to_string()];
    let script = Script::new(vec![Ok(first), Ok(act("done", &[], true))]);
    let kb = base();
    let (_, outcome, _, _) = go_kb(&script, &plain(), &jev(0.1), None, Some(&kb)).await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert!(!script.prompts.into_inner()[1].contains("ran no commands"));
}

#[tokio::test]
async fn a_failing_test_jev_judges_wrong_is_dropped_at_finish() {
    // b.sh fails until `fix b` runs; the model never runs it and says the
    // test is wrong.
    let script = Script::new(vec![
        Ok(freeze("write tests", &["cat > /tmp/acceptance/a.sh"])),
        Ok(act("b.sh compares lists of different lengths", &[], true)),
    ]);
    let jev = Jev {
        wrong: 0.9,
        ..jev(0.1)
    };
    let (state, outcome, _, log) = go_with(&script, &Limits::default(), &jev, None).await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert_eq!(state.dropped.len(), 1);
    assert_eq!(state.dropped[0].test.name, "b.sh");
    assert_eq!(state.tests.len(), 1);
    assert!(log.0.iter().any(|e| matches!(
        e,
        Event::Disputed { dropped, .. } if dropped == &["b.sh".to_string()]
    )));
}

#[tokio::test]
async fn a_failing_test_jev_judges_right_still_blocks_finish() {
    let script = Script::new(vec![
        Ok(freeze("write tests", &["cat > /tmp/acceptance/a.sh"])),
        Ok(act("done", &[], true)),
    ]);
    let limits = Limits {
        max_steps: Some(3),
        ..Limits::default()
    };
    let (state, outcome, _, log) = go_with(&script, &limits, &jev(0.1), None).await;
    assert_eq!(outcome.ending, Ending::StepLimit);
    assert!(state.dropped.is_empty());
    assert!(log.0.iter().any(|e| matches!(
        e,
        Event::Disputed { dropped, .. } if dropped.is_empty()
    )));
    let prompts = script.prompts.into_inner();
    assert!(prompts[2].contains("1 acceptance tests fail"));
}

/// Limits with the stronger model writing the tests on a hard task.
fn routed() -> Limits {
    Limits {
        route: crate::run::Route::Auto,
        ..Limits::default()
    }
}

#[tokio::test]
async fn the_stronger_model_is_off_by_default() {
    let luna = Script::new(vec![
        Ok(freeze("write tests", &["cat > /tmp/acceptance/a.sh"])),
        Ok(act("fix", &["fix b"], false)),
        Ok(act("done", &[], true)),
    ]);
    let strong = Script::new(Vec::new());
    let jev = jev(0.9);
    let (_, outcome, _, _) = go_with(&luna, &Limits::default(), &jev, Some(&strong)).await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert!(strong.prompts.into_inner().is_empty());
    // Jev isn't asked whether the task is hard.
    assert!(!jev.asked.borrow().contains(&route_set().id));
}

#[tokio::test]
async fn any_highly_relevant_entry_is_shown_in_full_unasked() {
    let script = Script::new(vec![Ok(act("done", &[], true))]);
    let kb = base();
    let jev = relevant(&[("stats.thing", 0.9)]);
    let _ = go_kb(&script, &plain(), &jev, None, Some(&kb)).await;
    let prompts = script.prompts.into_inner();
    assert!(prompts[0].contains("## stats.thing (in full)"));
    assert!(prompts[0].contains("The body of stats.thing."));
}

#[tokio::test]
async fn passing_tests_nudge_then_end_the_run() {
    let script = Script::new(vec![
        Ok(freeze("write tests", &["fix b"])),
        Ok(act("review", &["ls"], false)),
        Ok(act("review", &["ls"], false)),
        Ok(act("review", &["ls"], false)),
        Ok(act("review", &["ls"], false)),
        Ok(act("review", &["ls"], false)),
        Ok(act("review", &["ls"], false)),
    ]);
    let (_, outcome, _, _) = go(&script, &Limits::default()).await;
    // All tests pass from the freeze at step 1; the sixth passing step ends it.
    assert_eq!(outcome.ending, Ending::TestsHeld);
    assert_eq!(outcome.steps, 6);
    let prompts = script.prompts.into_inner();
    assert!(!prompts[2].contains("Every acceptance test has passed"));
    assert!(prompts[3].contains("Every acceptance test has passed for 3 steps in a row"));
}

#[tokio::test]
async fn a_finish_whose_code_contradicts_an_entry_is_sent_back_once() {
    let mut write = freeze("write tests", &["fix b"]);
    write.view = vec!["thing.py".to_string()];
    let script = Script::new(vec![
        Ok(write),
        Ok(act("done", &[], true)),
        Ok(act("done", &[], true)),
    ]);
    let kb = base();
    let jev = Jev {
        contradicts: 0.9,
        ..relevant(&[("stats.thing", 0.9)])
    };
    let (_, outcome, _, log) = go_kb(&script, &Limits::default(), &jev, None, Some(&kb)).await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert_eq!(outcome.steps, 3);
    let conformed: Vec<&Vec<String>> = log
        .0
        .iter()
        .filter_map(|e| match e {
            Event::Conformed { flagged, .. } => Some(flagged),
            _ => None,
        })
        .collect();
    // Checked once; the second finish is accepted.
    assert_eq!(conformed, [&vec!["stats.thing".to_string()]]);
    let prompts = script.prompts.into_inner();
    assert!(prompts[2].contains("contradicts these knowledge entries: stats.thing"));
}

#[tokio::test]
async fn code_that_follows_the_entries_finishes_at_once() {
    let mut write = freeze("write tests", &["fix b"]);
    write.view = vec!["thing.py".to_string()];
    let script = Script::new(vec![Ok(write), Ok(act("done", &[], true))]);
    let kb = base();
    let jev = relevant(&[("stats.thing", 0.9)]);
    let (_, outcome, _, log) = go_kb(&script, &Limits::default(), &jev, None, Some(&kb)).await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert_eq!(outcome.steps, 2);
    assert!(log.0.iter().any(|e| matches!(
        e,
        Event::Conformed { flagged, .. } if flagged.is_empty()
    )));
}

#[tokio::test]
async fn a_test_stuck_failing_is_checked_without_a_finish() {
    // b.sh fails until `fix b`, which never runs; the model keeps working.
    let script = Script::new(vec![Ok(freeze(
        "write tests",
        &["cat > /tmp/acceptance/a.sh"],
    ))]);
    let jev = Jev {
        wrong: 0.9,
        ..jev(0.1)
    };
    let limits = Limits {
        max_steps: Some(14),
        ..Limits::default()
    };
    let (state, _, _, log) = go_with(&script, &limits, &jev, None).await;
    assert_eq!(state.dropped.len(), 1);
    assert_eq!(state.dropped[0].test.name, "b.sh");
    // Frozen at step 1, failing from step 1; checked at its tenth failing step.
    assert_eq!(state.dropped[0].step, 10);
    let disputes = log
        .0
        .iter()
        .filter(|e| matches!(e, Event::Disputed { .. }))
        .count();
    assert_eq!(disputes, 1);
}
