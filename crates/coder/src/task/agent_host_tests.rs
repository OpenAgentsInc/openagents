use super::*;
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

/// A host whose agent `alice` plans from `script`, one list of actions
/// per request.
fn host(dir: &tempfile::TempDir, script: Vec<Vec<agent::NextAction>>) -> Agents {
    let root = dir.path().join("host");
    let workspace = dir.path().join("work");
    std::fs::create_dir_all(&workspace).unwrap();
    let store = Store::new(&root, "alice").unwrap();
    store.open(&workspace, clock()).unwrap();
    let script = Arc::new(Mutex::new(VecDeque::from(script)));
    let model: ModelFactory = Arc::new(move |_record: &Record| {
        let actions = script.lock().unwrap().pop_front().unwrap_or_default();
        Ok((
            Box::new(agent::Scripted {
                actions: actions.into(),
                prompts: Vec::new(),
            }) as Box<dyn Model + Send>,
            "scripted".to_string(),
        ))
    });
    Agents::new(&root, dir.path().join("tasks"), BTreeMap::new())
        .with_model(model)
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
fn the_host_runs_a_read_only_command_itself_and_reports_three_ways() {
    let dir = tempfile::tempdir().unwrap();
    let agents = host(
        &dir,
        vec![vec![
            agent::action(&["echo atif: 31 passed"], ""),
            agent::action(&[], "atif: 31 passed."),
        ]],
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
    assert!(seen.lines.iter().any(|l| l.contains("atif: 31 passed.")));
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
fn a_typist_pane_types_and_only_the_issued_step_is_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let agents = host(
        &dir,
        vec![vec![
            agent::action(&["cargo test -p atif"], ""),
            agent::action(&[], "atif: 31 passed."),
        ]],
    );
    ask(&agents, "k1", "run the atif tests", true).unwrap();
    let seen = until(&agents, |v| v.run.is_some());
    let step = seen.run.unwrap();
    assert!(step.typist);
    assert_eq!(step.command, "cargo test -p atif");
    assert_eq!(seen.activity, Activity::Testing);
    let ran = |step: u64| Operation::AgentRan {
        agent: "alice".into(),
        step,
        ran: wire::Ran {
            status: Some(0),
            output: "test result: ok. 31 passed".into(),
            ..wire::Ran::default()
        },
    };
    assert_eq!(
        agents.answer("r0", &owner(), &ran(step.step + 7)),
        Err(Code::Conflict)
    );
    agents.answer("r1", &owner(), &ran(step.step)).unwrap();
    until(&agents, |v| !v.busy && v.headline == "ok exit 0");
}

#[test]
fn anything_else_waits_for_confirm_or_reject() {
    let dir = tempfile::tempdir().unwrap();
    let agents = host(
        &dir,
        vec![vec![
            agent::action(&["touch notes.txt"], ""),
            agent::action(&[], "I left it alone."),
        ]],
    );
    ask(&agents, "k1", "make a notes file", false).unwrap();
    let seen = until(&agents, |v| v.pending.is_some());
    assert_eq!(seen.activity, Activity::Waiting);
    let proposal = seen.pending.unwrap();
    assert_eq!(proposal.command, "touch notes.txt");
    let answer = |step: u64| Operation::AnswerAgent {
        agent: "alice".into(),
        step,
        confirm: false,
    };
    assert_eq!(
        agents.answer("a0", &owner(), &answer(99)),
        Err(Code::Conflict)
    );
    agents
        .answer("a1", &owner(), &answer(proposal.step))
        .unwrap();
    until(&agents, |v| !v.busy && v.headline == "rejected");
    assert!(!dir.path().join("work/notes.txt").exists());
}

#[test]
fn stop_runs_the_sequence_and_pause_starts_nothing_new() {
    let dir = tempfile::tempdir().unwrap();
    let agents = host(&dir, vec![vec![agent::action(&["cargo test -p atif"], "")]]);
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
    let agents = host(&dir, vec![vec![agent::action(&[], "Understood.")]]);
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
