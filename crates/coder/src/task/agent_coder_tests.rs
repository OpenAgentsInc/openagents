//! The steering loop through the host (#10800), offline: a recorded plan,
//! recorded Coder turns, and recorded judgments.

use super::*;
use crate::task::agent_steer::{Judgment, Mind, Move, Plan, ScriptedJudge, ScriptedPlanner, Step};
use serde_json::json;
use std::time::Instant;

fn clock() -> u64 {
    1_791_158_400
}

fn owner() -> Principal {
    Principal {
        device: "owner".into(),
        grant: None,
        epoch: None,
    }
}

/// Coder running `command` to `exit`.
fn run(command: &str, exit: i32, output: &str) -> Vec<CoderEvent> {
    vec![
        CoderEvent::Tool {
            name: "Run".into(),
            input: json!(command),
            output: serde_json::Value::Null,
            running: true,
            delegation: None,
        },
        CoderEvent::Tool {
            name: "Run".into(),
            input: json!(command),
            output: json!({"command": command, "exit": exit, "output": output}),
            running: false,
            delegation: None,
        },
    ]
}

fn turn(events: Vec<CoderEvent>, reply: &str) -> coder_v1::Scripted {
    coder_v1::Scripted {
        events,
        ended: Some(Ended::Finished {
            reply: reply.into(),
            tokens: 0,
        }),
        ..coder_v1::Scripted::default()
    }
}

fn judged(done: f64, unsupported: f64, next: Move) -> Judgment {
    Judgment {
        done,
        unsupported,
        next,
        by: String::new(),
    }
}

fn steps(steps: &[(&str, &str)], verify: Option<&str>) -> Plan {
    Plan {
        understanding: "The owner wants this done.".into(),
        answer_directly: false,
        reply_if_direct: None,
        steps: steps
            .iter()
            .map(|(prompt, done_when)| Step {
                prompt: (*prompt).into(),
                done_when: (*done_when).into(),
            })
            .collect(),
        verify: verify.map(str::to_owned),
    }
}

/// What a test reads after a request: every turn Coder was given, and how
/// many times the host made an engine.
struct Seen {
    given: Arc<Mutex<Vec<coder_v1::Turn>>>,
    engines: Arc<Mutex<u32>>,
    asked: Arc<Mutex<Vec<crate::task::agent_steer::Ask>>>,
}

/// A host whose agent `alice` plans `plan`, plays `turns` one per prompt,
/// judges with `judgments`, and reports `report`.
fn host(
    dir: &tempfile::TempDir,
    plan: Plan,
    turns: Vec<coder_v1::Scripted>,
    judgments: Vec<Judgment>,
    report: Option<&str>,
) -> (Agents, Seen) {
    let root = dir.path().join("host");
    let workspace = dir.path().join("work");
    std::fs::create_dir_all(&workspace).unwrap();
    let store = Store::new(&root, "alice").unwrap();
    store.open(&workspace, clock()).unwrap();
    let sequence = coder_v1::Sequence::new(turns);
    let seen = Seen {
        given: sequence.given.clone(),
        engines: Arc::default(),
        asked: Arc::default(),
    };
    let engines = seen.engines.clone();
    let engine: EngineFactory = Arc::new(move |_record: &Record| {
        *engines.lock().unwrap() += 1;
        Ok((
            Box::new(sequence.clone()) as Box<dyn coder_v1::Engine>,
            "Coder V1 (recorded)".to_string(),
        ))
    });
    let asked = seen.asked.clone();
    let report = report.map(str::to_owned);
    let mind: crate::task::agent_steer::MindFactory = Arc::new(move |_record: &Record| {
        Ok(Mind {
            planner: Box::new(ScriptedPlanner {
                plan: Some(plan.clone()),
                report: report.clone(),
                asked: asked.clone(),
            }),
            judge: Some(Box::new(ScriptedJudge {
                judgments: judgments.clone().into(),
                states: Arc::default(),
            })),
            unjudged: String::new(),
        })
    });
    let agents = Agents::new(&root, dir.path().join("tasks"), BTreeMap::new())
        .with_engine(engine)
        .with_mind(mind)
        .with_coder_state(dir.path().join("coder"))
        .with_clock(clock);
    (agents, seen)
}

fn ask(agents: &Agents, key: &str, text: &str) {
    agents
        .answer(
            key,
            &owner(),
            &Operation::AskAgent {
                agent: "alice".into(),
                text: text.into(),
                workspace: None,
                context: String::new(),
                mode: Mode::Terminal,
                typist: false,
            },
        )
        .unwrap();
}

