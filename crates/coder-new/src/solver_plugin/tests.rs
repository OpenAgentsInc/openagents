use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::{Arc, atomic::AtomicBool};

use serde_json::{Value, json};

use super::*;
use crate::plugin_tools::ExecutionSettings;

thread_local! {
    /// The recording a test's run plays instead of starting one.
    pub(super) static REPLAY: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// A recorded solver run: issue 11338, a one-line docs fix that passed and
/// opened a pull request (`coder solver 11338`, its `events.jsonl`).
fn recording() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/solver-run-11338.jsonl")
}

fn settings(solver: bool) -> ExecutionSettings {
    let mut settings = crate::plugins::Plugins::default().execution_settings(std::env::temp_dir());
    settings.solver = solver;
    settings.memory = None;
    settings
}

#[test]
fn arguments_take_an_issue_in_each_form_and_nothing_else() {
    assert_eq!(arguments(&json!({"issue": 12})).unwrap(), (None, 12, false));
    assert_eq!(
        arguments(&json!({"issue": "#12"})).unwrap(),
        (None, 12, false)
    );
    assert_eq!(
        arguments(&json!({"issue": "acme/app#7", "no_pr": true})).unwrap(),
        (Some("acme/app".into()), 7, true)
    );
    assert!(arguments(&json!({"issue": "twelve"})).is_err());
    assert!(arguments(&json!({"issue": 12, "replay": "/tmp/x"})).is_err());
    assert!(arguments(&json!({})).is_err());
    assert_eq!(
        task(&json!({"issue": "#12", "no_pr": true})).unwrap(),
        "Solve OpenAgentsInc/openagents#12, without opening a pull request."
    );
    let options = options(
        &json!({"issue": "acme/app#7", "no_pr": true}),
        Path::new("/w"),
    )
    .unwrap();
    assert_eq!(options.inner.issue, 7);
    assert_eq!(options.inner.github, "acme/app");
    assert_eq!(options.inner.repo, PathBuf::from("/w"));
    assert!(!options.inner.open_pr);
}

#[test]
fn the_plugin_is_listed_and_on_by_default() {
    let definition = crate::plugin_definition::DEFINITIONS
        .iter()
        .find(|definition| definition.id == PLUGIN)
        .expect("solver is a registered plugin");
    assert!(definition.default_enabled);
    let plugins = crate::plugins::Plugins::default();
    assert!(plugins.definitions().any(|row| row.id == PLUGIN));
    assert!(plugins.bundled.enabled(PLUGIN));
    let execution = plugins.execution_settings(std::env::temp_dir());
    assert!(execution.solver);
    assert!(
        execution
            .defs()
            .iter()
            .any(|tool| tool["function"]["name"] == TOOL)
    );
    assert!(execution.instructions().contains("solver tool"));
}

/// Off: the tool is not offered, a call is refused before any run starts,
/// and the choice survives a restart (a fresh load of the same store).
#[tokio::test]
async fn turned_off_it_is_not_offered_and_refuses_and_stays_off() {
    let state = tempfile::tempdir().unwrap();
    let store = crate::plugin_store::Store::under(state.path());
    let mut bundled = crate::bundled_settings::BundledSettings::default();
    bundled.load(store.clone()).unwrap();
    bundled.set_live(true);
    assert!(bundled.enabled(PLUGIN));
    assert!(bundled.toggle(PLUGIN));
    assert!(!bundled.enabled(PLUGIN));
    assert_eq!(bundled.status(PLUGIN), "Disabled");

    let mut restarted = crate::bundled_settings::BundledSettings::default();
    restarted.load(store).unwrap();
    restarted.set_live(true);
    assert!(!restarted.enabled(PLUGIN), "the choice is saved");

    let execution = settings(false);
    assert!(
        !execution
            .defs()
            .iter()
            .any(|tool| tool["function"]["name"] == TOOL)
    );
    assert!(!execution.instructions().contains("solver tool"));
    REPLAY.with(|replay| *replay.borrow_mut() = Some(recording()));
    let mut events = Vec::new();
    let result = execution
        .execute(
            TOOL,
            json!({"issue": 11338}),
            None,
            &Arc::new(AtomicBool::new(false)),
            &mut |event| events.push(event),
        )
        .await;
    REPLAY.with(|replay| *replay.borrow_mut() = None);
    assert_eq!(
        result.unwrap_err(),
        "This tool is unknown or its plugin is turned off."
    );
    assert!(events.is_empty(), "no run started");
}

