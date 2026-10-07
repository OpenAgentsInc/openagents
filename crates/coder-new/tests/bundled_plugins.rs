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
use std::{ffi::OsString, path::PathBuf};

fn key(app: &mut App, code: KeyCode) {
    assert!(app.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE))));
}

fn select(app: &mut App, id: &str) {
    app.open_plugins();
    while app.plugins.selected_definition().id != id {
        key(app, KeyCode::Down);
    }
}

fn rendered_text(app: &mut App) -> String {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(110, 36)).unwrap();
    terminal
        .draw(|frame| coder_new::ui::render(frame, app))
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

struct AcpFixture {
    directory: tempfile::TempDir,
}

impl AcpFixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("bin")).unwrap();
        Self { directory }
    }

    fn executable(&self, name: &str) -> PathBuf {
        #[cfg(windows)]
        let filename = format!("{name}.exe");
        #[cfg(not(windows))]
        let filename = name;
        let path = self.directory.path().join("bin").join(filename);
        std::fs::write(&path, b"This fixture must never be executed.").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        path
    }

    fn variable(&self, name: &str) -> Option<OsString> {
        match name {
            "PATH" => Some(self.directory.path().join("bin").into_os_string()),
            "HOME" | "USERPROFILE" => Some(self.directory.path().as_os_str().to_owned()),
            _ => None,
        }
    }

    fn app(&self) -> App {
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app.load_plugin_settings(Store::under(self.directory.path()))
            .unwrap();
        app.plugins
            .bundled
            .discover_acp(&|name| self.variable(name));
        app
    }
}

fn acp_tool(app: &App, fixture: &AcpFixture) -> Option<serde_json::Value> {
    app.plugins
        .execution_settings(fixture.directory.path().into())
        .defs()
        .into_iter()
        .find(|tool| tool["function"]["name"] == "acp_subagent")
}

fn acp_ids(app: &App) -> Vec<&str> {
    app.plugins
        .bundled
        .acp_choices()
        .into_iter()
        .map(|agent| agent.id.as_str())
        .collect()
}

