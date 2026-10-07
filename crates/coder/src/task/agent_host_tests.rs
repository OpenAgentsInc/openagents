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
    assert_eq!(pane.session, "alice-coder");
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
    // The state is her record's, so a new host reads it.
    assert_eq!(store.load().unwrap().unwrap().state, State::Paused);
    // The owner asking her resumes her for the request.
    ask(&agents, "k3", "run it", false).unwrap();
    assert_eq!(view(&agents).state, "active");
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

#[test]
fn a_terminal_briefing_carries_scored_journal_rows_and_records_them() {
    let dir = tempfile::tempdir().unwrap();
    let agents = host(
        &dir,
        vec![
            turn(
                run("echo atif: 31 passed", 0, "atif: 31 passed"),
                "atif: 31 passed.",
            ),
            turn(vec![], "They passed."),
        ],
    );
    ask(&agents, "k1", "run the atif tests", false).unwrap();
    until(&agents, |v| !v.busy && v.headline == "ok exit 0");
    ask(&agents, "k2", "what did the atif tests say?", false).unwrap();
    until(&agents, |v| !v.busy && v.headline == "answered");
    let store = Store::new(&dir.path().join("host"), "alice").unwrap();
    let receipts: Vec<String> = store
        .journal(100)
        .unwrap()
        .into_iter()
        .filter(|e| e.text.starts_with("the briefing carried"))
        .map(|e| e.text)
        .collect();
    assert!(
        receipts.iter().any(|r| r.contains("journal rows")),
        "{receipts:?}"
    );
    let scores = std::fs::read_to_string(store.dir().join("scores.jsonl")).unwrap();
    assert!(scores.contains(crate::task::agent_recall::SCORE_SCHEMA));
}

/// Holds her Coder session as her pane does after the owner typed in it,
/// and hands it back when she asks: at once, or after `busy` of a turn of
/// its own.
fn pane_holding(dir: &tempfile::TempDir, busy: Option<Duration>) -> std::thread::JoinHandle<()> {
    let state = dir.path().join("coder");
    std::fs::create_dir_all(state.join("sessions")).unwrap();
    let lock = std::fs::File::create(coder_v1::lock_path(&state, "alice-coder")).unwrap();
    lock.try_lock().unwrap();
    let marker = coder_v1::reclaim_path(&state, "alice-coder");
    std::thread::spawn(move || {
        let start = Instant::now();
        while !marker.exists() {
            assert!(start.elapsed() < Duration::from_secs(20), "she never asked");
            std::thread::sleep(Duration::from_millis(10));
        }
        if let Some(busy) = busy {
            std::fs::write(&marker, "busy\n").unwrap();
            std::thread::sleep(busy);
        }
        drop(lock);
    })
}

/// Lines she says to the owner, without the commands she echoes.
fn spoken(view: &wire::AgentView) -> Vec<&String> {
    view.lines
        .iter()
        .filter(|l| !l.contains(": $ ") && !l.starts_with("you: "))
        .collect()
}

#[test]
fn she_takes_her_session_back_from_her_pane_and_answers() {
    let dir = tempfile::tempdir().unwrap();
    let agents = host(&dir, vec![turn(vec![], "I'm reading the repository.")]);
    let pane = pane_holding(&dir, None);
    ask(&agents, "k1", "what are you doing now", true).unwrap();
    let seen = until(&agents, |v| !v.busy && v.headline == "answered");
    pane.join().unwrap();
    assert!(
        seen.lines
            .iter()
            .any(|l| l.contains("I'm reading the repository."))
    );
    let marker = coder_v1::reclaim_path(&dir.path().join("coder"), "alice-coder");
    assert!(!marker.exists());
    for line in spoken(&seen) {
        assert_eq!(agent::instructs(line), None, "{line}");
    }
}

#[test]
fn she_waits_for_a_turn_running_in_her_session_and_says_so_once() {
    let dir = tempfile::tempdir().unwrap();
    let agents = host(&dir, vec![turn(vec![], "Done waiting.")]);
    let pane = pane_holding(&dir, Some(Duration::from_millis(600)));
    ask(&agents, "k1", "what are you doing now", true).unwrap();
    let seen = until(&agents, |v| !v.busy && v.headline == "answered");
    pane.join().unwrap();
    let waits = seen
        .lines
        .iter()
        .filter(|l| l.contains("as soon as the turn in my session finishes"))
        .count();
    assert_eq!(waits, 1, "{:?}", seen.lines);
    assert!(seen.lines.iter().any(|l| l.contains("Done waiting.")));
}

