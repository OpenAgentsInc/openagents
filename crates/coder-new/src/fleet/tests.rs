use super::*;
use crate::bundled_runtime::{AcpAgent, AgentTransport};
use std::time::{Duration, Instant};

fn git(dir: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
}

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    repo: PathBuf,
    host: Host,
    execution: ExecutionSettings,
}

/// A scratch repository, a fake `codex` that runs `script`, and a host
/// whose transcripts, leases and build folders live in the scratch folder.
fn fixture(script: &str) -> Fixture {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let repo = root.join("repo");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "--quiet"]);
    std::fs::write(repo.join("README.md"), "scratch\n").unwrap();
    git(&repo, &["add", "README.md"]);
    git(&repo, &["commit", "--quiet", "-m", "first"]);
    std::fs::create_dir(root.join("shared")).unwrap();
    let program = root.join("codex");
    std::fs::write(
        &program,
        format!(
            "#!/bin/sh\nSHARED='{}'\n{script}",
            root.join("shared").display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
    let host = Host {
        fleet: Fleet::default(),
        parent_session: Some("parent-chat".into()),
        transcripts: Some(root.join("transcripts")),
        leases: Some(root.join("leases")),
        target_pool: Some(root.join("pool")),
        pool_size: 2,
        floor_gb: 0,
        free_disk: |_| Ok(u64::MAX),
    };
    let mut execution = crate::plugins::Plugins::default().execution_settings(repo.clone());
    execution.acp = true;
    execution.microcoder = false;
    execution.agents = vec![AcpAgent {
        id: "codex".into(),
        name: "Codex".into(),
        program,
        transport: AgentTransport::CodexCli,
        arguments: vec![],
        mode: None,
        enabled: true,
    }];
    execution.fleet = Some(host.clone());
    Fixture {
        _dir: dir,
        root,
        repo,
        host,
        execution,
    }
}

const ANSWER: &str = r#"
printf '%s\n' '{"type":"thread.started","thread_id":"t","model":"gpt-6-sol"}'
printf '%s\n' "{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"$REPLY\"}}"
printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1000,"cached_input_tokens":0,"output_tokens":100}}'
"#;

