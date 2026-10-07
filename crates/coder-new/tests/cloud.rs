use coder_cloud::{Placement, Record, Spec, State};
use coder_new::{
    App, Mode, Screen,
    cloud_settings::{Configuration, Editor},
    cloud_tools,
    plugin_store::Store,
    programmatic::{Context, execute},
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;
use std::{
    collections::BTreeSet,
    sync::{Arc, atomic::AtomicBool},
};
fn context(root: &std::path::Path) -> Context {
    Context {
        root: root.join("state"),
        cwd: root.into(),
        environment: Default::default(),
        input: None,
        canceled: None,
        approvals: None,
    }
}
#[test]
fn cli_cloud_preferences_persist_without_keys_and_disabled_dispatch_creates_no_job() {
    let root = tempfile::tempdir().unwrap();
    let mut c = context(root.path());
    let run = |args: &[&str], c: &Context| {
        execute(
            &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            c,
            &mut |_| {},
        )
    };
    assert!(
        run(
            &[
                "delegate",
                "codex",
                "--task",
                "fixture",
                "--on",
                "boat",
                "--no-workspace"
            ],
            &c
        )
        .unwrap_err()
        .message
        .contains("Enable boat-cloud")
    );
    assert!(!c.root.join("remote").exists());
    c.input=Some(json!({"enabled":true,"mode":"coder","template":"fixture-runtime","credential_names":["OPENAI_API_KEY"],"workspace_paths":["src"]}).to_string());
    run(&["plugins", "configure", "boat-cloud", "--stdin"], &c).unwrap();
    let mut app = {
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app
    };
    app.load_plugin_settings(Store::under(&c.root)).unwrap();
    assert!(app.plugins.bundled.boat.enabled);
    assert_eq!(app.plugins.bundled.boat.mode, coder_cloud::Mode::Coder);
    assert_eq!(
        app.plugins.bundled.boat.credential_names,
        ["OPENAI_API_KEY"]
    );
    c.input = Some(json!({"api_key":"fixture-never-save"}).to_string());
    assert!(run(&["plugins", "configure", "boat-cloud", "--stdin"], &c).is_err());
    let stored = std::fs::read_to_string(c.root.join("bundled-plugins.json")).unwrap();
    assert!(!stored.contains("fixture-never-save"));
    run(&["plugins", "disable", "boat-cloud"], &c).unwrap();
    assert!(
        run(
            &[
                "delegate",
                "codex",
                "--task",
                "fixture",
                "--on",
                "boat",
                "--no-workspace"
            ],
            &c
        )
        .is_err()
    );
}
#[test]
fn native_arguments_preserve_remote_identity_and_refuse_substitutes_and_unadmitted_keys() {
    let mut config = Configuration {
        enabled: true,
        ..Default::default()
    };
    config.credential_names = vec!["OPENAI_API_KEY".into()];
    let targets = BTreeSet::from(["codex".into()]);
    let args = cloud_tools::arguments(
        Placement::Boat,
        &config,
        json!({"agent":"codex@boat","task":"fixture","model":"chosen-model","no_workspace":true}),
        &targets,
    )
    .unwrap();
    assert_eq!(args[0], "codex");
    assert!(args.windows(2).any(|p| p == ["--model", "chosen-model"]));
    for agent in ["microcoder@boat", "codex@gce", "codex"] {
        assert!(
            cloud_tools::arguments(
                Placement::Boat,
                &config,
                json!({"agent":agent,"task":"fixture"}),
                &targets
            )
            .is_err()
        );
    }
    assert!(
        cloud_tools::arguments(
            Placement::Boat,
            &config,
            json!({"agent":"codex@boat","task":"fixture","credential_names":["ANTHROPIC_API_KEY"]}),
            &targets
        )
        .is_err()
    );
    config.enabled = false;
    assert!(
        cloud_tools::arguments(
            Placement::Boat,
            &config,
            json!({"agent":"codex@boat","task":"fixture"}),
            &targets
        )
        .is_err()
    );
}
#[test]
fn cloud_editor_saves_modes_and_keeps_demo_execution_disabled() {
    let root = tempfile::tempdir().unwrap();
    let mut app = {
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app
    };
    app.load_plugin_settings(Store::under(root.path())).unwrap();
    app.plugins.selected = coder_new::plugin_definition::DEFINITIONS
        .iter()
        .position(|p| p.id == "boat-cloud")
        .unwrap();
    app.open_plugin_settings();
    assert!(app.screen == Screen::PluginSettings);
    app.handle(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));
    assert_eq!(
        app.plugins
            .bundled
            .cloud_editor
            .as_ref()
            .unwrap()
            .config
            .mode,
        coder_cloud::Mode::Coder
    );
    app.plugins.bundled.cloud_editor.as_mut().unwrap().focus = 5;
    app.handle(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));
    assert!(app.screen == Screen::Plugins);
    assert_eq!(app.plugins.bundled.boat.mode, coder_cloud::Mode::Coder);
    let mut editor = Editor::new(Placement::Gce, Configuration::gce());
    editor.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(editor.config.mode, coder_cloud::Mode::Coder);
    let mut demo = {
        let mut app = App::default();
        app.set_mode(Mode::Demo);
        app
    };
    demo.plugins.bundled.boat.enabled = true;
    assert!(
        !demo
            .plugins
            .execution_settings(root.path().into())
            .boat
            .enabled
    );
}
#[tokio::test]
async fn terminal_follow_uses_retained_results_and_streams_a_placement_qualified_child() {
    let root = tempfile::tempdir().unwrap();
    let state = root.path().join("state");
    let store = coder_cloud::Store::under(state.join("remote"));
    let lease = store.lease("remote1").unwrap();
    let mut r = Record::new(
        "remote1",
        Spec {
            placement: Placement::Boat,
            mode: coder_cloud::Mode::Integrated,
            agent: "codex".into(),
            task: "fixture".into(),
            model: None,
            reasoning: None,
            cwd: root.path().into(),
            timeout_seconds: 60,
            size: "small".into(),
            template: None,
            credential_names: vec![],
        },
    )
    .unwrap();
    r.state = State::Completed;
    r.cleanup_complete = true;
    r.result = Some(json!({"reply":"retained answer","model":"actual-model"}));
    lease.save(&r).unwrap();
    drop(lease);
    let config = Configuration {
        enabled: true,
        ..Default::default()
    };
    let mut events = vec![];
    let result = cloud_tools::execute(
        Placement::Boat,
        true,
        config,
        state,
        root.path().into(),
        json!({"operation":"follow","job":"remote1"}),
        &BTreeSet::new(),
        Arc::new(AtomicBool::new(false)),
        &mut |e| events.push(e),
    )
    .await
    .unwrap();
    assert_eq!(result["reply"], "retained answer");
    assert!(events.iter().all(|e|matches!(e,coder_new::bundled_runtime::RuntimeEvent::Delegation{name,..} if name=="codex@boat")));
    assert!(events.iter().any(|e|matches!(e,coder_new::bundled_runtime::RuntimeEvent::Delegation{event,..} if matches!(event.as_ref(),coder_new::bundled_runtime::RuntimeEvent::Tool{output,running:false,..} if output["model"]=="actual-model" && output["job"]=="remote1"))));
}

