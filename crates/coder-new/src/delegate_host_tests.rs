//! Coder's delegation host (#11511): call-time checks, and the parent's
//! tool list held fixed while plugins turn on and off.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serde_json::json;

use super::*;
use crate::bundled_runtime::AcpAgent;

fn agent(id: &str, name: &str) -> AcpAgent {
    AcpAgent {
        id: id.into(),
        name: name.into(),
        program: "/bin/sh".into(),
        ..serde_json::from_value(json!({"id":"x","name":"x","program":"x"})).unwrap()
    }
}

fn execution() -> ExecutionSettings {
    let mut execution = crate::App::default().agent_execution();
    execution.claude = true;
    execution.acp = true;
    execution.microcoder = true;
    execution.agents = vec![agent("codex", "Codex"), agent("opencode", "OpenCode")];
    execution
}

fn host(execution: ExecutionSettings) -> CoderHost {
    CoderHost::new(execution, None, Arc::new(AtomicBool::new(false)), None)
}

fn request(to: &str, model: Option<&str>) -> Request {
    Request {
        to: to.into(),
        model: model.map(str::to_owned),
        task: "x".into(),
        ..Request::default()
    }
}

#[test]
fn harnesses_and_models_are_checked_when_the_call_arrives() {
    let host = host(execution());
    let ids: Vec<String> = available(&execution(), false)
        .into_iter()
        .map(|harness| harness.id)
        .collect();
    assert_eq!(ids, vec!["claude-code", "codex", "opencode", "coder"]);
    assert_eq!(
        host.target(&request("auto", None)).unwrap().harness,
        "claude-code"
    );
    assert_eq!(
        host.target(&request("codex", Some("gpt-6-astra")))
            .unwrap()
            .model
            .as_deref(),
        Some("gpt-6-astra")
    );
    let error = host
        .target(&request("codex", Some("claude-opus-5-5")))
        .unwrap_err();
    assert!(error.contains("gpt-6.1-sol"), "{error}");
    let error = host
        .target(&request("opencode", Some("opencode/big-pickle")))
        .unwrap_err();
    assert!(error.contains("leave model out"), "{error}");
    // OpenCode as a harness, once installed: its models are taken.
    crate::opencode_chat::install_for_test(true);
    assert_eq!(
        host.target(&request(
            "opencode",
            Some(crate::opencode_chat::DEFAULT_MODEL)
        ))
        .unwrap()
        .model
        .as_deref(),
        Some(crate::opencode_chat::DEFAULT_MODEL)
    );
    crate::opencode_chat::install_for_test(false);
    let error = host.target(&request("cursor", None)).unwrap_err();
    assert!(error.contains("codex, opencode, coder"), "{error}");
    // Turned off after the chat started: refused at the call, not hidden.
    let mut off = execution();
    off.claude = false;
    off.microcoder = false;
    let error = self::host(off)
        .target(&request("claude-code", None))
        .unwrap_err();
    assert!(error.contains("not available"), "{error}");
    assert_eq!(
        self::host(execution())
            .target(&request("coder-loop", None))
            .unwrap()
            .harness,
        "coder"
    );
}