fn wait_notices(fleet: &Fleet, count: usize) -> Vec<agent_fleet::Notice> {
    let started = Instant::now();
    let mut notices = Vec::new();
    while notices.len() < count {
        notices.extend(fleet.drain_notices());
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "{} of {count} notices: {:?}",
            notices.len(),
            fleet.list()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    notices
}

fn start_one(f: &Fixture, task: &str) -> agent_fleet::AgentRow {
    start(
        &f.host,
        &f.execution,
        StartArguments {
            engine: "codex".into(),
            task: task.into(),
            name: None,
            background: None,
            worktree: None,
        },
        None,
    )
    .unwrap()
}

#[test]
fn three_agents_run_at_once_in_their_own_worktrees_and_each_reports_back() {
    // Each run waits until all three are running, so the reply proves they
    // overlapped; each writes a file only in its own checkout.
    let f = fixture(&format!(
        r#"cat > task
touch "$SHARED/$OPENAGENTS_AGENT_ID"
i=0
while [ "$(ls "$SHARED" | wc -l | tr -d ' ')" -lt 3 ] && [ $i -lt 400 ]; do sleep 0.05; i=$((i+1)); done
seen=$(ls "$SHARED" | wc -l | tr -d ' ')
pwd > where.txt
printf '%s' "$CARGO_TARGET_DIR" > target.txt
REPLY="saw $seen running"
{ANSWER}"#
    ));
    let rows: Vec<_> = ["fix the login", "write the docs", "add a test"]
        .iter()
        .map(|task| start_one(&f, task))
        .collect();
    assert_eq!(f.host.fleet.running(), 3);
    let notices = wait_notices(&f.host.fleet, 3);
    assert!(f.host.fleet.drain_notices().is_empty(), "one notice each");
    let mut worktrees = std::collections::BTreeSet::new();
    for notice in &notices {
        assert_eq!(notice.status, agent_fleet::Status::Done, "{notice:?}");
        assert_eq!(notice.report.as_deref(), Some("saw 3 running"));
        assert_eq!(notice.tokens, 1100);
        assert_eq!(notice.cost_usd, None);
        let worktree = notice.worktree.clone().unwrap();
        assert!(worktree.starts_with(f.repo.join(".coder/worktrees")));
        assert_eq!(
            std::fs::read_to_string(worktree.join("where.txt"))
                .unwrap()
                .trim(),
            worktree.canonicalize().unwrap().to_string_lossy()
        );
        let target = std::fs::read_to_string(worktree.join("target.txt")).unwrap();
        assert!(target.starts_with(&f.root.join("pool").to_string_lossy().into_owned()));
        assert!(notice.branch.as_deref().unwrap().starts_with("agent/"));
        assert!(notice.text().contains("Its work is on branch agent/"));
        worktrees.insert(worktree);
    }
    assert_eq!(worktrees.len(), 3, "three separate checkouts");
    assert!(
        !f.repo.join("where.txt").exists(),
        "the parent is untouched"
    );
    for row in &rows {
        let transcript: Value = serde_json::from_str(
            &std::fs::read_to_string(f.host.fleet.get(&row.id).unwrap().transcript.unwrap())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(transcript["extra"]["parent_session_id"], "parent-chat");
        assert_eq!(transcript["extra"]["background"], true);
        assert!(transcript.to_string().contains("saw 3 running"));
    }
}

#[test]
fn stop_ends_a_running_agent_keeps_its_work_and_frees_its_lease() {
    let f = fixture(
        r#"cat > task
echo half > half.txt
touch "$SHARED/started"
sleep 30
REPLY=late
"#,
    );
    let row = start_one(&f, "a long task");
    let started = Instant::now();
    while !f.root.join("shared/started").exists() {
        assert!(started.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(20));
    }
    let leases = f.host.leases.clone().unwrap();
    assert!(agent_fleet::lease::removable(&leases, &row.id).is_err());
    f.host.fleet.stop(&row.name).unwrap();
    let notices = wait_notices(&f.host.fleet, 1);
    assert!(started.elapsed() < Duration::from_secs(25), "stopped early");
    assert_eq!(notices[0].status, agent_fleet::Status::Stopped);
    assert!(notices[0].text().contains("was stopped"));
    let worktree = notices[0].worktree.clone().unwrap();
    assert!(worktree.join("half.txt").exists(), "work is kept");
    assert!(agent_fleet::lease::removable(&leases, &row.id).is_ok());
}

#[test]
fn a_message_sent_while_it_works_runs_one_more_step_before_one_notice() {
    let f = fixture(&format!(
        r#"cat >> tasks.txt
echo '---' >> tasks.txt
touch "$SHARED/run-$(ls "$SHARED" | wc -l | tr -d ' ')"
i=0
while [ ! -e "$SHARED/go" ] && [ $i -lt 400 ]; do sleep 0.05; i=$((i+1)); done
REPLY="done"
{ANSWER}"#
    ));
    let row = start_one(&f, "first job");
    let started = Instant::now();
    while !f.root.join("shared/run-0").exists() {
        assert!(started.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(20));
    }
    let tool = execute(
        &f.host,
        &f.execution,
        "agent_message",
        json!({"agent": row.name, "message": "also fix the typo"}),
        None,
    )
    .unwrap();
    assert_eq!(tool["delivered"], "queued");
    std::fs::write(f.root.join("shared/go"), "").unwrap();
    let notices = wait_notices(&f.host.fleet, 1);
    assert_eq!(notices[0].status, agent_fleet::Status::Done);
    let tasks =
        std::fs::read_to_string(notices[0].worktree.clone().unwrap().join("tasks.txt")).unwrap();
    assert_eq!(tasks.matches("---").count(), 2, "{tasks}");
    assert!(tasks.contains("also fix the typo"));
    std::thread::sleep(Duration::from_millis(200));
    assert!(f.host.fleet.drain_notices().is_empty());
    assert_eq!(f.host.fleet.get(&row.id).unwrap().tokens, 2200);

    // An ended agent resumes with a message, in the same worktree.
    let resumed = execute(
        &f.host,
        &f.execution,
        "agent_message",
        json!({"agent": row.id, "message": "and the changelog"}),
        None,
    )
    .unwrap();
    assert_eq!(resumed["delivered"], "resumed");
    let notices = wait_notices(&f.host.fleet, 1);
    assert_eq!(notices[0].status, agent_fleet::Status::Done);
    let tasks =
        std::fs::read_to_string(notices[0].worktree.clone().unwrap().join("tasks.txt")).unwrap();
    assert!(tasks.contains("and the changelog"), "{tasks}");
    assert_eq!(f.host.fleet.get(&row.id).unwrap().runs, 2);
}

#[test]
fn a_worktree_with_no_changes_is_removed_with_its_branch() {
    let f = fixture(&format!("cat > /dev/null\nREPLY=nothing\n{ANSWER}"));
    start_one(&f, "look only");
    let notices = wait_notices(&f.host.fleet, 1);
    assert_eq!(notices[0].status, agent_fleet::Status::Done);
    assert_eq!(notices[0].worktree, None);
    assert_eq!(notices[0].branch, None);
    let branches = std::process::Command::new("git")
        .arg("-C")
        .arg(&f.repo)
        .args(["branch"])
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&branches.stdout).contains("agent/"));
}