#[test]
fn selected_remote_chat_retains_model_and_continues_the_same_job() {
    use coder_new::{
        bundled_runtime::RuntimeEvent,
        live::{Entry, Update, Work},
    };
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.plugins.bundled.boat.enabled = true;
    let workspace = tempfile::tempdir().unwrap();
    app.submit("Parent request", workspace.path());
    app.request.take().unwrap();
    let update = RuntimeEvent::Tool {
        name: "remote_delegate".into(),
        input: json!(null),
        output: json!({"job":"retained-remote","reply":"Remote answer","model":"actual/provider-model","tokens":7}),
        running: false,
    };
    app.apply_update(Update::Delegation {
        id: app.request_id,
        delegation: "retained-remote".into(),
        name: "codex@boat".into(),
        task: "Remote task".into(),
        event: update,
    });
    app.apply_update(Update::Finished {
        id: app.request_id,
        result: Ok(openrouter::Streamed::default()),
    });
    assert!(!app.delegations[0].running);
    assert!(
        matches!(app.delegations[0].chat.entries.last().unwrap(), Entry::Assistant { model: Some(model), .. } if model == "actual/provider-model")
    );
    app.selected_agent = Some(0);
    app.handle(Event::Paste("Continue this work".into()));
    app.handle(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));
    match app.request.take().unwrap().kind {
        Work::Delegate {
            tool,
            arguments,
            name,
            ..
        } => {
            assert_eq!(tool, "boat_job");
            assert_eq!(name, "codex@boat");
            assert_eq!(arguments["operation"], "continue");
            assert_eq!(arguments["job"], "retained-remote");
            assert!(
                arguments["message"]
                    .as_str()
                    .unwrap()
                    .contains("Remote answer")
            );
        }
        _ => panic!("Remote chat must continue its retained job."),
    }
    app.plugins.bundled.boat.enabled = false;
    app.cancel_request();
    app.delegations[0].running = false;
    app.delegations[0].chat.busy = false;
    app.handle(Event::Paste("Keep this draft".into()));
    app.handle(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));
    assert!(app.request.is_none());
    assert_eq!(app.draft.text, "Keep this draft");
}
