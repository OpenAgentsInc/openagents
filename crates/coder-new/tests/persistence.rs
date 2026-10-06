use std::{fs, path::Path};

use coder_new::{App, Mode, Screen, plugin_store::Store, plugins::SettingsFocus, snapshot, ui};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

const FIRST_KEY: &str = "persistence_probe_one";
const SECOND_KEY: &str = "persistence_probe_two";

fn key(app: &mut App, code: KeyCode) {
    assert!(app.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE))));
}

fn paste(app: &mut App, text: &str) {
    assert!(app.handle(Event::Paste(text.into())));
}

fn focus(app: &mut App, target: SettingsFocus) {
    for _ in 0..6 {
        if app.plugins.focus == target {
            return;
        }
        key(app, KeyCode::Tab);
    }
    panic!("settings focus did not reach the requested field");
}

fn load(dir: &Path, mode: Mode) -> App {
    let mut app = App::default();
    app.set_mode(mode);
    app.load_plugin_settings(Store::under(dir.to_owned()))
        .unwrap();
    app
}

fn edit_settings(app: &mut App, input: &str, model: &str) {
    app.open_plugin_settings();
    paste(app, input);
    focus(app, SettingsFocus::Model);
    key(app, KeyCode::End);
    for _ in 0..app.plugins.field(false).0.chars().count() {
        key(app, KeyCode::Backspace);
    }
    paste(app, model);
    focus(app, SettingsFocus::Save);
}

fn save_settings(app: &mut App, input: &str, model: &str) {
    edit_settings(app, input, model);
    key(app, KeyCode::Enter);
    assert!(app.screen == Screen::Plugins);
    assert!(app.plugins.storage_error.is_none());
    app.cancel_request();
}

fn render(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(110, 36)).unwrap();
    terminal.draw(|frame| ui::render(frame, app)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..36)
        .map(|y| {
            (0..110)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn live_settings_and_key_changes_survive_restart_through_the_plugin_ui() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("coder-new");
    let mut app = load(&dir, Mode::Live);
    assert!(!dir.exists());
    app.open_plugins();
    key(&mut app, KeyCode::Char(' '));
    save_settings(&mut app, FIRST_KEY, "first-model");

    let mut reopened = load(&dir, Mode::Live);
    assert!(reopened.plugins.enabled);
    assert!(reopened.plugins.key_configured);
    assert_eq!(reopened.plugins.model, "first-model");
    assert_eq!(
        reopened.plugins.key_for_request().unwrap().expose(),
        FIRST_KEY
    );
    assert_eq!(reopened.plugins.status(), "Configured");
    reopened.open_plugin_settings();
    assert_eq!(reopened.plugins.field(true).0, "");
    assert!(!render(&mut reopened).contains(FIRST_KEY));
    key(&mut reopened, KeyCode::Esc);
    save_settings(&mut reopened, SECOND_KEY, "second-model");

    let mut replaced = load(&dir, Mode::Live);
    assert!(replaced.plugins.enabled);
    assert_eq!(replaced.plugins.model, "second-model");
    assert_eq!(
        replaced.plugins.key_for_request().unwrap().expose(),
        SECOND_KEY
    );
    replaced.open_plugins();
    key(&mut replaced, KeyCode::Char(' '));
    let mut disabled = load(&dir, Mode::Live);
    assert!(!disabled.plugins.enabled);
    assert!(disabled.plugins.key_configured);
    assert_eq!(disabled.plugins.model, "second-model");
    disabled.open_plugin_settings();
    focus(&mut disabled, SettingsFocus::RemoveKey);
    key(&mut disabled, KeyCode::Enter);
    focus(&mut disabled, SettingsFocus::Save);
    key(&mut disabled, KeyCode::Enter);

    let removed = load(&dir, Mode::Live);
    assert!(!removed.plugins.enabled);
    assert!(!removed.plugins.key_configured);
    assert!(removed.plugins.key_for_request().is_none());
    assert_eq!(removed.plugins.model, "second-model");
}

#[test]
fn drafts_key_checks_cancel_and_demo_edits_leave_live_storage_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("coder-new");
    let mut app = load(&dir, Mode::Live);
    app.open_plugins();
    key(&mut app, KeyCode::Char(' '));
    save_settings(&mut app, FIRST_KEY, "live-model");
    let path = dir.join("plugins.json");
    let original = fs::read(&path).unwrap();

    edit_settings(&mut app, SECOND_KEY, "unsaved-model");
    assert_eq!(fs::read(&path).unwrap(), original);
    focus(&mut app, SettingsFocus::TestKey);
    key(&mut app, KeyCode::Enter);
    assert!(app.request.is_some());
    assert_eq!(fs::read(&path).unwrap(), original);
    key(&mut app, KeyCode::Esc);
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(app.plugins.model, "live-model");
    assert_eq!(app.plugins.key_for_request().unwrap().expose(), FIRST_KEY);

    app.set_mode(Mode::Demo);
    app.open_plugins();
    key(&mut app, KeyCode::Char(' '));
    save_settings(&mut app, SECOND_KEY, "demo-model");
    assert!(app.plugins.enabled);
    assert!(app.plugins.key_configured);
    assert!(app.plugins.key_for_request().is_none());
    assert!(app.plugins.key_for_check().is_none());
    assert_eq!(fs::read(&path).unwrap(), original);
    let _ = snapshot::svg(&mut app, 110, 36);
    assert_eq!(fs::read(&path).unwrap(), original);

    let mut reopened = load(&dir, Mode::Demo);
    assert!(!reopened.plugins.enabled);
    assert!(!reopened.plugins.key_configured);
    assert!(reopened.plugins.model.is_empty());
    assert!(reopened.plugins.key_for_request().is_none());
    reopened.set_mode(Mode::Live);
    assert!(reopened.plugins.enabled);
    assert_eq!(reopened.plugins.model, "live-model");
    assert_eq!(
        reopened.plugins.key_for_request().unwrap().expose(),
        FIRST_KEY
    );
    reopened.set_mode(Mode::Demo);
    assert!(reopened.plugins.key_for_request().is_none());
    assert!(!render(&mut reopened).contains(FIRST_KEY));
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn failed_save_keeps_the_saved_live_configuration_and_editable_draft() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("coder-new");
    let mut app = load(&dir, Mode::Live);
    save_settings(&mut app, FIRST_KEY, "saved-model");
    fs::remove_dir_all(&dir).unwrap();
    fs::write(&dir, "blocks-directory-creation").unwrap();

    edit_settings(&mut app, SECOND_KEY, "replacement-model");
    key(&mut app, KeyCode::Enter);
    assert!(app.screen == Screen::PluginSettings);
    assert!(!app.plugins.saved);
    assert_eq!(app.plugins.model, "saved-model");
    assert_eq!(app.plugins.key_for_request().unwrap().expose(), FIRST_KEY);
    assert_eq!(app.plugins.field(false).0, "replacement-model");
    assert!(!app.plugins.field(true).0.is_empty());
    assert!(app.request.is_none());
    let error = app.plugins.storage_error.clone().unwrap();
    assert!(!error.contains(FIRST_KEY));
    assert!(!error.contains(SECOND_KEY));
    let shown = render(&mut app);
    assert!(shown.contains(&error));
    assert!(!shown.contains(FIRST_KEY));
    assert!(!shown.contains(SECOND_KEY));
}