/// The cache check's second half: the parent's tool list is the same
/// bytes before and after a harness plugin is toggled.
#[tokio::test(flavor = "multi_thread")]
async fn toggling_a_harness_plugin_leaves_the_tool_list_alone() {
    let bytes = |execution: ExecutionSettings| async move {
        let (events, _) = tokio::sync::mpsc::unbounded_channel();
        let server = crate::claude_tools::tool_server(
            &crate::claude_tools::Tools::new(execution),
            CLAUDE,
            events,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        (
            server.list_bytes().await,
            coder_sdk::codex::dynamic_tools(&server).to_string(),
        )
    };
    let on = execution();
    let before = bytes(on.clone()).await;
    let toggles: Vec<(&str, Box<dyn Fn(&mut ExecutionSettings)>)> = vec![
        ("Coder loop off", Box::new(|e| e.microcoder = false)),
        ("local agents off", Box::new(|e| e.acp = false)),
        ("OpenCode off", Box::new(|e| e.agents[1].enabled = false)),
        (
            "an agent installed",
            Box::new(|e| e.agents.push(agent("grok-build", "Grok Build"))),
        ),
    ];
    for (toggle, apply) in toggles {
        let mut changed = on.clone();
        apply(&mut changed);
        assert_eq!(bytes(changed.clone()).await, before, "{toggle}");
        assert_eq!(
            crate::claude_tools::instructions(&crate::claude_tools::Tools::new(changed)),
            crate::claude_tools::instructions(&crate::claude_tools::Tools::new(on.clone())),
            "{toggle}"
        );
    }
    // Claude Code off also takes Solver away and narrows the routing
    // preference tool (#11516, #11518): those are their own plugins' tools,
    // not delegation's. `delegate` itself stays the same bytes.
    let mut claude_off = on.clone();
    claude_off.claude = false;
    let (events, _) = tokio::sync::mpsc::unbounded_channel();
    let delegate = |execution: ExecutionSettings| {
        crate::claude_tools::tool_server(
            &crate::claude_tools::Tools::new(execution),
            CLAUDE,
            events.clone(),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap()
        .tools()
        .iter()
        .find(|tool| tool.name == "delegate")
        .map(|tool| (tool.description.clone(), tool.schema.to_string()))
    };
    assert_eq!(delegate(claude_off), delegate(on.clone()));
    let names = crate::claude_tools::names(&on);
    assert!(names.iter().any(|name| name == "delegate"), "{names:?}");
    for gone in ["agent", "acp_subagent", "microcoder"] {
        assert!(!names.iter().any(|name| name == gone), "{gone}");
    }
}

#[test]
fn the_coder_loop_offers_the_same_delegate_tool() {
    let mut execution = execution();
    execution.shell = true;
    let definition = execution
        .defs()
        .into_iter()
        .find(|tool| tool["function"]["name"] == "delegate")
        .unwrap();
    assert_eq!(definition, coder_sdk::delegation::function_definition());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_writing_child_outside_git_is_refused_and_reported_short() {
    let dir = tempfile::tempdir().unwrap();
    let mut execution = execution();
    execution.cwd = dir.path().to_owned();
    execution.fleet = None;
    let delegator = Delegator::new(
        Arc::new(host(execution)),
        Lineage::root("run-x", CODER),
        Authority::default(),
        &dir.path().join("runs"),
    );
    let report = delegator
        .delegate(json!({"to": "codex", "task": "edit"}))
        .await
        .unwrap();
    assert_eq!(report.outcome, Outcome::Failed);
    assert!(report.error.unwrap().contains("read_only"));
    let error = delegator
        .delegate(json!({"to": "codex", "task": "x", "background": true}))
        .await
        .unwrap_err();
    assert!(error.contains("agent tray"), "{error}");
}

/// rc.14: a delegation with no task starts nothing: the model hears why,
/// and no card, tray row or notice appears.
#[tokio::test(flavor = "multi_thread")]
async fn an_empty_task_is_refused_before_any_agent_shows() {
    let dir = tempfile::tempdir().unwrap();
    let mut execution = execution();
    execution.cwd = dir.path().to_owned();
    execution.fleet = None;
    for task in ["", "   \n", "...", "{}"] {
        let mut events = Vec::new();
        let error = execute(
            &execution,
            CODER,
            json!({"to": "claude", "task": task, "background": true}),
            None,
            &Arc::new(AtomicBool::new(false)),
            &mut |event| events.push(event),
        )
        .await
        .unwrap_err();
        assert!(error.contains("A delegation needs a task"), "{error}");
        assert!(events.is_empty(), "{task:?} drew {events:?}");
    }
    // The schema the harness sees requires a non-empty task.
    let schema = coder_sdk::delegation::schema();
    assert!(
        schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("task"))
    );
    assert_eq!(schema["properties"]["task"]["minLength"], 1);
}

/// A git repository with one commit, for a writing child's worktree.
#[cfg(unix)]
fn repo(dir: &std::path::Path) {
    for args in [
        &["init", "-q", "-b", "main"][..],
        &[
            "-c",
            "user.email=a@b",
            "-c",
            "user.name=a",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "init",
        ][..],
    ] {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }
}

/// `program` as the installed ACP agent `id`, the only agent on.
#[cfg(unix)]
fn on_acp(repo: &std::path::Path, id: &str, name: &str, program: PathBuf) -> ExecutionSettings {
    let mut execution = execution();
    execution.cwd = repo.to_owned();
    execution.fleet = None;
    let mut agent = agent(id, name);
    agent.program = program;
    if id == "cursor" {
        agent.arguments = acp_client::cursor::arguments();
    }
    execution.agents = vec![agent];
    execution
}

/// A delegated ACP child starts on the Coder SDK's ACP backend with its
/// agent's gate (#11534): Grok Build runs Coder's bridge from a
/// `--plugin-dir` plugin, and a merge the floors hold is refused in the
/// child, with the reason sent to it as the next prompt.
#[tokio::test(flavor = "multi_thread")]
#[cfg(unix)]
async fn a_delegated_grok_child_runs_under_its_hooks_and_a_held_merge_is_refused() {
    let _gate_lock = crate::approval::test_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main");
    std::fs::create_dir_all(&main).unwrap();
    repo(&main);
    let agent_dir = dir.path().join("agent");
    std::fs::create_dir_all(&agent_dir).unwrap();
    let mut blocks = acp_client::replay::blocks(acp_client::replay::GROK_TURN);
    let completed = std::mem::replace(
        &mut blocks[2],
        vec![
            json!({"jsonrpc":"2.0","id":"permission-1","method":"session/request_permission","params":{"sessionId":"grok-session-1","toolCall":{"toolCallId":"merge-1","title":"gh pr merge 1 --squash","kind":"execute","rawInput":{"command":"gh pr merge 1 --squash"}},"options":[{"optionId":"allow","name":"Allow","kind":"allow_once"},{"optionId":"deny","name":"Deny","kind":"reject_once"}]}}),
        ],
    );
    // The refused step's reason comes as prompt 4; prompt 3 ends after the
    // answer.
    blocks.push(vec![
        json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"grok-session-1","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":" The merge was held."}}}}),
        json!({"jsonrpc":"2.0","id":4,"result":{"stopReason":"end_turn"}}),
    ]);
    blocks.push(completed);
    let program = acp_client::replay::script(&agent_dir, &blocks);
    let script = std::fs::read_to_string(&program).unwrap().replace(
        "case \"$line\" in",
        "case \"$line\" in\n    *'\"id\":\"permission-1\"'*) cat \"$dir/5.jsonl\" ;;",
    );
    std::fs::write(&program, script).unwrap();
    let execution = on_acp(&main, "grok-build", "Grok Build", program);
    let records = dir.path().join("routing");
    crate::routing_record::record_to(Some((records.clone(), facts())));
    let host = host(execution);
    crate::routing_record::record_to(None);
    let delegator = Delegator::new(
        Arc::new(host),
        Lineage::root("run-x", CODER),
        Authority::default(),
        &dir.path().join("runs"),
    );
    let report = delegator
        .delegate(json!({"to": "grok-build", "task": "Merge PR 1."}))
        .await
        .unwrap();
    assert_eq!(report.outcome, Outcome::Done, "{report:?}");
    assert_eq!(report.harness, "grok-build");
    let arguments = acp_client::replay::arguments(&agent_dir);
    assert!(
        arguments.iter().any(|arg| arg == "--plugin-dir"),
        "{arguments:?}"
    );
    let sent = acp_client::replay::received(&agent_dir);
    let answer = sent
        .iter()
        .find(|frame| frame["id"] == "permission-1")
        .unwrap();
    assert_eq!(answer["result"]["outcome"]["optionId"], "deny", "{answer}");
    let prompts: Vec<&serde_json::Value> = sent
        .iter()
        .filter(|frame| frame["method"] == "session/prompt")
        .collect();
    assert_eq!(prompts.len(), 2, "{prompts:?}");
    let told = prompts[1]["params"]["prompt"].to_string();
    assert!(told.contains("gh pr merge"), "{told}");
    // The child's transcript has its run, not the parent's.
    let transcript = std::fs::read_to_string(report.transcript.unwrap()).unwrap();
    assert!(transcript.contains("The merge was held."), "{transcript}");
    // Its routing record names the agent and the version it reported
    // (#11535), as a delegation the parent's call chose.
    let lines = crate::routing_record::read(&records);
    let chose = lines
        .iter()
        .find_map(|line| match line {
            crate::routing_record::Line::Chose(chose) => Some(chose),
            _ => None,
        })
        .expect("a chose line");
    assert_eq!(chose.kind, crate::routing_record::Kind::Delegation);
    assert_eq!(chose.rule, crate::routing_record::Rule::Delegated);
    assert_eq!(chose.agent.as_ref().unwrap().id, "grok-build");
    let ended = lines
        .iter()
        .find_map(|line| match line {
            crate::routing_record::Line::Ended(ended) => Some(ended),
            _ => None,
        })
        .expect("an ended line");
    let agent = ended.agent.as_ref().unwrap();
    assert_eq!(
        (agent.id.as_str(), agent.version.as_deref()),
        ("grok-build", Some("synthetic"))
    );
    assert_eq!(ended.ended, crate::routing_record::Ended::Done);
}

