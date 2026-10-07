use super::*;
use serde_json::json;
use std::time::Instant;

use super::coder_v1::Scripted as Recorded;

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

/// Coder running `command` to `exit`, as its events report it.
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

/// A recorded Coder turn: `events`, then `reply`.
fn turn(events: Vec<CoderEvent>, reply: &str) -> Recorded {
    Recorded {
        events,
        ended: Some(Ended::Finished {
            reply: reply.into(),
            tokens: 0,
        }),
        ..Recorded::default()
    }
}

/// An engine that plays one recorded turn per request.
fn engine(script: Vec<Recorded>) -> EngineFactory {
    let script = Arc::new(Mutex::new(VecDeque::from(script)));
    Arc::new(move |_record: &Record| {
        let turn = script.lock().unwrap().pop_front().unwrap_or_default();
        Ok((
            Box::new(turn) as Box<dyn coder_v1::Engine>,
            "Coder V1 (recorded)".to_string(),
        ))
    })
}

/// A host whose agent `alice` runs `script` in place of Coder V1, one
/// recorded turn per request.
fn host(dir: &tempfile::TempDir, script: Vec<Recorded>) -> Agents {
    let root = dir.path().join("host");
    let workspace = dir.path().join("work");
    std::fs::create_dir_all(&workspace).unwrap();
    let store = Store::new(&root, "alice").unwrap();
    store.open(&workspace, clock()).unwrap();
    Agents::new(&root, dir.path().join("tasks"), BTreeMap::new())
        .with_engine(engine(script))
        .with_coder_state(dir.path().join("coder"))
        .with_clock(clock)
}

fn ask(agents: &Agents, key: &str, text: &str, typist: bool) -> Result<serde_json::Value, Code> {
    agents.answer(
        key,
        &owner(),
        &Operation::AskAgent {
            agent: "alice".into(),
            text: text.into(),
            workspace: None,
            context: String::new(),
            mode: Mode::Terminal,
            typist,
        },
    )
}

fn view(agents: &Agents) -> wire::AgentView {
    agents.list().agents.into_iter().next().unwrap()
}

/// Waits until `done` holds for her view.
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

#[test]
fn coder_runs_a_read_only_request_and_she_reports_three_ways() {
    let dir = tempfile::tempdir().unwrap();
    let agents = host(
        &dir,
        vec![turn(
            run("echo atif: 31 passed", 0, "atif: 31 passed"),
            "atif: 31 passed.",
        )],
    );
    ask(&agents, "k1", "run the atif tests", false).unwrap();
    // A retry of the same request asks once.
    ask(&agents, "k1", "run the atif tests", false).unwrap();
    let seen = until(&agents, |v| !v.busy && v.headline == "ok exit 0");
    assert!(
        seen.lines
            .iter()
            .any(|l| l == "alice: $ echo atif: 31 passed")
    );
    assert_eq!(
        seen.lines
            .iter()
            .filter(|l| l.contains("atif: 31 passed."))
            .count(),
        1,
        "the reply shows once"
    );
    assert_eq!(seen.service.requests, 1);
    let reports = agents.reports();
    assert_eq!(reports.len(), 1);
    let report = &reports[0];
    assert_eq!(report.headline, "alice: ok exit 0");
    assert_eq!(report.thread, thread_id("alice"));
    assert_eq!(report.subject.len(), 64);
    assert!(
        report.text.contains("echo atif: 31 passed (exit 0)"),
        "{}",
        report.text
    );
    assert!(agents.reports().is_empty(), "drained");
    let Ok(log) = agents.answer(
        "k2",
        &owner(),
        &Operation::AgentLog {
            agent: "alice".into(),
            after: None,
        },
    ) else {
        panic!("log");
    };
    let log: wire::Journal = serde_json::from_value(log).unwrap();
    assert!(log.journal.iter().any(|row| row.kind == "request"));
    let journal =
        std::fs::read_to_string(dir.path().join("host/agents/alice/journal.jsonl")).unwrap();
    assert!(journal.contains("\"from\":\"owner\""));
}