#[test]
fn bundled_plugins_are_selectable_and_default_on_with_router_off() {
    let mut app = App::default();
    assert_eq!(DEFINITIONS.len(), 8);
    app.open_plugins();
    for definition in DEFINITIONS {
        assert_eq!(app.plugins.selected_definition().id, definition.id);
        assert_eq!(
            app.plugins.enabled_for(definition.id),
            definition.default_enabled
        );
        key(&mut app, KeyCode::Enter);
        assert!(app.screen == Screen::PluginSettings);
        let preview = rendered_text(&mut app);
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
fn acp_picker_detects_agents_and_persists_each_toggle_without_exposing_programs() {
    let fixture = AcpFixture::new();
    fixture.executable("grok");
    fixture.executable("opencode");
    let mut app = fixture.app();
    assert_eq!(acp_ids(&app), ["grok-build", "opencode"]);
    assert!(app.plugins.bundled.acp_choices().iter().all(|a| a.enabled));
    let tool = acp_tool(&app, &fixture).unwrap();
    assert_eq!(
        tool["function"]["parameters"]["properties"]["agent"]["enum"],
        json!(["grok-build", "opencode"])
    );
    assert!(
        !tool
            .to_string()
            .contains(fixture.directory.path().to_str().unwrap())
    );
    assert!(!tool.to_string().contains("program"));
    select(&mut app, "acp-subagents");
    key(&mut app, KeyCode::Enter);
    let preview = rendered_text(&mut app);
    assert!(preview.contains("Grok Build"));
    assert!(preview.contains("OpenCode"));
    assert!(!preview.contains("Paste a JSON"));
    key(&mut app, KeyCode::End);
    assert_eq!(app.plugins.bundled.acp_selected, 1);
    key(&mut app, KeyCode::Char(' '));
    assert!(app.screen == Screen::PluginSettings);
    assert!(!app.plugins.bundled.acp_choices()[1].enabled);

    let mut restored = fixture.app();
    assert!(!restored.plugins.bundled.acp_choices()[1].enabled);
    fixture.executable("goose");
    select(&mut restored, "acp-subagents");
    key(&mut restored, KeyCode::Enter);
    key(&mut restored, KeyCode::Char('r'));
    assert_eq!(acp_ids(&restored), ["grok-build", "opencode", "goose"]);
    assert!(restored.plugins.bundled.acp_choices()[2].enabled);
    let tool = acp_tool(&restored, &fixture).unwrap();
    assert_eq!(
        tool["function"]["parameters"]["properties"]["agent"]["enum"],
        json!(["grok-build", "goose"])
    );
    key(&mut restored, KeyCode::Esc);
    key(&mut restored, KeyCode::Char(' '));
    assert!(acp_tool(&restored, &fixture).is_none());
}

#[test]
fn acp_picker_omits_missing_agents_and_retains_their_disabled_preference() {
    let fixture = AcpFixture::new();
    let program = fixture.executable("opencode");
    let mut app = fixture.app();
    select(&mut app, "acp-subagents");
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Enter);
    assert!(!app.plugins.bundled.acp_choices()[0].enabled);
    std::fs::remove_file(program).unwrap();
    key(&mut app, KeyCode::Char('r'));
    assert!(app.plugins.bundled.acp_choices().is_empty());
    assert!(acp_tool(&app, &fixture).is_none());
    assert_eq!(app.plugins.bundled.acp_agents.len(), 1);
    assert!(!app.plugins.bundled.acp_agents[0].enabled);
    let mut restored = fixture.app();
    assert!(restored.plugins.bundled.acp_choices().is_empty());
    fixture.executable("opencode");
    restored.plugins.bundled.refresh_acp();
    assert_eq!(acp_ids(&restored), ["opencode"]);
    assert!(!restored.plugins.bundled.acp_choices()[0].enabled);
    assert!(acp_tool(&restored, &fixture).is_none());
}

#[test]
fn acp_picker_ignores_paste_and_keeps_demo_toggles_separate_from_live_settings() {
    let fixture = AcpFixture::new();
    fixture.executable("grok");
    fixture.executable("opencode");
    let mut app = fixture.app();
    select(&mut app, "acp-subagents");
    key(&mut app, KeyCode::Enter);
    let pasted = json!([{"id":"reviewer","name":"Review agent","program":"/fixture/reviewer"}]);
    assert!(app.handle(Event::Paste(pasted.to_string())));
    assert_eq!(acp_ids(&app), ["grok-build", "opencode"]);
    key(&mut app, KeyCode::Down);
    assert_eq!(app.plugins.bundled.acp_selected, 1);
    key(&mut app, KeyCode::Up);
    assert_eq!(app.plugins.bundled.acp_selected, 0);
    key(&mut app, KeyCode::End);
    key(&mut app, KeyCode::Home);
    assert_eq!(app.plugins.bundled.acp_selected, 0);
    key(&mut app, KeyCode::Enter);
    assert!(!app.plugins.bundled.acp_choices()[0].enabled);

    app.set_mode(Mode::Demo);
    assert!(app.plugins.bundled.acp_choices().iter().all(|a| a.enabled));
    select(&mut app, "acp-subagents");
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::End);
    key(&mut app, KeyCode::Char(' '));
    assert!(!app.plugins.bundled.acp_choices()[1].enabled);
    app.set_mode(Mode::Live);
    assert!(!app.plugins.bundled.acp_choices()[0].enabled);
    assert!(app.plugins.bundled.acp_choices()[1].enabled);
    let restored = fixture.app();
    assert!(!restored.plugins.bundled.acp_choices()[0].enabled);
    assert!(restored.plugins.bundled.acp_choices()[1].enabled);
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
