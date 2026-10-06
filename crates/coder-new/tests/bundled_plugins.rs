use coder_new::{
    App, Mode, Screen,
    live::{Entry, Update, Work},
    plugin_definition::DEFINITIONS,
    plugin_store::Store,
    plugins::{Connection, SettingsFocus},
    snapshot,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;

fn key(app: &mut App, code: KeyCode) {
    assert!(app.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE))));
}

fn select(app: &mut App, id: &str) {
    app.open_plugins();
    while app.plugins.selected_definition().id != id {
        key(app, KeyCode::Down);
    }
}

#[test]
fn bundled_plugins_are_selectable_and_default_on_with_router_off() {
    let mut app = App::default();
    assert_eq!(DEFINITIONS.len(), 5);
    app.open_plugins();
    for definition in DEFINITIONS {
        assert_eq!(app.plugins.selected_definition().id, definition.id);
        assert_eq!(
            app.plugins.enabled_for(definition.id),
            definition.default_enabled
        );
        key(&mut app, KeyCode::Enter);
        assert!(app.screen == Screen::PluginSettings);
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(110, 36)).unwrap();
        terminal
            .draw(|frame| coder_new::ui::render(frame, &mut app))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let preview: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        assert!(preview.contains(definition.name));
        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::Down);
    }
    app.set_mode(Mode::Live);
    assert!(app.handle(Event::Paste("Explain this checkout.".into())));
    key(&mut app, KeyCode::Enter);
    let request = app.request.take().unwrap();
    assert!(matches!(request.kind, Work::Microcoder { .. }));
    assert!(request.key.expose().is_empty());
}

#[test]
fn adding_the_first_router_key_enables_it_but_saving_after_explicit_off_does_not() {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.open_plugin_settings();
    assert!(app.handle(Event::Paste("fixture-router-credential".into())));
    app.plugins.focus = SettingsFocus::Save;
    key(&mut app, KeyCode::Enter);
    assert!(app.plugins.enabled);
    key(&mut app, KeyCode::Char(' '));
    assert!(!app.plugins.enabled);
    app.open_plugin_settings();
    assert!(app.handle(Event::Paste("replacement-fixture-credential".into())));
    app.plugins.focus = SettingsFocus::Save;
    key(&mut app, KeyCode::Enter);
    assert!(!app.plugins.enabled);
    let settings = app
        .plugins
        .execution_settings(std::path::PathBuf::from("/fixture"));
    assert_eq!(
        settings.redact_text("replacement-fixture-credential"),
        "[redacted]"
    );
}

#[test]
fn jev_key_configuration_checks_the_sdk_provider_and_preserves_private_settings() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.load_plugin_settings(Store::under(directory.path()))
        .unwrap();
    select(&mut app, "jev");
    key(&mut app, KeyCode::Enter);
    assert!(app.handle(Event::Paste("fixture-jev-credential".into())));
    assert!(!snapshot::svg(&mut app, 80, 24).contains("fixture-jev-credential"));
    app.plugins.bundled.focus = SettingsFocus::Save;
    key(&mut app, KeyCode::Enter);
    assert!(app.screen == Screen::Plugins);
    let request = app.request.take().unwrap();
    assert!(matches!(request.kind, Work::CheckJev { .. }));
    assert_eq!(request.key.expose(), "fixture-jev-credential");
    app.apply_update(Update::CheckedJev {
        id: request.id,
        result: Ok(vec!["jev-latest".into()]),
    });
    assert!(matches!(
        app.plugins.bundled.connection,
        Connection::Verified
    ));
    assert!(!matches!(app.plugins.connection, Connection::Verified));
    key(&mut app, KeyCode::Char(' '));
    let settings = app.plugins.execution_settings(directory.path().into());
    assert!(
        !settings
            .defs()
            .iter()
            .any(|tool| tool["function"]["name"] == "jev")
    );
    assert_eq!(settings.redact_text("fixture-jev-credential"), "[redacted]");
    let mut restored = App::default();
    restored.set_mode(Mode::Live);
    restored
        .load_plugin_settings(Store::under(directory.path()))
        .unwrap();
    assert!(!restored.plugins.bundled.jev_enabled);
    assert!(restored.plugins.bundled.jev_key().is_some());
}

#[test]
fn acp_editor_saves_named_agents_without_exposing_their_executables_to_models() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.load_plugin_settings(Store::under(directory.path()))
        .unwrap();
    select(&mut app, "acp-subagents");
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Home);
    key(&mut app, KeyCode::Delete);
    key(&mut app, KeyCode::Delete);
    let agents = json!([{"id":"reviewer","name":"Review agent","program":"/fixture/reviewer","arguments":["--acp"],"enabled":true}]);
    assert!(app.handle(Event::Paste(agents.to_string())));
    assert!(app.handle(Event::Key(KeyEvent::new(
        KeyCode::Char('s'),
        KeyModifiers::CONTROL
    ))));
    assert!(app.screen == Screen::Plugins);
    let settings = app.plugins.execution_settings(directory.path().into());
    let tools = settings.defs();
    let tool = tools
        .iter()
        .find(|tool| tool["function"]["name"] == "acp_subagent")
        .unwrap();
    assert_eq!(
        tool["function"]["parameters"]["properties"]["agent"]["enum"],
        json!(["reviewer"])
    );
    assert!(!tool.to_string().contains("/fixture/reviewer"));
    app.set_mode(Mode::Demo);
    assert!(app.plugins.bundled.acp_agents.is_empty());
    app.set_mode(Mode::Live);
    assert_eq!(app.plugins.bundled.acp_agents[0].id, "reviewer");
}

#[test]
fn live_tool_progress_survives_typing_and_canceled_updates_cannot_revive_it() {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.live.busy = true;
    let id = app.request_id;
    let tool = |running, output| Update::Tool {
        id,
        name: "jev".into(),
        input: json!({"state":"fixture"}),
        output,
        running,
    };
    app.apply_update(tool(true, json!(null)));
    app.tick();
    assert!(app.handle(Event::Paste("next prompt".into())));
    assert_eq!(app.animation_frame, 1);
    app.apply_update(tool(false, json!({"choice":"a"})));
    assert_eq!(app.live.entries.len(), 1);
    assert!(matches!(
        &app.live.entries[0],
        Entry::Tool { running: false, .. }
    ));
    app.apply_update(tool(true, json!(null)));
    key(&mut app, KeyCode::Esc);
    app.apply_update(tool(false, json!({"late":"result"})));
    assert!(!snapshot::svg(&mut app, 110, 36).contains("late"));
    assert!(
        app.live
            .entries
            .iter()
            .all(|entry| !matches!(entry, Entry::Tool { running: true, .. }))
    );
    assert!(
        app.live
            .messages()
            .iter()
            .all(|message| message.role != "tool")
    );
}