/// Nothing she tells the owner asks them to press a key, quit a program,
/// run a command, or follow a session, whatever went wrong.
#[test]
fn her_replies_never_send_the_owner_through_hoops() {
    let failed = |why: &str| Recorded {
        ended: Some(Ended::Failed(why.into())),
        ..Recorded::default()
    };
    let dir = tempfile::tempdir().unwrap();
    let agents = host(
        &dir,
        vec![
            failed("Another process is using this chat session."),
            failed("Coder V1 exited (1): run `openagents coder status --json` in ~/x"),
        ],
    );
    ask(&agents, "k1", "what are you doing now", true).unwrap();
    until(&agents, |v| !v.busy && v.headline == "session busy");
    ask(&agents, "k2", "and now", true).unwrap();
    let seen = until(&agents, |v| !v.busy && v.headline == "coder failed");
    for line in spoken(&seen) {
        assert_eq!(agent::instructs(line), None, "{line}");
    }
    for report in agents.reports() {
        assert_eq!(agent::instructs(&report.text), None, "{}", report.text);
    }

    // No Coder at all.
    let dir = tempfile::tempdir().unwrap();
    let agents = host(&dir, vec![]).with_engine(Arc::new(|_: &Record| {
        Err("Coder V1 is not installed: install Coder with scripts/install-coder.sh".into())
    }));
    ask(&agents, "k1", "hello", false).unwrap();
    let seen = until(&agents, |v| !v.busy && v.headline == "no coder");
    for line in spoken(&seen) {
        assert_eq!(agent::instructs(line), None, "{line}");
    }

    // Paused: the owner's request resumes her.
    let dir = tempfile::tempdir().unwrap();
    let agents = host(&dir, vec![turn(vec![], "Back at it.")]);
    agents.pause("alice", true, "owner").unwrap();
    ask(&agents, "k1", "hello", false).unwrap();
    let seen = until(&agents, |v| !v.busy && v.headline == "answered");
    assert_eq!(seen.state, "active");

    // A missing agent or workspace is one plain sentence.
    for (agent, workspace) in [("bob", None), ("alice", Some("nowhere".to_string()))] {
        let refused = agents.answer(
            "k3",
            &owner(),
            &Operation::AskAgent {
                agent: agent.into(),
                text: "hi".into(),
                workspace,
                context: String::new(),
                mode: Mode::Terminal,
                typist: false,
            },
        );
        let code = refused.expect_err("refused");
        let why = coder_host::tasks::take_reason(code).unwrap();
        assert_eq!(agent::instructs(&why), None, "{why}");
    }
}

struct Capacity;

impl Facts for Capacity {
    fn head(&self, _: &Path) -> Option<String> {
        None
    }
    fn issues(
        &self,
        _: &str,
        _: &str,
    ) -> Result<
        (
            Vec<crate::task::issue_pick::Open>,
            Vec<crate::task::issue_pick::Pull>,
        ),
        String,
    > {
        Ok((Vec::new(), Vec::new()))
    }
    fn capacity(&self) -> bool {
        true
    }
}