#[test]
fn the_tools_refuse_without_git_under_the_floor_and_for_unknown_engines() {
    let f = fixture("exit 0\n");
    let names: Vec<_> = tool_definitions(&f.execution)
        .iter()
        .map(|tool| tool["function"]["name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        names,
        ["agent", "agent_list", "agent_message", "agent_stop"]
    );
    assert!(
        f.execution
            .defs()
            .iter()
            .any(|tool| tool["function"]["name"] == "agent")
    );
    assert!(f.execution.instructions().contains("Background agents"));

    let mut outside = f.execution.clone();
    outside.cwd = f.root.join("shared");
    let error = execute(
        &f.host,
        &outside,
        "agent",
        json!({"engine":"codex","task":"x"}),
        None,
    )
    .unwrap_err();
    assert!(error.contains("not in one"), "{error}");

    let mut low = f.host.clone();
    low.floor_gb = 20;
    low.free_disk = |_| Ok(3_000_000_000);
    let error = execute(
        &low,
        &f.execution,
        "agent",
        json!({"engine":"codex","task":"x"}),
        None,
    )
    .unwrap_err();
    assert!(error.contains("Only 3 GB is free"), "{error}");

    let error = execute(
        &f.host,
        &f.execution,
        "agent",
        json!({"engine":"nobody","task":"x"}),
        None,
    )
    .unwrap_err();
    assert!(error.contains("not available"), "{error}");
    assert!(
        execute(
            &f.host,
            &f.execution,
            "agent_stop",
            json!({"agent":"x"}),
            None
        )
        .is_err()
    );
    assert_eq!(f.host.fleet.list().len(), 0);

    let mut off = f.execution.clone();
    off.fleet = None;
    assert!(
        !off.defs()
            .iter()
            .any(|tool| tool["function"]["name"] == "agent")
    );
}

#[test]
fn codex_cost_is_hidden_and_api_charges_are_kept() {
    let (tokens, cost) = usage(&json!({
        "transport":"codex-cli","model":"gpt-6-sol:high","tokens":1100,
        "usage":{"input_tokens":1000,"cached_input_tokens":500,"output_tokens":100}
    }));
    assert_eq!(tokens, 1100);
    assert_eq!(cost, None);
    assert_eq!(
        usage(&json!({"cost_usd":0.5,"usage":{"input_tokens":1,"output_tokens":2}})),
        (3, Some(0.5))
    );
    assert_eq!(usage(&json!({"reply":"x"})), (0, None));
}

/// The terminal shows a finished agent in its rail and hands its notice to
/// the model: mid-turn through the turn's inbox, otherwise as the next turn.
#[test]
fn notices_reach_the_parent_chat_without_the_user_asking() {
    let mut app = crate::App::default();
    app.set_mode(crate::Mode::Live);
    app.plugins.enabled = true;
    app.plugins.key_configured = true;
    let control = app
        .fleet
        .start(agent_fleet::Spec {
            engine: "codex".into(),
            task: "fix the login".into(),
            ..agent_fleet::Spec::default()
        })
        .unwrap();
    control.event(RuntimeEvent::Text("working".into()));
    app.poll_fleet();
    assert_eq!(app.delegations.len(), 1);
    assert!(app.delegations[0].background);
    assert!(app.delegations[0].running);

    // Mid-turn: the notice goes to the running turn's inbox.
    app.live.busy = true;
    control.finish(agent_fleet::Outcome::Done("All fixed.".into()));
    app.poll_fleet();
    assert!(!app.delegations[0].running);
    let inbox = app.prompt_inbox.lock().unwrap().clone();
    assert_eq!(inbox.len(), 1);
    let text = inbox[0].lock().unwrap().clone().unwrap();
    assert!(text.starts_with("Background agent fix-the-login (codex) finished"));
    assert!(text.contains("All fixed."));

    // The turn ended without reading it: it starts the next turn.
    app.live.busy = false;
    app.process_prompt_queue();
    let request = app.request.take().expect("the notice starts a turn");
    match request.kind {
        crate::live::Work::Chat { messages, .. }
        | crate::live::Work::Microcoder { messages, .. } => {
            assert!(
                messages
                    .iter()
                    .any(|m| m.role == "user" && m.content.contains("All fixed."))
            );
        }
        _ => panic!("a chat turn"),
    }

    // The exported trace carries the agent as a subagent trajectory.
    let document = crate::trajectory::main_document(&app, Path::new("."));
    let children = document["subagent_trajectories"].as_array().unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0]["extra"]["background"], true);
    assert_eq!(children[0]["extra"]["status"], "done");
}

