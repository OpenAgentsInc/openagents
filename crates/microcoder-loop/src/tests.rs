//! The loop with a fake model, a fake Jev, and a fake environment.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::Duration;

use serde_json::Value;

use crate::env::Env;
use knowledge::search::Retriever;
use knowledge::{Base, Entry};

use crate::models::{
    Generate, Generated, Judge, Judgment, NextAction, QuestionSet, conform_set, coverage_set,
    credible_set, dispute_set, knowledge_set, question_set, requirements_set, route_set,
    target_set,
};
use crate::run::{Ending, Event, Limits, Models, Observer, run};
use crate::state::{CommandResult, State};

struct Script {
    replies: RefCell<VecDeque<Result<NextAction, String>>>,
    prompts: RefCell<Vec<String>>,
    /// Each reply's cost; `None` is an unpriced reply.
    usd: Option<f64>,
    /// Each unpriced reply's bound, when it has one.
    upper: Option<f64>,
}

impl Script {
    fn new(replies: Vec<Result<NextAction, String>>) -> Self {
        Script {
            replies: RefCell::new(replies.into()),
            prompts: RefCell::new(Vec::new()),
            usd: Some(0.01),
            upper: None,
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
            usd: self.usd,
            known_usd: self.usd.unwrap_or(0.0),
            cost_unknown: self.usd.is_none().then(|| "fake has no price".to_string()),
            usd_upper: self.usd.or(self.upper),
            cost_basis: crate::models::Basis::ListPrice,
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
    /// Every coverage answer.
    uncovered: f64,
    /// The answer to `progress` each step, when set.
    progress: Option<f64>,
    /// The answer to `repeating` each step, when set.
    repeating: Option<f64>,
    /// The `complete` score each step, as a fraction, when set.
    complete: Option<f64>,
    /// Every requirements answer.
    unchecked: f64,
    /// Every credibility answer.
    doubt: f64,
    /// The numeric-target answers: `target`, then `measured`.
    target: (f64, f64),
    relevance: Vec<(&'static str, f64)>,
    /// The id of every question set asked.
    asked: RefCell<Vec<String>>,
}

fn jev(hard: f64) -> Jev {
    Jev {
        hard,
        wrong: 0.1,
        contradicts: 0.1,
        uncovered: 0.1,
        progress: None,
        repeating: None,
        complete: None,
        unchecked: 0.1,
        doubt: 0.1,
        target: (0.1, 0.9),
        relevance: Vec::new(),
        asked: RefCell::new(Vec::new()),
    }
}

impl Judge for Jev {
    async fn judge(&self, set: &QuestionSet, state: &Value) -> Judgment {
        self.asked.borrow_mut().push(set.id.clone());
        let answers = if set.id == route_set().id {
            vec![("hard".to_string(), self.hard)]
        } else if set.id == coverage_set().id {
            vec![("uncovered".to_string(), self.uncovered)]
        } else if set.id == requirements_set().id {
            set.questions
                .iter()
                .map(|q| (q.id.clone(), self.unchecked))
                .collect()
        } else if set.id == credible_set().id {
            vec![("doubt".to_string(), self.doubt)]
        } else if set.id == target_set().id {
            vec![
                ("target".to_string(), self.target.0),
                ("measured".to_string(), self.target.1),
            ]
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
            let mut answers = vec![("done".to_string(), 0.3)];
            if let Some(p) = self.progress {
                answers.push(("progress".to_string(), p));
            }
            if let Some(p) = self.repeating {
                answers.push(("repeating".to_string(), p));
            }
            answers
        };
        let scores = if set.id == question_set().id {
            self.complete
                .map(|c| vec![(crate::models::COMPLETE.to_string(), c)])
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        Judgment {
            answers,
            scores,
            usd: Some(0.001),
            cost_unknown: None,
            usd_upper: Some(0.001),
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
        if path == "/tmp/acceptance/c.sh" {
            return Some("check c".to_string());
        }
        if path == "/tmp/acceptance/b.sh" {
            return Some("check b".to_string());
        }
        if path == "/tmp/oracle/o.sh" {
            return Some("check o".to_string());
        }
        if path == "/tmp/oracle/t.sh" {
            return Some("true".to_string());
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
                output: if self.ran.borrow().iter().any(|c| c == "write c") {
                    "/tmp/acceptance/a.sh\n/tmp/acceptance/b.sh\n/tmp/acceptance/c.sh\n"
                } else {
                    "/tmp/acceptance/a.sh\n/tmp/acceptance/b.sh\n"
                }
                .to_string(),
            };
        }
        if command.starts_with("ls -1 /tmp/oracle") {
            return CommandResult {
                command: command.to_string(),
                exit: Some(0),
                timed_out: false,
                seconds: 0.0,
                output: "/tmp/oracle/o.sh\n/tmp/oracle/t.sh\n".to_string(),
            };
        }
        // The tests `check b` and `check o` fail until the model has run
        // `fix b` and `fix o`.
        let fixed = |what: &str| self.ran.borrow().iter().any(|c| c == what);
        let fails = command.starts_with("fail")
            || (command == "check b" && !fixed("fix b"))
            || (command == "check o" && !fixed("fix o"));
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
        reply: String::new(),
        ask: crate::models::Ask::None,
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
        task: "Make the thing work well.".to_string(),
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
    assert!((outcome.model_usd.unwrap() - 0.02).abs() < 1e-9);
    assert!((outcome.jev_usd.unwrap() - 0.002).abs() < 1e-9);
    assert!((outcome.usd.unwrap() - 0.022).abs() < 1e-9);
    assert_eq!(outcome.usd_upper, outcome.usd);
    assert!(outcome.cost_unknown.is_empty());
    assert!(matches!(log.0.last(), Some(Event::Ended { .. })));
}

/// A generator that records when it is warmed and asked.
struct Warmed {
    script: Script,
    calls: RefCell<Vec<String>>,
}

impl Generate for Warmed {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        self.calls
            .borrow_mut()
            .push(format!("generate {}", system.len()));
        self.script.generate(system, prompt).await
    }

    fn warm(&self, system: &str) {
        self.calls
            .borrow_mut()
            .push(format!("warm {}", system.len()));
    }
}

#[tokio::test]
async fn the_first_judgment_can_run_beside_the_first_generation() {
    let script = Script::new(vec![
        Ok(act("look", &["ls"], false)),
        Ok(act("done", &[], true)),
    ]);
    let beside = Limits {
        first_judgment_beside: true,
        ..plain()
    };
    let (_, outcome, ran, log) = go(&script, &beside).await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert_eq!(ran, ["ls"]);
    let prompts = script.prompts.borrow().clone();
    // The first prompt says nothing has been judged; the second carries
    // the judgment made after the first step's command ran.
    assert!(prompts[0].contains(crate::run::FIRST_STEP_JEV));
    assert!(!prompts[0].contains("- done: probability"));
    assert!(prompts[1].contains("- done: probability 0.30"));
    // Both judgments are still asked, recorded, and paid for.
    let judged = log
        .0
        .iter()
        .filter(|event| matches!(event, Event::Judged { .. }))
        .count();
    assert_eq!(judged, 2);
    assert!((outcome.jev_usd.unwrap() - 0.002).abs() < 1e-9);
    // Off by default: the first prompt carries the judgment.
    let script = Script::new(vec![Ok(act("done", &[], true))]);
    go(&script, &plain()).await;
    assert!(script.prompts.borrow()[0].contains("- done: probability 0.30"));
}

#[tokio::test]
async fn the_generator_is_warmed_before_the_first_step_and_while_commands_run() {
    let generator = Warmed {
        script: Script::new(vec![
            Ok(act("look", &["ls"], false)),
            Ok(act("done", &[], true)),
        ]),
        calls: RefCell::new(Vec::new()),
    };
    let env = Fake {
        ran: RefCell::new(Vec::new()),
    };
    let set = question_set();
    let route = route_set();
    let judge = jev(0.1);
    let models = Models {
        generator: &generator,
        judge: &judge,
        set: &set,
        route: &route,
        strong: None,
        knowledge: None,
    };
    let limits = plain();
    let (_, outcome) = run(
        state(),
        "Solve this task.",
        &env,
        &models,
        &limits,
        &mut Log::default(),
    )
    .await;
    assert_eq!(outcome.ending, Ending::Finished);
    let system = crate::run::system_prompt(false, &limits).len();
    assert_eq!(
        *generator.calls.borrow(),
        [
            format!("warm {system}"),
            format!("generate {system}"),
            format!("warm {system}"),
            format!("generate {system}"),
        ]
    );
}

#[tokio::test]
async fn an_unpriced_reply_leaves_the_cost_unknown_not_zero() {
    let mut script = Script::new(vec![
        Ok(act("look", &["ls"], false)),
        Ok(act("done", &[], true)),
    ]);
    script.usd = None;
    let (_, outcome, _, _) = go(&script, &plain()).await;
    assert_eq!(outcome.model_usd, None);
    assert_eq!(outcome.usd, None);
    // Jev's part is still known, and counts toward the lower bound.
    assert!((outcome.known_usd - 0.002).abs() < 1e-9);
    assert_eq!(outcome.cost_unknown.len(), 2);
    assert_eq!(outcome.cost_unknown[0].at, "step 1 model");
    assert_eq!(outcome.cost_unknown[0].reason, "fake has no price");
    assert_eq!(
        outcome.usd_upper, None,
        "an unbounded call leaves no upper bound"
    );
    let record = serde_json::to_value(&outcome).unwrap();
    assert!(record["model_usd"].is_null());
    assert!(record["usd"].is_null());
    assert!(record["usd_upper"].is_null());
    assert!(record["cost_unknown"][0]["usd_upper"].is_null());
}

#[tokio::test]
async fn bounded_unpriced_replies_give_the_run_an_upper_bound() {
    let mut script = Script::new(vec![
        Ok(act("look", &["ls"], false)),
        Ok(act("done", &[], true)),
    ]);
    script.usd = None;
    script.upper = Some(0.07);
    let (_, outcome, _, _) = go(&script, &plain()).await;
    assert_eq!(outcome.usd, None);
    // Jev's $0.002 is known; each of the two model calls is at most $0.07.
    assert!((outcome.known_usd - 0.002).abs() < 1e-9);
    assert!((outcome.usd_upper.unwrap() - 0.142).abs() < 1e-9);
    let record = serde_json::to_value(&outcome).unwrap();
    assert_eq!(record["cost_unknown"][1]["at"], "step 2 model");
    assert!((record["cost_unknown"][1]["usd_upper"].as_f64().unwrap() - 0.07).abs() < 1e-12);
    assert_eq!(
        crate::run::total_text(&outcome),
        "cost unknown, between $0.0020 and $0.1420 (2 calls unpriced)"
    );
}

#[tokio::test]
async fn the_retrieval_mode_names_embeddings_lexical_or_mixed() {
    use crate::run::retrieval_summary;
    let script = Script::new(vec![Ok(act("done", &[], true))]);
    let mut outcome = go(&script, &plain()).await.1;
    assert_eq!(
        retrieval_summary(false, None, None, &outcome)["mode"],
        "off"
    );
    let lexical = retrieval_summary(true, None, Some("no key"), &outcome);
    assert_eq!(
        (lexical["mode"].as_str(), lexical["reason"].as_str()),
        (Some("lexical"), Some("no key"))
    );
    let embedder = Some(("openai", "openai/text-embedding-3-small", "list_price"));
    outcome.embedding_searches = 3;
    let all = retrieval_summary(true, embedder, None, &outcome);
    assert_eq!(all["mode"], "embeddings");
    assert_eq!(all["embedding_provider"], "openai");
    assert!(all["reason"].is_null());
    outcome.lexical_searches = 1;
    outcome.lexical_reasons = vec!["the embeddings call failed: 402".to_string()];
    let mixed = retrieval_summary(true, embedder, None, &outcome);
    assert_eq!(mixed["mode"], "mixed");
    assert_eq!(mixed["reason"], "the embeddings call failed: 402");
    outcome.embedding_searches = 0;
    assert_eq!(
        retrieval_summary(true, embedder, None, &outcome)["mode"],
        "lexical"
    );
    // An opted-in Vertex AI embedder is named by provider and model.
    let vertex = knowledge::search::Embedder::with_vertex(knowledge::search::vertex::Vertex::new(
        "http://127.0.0.1:9",
        knowledge::search::vertex::Token::Gcloud,
    ));
    outcome.embedding_searches = 2;
    outcome.lexical_searches = 0;
    let named = retrieval_summary(
        true,
        Some((vertex.provider.as_str(), &vertex.model, vertex.basis())),
        None,
        &outcome,
    );
    assert_eq!(named["mode"], "embeddings");
    assert_eq!(named["embedding_provider"], "vertex");
    assert_eq!(named["embedding_model"], "vertex/text-embedding-005");
    assert_eq!(named["embedding_cost_basis"], "list_price");
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

/// A Coder run for a person has no step or time budget: one that keeps
/// making progress goes on past every limit Coder used to set (24 steps
/// for a chat run, 40 and 80 at the delegate door, 100 in the issue
/// flow's policy) until the model finishes.
#[tokio::test]
async fn a_run_with_no_budget_goes_past_the_old_limits() {
    for judged in [true, false] {
        let mut replies: Vec<Result<NextAction, String>> = (1..=130)
            .map(|n| {
                let command = format!("echo part {n}");
                Ok(act("next part", &[command.as_str()], false))
            })
            .collect();
        replies.push(Ok(act("done", &[], true)));
        let script = Script::new(replies);
        let limits = Limits {
            acceptance: false,
            ..Limits::unbounded()
        };
        assert_eq!((limits.max_steps, limits.max_seconds), (None, None));
        // With Jev judging every step progressing, and without Jev (the
        // rule sees each command new).
        let mut judge = jev(0.1);
        if judged {
            judge.progress = Some(0.8);
            judge.repeating = Some(0.1);
        }
        let (_, outcome, ran, _) = go_with(&script, &limits, &judge, None).await;
        assert_eq!(outcome.ending, Ending::Finished, "judged {judged}");
        assert_eq!(outcome.steps, 131);
        assert_eq!(ran.len(), 130);
    }
}

/// The stuck guard alone ends a run that repeats a failed approach
/// without progress: after its window of judged steps in a row, before
/// the next model call.
#[tokio::test]
async fn a_repeating_run_without_progress_still_ends() {
    let script = Script::new(Vec::new());
    let limits = Limits {
        acceptance: false,
        ..Limits::unbounded()
    };
    let mut judge = jev(0.1);
    judge.progress = Some(0.2);
    judge.repeating = Some(0.9);
    let (_, outcome, ran, _) = go_with(&script, &limits, &judge, None).await;
    assert!(
        matches!(&outcome.ending, Ending::Stuck(why) if why.contains("8 steps in a row")),
        "{:?}",
        outcome.ending
    );
    // Step 1's judgment has nothing to judge; steps 2 to 9 are judged
    // stuck, and step 9 never reaches the model.
    assert_eq!(ran.len(), crate::run::STUCK_STEPS);
    assert_eq!(script.prompts.borrow().len(), crate::run::STUCK_STEPS);
}

/// Without Jev's answers, the rule decides: the same command with the
/// same output, step after step, is stuck too. A window under the minimum
/// is raised to it, so the guard is never trigger-happy.
#[tokio::test]
async fn the_stuck_guard_falls_back_to_the_rule_and_has_a_minimum_window() {
    let script = Script::new(Vec::new());
    let limits = Limits {
        acceptance: false,
        stuck_steps: Some(1),
        ..Limits::unbounded()
    };
    let (_, outcome, ran, _) = go(&script, &limits).await;
    assert!(matches!(outcome.ending, Ending::Stuck(_)));
    assert_eq!(ran.len(), crate::run::MIN_STUCK_STEPS + 1);
    // Progress in between resets the count: Jev judges each step moving
    // forward, so the same command never ends the run.
    let mut replies: Vec<Result<NextAction, String>> = (0..20)
        .map(|_| Ok(act("again", &["true"], false)))
        .collect();
    replies.push(Ok(act("done", &[], true)));
    let script = Script::new(replies);
    let mut judge = jev(0.1);
    judge.progress = Some(0.7);
    judge.repeating = Some(0.9);
    let (_, outcome, _, _) = go_with(&script, &limits, &judge, None).await;
    assert_eq!(outcome.ending, Ending::Finished);
}

/// Each step's judgment carries Jev's estimate of how much of the task is
/// complete, asked in the same request as its Nouls.
#[tokio::test]
async fn each_judgment_carries_the_completion_estimate() {
    let script = Script::new(vec![
        Ok(act("look", &["ls"], false)),
        Ok(act("done", &[], true)),
    ]);
    let mut judge = jev(0.1);
    judge.complete = Some(0.4);
    let (_, _, _, log) = go_with(&script, &plain(), &judge, None).await;
    let estimates: Vec<Option<f64>> = log
        .0
        .iter()
        .filter_map(|event| match event {
            Event::Judged { judgment, .. } => Some(judgment.complete()),
            _ => None,
        })
        .collect();
    assert_eq!(estimates, [Some(0.4), Some(0.4)]);
    let recorded = serde_json::to_value(&log.0[0]).unwrap();
    assert_eq!(recorded["judgment"]["scores"][0][0], "complete");
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
    assert!((outcome.jev_usd.unwrap() - 0.003).abs() < 1e-9);
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

#[tokio::test]
async fn uncovered_requirements_prompt_new_tests_that_a_later_freeze_adds() {
    let script = Script::new(vec![
        Ok(freeze("write tests", &["fix b"])),
        Ok(freeze("add a test", &["write c"])),
        Ok(act("done", &[], true)),
    ]);
    let jev = Jev {
        uncovered: 0.9,
        ..jev(0.1)
    };
    let limits = Limits {
        max_steps: Some(3),
        ..Limits::default()
    };
    let (state, _, _, log) = go_with(&script, &limits, &jev, None).await;
    let prompts = script.prompts.into_inner();
    assert!(prompts[1].contains("Jev judged that the task states something no test checks"));
    assert!(prompts[2].contains("Step 2 added 1 frozen tests: c.sh."));
    assert_eq!(state.tests.len(), 3);
    // Checked once at the first green step and again after the addition.
    let covered = log
        .0
        .iter()
        .filter(|e| matches!(e, Event::Covered { .. }))
        .count();
    assert_eq!(covered, 2);
}

#[tokio::test]
async fn passing_tests_dont_end_a_run_that_is_still_making_progress() {
    let script = Script::new(vec![Ok(freeze("write tests", &["fix b"]))]);
    let jev = Jev {
        progress: Some(0.9),
        ..jev(0.1)
    };
    let limits = Limits {
        max_steps: Some(30),
        ..Limits::default()
    };
    let (_, outcome, _, _) = go_with(&script, &limits, &jev, None).await;
    // Not at 6 passing steps; only at three times that.
    assert_eq!(outcome.ending, Ending::TestsHeld);
    assert_eq!(outcome.steps, 18);
}

fn gated(gates: crate::gate::Gates) -> Limits {
    Limits {
        max_steps: Some(20),
        gates,
        ..Limits::default()
    }
}

fn gate_checks(log: &Log) -> Vec<(String, bool)> {
    log.0
        .iter()
        .filter_map(|e| match e {
            Event::Gated { checked, .. } => Some((checked.check.clone(), checked.refused)),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn with_the_gates_off_a_green_finish_ends_the_run_at_once() {
    let script = Script::new(vec![
        Ok(freeze("write tests", &["fix b"])),
        Ok(act("done", &[], true)),
    ]);
    let jev = Jev {
        unchecked: 0.9,
        doubt: 0.9,
        ..jev(0.1)
    };
    let (_, outcome, _, log) = go_with(&script, &Limits::default(), &jev, None).await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert_eq!(outcome.steps, 2);
    assert!(gate_checks(&log).is_empty());
    let asked = jev.asked.into_inner();
    assert!(!asked.contains(&requirements_set().id));
    assert!(!asked.contains(&credible_set().id));
}

#[tokio::test]
async fn an_unchecked_statement_sends_a_green_finish_back_and_is_named_once() {
    let script = Script::new(vec![
        Ok(freeze("write tests", &["fix b"])),
        Ok(act("done", &[], true)),
        Ok(freeze("add a test", &["write c"])),
        Ok(act("done", &[], true)),
    ]);
    let jev = Jev {
        unchecked: 0.9,
        ..jev(0.1)
    };
    let gates = crate::gate::Gates {
        requirements: true,
        ..crate::gate::Gates::default()
    };
    let (_, outcome, _, log) = go_with(&script, &gated(gates), &jev, None).await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert_eq!(outcome.steps, 4);
    // Sent back at step 2; at step 4 the only statement was already named.
    assert_eq!(
        gate_checks(&log),
        [
            ("requirements".to_string(), true),
            ("requirements".to_string(), false)
        ]
    );
    let prompts = script.prompts.into_inner();
    assert!(prompts[2].contains(
        "no test checks these statements of the task:\n1. \"Make the thing work well.\""
    ));
}

#[tokio::test]
async fn held_tests_go_back_while_the_models_reasoning_doubts_the_solution() {
    let script = Script::new(vec![Ok(freeze("write tests", &["fix b"]))]);
    let jev = Jev {
        doubt: 0.9,
        ..jev(0.1)
    };
    let gates = crate::gate::Gates {
        credible: true,
        ..crate::gate::Gates::default()
    };
    let limits = Limits {
        max_steps: Some(40),
        ..gated(gates)
    };
    let (_, outcome, _, log) = go_with(&script, &limits, &jev, None).await;
    // Held at step 6, sent back twice, then ended six green steps later.
    assert_eq!(outcome.ending, Ending::TestsHeld);
    assert_eq!(outcome.steps, 18);
    assert_eq!(
        gate_checks(&log),
        [
            ("credible".to_string(), true),
            ("credible".to_string(), true),
        ]
    );
    let prompts = script.prompts.into_inner();
    assert!(prompts[6].contains("your own recent reasoning doubts the solution"));
}

#[tokio::test]
async fn the_credibility_check_asks_the_model_to_say_so_in_the_system_text() {
    struct System(RefCell<Vec<String>>);
    impl Generate for System {
        async fn generate(&self, system: &str, _prompt: &str) -> Generated {
            self.0.borrow_mut().push(system.to_string());
            Script::new(vec![Ok(act("done", &[], true))])
                .generate(system, "")
                .await
        }
    }
    let seen = System(RefCell::new(Vec::new()));
    let gates = crate::gate::Gates {
        credible: true,
        ..crate::gate::Gates::default()
    };
    let limits = Limits {
        max_steps: Some(1),
        ..gated(gates)
    };
    let env = Fake {
        ran: RefCell::new(Vec::new()),
    };
    let set = question_set();
    let route = route_set();
    let judge = jev(0.1);
    let models = Models {
        generator: &seen,
        judge: &judge,
        set: &set,
        route: &route,
        strong: None,
        knowledge: None,
    };
    let _ = run(
        state(),
        "Solve this task.",
        &env,
        &models,
        &limits,
        &mut Log::default(),
    )
    .await;
    assert!(seen.0.borrow()[0].ends_with(crate::run::CREDIBLE_SYSTEM));
}

#[tokio::test]
async fn an_unmeasured_numeric_target_sends_the_run_back_once() {
    let script = Script::new(vec![
        Ok(freeze("write tests", &["fix b"])),
        Ok(act("done", &[], true)),
        Ok(act("done", &[], true)),
    ]);
    let jev = Jev {
        target: (0.9, 0.1),
        ..jev(0.1)
    };
    let gates = crate::gate::Gates {
        target: true,
        ..crate::gate::Gates::default()
    };
    let (_, outcome, _, log) = go_with(&script, &gated(gates), &jev, None).await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert_eq!(outcome.steps, 3);
    assert_eq!(gate_checks(&log), [("target".to_string(), true)]);
}

#[tokio::test]
async fn adversarial_rounds_stop_at_their_count() {
    let script = Script::new(vec![
        Ok(freeze("write tests", &["fix b"])),
        Ok(act("done", &[], true)),
        Ok(act("done", &[], true)),
        Ok(act("done", &[], true)),
    ]);
    let gates = crate::gate::Gates {
        adversarial: 2,
        ..crate::gate::Gates::default()
    };
    let (_, outcome, _, log) = go_with(&script, &gated(gates), &jev(0.1), None).await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert_eq!(outcome.steps, 4);
    assert_eq!(
        gate_checks(&log),
        [
            ("adversarial".to_string(), true),
            ("adversarial".to_string(), true)
        ]
    );
    assert!(script.prompts.into_inner()[2].contains("try to break the solution"));
}

#[tokio::test]
async fn adversarial_rounds_stop_once_the_budget_share_is_spent() {
    let script = Script::new(vec![
        Ok(freeze("write tests", &["fix b"])),
        Ok(act("done", &[], true)),
    ]);
    let gates = crate::gate::Gates {
        adversarial: 2,
        budget_fraction: 0.0,
        ..crate::gate::Gates::default()
    };
    let (_, outcome, _, log) = go_with(&script, &gated(gates), &jev(0.1), None).await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert_eq!(outcome.steps, 2);
    assert!(gate_checks(&log).is_empty());
}

#[tokio::test]
async fn the_oracle_is_written_first_and_blocks_the_finish_until_it_passes() {
    let script = Script::new(vec![
        // The oracle session: one step that writes the checks.
        Ok(act("write the oracle", &["cat > /tmp/oracle/o.sh"], true)),
        // The loop.
        Ok(freeze("write tests", &["fix b"])),
        Ok(act("done", &[], true)),
        Ok(act("fix o", &["fix o"], false)),
        Ok(act("done", &[], true)),
    ]);
    let gates = crate::gate::Gates {
        oracle: true,
        ..crate::gate::Gates::default()
    };
    let (state, outcome, _, log) = go_with(&script, &gated(gates), &jev(0.1), None).await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert_eq!(outcome.steps, 4);
    // `t.sh` passed on the untouched workspace, so only `o.sh` was kept.
    let report = log.0.iter().find_map(|e| match e {
        Event::Oracle { report } => Some(report.clone()),
        _ => None,
    });
    let report = report.expect("an oracle event");
    assert_eq!(report.kept, ["oracle-o.sh"]);
    assert_eq!(report.trivial, ["oracle-t.sh"]);
    let names: Vec<&str> = state.tests.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["oracle-o.sh", "a.sh", "b.sh"]);
    let prompts = script.prompts.into_inner();
    assert!(prompts[0].contains("Don't solve the task"));
    assert!(prompts[0].contains("The session has 8 steps"));
    assert!(prompts[1].contains("independent checks from the task's statement alone"));
    assert!(prompts[1].contains("oracle-o.sh"));
    assert!(prompts[3].contains("## oracle-o.sh: FAIL"));
    // The oracle session's reply is priced into the model's spend.
    assert!((outcome.model_usd.unwrap() - 0.05).abs() < 1e-9);
}

fn asking(question: &str) -> NextAction {
    NextAction {
        reply: question.to_string(),
        ask: crate::models::Ask::Question,
        ..act("the user must choose", &[], false)
    }
}

#[tokio::test]
async fn a_question_ends_the_turn_only_when_someone_can_answer() {
    let script = Script::new(vec![Ok(asking("One line or two?"))]);
    let limits = Limits {
        ask: true,
        ..plain()
    };
    let (_, outcome, ran, _) = go(&script, &limits).await;
    assert_eq!(
        outcome.ending,
        Ending::Asked {
            ask: crate::models::Ask::Question
        }
    );
    assert!(ran.is_empty());
    // Nobody answers a benchmark: the step is told so and the loop goes on.
    let script = Script::new(vec![
        Ok(asking("One line or two?")),
        Ok(act("decided", &[], true)),
    ]);
    let (state, outcome, _, _) = go(&script, &plain()).await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert_eq!(outcome.steps, 2);
    assert!(state.actions.len() >= 2);
    // A question must run nothing.
    let mut with_commands = asking("One line or two?");
    with_commands.commands = vec!["ls".into()];
    let script = Script::new(vec![Ok(with_commands), Ok(act("done", &[], true))]);
    let (_, outcome, ran, _) = go(&script, &limits).await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert_eq!(ran, ["ls"]);
}

/// Every step is approved in advance (#10104): a step that still asks for
/// approval, even with someone there to answer, never ends the turn; its
/// commands run, the next step is told the approval stands, and the run
/// finishes with no question to the user.
#[tokio::test]
async fn an_approval_ask_is_granted_and_the_run_goes_on() {
    let limits = Limits {
        ask: true,
        ..plain()
    };
    let mut push = act("push the note", &["git push origin HEAD:main"], false);
    push.reply = "May I push to main?".into();
    push.ask = crate::models::Ask::Approval;
    let script = Script::new(vec![Ok(push), Ok(act("pushed", &[], true))]);
    let (_, outcome, ran, _) = go(&script, &limits).await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert_eq!(ran, ["git push origin HEAD:main"]);
    let prompts = script.prompts.into_inner();
    assert!(
        prompts[1].contains("every step is already approved"),
        "{}",
        prompts[1]
    );
    // An approval ask with nothing to run is granted too: the turn goes on.
    let mut bare = asking("May I push to main?");
    bare.ask = crate::models::Ask::Approval;
    let script = Script::new(vec![Ok(bare), Ok(act("pushed", &[], true))]);
    let (_, outcome, _, _) = go(&script, &limits).await;
    assert_eq!(outcome.ending, Ending::Finished);
    // The model is told every step is approved, and never offered approval.
    let system = crate::run::system_prompt(false, &limits);
    assert!(system.contains("Every step is already approved"));
    assert!(system.contains("Never ask for permission or confirmation"));
    let schema = crate::models::next_action_schema();
    assert_eq!(
        schema["properties"]["ask"]["enum"],
        serde_json::json!(["none", "question"])
    );
}

/// [`Fake`], stopped from outside once `stop` is set: by a command named
/// `stop here`, or by whoever holds the flag.
struct Stoppable<'a> {
    fake: Fake,
    stop: &'a std::cell::Cell<bool>,
}

impl Env for Stoppable<'_> {
    async fn read(&self, path: &str) -> Option<String> {
        self.fake.read(path).await
    }

    async fn run(&self, command: &str, deadline: Duration) -> CommandResult {
        if command == "stop here" {
            self.stop.set(true);
        }
        self.fake.run(command, deadline).await
    }

    fn stopped(&self) -> bool {
        self.stop.get()
    }
}

/// A model whose second call is cut short by a stop: it sets the flag and
/// comes back as an interrupted request, the way a task owner's recorded
/// generator does.
struct CutShort<'a> {
    calls: std::cell::Cell<usize>,
    stop: &'a std::cell::Cell<bool>,
}

impl Generate for CutShort<'_> {
    async fn generate(&self, _system: &str, _prompt: &str) -> Generated {
        self.calls.set(self.calls.get() + 1);
        if self.calls.get() == 1 {
            return Generated {
                action: Ok(act("work", &["make"], false)),
                model: "fake".into(),
                prompt_tokens: 10,
                completion_tokens: 5,
                usd: Some(0.01),
                known_usd: 0.01,
                cost_unknown: None,
                usd_upper: Some(0.01),
                cost_basis: crate::models::Basis::ListPrice,
                milliseconds: 1,
            };
        }
        self.stop.set(true);
        crate::failover::refused_generation(
            "fake",
            true,
            "The task was cancelled or reached its host deadline.",
        )
    }
}

fn generated_events(log: &Log) -> Vec<&Generated> {
    log.0
        .iter()
        .filter_map(|event| match event {
            Event::Generated { generated, .. } => Some(generated),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_stop_while_a_command_runs_ends_the_loop_with_no_further_model_call() {
    let stop = std::cell::Cell::new(false);
    let script = Script::new(vec![
        Ok(act("work", &["stop here", "never run"], false)),
        Ok(act("done", &[], true)),
    ]);
    let env = Stoppable {
        fake: Fake {
            ran: RefCell::new(Vec::new()),
        },
        stop: &stop,
    };
    let mut log = Log::default();
    let (set, route) = (question_set(), route_set());
    let models = Models {
        generator: &script,
        judge: &jev(0.1),
        set: &set,
        route: &route,
        strong: None,
        knowledge: None,
    };
    let (state, outcome) = run(
        state(),
        "Solve this task.",
        &env,
        &models,
        &plain(),
        &mut log,
    )
    .await;
    assert_eq!(outcome.ending, Ending::Stopped);
    assert_eq!(
        script.prompts.borrow().len(),
        1,
        "no model call after the stop"
    );
    assert_eq!(*env.fake.ran.borrow(), ["stop here"]);
    assert_eq!(state.actions[0].skipped, ["never run"]);
    let generated = generated_events(&log);
    assert_eq!(generated.len(), 1);
    assert!(
        generated.iter().all(|g| g.action.is_ok()),
        "no failed reply"
    );
    assert!(matches!(log.0.last(), Some(Event::Ended { .. })));
    assert_eq!(
        serde_json::to_value(&outcome).unwrap()["ending"]["reason"],
        "stopped"
    );
}

#[tokio::test]
async fn a_model_call_cut_short_by_a_stop_is_interrupted_not_failed() {
    let stop = std::cell::Cell::new(false);
    let generator = CutShort {
        calls: std::cell::Cell::new(0),
        stop: &stop,
    };
    let env = Stoppable {
        fake: Fake {
            ran: RefCell::new(Vec::new()),
        },
        stop: &stop,
    };
    let mut log = Log::default();
    let (set, route) = (question_set(), route_set());
    let models = Models {
        generator: &generator,
        judge: &jev(0.1),
        set: &set,
        route: &route,
        strong: None,
        knowledge: None,
    };
    let (_, outcome) = run(
        state(),
        "Solve this task.",
        &env,
        &models,
        &plain(),
        &mut log,
    )
    .await;
    assert_eq!(outcome.ending, Ending::Stopped);
    assert_eq!(generator.calls.get(), 2, "the stop admits no third call");
    assert_eq!(*env.fake.ran.borrow(), ["make"]);
    // The interrupted call is no reply: only the first step's is recorded.
    let generated = generated_events(&log);
    assert_eq!(generated.len(), 1);
    assert!(generated[0].action.is_ok());
    // Its cost stays unknown in the outcome, not zero.
    assert_eq!(outcome.model_usd, None);
    assert!(
        outcome
            .cost_unknown
            .iter()
            .any(|unknown| unknown.at == "step 2 model"
                && unknown.reason == "interrupted model request may still consume tokens")
    );
    let ended = log
        .0
        .iter()
        .filter(|event| matches!(event, Event::Ended { .. }))
        .count();
    assert_eq!(ended, 1);
}

#[tokio::test]
async fn a_stop_before_the_first_step_makes_no_model_call() {
    let stop = std::cell::Cell::new(true);
    let script = Script::new(vec![Ok(act("done", &[], true))]);
    let env = Stoppable {
        fake: Fake {
            ran: RefCell::new(Vec::new()),
        },
        stop: &stop,
    };
    let mut log = Log::default();
    let (set, route) = (question_set(), route_set());
    let judge = jev(0.1);
    let models = Models {
        generator: &script,
        judge: &judge,
        set: &set,
        route: &route,
        strong: None,
        knowledge: None,
    };
    let (_, outcome) = run(
        state(),
        "Solve this task.",
        &env,
        &models,
        &plain(),
        &mut log,
    )
    .await;
    assert_eq!(outcome.ending, Ending::Stopped);
    assert!(script.prompts.borrow().is_empty());
    assert!(judge.asked.borrow().is_empty());
    assert_eq!(outcome.steps, 0);
}

/// [`Fake`], with messages from the person: each is sent after the
/// command that names it runs.
struct Steered {
    fake: Fake,
    after: Vec<(&'static str, &'static str)>,
    waiting: RefCell<Vec<String>>,
}

impl Env for Steered {
    async fn read(&self, path: &str) -> Option<String> {
        self.fake.read(path).await
    }

    async fn run(&self, command: &str, deadline: Duration) -> CommandResult {
        for (after, text) in &self.after {
            if *after == command {
                self.waiting.borrow_mut().push((*text).to_owned());
            }
        }
        self.fake.run(command, deadline).await
    }

    fn steering(&self) -> Vec<String> {
        std::mem::take(&mut *self.waiting.borrow_mut())
    }
}

#[tokio::test]
async fn a_message_sent_while_the_turn_runs_is_read_at_its_next_step() {
    // The person writes while the last command runs; the model had already
    // said it was finished, so the turn goes on to answer them.
    let script = Script::new(vec![
        Ok(act("look", &["ls"], false)),
        Ok(act("wrap up", &["make"], true)),
        Ok(act("use tabs", &["fix tabs"], true)),
    ]);
    let env = Steered {
        fake: Fake {
            ran: RefCell::new(Vec::new()),
        },
        after: vec![
            ("ls", "Use tabs, not spaces."),
            ("make", "And keep the README."),
        ],
        waiting: RefCell::new(Vec::new()),
    };
    let set = question_set();
    let route = route_set();
    let models = Models {
        generator: &script,
        judge: &jev(0.1),
        set: &set,
        route: &route,
        strong: None,
        knowledge: None,
    };
    let mut log = Log::default();
    let (state, outcome) = run(
        state(),
        "Solve this task.",
        &env,
        &models,
        &plain(),
        &mut log,
    )
    .await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert_eq!(env.fake.ran.into_inner(), ["ls", "make", "fix tabs"]);
    let prompts = script.prompts.borrow().clone();
    assert!(!prompts[0].contains("Messages from the user"));
    assert!(prompts[1].contains("- Before step 2: Use tabs, not spaces."));
    // Every later step still reads it, with the next one.
    assert!(prompts[2].contains("- Before step 2: Use tabs, not spaces."));
    assert!(prompts[2].contains("- Before step 3: And keep the README."));
    assert_eq!(
        state.steering,
        [
            (2, "Use tabs, not spaces.".to_owned()),
            (3, "And keep the README.".to_owned())
        ]
    );
}

/// A run whose state carries checks the host froze before it (the
/// delegate recipe, #10208), with `check b` failing until `fix b` ran.
async fn go_checked(script: &Script, limits: &Limits) -> (State, crate::run::Outcome, Vec<String>) {
    let env = Fake {
        ran: RefCell::new(Vec::new()),
    };
    let mut log = Log::default();
    let (set, route, jev) = (question_set(), route_set(), jev(0.1));
    let models = Models {
        generator: script,
        judge: &jev,
        set: &set,
        route: &route,
        strong: None,
        knowledge: None,
    };
    let mut start = state();
    start.tests = vec![crate::state::Test {
        name: "check b".to_string(),
        script: "check b".to_string(),
        passed_at_freeze: Some(false),
    }];
    start.frozen_at = Some(0);
    let (state, outcome) = run(start, "Solve this task.", &env, &models, limits, &mut log).await;
    (state, outcome, env.ran.into_inner())
}

#[tokio::test]
async fn host_frozen_checks_end_the_run_once_they_pass() {
    let script = Script::new(vec![
        Ok(act("look", &["ls"], false)),
        Ok(act("fix", &["fix b"], false)),
        Ok(act("review", &["ls"], false)),
    ]);
    let limits = Limits {
        checks_stop: Some(1),
        ..plain()
    };
    let (state, outcome, ran) = go_checked(&script, &limits).await;
    assert_eq!(outcome.ending, Ending::ChecksPassed);
    assert_eq!(outcome.steps, 2);
    assert!(state.test_results.iter().all(CommandResult::ok));
    // After step 1 (failing) and step 2 (passing), and never again.
    assert_eq!(ran.iter().filter(|c| *c == "check b").count(), 2);
}

#[tokio::test]
async fn a_finish_on_the_step_that_makes_the_checks_pass_is_a_finish() {
    let script = Script::new(vec![Ok(act("fix", &["fix b"], true))]);
    let limits = Limits {
        checks_stop: Some(1),
        ..plain()
    };
    let (_, outcome, ran) = go_checked(&script, &limits).await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert_eq!(ran, vec!["fix b".to_string(), "check b".to_string()]);
}

#[tokio::test]
async fn without_checks_stop_frozen_checks_never_run_in_a_plain_loop() {
    let script = Script::new(vec![
        Ok(act("fix", &["fix b"], false)),
        Ok(act("done", &[], true)),
    ]);
    let (_, outcome, ran) = go_checked(&script, &plain()).await;
    assert_eq!(outcome.ending, Ending::Finished);
    assert!(!ran.iter().any(|c| c == "check b"));
}