#[test]
fn failed_toggle_leaves_the_live_enabled_preference_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("coder-new");
    let mut app = load(&dir, Mode::Live);
    app.open_plugins();
    key(&mut app, KeyCode::Char(' '));
    assert!(app.plugins.enabled);
    fs::remove_dir_all(&dir).unwrap();
    fs::write(&dir, "blocks-directory-creation").unwrap();

    key(&mut app, KeyCode::Char(' '));
    assert!(app.plugins.enabled);
    let error = app.plugins.storage_error.clone().unwrap();
    assert!(render(&mut app).contains(&error));
}

#[test]
fn invalid_existing_settings_are_reported_and_preserved_until_successfully_reloaded() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("coder-new");
    fs::create_dir(&dir).unwrap();
    let path = dir.join("plugins.json");
    let invalid = b"malformed_private_configuration";
    fs::write(&path, invalid).unwrap();
    let mut app = App::default();
    app.set_mode(Mode::Live);
    let error = app
        .load_plugin_settings(Store::under(dir.clone()))
        .unwrap_err();
    assert!(!error.contains("malformed_private_configuration"));
    assert!(app.plugins.storage_error.is_some());
    app.open_plugins();
    assert!(render(&mut app).contains(&error));
    key(&mut app, KeyCode::Char(' '));
    assert!(!app.plugins.enabled);
    assert_eq!(fs::read(&path).unwrap(), invalid);

    edit_settings(&mut app, FIRST_KEY, "unsaved-model");
    key(&mut app, KeyCode::Enter);
    assert!(app.screen == Screen::PluginSettings);
    assert!(app.plugins.key_for_request().is_none());
    assert!(app.plugins.model.is_empty());
    assert_eq!(fs::read(&path).unwrap(), invalid);
    key(&mut app, KeyCode::Esc);

    fs::remove_file(&path).unwrap();
    app.load_plugin_settings(Store::under(dir.clone())).unwrap();
    assert!(app.plugins.storage_error.is_none());
    key(&mut app, KeyCode::Char(' '));
    assert!(app.plugins.enabled);
    assert!(load(&dir, Mode::Live).plugins.enabled);
}