#[test]
fn her_pane_follows_her_coder_session_and_a_key_takes_it_over() {
    let dir = tempfile::tempdir().unwrap();
    let mut working = turn(run("cargo test -p atif", 0, "ok"), "");
    working.hold = true;
    let agents = host(&dir, vec![working]);
    ask(&agents, "k1", "run the atif tests", true).unwrap();
    let seen = until(&agents, |v| v.run.is_some());
    let step = seen.run.unwrap();
    assert!(step.typist);
    let pane = step.coder.expect("a Coder pane");
    assert_eq!(pane.session, "agent-alice");
    assert!(step.command.starts_with("Coder V1 session"));
    let took = |step: u64| Operation::AgentRan {
        agent: "alice".into(),
        step,
        ran: wire::Ran {
            taken_back: true,
            ..wire::Ran::default()
        },
    };
    assert_eq!(
        agents.answer("r0", &owner(), &took(step.step + 7)),
        Err(Code::Conflict)
    );
    agents.answer("r1", &owner(), &took(step.step)).unwrap();
    let ended = until(&agents, |v| !v.busy && v.headline == "taken over");
    assert!(ended.run.is_none());
    let journal = Store::new(&dir.path().join("host"), "alice")
        .unwrap()
        .journal(100)
        .unwrap();
    assert!(journal.iter().any(|e| e.kind == Kind::Takeback));
}

#[test]
fn coders_approvals_wait_for_confirm_or_reject() {
    let dir = tempfile::tempdir().unwrap();
    let asks = |reply: &str| {
        let mut events = vec![CoderEvent::Approval {
            id: 1,
            command: "touch notes.txt".into(),
            why: "it writes a file".into(),
        }];
        events.extend(run("touch notes.txt", 0, ""));
        turn(events, reply)
    };
    let agents = host(&dir, vec![asks("I left it alone."), asks("Made it.")]);
    ask(&agents, "k1", "make a notes file", false).unwrap();
    let seen = until(&agents, |v| v.pending.is_some());
    assert_eq!(seen.activity, Activity::Waiting);
    let proposal = seen.pending.unwrap();
    assert_eq!(proposal.command, "touch notes.txt");
    let answer = |step: u64, confirm: bool| Operation::AnswerAgent {
        agent: "alice".into(),
        step,
        confirm,
    };
    assert_eq!(
        agents.answer("a0", &owner(), &answer(99, false)),
        Err(Code::Conflict)
    );
    agents
        .answer("a1", &owner(), &answer(proposal.step, false))
        .unwrap();
    until(&agents, |v| !v.busy && v.headline == "rejected");

    ask(&agents, "k2", "make a notes file after all", false).unwrap();
    let proposal = until(&agents, |v| v.pending.is_some()).pending.unwrap();
    agents
        .answer("a2", &owner(), &answer(proposal.step, true))
        .unwrap();
    until(&agents, |v| !v.busy && v.headline == "ok exit 0");
    let kinds: Vec<Kind> = Store::new(&dir.path().join("host"), "alice")
        .unwrap()
        .journal(100)
        .unwrap()
        .into_iter()
        .map(|e| e.kind)
        .collect();
    for kind in [Kind::Proposed, Kind::Rejected, Kind::Confirmed, Kind::Ran] {
        assert!(kinds.contains(&kind), "{kind:?} in {kinds:?}");
    }
}

#[test]
fn stop_runs_the_sequence_and_pause_starts_nothing_new() {
    let dir = tempfile::tempdir().unwrap();
    let mut working = turn(run("cargo test -p atif", 0, "ok"), "");
    working.hold = true;
    let agents = host(&dir, vec![working]);
    let store = Store::new(&dir.path().join("host"), "alice").unwrap();
    let jobs = Jobs::new(store.clone());
    jobs.add(
        agent_jobs::template("nightly-check", "", None, None, 0, clock()).unwrap(),
        clock(),
    )
    .unwrap();
    jobs.edit("nightly-check", agent_jobs::Edit::On, clock())
        .unwrap();
    ask(&agents, "k1", "run the atif tests", true).unwrap();
    let seen = until(&agents, |v| v.run.is_some());
    assert_eq!(seen.jobs, [1, 1]);
    agents
        .answer(
            "s1",
            &owner(),
            &Operation::StopAgent {
                agent: "alice".into(),
                reason: "enough".into(),
            },
        )
        .unwrap();
    let stopped = until(&agents, |v| !v.busy);
    assert_eq!(stopped.state, "stopped");
    assert_eq!(stopped.release, 1);
    assert_eq!(stopped.jobs, [0, 1]);
    let journal = store.journal(100).unwrap();
    for step in 1..=4 {
        assert!(
            journal
                .iter()
                .any(|e| e.text.starts_with(&format!("stop {step} of 4"))),
            "step {step}: {journal:?}"
        );
    }
    // Stopped, she takes nothing new.
    assert_eq!(ask(&agents, "k2", "run it", false), Err(Code::Conflict));
    let resume = Operation::ResumeSeat {
        seat: "alice".into(),
    };
    agents.answer("p1", &owner(), &resume).unwrap();
    assert_eq!(view(&agents).state, "active");
    let pause = Operation::PauseSeat {
        seat: "alice".into(),
    };
    agents.answer("p2", &owner(), &pause).unwrap();
    assert_eq!(view(&agents).state, "paused");
    assert_eq!(view(&agents).activity, Activity::Paused);
    assert_eq!(ask(&agents, "k3", "run it", false), Err(Code::Conflict));
    // The state is her record's, so a new host reads it.
    assert_eq!(store.load().unwrap().unwrap().state, State::Paused);
}