#[test]
fn a_reflect_occurrence_runs_the_reflection_and_meters_its_cost() {
    use crate::task::agent_reflect::{Recorded, Script, ScriptInsight, ScriptQuestion, Scripted};
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(&dir.path().join("host"), "alice").unwrap();
    let script = Script {
        schema: crate::task::agent_reflect::SCRIPT_SCHEMA.into(),
        fixture_digest: String::new(),
        at: clock(),
        model: "scripted".into(),
        usd: 0.02,
        questions: vec![ScriptQuestion {
            question: "What keeps stopping me?".into(),
            insights: vec![ScriptInsight {
                text: "The owner stopped me fifteen times in one day.".into(),
                because: vec!["journal:4".into(), "journal:5".into()],
                supported: 0.9,
                preference: 0.1,
                expect: "stored".into(),
            }],
        }],
    };
    let reflector: crate::task::agent_reflect::ServicesFactory = Arc::new(move |_: &Store| {
        Ok(crate::task::agent_reflect::Services {
            writer: Box::new(Scripted::new(script.clone())),
            verify: Box::new(Recorded::new(&script)),
            recall: crate::task::agent_recall::Services::offline(),
        })
    });
    // The insight is about the owner, so drafting keeps it private.
    let judged = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let wrote = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (j, w) = (judged.clone(), wrote.clone());
    let sharer: crate::task::agent_share::ServicesFactory = Arc::new(move |_: &Store| {
        use crate::task::agent_share::fake::{Fixed, Same};
        Ok(crate::task::agent_share::Services {
            writer: Box::new(Same {
                text: String::new(),
                calls: w.clone(),
            }),
            judge: Box::new(Fixed {
                general: 0.9,
                about_owner: 0.9,
                calls: j.clone(),
            }),
            corpus: knowledge::lint::Corpus::default(),
        })
    });
    let agents = host(&dir, Vec::new())
        .with_facts(Arc::new(Capacity))
        .with_reflector(reflector)
        .with_sharer(sharer);
    let jobs = Jobs::new(store.clone());
    jobs.add(
        agent_jobs::template("reflect", "", None, None, 0, clock() - 10).unwrap(),
        clock() - 10,
    )
    .unwrap();
    jobs.edit("reflect", agent_jobs::Edit::On, clock() - 10)
        .unwrap();
    // Fifteen stops since it was turned on pass the early threshold.
    for _ in 0..15 {
        store
            .append(&Entry::new(
                clock() - 5,
                Kind::Control,
                "the owner stopped alice",
            ))
            .unwrap();
    }
    agents.tick();
    let start = Instant::now();
    let run = loop {
        let journal = store.journal(200).unwrap();
        if let Some(run) = journal
            .iter()
            .find(|e| e.text.starts_with(crate::task::agent_reflect::RUN_PREFIX))
        {
            break run.clone();
        }
        assert!(start.elapsed() < Duration::from_secs(20), "{journal:?}");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(
        run.text
            .starts_with("reflection run (early: importance 151"),
        "{}",
        run.text
    );
    assert!(
        run.text.ends_with("stored 1, proposed 0, dropped 0"),
        "{}",
        run.text
    );
    let memory = Memory::new(store.clone(), secret_screen::Screen::shapes());
    assert!(
        memory
            .entries()
            .unwrap()
            .iter()
            .any(|e| e.kind == MemoryKind::Insight && e.sources == ["journal:4", "journal:5"])
    );
    let start = Instant::now();
    loop {
        let job = &jobs.load().unwrap()[0];
        if job.budget.unmetered == 0 {
            assert!((job.budget.spent - 0.04).abs() < 1e-9, "{job:?}");
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(20), "{job:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
    let journal = store.journal(400).unwrap();
    assert!(
        journal.iter().any(|e| e
            .text
            .contains("as a knowledge entry: it is about the owner")),
        "{journal:?}"
    );
    assert_eq!(judged.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(wrote.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(!crate::task::agent_share::drafts_dir(&store).exists());
}

#[test]
fn her_morning_plan_is_visible_and_the_owners_request_replans_it() {
    use crate::task::agent_plan;
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(&dir.path().join("host"), "alice").unwrap();
    let agents = host(&dir, vec![turn(Vec::new(), "Looked at it.")]).with_facts(Arc::new(Capacity));
    let memory = Memory::new(store.clone(), secret_screen::Screen::shapes());
    let insight = memory
        .add(
            MemoryKind::Insight,
            Author::Agent,
            "The owner reviews changes after lunch.",
            vec!["journal:1".into()],
            clock() - 100,
        )
        .unwrap();
    let draft = json!({"blocks": [{
        "source": format!("memory:{insight}"),
        "node": agent_plan::DESK,
        "start": "13:00",
        "minutes": 60,
        "title": "Bring the morning's changes for review",
    }]})
    .to_string();
    let calls = Arc::new(Mutex::new(0_usize));
    let counted = calls.clone();
    let planner: agent_plan::ServicesFactory = Arc::new(move |_: &Store| {
        *counted.lock().unwrap() += 1;
        Ok(agent_plan::Services {
            writer: Box::new(agent_plan::Scripted::new([draft.clone()])),
            judge: Box::new(agent_plan::Answers::default()),
        })
    });
    let agents = agents.with_planner(planner);
    let jobs = Jobs::new(store.clone());
    let then = clock() - 2 * 86_400;
    for name in ["plan", "nightly-check"] {
        jobs.add(
            agent_jobs::template(name, "", None, None, 0, then).unwrap(),
            then,
        )
        .unwrap();
    }
    jobs.edit("plan", agent_jobs::Edit::On, then).unwrap();
    // The check is on, but its slot passed before it was: it fires
    // tomorrow, and today's plan holds it.
    jobs.edit("nightly-check", agent_jobs::Edit::On, clock())
        .unwrap();
    agents.tick();
    let plan = until(&agents, |v| v.plan.is_some()).plan.unwrap();
    let sources: Vec<&str> = plan.blocks.iter().map(|b| b.source.as_str()).collect();
    let insight_source = format!("memory:{insight}");
    assert_eq!(sources, ["job:nightly-check", insight_source.as_str()]);
    // The occurrence asked for one draft; following the plan asked
    // nothing, since no block is under way at midnight.
    assert_eq!(*calls.lock().unwrap(), 1);
    // The owner's request interrupts at once, by code.
    ask(&agents, "k-owner-1", "look at the failing deploy", false).unwrap();
    let replanned = until(&agents, |v| {
        v.plan.as_ref().is_some_and(|p| !p.replans.is_empty())
    })
    .plan
    .unwrap();
    let current = replanned.current_block().unwrap();
    assert_eq!(
        (current.source.as_str(), current.start),
        ("request:kowner1", 0)
    );
    assert_eq!(
        replanned.replans[0].kind,
        coder_host::access::day_plan::Replanned::Interrupt
    );
    let journal = store.journal(200).unwrap();
    assert!(
        journal.iter().any(
            |e| e.kind == Kind::Plan && e.text.starts_with("day plan for 2026-10-05: 2 blocks")
        )
    );
    assert!(journal.iter().any(|e| e.text.contains("react_now by code")));
}