/// The facts a test's records read.
fn facts() -> crate::harness_plugin::Facts {
    let health = crate::harness_plugin::Health {
        signed_in: false,
        at_limit: false,
    };
    crate::harness_plugin::Facts {
        local: true,
        claude_plugin_on: false,
        claude: health,
        codex_on: false,
        codex: health,
        opencode_on: false,
        opencode: health,
        openrouter_key: false,
        leaning: crate::routing_preferences::Leaning::default(),
        failed: Default::default(),
    }
}

/// Cursor stays emulated in a delegation (#11534): the child is told
/// plainly what Coder can't hold before it acts, and its asks are still
/// answered by Coder's hooks.
#[tokio::test(flavor = "multi_thread")]
#[cfg(unix)]
async fn a_delegated_cursor_child_says_plainly_what_coder_cannot_hold() {
    let _gate_lock = crate::approval::test_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main");
    std::fs::create_dir_all(&main).unwrap();
    repo(&main);
    let agent_dir = dir.path().join("agent");
    std::fs::create_dir_all(&agent_dir).unwrap();
    // The stand-in waits for Coder's answer to each request, as Cursor
    // does, so the turn can't close before the answer is logged.
    let program = acp_client::replay::script_answered(
        &agent_dir,
        &acp_client::replay::blocks(acp_client::replay::CURSOR_TURN),
    );
    let (sender, mut events) = tokio::sync::mpsc::unbounded_channel();
    let execution = on_acp(&main, "cursor", "Cursor", program);
    let delegator = Delegator::new(
        Arc::new(CoderHost::new(
            execution,
            None,
            Arc::new(AtomicBool::new(false)),
            Some(sender),
        )),
        Lineage::root("run-x", CODER),
        Authority::default(),
        &dir.path().join("runs"),
    );
    let report = delegator
        .delegate(json!({"to": "cursor", "task": "Run echo, then reply done."}))
        .await
        .unwrap();
    assert_eq!(report.outcome, Outcome::Done, "{report:?}");
    let mut notice = None;
    while let Ok(event) = events.try_recv() {
        if let RuntimeEvent::Delegation { event, .. } = event
            && let RuntimeEvent::Text(text) = *event
            && text.contains("doesn't ask before every step")
        {
            notice = Some(text);
        }
    }
    // The notice is a status, never words of the agent's reply (#11556).
    assert_eq!(notice, None);
    let sent = acp_client::replay::received(&agent_dir);
    assert!(
        sent.iter()
            .any(|frame| frame["result"]["outcome"]["optionId"].is_string()),
        "Cursor's ask was answered"
    );
}