/// A delegation dispatches to the solver, and the recorded run's changes
/// become the child chat: its hands' tool calls and words, the closing
/// line, and a result with the outcome, pull request, cost and folder.
#[tokio::test]
async fn a_delegation_plays_the_run_into_its_child_chat_and_result() {
    REPLAY.with(|replay| *replay.borrow_mut() = Some(recording()));
    let execution = settings(true);
    let arguments = json!({"issue": "#11338", "no_pr": true});
    let mut events = Vec::new();
    let result = execution
        .execute(
            TOOL,
            arguments.clone(),
            None,
            &Arc::new(AtomicBool::new(false)),
            &mut |event| events.push(event),
        )
        .await;
    REPLAY.with(|replay| *replay.borrow_mut() = None);
    let result = result.expect("the replayed run returns a result");
    assert_eq!(result["outcome"], "passed");
    assert_eq!(result["issue"], 11338);
    assert_eq!(
        result["pull_request"],
        "https://github.com/OpenAgentsInc/openagents/pull/11345"
    );
    assert!(
        (result["cost_usd"].as_f64().unwrap() - 0.107).abs() < 0.001,
        "all four hands"
    );
    assert!(
        result["run_folder"]
            .as_str()
            .unwrap()
            .ends_with("solver-runs/11338-1791750565-d14153fd2bff6411")
    );
    let reply = result["reply"].as_str().unwrap();
    assert!(
        reply.starts_with("Solver passed on issue #11338."),
        "{reply}"
    );
    assert!(reply.contains("Pull request: https://github.com/"));

    // The child chat, as the terminal builds it from those events.
    let mut app = crate::App::default();
    let task = task(&arguments).unwrap();
    let send = |app: &mut crate::App, event: RuntimeEvent| {
        app.apply_delegation("call-1".into(), NAME.into(), task.clone(), event);
    };
    send(
        &mut app,
        RuntimeEvent::Tool {
            name: TOOL.into(),
            input: arguments.clone(),
            output: Value::Null,
            running: true,
        },
    );
    assert!(app.delegations[0].running, "the rail shows it working");
    for event in events {
        send(&mut app, event);
    }
    send(
        &mut app,
        RuntimeEvent::Tool {
            name: TOOL.into(),
            input: Value::Null,
            output: result.clone(),
            running: false,
        },
    );
    let child = &app.delegations[0];
    assert_eq!(child.name, "Solver");
    assert!(!child.running);
    let entries = &child.chat.entries;
    assert!(matches!(&entries[0], Entry::User(text) if text == &task));
    let tools: Vec<&str> = entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::Tool { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    for hand_tool in [
        "Decision",
        "Grep",
        "Read",
        "Edit",
        "Run",
        issue_run::SUMMARY,
    ] {
        assert!(tools.contains(&hand_tool), "{hand_tool} in {tools:?}");
    }
    assert!(
        entries
            .iter()
            .all(|entry| !matches!(entry, Entry::Tool { running: true, .. })),
        "every call finished"
    );
    let words: String = entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::Assistant { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(words.contains("Solver ended: passed"), "{words}");
    assert!(words.contains("dead link"), "the Coder's reply is shown");
    // The main conversation's rail entry carries the result.
    assert!(app.live.entries.iter().any(|entry| matches!(entry,
        Entry::Delegation { name, running: false, output, .. }
            if name == "Solver" && output["outcome"] == "passed")));
}

/// Stopping the chat stops the run and says so.
#[tokio::test]
async fn a_stopped_delegation_stops_the_run() {
    let mut options = options(&json!({"issue": 11338}), Path::new("/")).unwrap();
    options.inner.replay = Some(recording());
    options.inner.speed = 0.01;
    let cancel = AtomicBool::new(true);
    let mut events = Vec::new();
    let result = run(crate::solver::start(options), &cancel, &mut |event| {
        events.push(event)
    })
    .await;
    assert_eq!(result.unwrap_err(), "The solver run was stopped.");
}

#[test]
fn a_run_that_never_set_up_reports_its_closing_line() {
    let mut mapper = Mapper::default();
    let mut events = Vec::new();
    mapper.apply(
        Event::Notice(
            "Solver ended: setup_failed (The checkout is not clean). Recording: /r/x/events.jsonl."
                .into(),
        ),
        &mut |event| events.push(event),
    );
    let result = mapper.result().unwrap();
    assert_eq!(result["outcome"], "setup_failed");
    assert_eq!(result["run_folder"], "/r/x");
    assert!(result["pull_request"].is_null());
    assert!(
        result["reply"]
            .as_str()
            .unwrap()
            .starts_with("Solver could not set up.")
    );
    assert!(Mapper::default().result().is_err());
}

#[test]
fn with_claude_off_solver_is_offered_only_on_a_signed_in_gateway() {
    // #11518: nothing for Solver to run on, no Solver tool.
    let offered = |settings: &ExecutionSettings| {
        settings
            .defs()
            .iter()
            .any(|definition| definition["function"]["name"] == TOOL)
    };
    let mut settings = settings(true);
    assert!(settings.claude && offered(&settings));
    settings.claude = false;
    settings.connections = None;
    assert!(!runnable(&settings) && !offered(&settings));
    settings.connections = Some(
        serde_json::from_value(json!({
            "origin": "http://fixture", "account": "acct", "label": "Fixture",
            "expires_at": u64::MAX, "token": "fixture-solver-token",
        }))
        .unwrap(),
    );
    assert!(runnable(&settings) && offered(&settings));
}