#[test]
fn memory_takes_notes_and_waits_for_accepted_preferences() {
    let dir = tempfile::tempdir().unwrap();
    let agents = host(&dir, vec![turn(vec![], "Understood.")]);
    ask(
        &agents,
        "k1",
        "remember that mobile is its own workspace",
        false,
    )
    .unwrap();
    until(&agents, |v| !v.busy && v.headline == "noted");
    ask(
        &agents,
        "k2",
        "always run the tests before you report",
        false,
    )
    .unwrap();
    let seen = until(&agents, |v| !v.busy && v.headline == "answered");
    assert_eq!(seen.candidates, 1);
    let list = |agents: &Agents| -> wire::Memory {
        serde_json::from_value(
            agents
                .answer(
                    "m",
                    &owner(),
                    &Operation::ListAgentMemory {
                        agent: "alice".into(),
                        after: None,
                    },
                )
                .unwrap(),
        )
        .unwrap()
    };
    let memory = list(&agents);
    assert!(
        memory
            .memory
            .iter()
            .any(|m| m.kind == "note" && m.state == "active")
    );
    let candidate = memory
        .memory
        .iter()
        .find(|m| m.state == "candidate")
        .unwrap()
        .id;
    agents
        .answer(
            "m1",
            &owner(),
            &Operation::EditAgentMemory {
                agent: "alice".into(),
                edit: wire::MemoryEdit::Accept { id: candidate },
            },
        )
        .unwrap();
    assert_eq!(view(&agents).candidates, 0);
    let token = format!("ghp_{}", "Zz9".repeat(12));
    let refused = agents.answer(
        "m2",
        &owner(),
        &Operation::EditAgentMemory {
            agent: "alice".into(),
            edit: wire::MemoryEdit::Note {
                text: format!("token {token}"),
            },
        },
    );
    assert_eq!(refused, Err(Code::Conflict));
}

#[test]
fn the_phase_one_record_moves_to_alice() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("host");
    let old = Store::new(&root, agent::LEGACY_NAME).unwrap();
    let mut record = old.open(dir.path(), 5).unwrap();
    record.look = "workshop".into();
    old.save(&record).unwrap();
    let agents = Agents::new(&root, dir.path().join("tasks"), BTreeMap::new()).with_clock(clock);
    assert!(agents.holds("alice"));
    let list = agents.list();
    assert_eq!(list.agents.len(), 1);
    assert_eq!(list.agents[0].name, "alice");
    assert_eq!(list.agents[0].look, "alice");
    assert!(!root.join("agents/ada").exists());
    let journal = Store::new(&root, "alice").unwrap().journal(10).unwrap();
    assert_eq!(journal[0].kind, Kind::Created);
    assert_eq!(journal[1].kind, Kind::Migrated);
}

#[test]
fn modes_and_ids_are_stable() {
    assert_eq!(choose_mode("run the atif tests"), Mode::Terminal);
    assert_eq!(choose_mode("what fails in atif?"), Mode::Terminal);
    assert_eq!(choose_mode("fix the typo in the README"), Mode::Task);
    assert_eq!(choose_mode("add a test for the chunk reader"), Mode::Task);
    assert_eq!(thread_id("alice"), thread_id("alice"));
    assert_eq!(thread_id("alice").len(), 32);
    assert_ne!(subject("h", "alice"), subject("h", "bob"));
}