fn view(agents: &Agents) -> wire::AgentView {
    agents.list().agents.into_iter().next().unwrap()
}

fn until(agents: &Agents, done: impl Fn(&wire::AgentView) -> bool) -> wire::AgentView {
    let start = Instant::now();
    loop {
        let seen = view(agents);
        if done(&seen) {
            return seen;
        }
        assert!(start.elapsed() < Duration::from_secs(20), "{seen:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn finished(agents: &Agents) -> wire::AgentView {
    until(agents, |v| !v.busy && !v.headline.is_empty())
}

fn journal(dir: &tempfile::TempDir) -> Vec<Entry> {
    Store::new(&dir.path().join("host"), "alice")
        .unwrap()
        .journal(200)
        .unwrap()
}

fn prompts(seen: &Seen) -> Vec<String> {
    seen.given
        .lock()
        .unwrap()
        .iter()
        .map(|t| t.prompt.clone())
        .collect()
}

#[test]
fn a_question_she_can_answer_never_reaches_coder() {
    let dir = tempfile::tempdir().unwrap();
    let plan = Plan {
        understanding: "The owner asks what I worked on.".into(),
        answer_directly: true,
        reply_if_direct: Some("I ran the atif tests this morning, and they passed.".into()),
        steps: vec![],
        verify: None,
    };
    let (agents, seen) = host(&dir, plan, vec![], vec![], None);
    ask(&agents, "k1", "what did you work on today?");
    let view = finished(&agents);
    assert_eq!(view.headline, "answered");
    assert!(
        view.lines
            .iter()
            .any(|l| l == "alice: I ran the atif tests this morning, and they passed.")
    );
    assert_eq!(*seen.engines.lock().unwrap(), 0, "no Coder engine was made");
    assert!(seen.given.lock().unwrap().is_empty());
    let kinds: Vec<Kind> = journal(&dir).into_iter().map(|e| e.kind).collect();
    assert!(kinds.contains(&Kind::Plan) && kinds.contains(&Kind::Report));
    assert!(!kinds.contains(&Kind::Prompt));
    // Her own call carries her definition; Coder's never does.
    let asked = seen.asked.lock().unwrap();
    assert!(asked[0].system.contains("You are alice"));
}

#[test]
fn one_step_runs_is_judged_done_and_she_reports_in_her_voice() {
    let dir = tempfile::tempdir().unwrap();
    let (agents, seen) = host(
        &dir,
        steps(
            &[(
                "Run the atif tests with cargo test -p atif and say whether they pass.",
                "cargo test -p atif ran and its result is reported",
            )],
            None,
        ),
        vec![turn(
            run("cargo test -p atif", 0, "31 passed"),
            "All 31 atif tests pass.",
        )],
        vec![judged(0.95, 0.02, Move::Continue)],
        Some("The atif tests pass: all 31 of them. Nothing needed fixing."),
    );
    ask(&agents, "k1", "run the atif tests and tell me if they pass");
    let view = finished(&agents);
    assert_eq!(view.headline, "ok exit 0");
    for line in [
        "alice: asking Coder to run the atif tests with cargo test -p atif and say whether they pass",
        "alice: $ cargo test -p atif",
        "alice: that step is done.",
        "alice: The atif tests pass: all 31 of them. Nothing needed fixing.",
    ] {
        assert!(
            view.lines.iter().any(|l| l == line),
            "{line} in {:?}",
            view.lines
        );
    }
    assert_eq!(prompts(&seen).len(), 1);
    let rows = journal(&dir);
    let kinds: Vec<Kind> = rows.iter().map(|e| e.kind).collect();
    for kind in [
        Kind::Request,
        Kind::Plan,
        Kind::Prompt,
        Kind::Ran,
        Kind::Judgment,
        Kind::Report,
    ] {
        assert!(kinds.contains(&kind), "{kind:?} in {kinds:?}");
    }
    let judgment = rows.iter().find(|e| e.kind == Kind::Judgment).unwrap();
    assert!(judgment.text.contains("done 0.95"), "{}", judgment.text);
    let report = agents.reports();
    assert_eq!(report[0].headline, "alice: ok exit 0");
}

#[test]
fn a_failed_step_is_corrected_and_then_passes() {
    let dir = tempfile::tempdir().unwrap();
    let (agents, seen) = host(
        &dir,
        steps(
            &[("Make the atif tests pass.", "cargo test -p atif exits 0")],
            None,
        ),
        vec![
            turn(
                run("cargo test -p atif", 101, "1 failed"),
                "One test fails.",
            ),
            turn(
                run("cargo test -p atif", 0, "31 passed"),
                "Fixed it; all pass now.",
            ),
        ],
        vec![
            judged(0.05, 0.0, Move::Correct),
            judged(0.9, 0.0, Move::Continue),
        ],
        Some("One atif test failed, and Coder fixed it. All 31 pass now."),
    );
    ask(&agents, "k1", "make the atif tests pass");
    let view = finished(&agents);
    assert_eq!(view.headline, "ok exit 0");
    let prompts = prompts(&seen);
    assert_eq!(prompts.len(), 2);
    assert!(
        prompts[1].starts_with("That didn't work (`cargo test -p atif` exited 101)"),
        "{}",
        prompts[1]
    );
    assert!(
        view.lines
            .iter()
            .any(|l| l == "alice: that failed with exit 101, so I'm asking Coder to fix it.")
    );
}

#[test]
fn an_unsupported_claim_is_caught_and_verified() {
    let dir = tempfile::tempdir().unwrap();
    let (agents, seen) = host(
        &dir,
        steps(
            &[("Run the atif tests.", "cargo test -p atif ran")],
            Some("cargo test -p atif exits 0"),
        ),
        vec![
            turn(vec![], "All atif tests passed."),
            turn(run("cargo test -p atif", 0, "31 passed"), "31 passed."),
            turn(
                run("cargo test -p atif", 0, "31 passed"),
                "Still 31 passed.",
            ),
        ],
        vec![
            judged(0.7, 0.9, Move::Continue),
            judged(0.95, 0.0, Move::Continue),
            judged(0.95, 0.0, Move::Continue),
        ],
        Some("The atif tests pass; I had Coder show it."),
    );
    ask(&agents, "k1", "do the atif tests pass?");
    let view = finished(&agents);
    assert_eq!(view.headline, "ok exit 0");
    let prompts = prompts(&seen);
    assert_eq!(prompts.len(), 3, "{prompts:?}");
    assert!(
        prompts[1].starts_with("Show me the evidence"),
        "{}",
        prompts[1]
    );
    assert!(prompts[2].starts_with("Check the result without changing anything"));
    assert!(
        view.lines
            .iter()
            .any(|l| l
                == "alice: Coder says it worked but didn't show it, so I'm asking it to check.")
    );
    assert!(view.lines.iter().any(|l| l == "alice: the check holds."));
}

#[test]
fn her_policy_confirms_a_routine_approval_without_the_owner() {
    let dir = tempfile::tempdir().unwrap();
    let mut events = vec![CoderEvent::Approval {
        id: 1,
        command: "cargo fmt".into(),
        why: "it rewrites files".into(),
    }];
    events.extend(run("cargo fmt", 0, ""));
    let (agents, _seen) = host(
        &dir,
        steps(
            &[("Format the code with cargo fmt.", "cargo fmt exits 0")],
            None,
        ),
        vec![turn(events, "Formatted.")],
        vec![judged(0.9, 0.0, Move::Continue)],
        None,
    );
    ask(&agents, "k1", "format the code");
    let view = finished(&agents);
    assert_eq!(view.headline, "ok exit 0");
    let rows = journal(&dir);
    assert!(
        rows.iter()
            .any(|e| e.kind == Kind::Confirmed && e.text.contains("by her policy")),
        "{rows:?}"
    );
    assert!(!rows.iter().any(|e| e.kind == Kind::Proposed));
    assert!(
        view.lines
            .iter()
            .any(|l| l == "alice: confirmed cargo fmt under my policy")
    );
}

#[test]
fn a_risky_approval_goes_to_the_owner_and_a_rejection_ends_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut events = vec![CoderEvent::Approval {
        id: 1,
        command: "rm -rf target".into(),
        why: "it deletes files".into(),
    }];
    events.extend(run("rm -rf target", 0, ""));
    let (agents, seen) = host(
        &dir,
        steps(&[("Clean the build directory.", "target is gone")], None),
        vec![turn(events, "I left target alone.")],
        vec![judged(0.1, 0.0, Move::FollowUp)],
        None,
    );
    ask(&agents, "k1", "clean the build");
    let pending = until(&agents, |v| v.pending.is_some()).pending.unwrap();
    assert_eq!(pending.command, "rm -rf target");
    agents
        .answer(
            "a1",
            &owner(),
            &Operation::AnswerAgent {
                agent: "alice".into(),
                step: pending.step,
                confirm: false,
            },
        )
        .unwrap();
    let view = finished(&agents);
    assert_eq!(view.headline, "rejected");
    assert_eq!(prompts(&seen).len(), 1, "no follow-up works around it");
    let kinds: Vec<Kind> = journal(&dir).into_iter().map(|e| e.kind).collect();
    assert!(kinds.contains(&Kind::Proposed) && kinds.contains(&Kind::Rejected));
}

#[test]
fn what_she_never_does_never_reaches_coder() {
    // A plan step that asks for it is refused before Coder is asked.
    let dir = tempfile::tempdir().unwrap();
    let (agents, seen) = host(
        &dir,
        steps(
            &[("Push the branch to origin.", "the branch is on origin")],
            None,
        ),
        vec![],
        vec![],
        None,
    );
    ask(&agents, "k1", "push my branch");
    let view = finished(&agents);
    assert_eq!(view.headline, "refused");
    assert_eq!(*seen.engines.lock().unwrap(), 0);
    assert!(seen.given.lock().unwrap().is_empty());
    assert!(journal(&dir).iter().any(|e| e.kind == Kind::Refused));
    assert!(
        view.lines
            .iter()
            .any(|l| l == "alice: I won't ask Coder to push; that's never mine to do.")
    );

    // Coder proposing it is answered no by her policy, without the owner.
    let dir = tempfile::tempdir().unwrap();
    let mut events = vec![CoderEvent::Approval {
        id: 1,
        command: "git push origin main".into(),
        why: "it publishes".into(),
    }];
    events.extend(run("git push origin main", 0, ""));
    let (agents, _seen) = host(
        &dir,
        steps(&[("Commit the change.", "the change is committed")], None),
        vec![turn(events, "I committed it and didn't push.")],
        vec![judged(0.9, 0.0, Move::Continue)],
        None,
    );
    ask(&agents, "k1", "commit it");
    let view = finished(&agents);
    assert!(view.pending.is_none());
    let rows = journal(&dir);
    assert!(
        rows.iter()
            .any(|e| e.kind == Kind::Rejected && e.text.contains("I never push"))
    );
    assert!(!rows.iter().any(|e| e.kind == Kind::Proposed));
    assert!(!rows.iter().any(|e| e.kind == Kind::Ran));
}

#[test]
fn her_coder_session_carries_no_persona() {
    let dir = tempfile::tempdir().unwrap();
    let (agents, seen) = host(
        &dir,
        steps(&[("Run git status.", "git status ran")], None),
        vec![turn(run("git status", 0, "clean"), "The tree is clean.")],
        vec![judged(0.9, 0.0, Move::Continue)],
        None,
    );
    ask(&agents, "k1", "is my tree clean?");
    finished(&agents);
    let record = Store::new(&dir.path().join("host"), "alice")
        .unwrap()
        .load()
        .unwrap()
        .unwrap();
    for given in seen.given.lock().unwrap().iter() {
        assert_eq!(given.session, "alice-coder");
        assert_eq!(given.instructions, None);
        assert!(given.approvals);
        assert!(!given.prompt.contains("You are"), "{}", given.prompt);
        assert!(!given.prompt.contains(&record.charter));
        assert!(!given.prompt.contains("alice"), "{}", given.prompt);
    }
}

/// Coder V1 itself, as a program that writes what it was run with: her
/// session gets no instructions, so no session file can carry them.
#[cfg(unix)]
#[test]
fn the_coder_command_she_runs_has_no_instructions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let program = dir.path().join("openagents");
    let argv = dir.path().join("argv");
    std::fs::write(
        &program,
        format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\"; done > {}\n\
             echo '{{\"event\":\"finished\",\"reply\":\"ok\",\"tokens\":1}}'\n",
            argv.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (agents, _seen) = host(
        &dir,
        steps(&[("Say hello.", "Coder answered")], None),
        vec![],
        vec![judged(0.9, 0.0, Move::Continue)],
        None,
    );
    let cli = coder_v1::Cli {
        program,
        env: Vec::new(),
    };
    let agents = agents.with_engine(Arc::new(move |_: &Record| {
        Ok((
            Box::new(cli.clone()) as Box<dyn coder_v1::Engine>,
            "Coder V1".to_string(),
        ))
    }));
    ask(&agents, "k1", "say hello");
    assert_eq!(finished(&agents).headline, "answered");
    let argv = std::fs::read_to_string(argv).unwrap();
    let words: Vec<&str> = argv.lines().collect();
    assert!(words.windows(2).any(|w| w == ["--session", "alice-coder"]));
    assert!(!words.contains(&"--instructions"), "{words:?}");
    assert!(!argv.contains("You are"));
}