#[test]
fn without_a_model_the_notice_is_shown_and_no_turn_starts() {
    let mut app = crate::App::default();
    app.set_mode(crate::Mode::Live);
    let control = app
        .fleet
        .start(agent_fleet::Spec {
            engine: "codex".into(),
            task: "t".into(),
            ..agent_fleet::Spec::default()
        })
        .unwrap();
    app.fleet.stop("agent-1").unwrap();
    control.finish(agent_fleet::Outcome::Failed("canceled".into()));
    app.poll_fleet();
    assert!(
        app.live
            .entries
            .iter()
            .any(|entry| matches!(entry, Entry::User(text) if text.contains("was stopped")))
    );
    app.process_prompt_queue();
    assert!(app.request.is_none());
}

#[test]
fn the_agents_panel_lists_stops_and_opens_agents() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut app = crate::App::default();
    app.set_mode(crate::Mode::Live);
    let a = app
        .fleet
        .start(agent_fleet::Spec {
            engine: "codex".into(),
            task: "first".into(),
            ..agent_fleet::Spec::default()
        })
        .unwrap();
    let _b = app
        .fleet
        .start(agent_fleet::Spec {
            engine: "claude".into(),
            task: "second".into(),
            ..agent_fleet::Spec::default()
        })
        .unwrap();
    a.add_usage(31_200, Some(0.42));
    app.draft.text = "/agents".into();
    app.handle(crossterm::event::Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));
    assert!(app.agents_panel.is_some());
    let shown: String = crate::ui::agents::lines(&app, 100)
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
                + "\n"
        })
        .collect();
    assert!(shown.contains("2 running"), "{shown}");
    assert!(shown.contains("31.2k"), "{shown}");
    assert!(shown.contains("$0.42"), "{shown}");
    assert!(oa_copy::violations(&shown, &[]).is_empty(), "{shown}");
    let key = |app: &mut crate::App, code| {
        app.handle(crossterm::event::Event::Key(KeyEvent::new(
            code,
            KeyModifiers::NONE,
        )))
    };
    key(&mut app, KeyCode::Char('s'));
    assert!(a.stopped(), "s stops the chosen agent");
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Char('m'));
    assert!(app.agents_panel.is_none());
    let index = app.selected_agent.unwrap();
    assert_eq!(app.delegations[index].name, "second");
    assert!(app.delegations[index].background);
}

/// A background agent started here shows on the desktop app's Agents
/// panel, through this process's board, and Stop there stops it (#11180).
#[test]
fn the_desktop_board_lists_agents_and_carries_out_stop() {
    let home = tempfile::tempdir().unwrap();
    let mut app = crate::App::default();
    app.publish_agents(home.path());
    let dir = agent_fleet::board::dir(home.path());
    assert!(
        agent_fleet::board::read(&dir, |_, _| true).is_empty(),
        "no agents, no board"
    );
    let control = app
        .fleet
        .start(agent_fleet::Spec {
            engine: "codex".into(),
            task: "fix the login".into(),
            ..agent_fleet::Spec::default()
        })
        .unwrap();
    app.poll_fleet();
    let boards = agent_fleet::board::read(&dir, |_, _| true);
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].pid, std::process::id());
    assert_eq!(boards[0].agents.len(), 1);
    assert_eq!(boards[0].agents[0].name, "fix-the-login");
    assert_eq!(boards[0].agents[0].status, agent_fleet::Status::Running);

    agent_fleet::board::request_stop(&dir, std::process::id(), "agent-1").unwrap();
    // Requests are looked for every half second.
    let started = Instant::now();
    while !control.stopped() {
        assert!(started.elapsed() < Duration::from_secs(5), "never stopped");
        std::thread::sleep(Duration::from_millis(50));
        app.poll_fleet();
    }
    control.finish(agent_fleet::Outcome::Failed("canceled".into()));
    app.poll_fleet();
    let boards = agent_fleet::board::read(&dir, |_, _| true);
    assert_eq!(boards[0].agents[0].status, agent_fleet::Status::Stopped);

    drop(app);
    assert!(agent_fleet::board::read(&dir, |_, _| true).is_empty());
}