/// #11575: a child's card opens running and closes with the child's own
/// report, so the turn's end finds nothing to mark "The turn ended before
/// this agent reported back"; the model reads the delegation's result
/// once, the export has one call for it, and a reopened chat still shows
/// the card.
#[tokio::test(flavor = "multi_thread")]
#[cfg(unix)]
async fn a_finished_child_closes_its_card_and_is_one_result() {
    let _gate_lock = crate::approval::test_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let dir = tempfile::tempdir().unwrap();
    let mut execution = execution();
    execution.cwd = dir.path().to_owned();
    execution.fleet = None;
    let mut missing = agent("goose", "Goose");
    missing.program = dir.path().join("no-such-agent");
    execution.agents = vec![missing];
    let arguments = json!({"to": "goose", "task": "Read Cargo.toml.", "read_only": true});
    let mut events = Vec::new();
    let output = execute(
        &execution,
        CODER,
        arguments.clone(),
        None,
        &Arc::new(AtomicBool::new(false)),
        &mut |event| events.push(event),
    )
    .await
    .unwrap();
    assert_eq!(output["outcome"], "failed", "{output}");
    let child = output["child"].as_str().unwrap().to_owned();
    // The card's first event opens it and its last closes it with the report.
    let card = |event: &RuntimeEvent| match event {
        RuntimeEvent::Delegation { event, .. } => match event.as_ref() {
            RuntimeEvent::Tool {
                name,
                output,
                running,
                ..
            } if name == coder_sdk::delegation::TOOL => Some((output.clone(), *running)),
            _ => None,
        },
        _ => None,
    };
    assert_eq!(card(events.first().unwrap()), Some((Value::Null, true)));
    assert_eq!(card(events.last().unwrap()), Some((output.clone(), false)));

    let mut app = crate::App::default();
    app.set_mode(crate::Mode::Live);
    app.live.busy = true;
    let id = app.request_id;
    app.apply_update(crate::live::Update::Tool {
        id,
        name: coder_sdk::delegation::TOOL.into(),
        input: arguments.clone(),
        output: Value::Null,
        running: true,
        at: None,
    });
    for event in events {
        let RuntimeEvent::Delegation {
            id: delegation,
            name,
            task,
            event,
        } = event
        else {
            panic!("a child event")
        };
        app.apply_update(crate::live::Update::Delegation {
            id,
            delegation,
            name,
            task,
            event: *event,
        });
    }
    app.apply_update(crate::live::Update::Tool {
        id,
        name: coder_sdk::delegation::TOOL.into(),
        input: arguments,
        output: output.clone(),
        running: false,
        at: None,
    });
    app.apply_update(crate::live::Update::Finished {
        id,
        result: Ok(openrouter::Streamed {
            text: "One delegation failed.".into(),
            ..openrouter::Streamed::default()
        }),
    });
    assert!(!app.delegations[0].running);
    let entries = serde_json::to_string(
        &app.live
            .entries
            .iter()
            .filter_map(|entry| match entry {
                crate::live::Entry::Delegation {
                    output, running, ..
                } => Some((output.clone(), *running)),
                _ => None,
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    assert!(!entries.contains("turn ended"), "{entries}");
    assert!(entries.contains(&child), "{entries}");
    // The model reads the result once: the tool's, not the card's too.
    let messages = app.live.messages();
    assert_eq!(
        messages
            .iter()
            .filter(|message| message.content.contains(&child))
            .count(),
        1,
        "{messages:?}"
    );
    // One call in the export, with the card folded into it.
    let document = crate::trajectory::document(&app.live, "t", "m", dir.path());
    let calls: Vec<&Value> = document["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|step| step.pointer("/tool_calls/0"))
        .collect();
    assert_eq!(calls.len(), 1, "{document}");
    assert_eq!(calls[0]["function_name"], "delegate");
    assert!(
        calls[0]["extra"]["delegation"]["id"]
            .as_str()
            .unwrap()
            .ends_with(&child)
    );
    assert_eq!(document["final_metrics"]["extra"]["tool_calls_total"], 1);
    assert!(!document.to_string().contains("turn ended"));
    // A reopened chat has the call and its card.
    let restored = crate::trajectory::from_document(&document).unwrap();
    assert!(
        restored.entries.iter().any(|entry| matches!(entry,
        crate::live::Entry::Delegation { running: false, output, .. } if output["child"] == child))
    );
    assert_eq!(
        restored
            .messages()
            .iter()
            .filter(|m| m.content.contains(&child))
            .count(),
        1
    );
}

/// #11575: Codex is a delegation target when the Codex plugin is on and
/// Codex is installed and signed in, with ACP Subagents off; with the
/// plugin off, or Codex not ready, it isn't.
#[test]
fn codex_is_a_target_through_its_plugin_without_acp_subagents() {
    let mut execution = execution();
    execution.acp = false;
    execution.codex = true;
    let ids = |execution: &ExecutionSettings, ready: bool| -> Vec<String> {
        available_with(execution, false, ready)
            .into_iter()
            .map(|harness| harness.id)
            .collect()
    };
    assert_eq!(ids(&execution, true), vec!["claude-code", "codex", "coder"]);
    assert_eq!(ids(&execution, false), vec!["claude-code", "coder"]);
    // The ACP agent still counts with ACP Subagents on.
    execution.acp = true;
    assert!(ids(&execution, false).contains(&"codex".to_owned()));
}

/// #11575: OpenCode's configured model refused by its provider (Cursor's
/// "Update Required") runs the same task once more on OpenCode's free
/// default, through OpenCode's inline configuration (the person's file is
/// untouched); the child's chat says so and the report names the model it
/// ran on.
#[tokio::test(flavor = "multi_thread")]
#[cfg(unix)]
async fn a_refused_opencode_model_runs_again_on_the_free_default() {
    use std::os::unix::fs::PermissionsExt;
    let _gate_lock = crate::approval::test_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main");
    std::fs::create_dir_all(&main).unwrap();
    repo(&main);
    // The refused run: OpenCode's recorded refusal, in Cursor's words.
    let refused_dir = dir.path().join("refused");
    std::fs::create_dir_all(&refused_dir).unwrap();
    let mut refused = acp_client::replay::blocks(acp_client::replay::OPENCODE_REFUSED);
    let last = refused.last_mut().unwrap().last_mut().unwrap();
    last["error"]["message"] = json!(
        "Internal error: Update Required Your version of Cursor is no longer supported. Please update to the latest version at cursor.com/downloads to continue."
    );
    let refused = acp_client::replay::script(&refused_dir, &refused);
    let ok_dir = dir.path().join("ok");
    std::fs::create_dir_all(&ok_dir).unwrap();
    let ok = acp_client::replay::script(
        &ok_dir,
        &acp_client::replay::blocks(acp_client::replay::OPENCODE_TURN),
    );
    let configs = dir.path().join("configs");
    let program = dir.path().join("opencode");
    std::fs::write(
        &program,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$OPENCODE_CONFIG_CONTENT\" >> '{configs}'\ncase \"$OPENCODE_CONFIG_CONTENT\" in\n  *cat-jam*) exec '{ok}' \"$@\" ;;\n  *) exec '{refused}' \"$@\" ;;\nesac\n",
            configs = configs.display(),
            ok = ok.display(),
            refused = refused.display(),
        ),
    )
    .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    *TEST_OPENCODE_MODEL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some("cursor/composer-2.5".into());
    let mut execution = on_acp(&main, "opencode", "OpenCode", program);
    execution.agents[0].arguments = acp_client::opencode::arguments();
    let (sender, mut events) = tokio::sync::mpsc::unbounded_channel();
    let delegator = Delegator::new(
        Arc::new(CoderHost::new(
            execution,
            None,
            Arc::new(AtomicBool::new(false)),
            Some(sender),
        )),
        Lineage::root("run-x", CODER),
        Authority::default(),
        &dir.path().join("runs"),
    );
    let report = delegator
        .delegate(json!({"to": "opencode", "task": "Say hello."}))
        .await
        .unwrap();
    *TEST_OPENCODE_MODEL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    assert_eq!(report.outcome, Outcome::Done, "{report:?}");
    assert_eq!(
        report.model.as_deref(),
        Some(crate::opencode_chat::DEFAULT_MODEL)
    );
    assert!(
        report.summary.contains("cursor/composer-2.5 failed"),
        "{report:?}"
    );
    assert!(report.summary.contains("Cat Jam"), "{report:?}");
    let configs = std::fs::read_to_string(&configs).unwrap();
    let configs: Vec<&str> = configs.lines().collect();
    assert_eq!(configs.len(), 2, "{configs:?}");
    assert!(!configs[0].contains("cat-jam"), "{configs:?}");
    // The free model, under the gate's permission.
    assert!(configs[1].contains("opencode/cat-jam-free"), "{configs:?}");
    assert!(configs[1].contains("permission"), "{configs:?}");
    let mut notice = None;
    let mut model = None;
    while let Ok(event) = events.try_recv() {
        if let RuntimeEvent::Delegation { event, .. } = event {
            match *event {
                RuntimeEvent::Tool { name, output, .. } if name == crate::claude_stream::NOTICE => {
                    notice = output["text"].as_str().map(str::to_owned);
                }
                // The rail's model moves to the free one with the notice.
                RuntimeEvent::Model(named) if notice.is_some() && model.is_none() => {
                    model = Some(named);
                }
                _ => {}
            }
        }
    }
    let notice = notice.expect("the child's chat says it ran on the free model");
    assert!(
        notice.contains("cursor/composer-2.5") && notice.contains("Cat Jam"),
        "{notice}"
    );
    assert_eq!(model.as_deref(), Some(crate::opencode_chat::DEFAULT_MODEL));
}

/// The retry is for a provider refusal only, never an ordinary failure.
#[test]
fn only_a_provider_refusal_moves_opencode_to_its_free_model() {
    assert!(opencode_provider_refused(
        "OpenCode on cursor/composer-2.5 refused this version: Internal error: Update Required"
    ));
    assert!(opencode_provider_refused("Model not found: cursor/x"));
    assert!(!opencode_provider_refused(
        "The agent could not finish the task: it timed out."
    ));
}

/// #11575: a child that ends saying and changing nothing is not done.
#[tokio::test(flavor = "multi_thread")]
#[cfg(unix)]
async fn a_child_that_ends_without_an_answer_failed() {
    let _gate_lock = crate::approval::test_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main");
    std::fs::create_dir_all(&main).unwrap();
    repo(&main);
    let agent_dir = dir.path().join("agent");
    std::fs::create_dir_all(&agent_dir).unwrap();
    let mut blocks = acp_client::replay::blocks(acp_client::replay::OPENCODE_TURN);
    for block in &mut blocks {
        block.retain(|frame| frame["params"]["update"]["sessionUpdate"] != "agent_message_chunk");
    }
    let program = acp_client::replay::script(&agent_dir, &blocks);
    let mut execution = on_acp(&main, "opencode", "OpenCode", program);
    execution.agents[0].arguments = acp_client::opencode::arguments();
    let delegator = Delegator::new(
        Arc::new(host(execution)),
        Lineage::root("run-x", CODER),
        Authority::default(),
        &dir.path().join("runs"),
    );
    let report = delegator
        .delegate(json!({"to": "opencode", "task": "Say hello.", "read_only": true}))
        .await
        .unwrap();
    assert_eq!(report.outcome, Outcome::Failed, "{report:?}");
    assert!(
        report
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("ended without an answer"),
        "{report:?}"
    );
}

/// #11593: a `microcoder` call runs Microcoder, and with no model named
/// its card says the model it runs on while signed in: Gemini 4 Argon
/// through the OpenAgents gateway. Signed out, it says only Microcoder.
#[test]
fn a_microcoder_call_names_microcoder_on_gemini_4_argon() {
    let mut signed_in = execution();
    signed_in.connections = Some(openagents_login::Saved::with_token(
        "https://openagents.com",
        "acct",
        "Fixture",
        u64::MAX,
        "fixture-token",
    ));
    let target = host(signed_in)
        .target(&request("microcoder", None))
        .unwrap();
    assert_eq!(target.harness, CODER);
    assert_eq!(target.name, "Microcoder (Gemini 4 Argon)");
    assert_eq!(label(&target), "Microcoder (Gemini 4 Argon)");
    let mut signed_out = execution();
    signed_out.connections = None;
    let target = host(signed_out)
        .target(&request("microcoder", None))
        .unwrap();
    assert_eq!(target.name, "Microcoder");
}