/// The owner sets her up through `studio.agent.new`: the host offers its
/// checkouts, refuses a path that is not a Git checkout, makes her with a
/// key it attests with the owner key, and she then takes a request.
#[test]
fn the_owner_sets_her_up_and_she_takes_a_request() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("host");
    let app = dir.path().join("app");
    std::fs::create_dir_all(app.join(".git")).unwrap();
    std::fs::create_dir_all(app.join("src")).unwrap();
    let plain = dir.path().join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    let workspaces = BTreeMap::from([("app".to_string(), app.clone())]);
    let agents = Agents::new(&root, dir.path().join("tasks"), workspaces)
        .with_engine(engine(vec![turn(
            run("echo atif: 31 passed", 0, "atif: 31 passed"),
            "atif: 31 passed.",
        )]))
        .with_coder_state(dir.path().join("coder"))
        .with_clock(clock);
    assert!(agents.list().agents.is_empty(), "no agent yet");
    assert_eq!(
        ask(&agents, "k0", "run the atif tests", false),
        Err(Code::Forbidden),
        "nobody to ask before setup"
    );

    // The host offers its workspace.
    let canonical = app.canonicalize().unwrap();
    let places = agents.places();
    assert_eq!(places.places.len(), 1, "{places:?}");
    assert_eq!(places.places[0].path, canonical.display().to_string());
    assert_eq!(places.places[0].from, "the host's workspace app");
    let listed: wire::Places = serde_json::from_value(
        agents
            .answer("k1", &owner(), &Operation::ListAgentWorkspaces {})
            .unwrap(),
    )
    .unwrap();
    assert_eq!(listed, places);

    // A path that is not a Git checkout is refused, and nothing is made.
    for bad in [plain.clone(), dir.path().join("missing")] {
        assert_eq!(agents.create("alice", &bad, None), Err(Code::Malformed));
    }
    assert!(agents.list().agents.is_empty());

    // A folder inside the checkout is a workspace; the owner key attests
    // her own key for a year.
    let owner_key = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    let made: wire::Made = serde_json::from_value(
        agents
            .create("alice", &app.join("src"), Some(&owner_key))
            .unwrap(),
    )
    .unwrap();
    assert!(!made.existed);
    assert_eq!(made.workspace, canonical.join("src").display().to_string());
    assert_eq!(made.attested_until, Some(clock() + 365 * 86_400));
    assert_eq!(made.pubkey.len(), 64);
    let her = view(&agents);
    assert_eq!((her.name.as_str(), her.state.as_str()), ("alice", "active"));
    assert_eq!(her.attested_until, made.attested_until);
    let store = Store::new(&root, "alice").unwrap();
    assert!(store.key().unwrap().is_some());
    let journal = store.journal(10).unwrap();
    assert_eq!(journal[0].kind, Kind::Created);
    assert!(
        journal
            .iter()
            .any(|e| e.text == "the owner set her up at her workstation")
    );

    // Setting her up again changes nothing.
    let again: wire::Made =
        serde_json::from_value(agents.create("alice", &plain, Some(&owner_key)).unwrap()).unwrap();
    assert!(again.existed);
    assert_eq!(again.workspace, made.workspace);
    assert_eq!(again.pubkey, made.pubkey);

    // She takes a request, and her command runs in her workspace.
    ask(&agents, "k2", "run the atif tests", false).unwrap();
    let seen = until(&agents, |v| !v.busy && v.headline == "ok exit 0");
    assert!(
        seen.lines
            .iter()
            .any(|l| l == "alice: $ echo atif: 31 passed")
    );
}

/// Without the owner key on the host, she is made with her own key and no
/// attestation, which the answer says.
#[test]
fn a_host_without_the_owner_key_makes_her_unattested() {
    let dir = tempfile::tempdir().unwrap();
    let app = dir.path().join("app");
    std::fs::create_dir_all(app.join(".git")).unwrap();
    let agents = Agents::new(
        dir.path().join("host"),
        dir.path().join("tasks"),
        BTreeMap::new(),
    )
    .with_clock(clock);
    let made: wire::Made =
        serde_json::from_value(agents.create("alice", &app, None).unwrap()).unwrap();
    assert_eq!(made.attested_until, None);
    assert_eq!(made.pubkey.len(), 64);
    assert_eq!(agents.places().places.len(), 1, "her own workspace");
}
