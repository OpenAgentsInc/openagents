use coder_new::{App, Screen, plugins::SettingsFocus, snapshot, ui};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

const KEY_INPUT: &str = "redaction_probe";

fn key(app: &mut App, code: KeyCode) {
    assert!(app.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE))));
}

fn paste(app: &mut App, text: &str) {
    assert!(app.handle(Event::Paste(text.into())));
}

struct Canvas {
    text: String,
    cursor: (u16, u16),
    cursor_visible: bool,
}

fn render(app: &mut App, width: u16, height: u16) -> Canvas {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| ui::render(frame, app)).unwrap();
    let cursor = terminal.get_cursor_position().unwrap();
    let buffer = terminal.backend().buffer();
    let text = (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    Canvas {
        text,
        cursor: (cursor.x, cursor.y),
        cursor_visible: terminal.backend().cursor_visible(),
    }
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

fn configure(app: &mut App, model: &str) {
    app.open_plugin_settings();
    paste(app, KEY_INPUT);
    key(app, KeyCode::Enter);
    replace_model(app, model);
    key(app, KeyCode::Enter);
    key(app, KeyCode::Enter);
    assert!(app.screen == Screen::Plugins);
    assert!(app.plugins.key_configured);
}

fn replace_model(app: &mut App, text: &str) {
    focus(app, SettingsFocus::Model);
    key(app, KeyCode::End);
    for _ in 0..app.plugins.field(false).0.chars().count() {
        key(app, KeyCode::Backspace);
    }
    paste(app, text);
}

fn svg_text(svg: &str) -> String {
    svg.lines()
        .filter(|line| line.starts_with("<text "))
        .map(|line| {
            let (_, text) = line.split_once('>').unwrap();
            text.strip_suffix("</text>").unwrap()
        })
        .collect()
}

#[test]
fn plugin_navigation_returns_to_the_chat_without_changing_chat_state() {
    let mut app = App::default();
    key(&mut app, KeyCode::Down);
    paste(&mut app, "local chat message");
    key(&mut app, KeyCode::Enter);
    paste(&mut app, "unsent chat draft");
    key(&mut app, KeyCode::Left);
    app.scroll = 7;
    let draft = (app.draft.text.clone(), app.draft.cursor);
    let messages = app.messages.clone();
    let selected = app.selected_agent;

    key(&mut app, KeyCode::F(2));
    assert!(app.screen == Screen::Plugins);
    assert!(render(&mut app, 80, 24).text.contains("OpenRouter BYOK"));
    paste(&mut app, "ignored in the manager");
    key(&mut app, KeyCode::Char('z'));
    key(&mut app, KeyCode::Enter);
    assert!(app.screen == Screen::PluginSettings);
    paste(&mut app, KEY_INPUT);
    key(&mut app, KeyCode::Esc);
    assert!(app.screen == Screen::Plugins);
    key(&mut app, KeyCode::F(2));
    assert!(app.screen == Screen::Conversation);
    assert_eq!((app.draft.text.clone(), app.draft.cursor), draft);
    assert_eq!(app.messages, messages);
    assert_eq!(app.selected_agent, selected);
    assert_eq!(app.scroll, 7);
    assert!(!app.plugins.key_configured);
    assert!(app.plugins.field(true).0.is_empty());

    key(&mut app, KeyCode::Tab);
    assert!(app.screen == Screen::Conversation);
    key(&mut app, KeyCode::F(2));
    key(&mut app, KeyCode::Esc);
    assert!(app.screen == Screen::Conversation);
    assert_eq!((app.draft.text.clone(), app.draft.cursor), draft);
    assert_eq!(app.selected_agent, selected);

    let mut command_app = App::default();
    paste(&mut command_app, "/plugins");
    key(&mut command_app, KeyCode::Enter);
    assert!(command_app.screen == Screen::Plugins);
    assert!(command_app.messages.is_empty());
    assert!(command_app.draft.text.is_empty());
}

#[test]
fn plugin_shortcuts_open_over_other_screens_without_changing_drafts() {
    for (code, modifiers) in [
        (KeyCode::Char('p'), KeyModifiers::CONTROL),
        (KeyCode::Char('P'), KeyModifiers::SUPER),
    ] {
        let mut app = App::default();
        key(&mut app, KeyCode::Down);
        paste(&mut app, "unsent chat draft");
        key(&mut app, KeyCode::Left);
        app.scroll = 7;
        let draft = (app.draft.text.clone(), app.draft.cursor);
        let selected = app.selected_agent;
        app.plugins.enabled = true;
        app.open_models();
        assert!(app.model_picker.is_some());

        let shortcut = KeyEvent::new(code, modifiers);
        assert!(app.handle(Event::Key(shortcut)));
        assert!(app.screen == Screen::Plugins);
        assert!(app.model_picker.is_none());
        assert!(app.request.is_none());
        key(&mut app, KeyCode::Enter);
        paste(&mut app, KEY_INPUT);
        let settings_draft = app.plugins.field(true);
        assert!(app.handle(Event::Key(shortcut)));
        assert!(app.screen == Screen::Plugins);
        assert_eq!(app.plugins.field(true), settings_draft);
        assert!(!app.plugins.key_configured);
        key(&mut app, KeyCode::Esc);
        assert!(app.screen == Screen::Conversation);
        assert_eq!((app.draft.text.clone(), app.draft.cursor), draft);
        assert_eq!(app.selected_agent, selected);
        assert_eq!(app.scroll, 7);

        let mut released = shortcut;
        released.kind = KeyEventKind::Release;
        assert!(app.handle(Event::Key(released)));
        assert!(app.screen == Screen::Conversation);
        key(&mut app, KeyCode::Char('p'));
        assert!(app.screen == Screen::Conversation);
        assert_eq!(app.draft.text, "unsent chat drafpt");
    }
}

#[test]
fn enablement_and_key_configuration_have_separate_states() {
    let mut app = App::default();
    app.open_plugins();
    assert!(!app.plugins.enabled);
    assert!(!app.plugins.key_configured);
    assert_eq!(app.plugins.model, "openrouter/free");
    assert_eq!(app.plugins.status(), "Disabled");
    key(&mut app, KeyCode::Char(' '));
    assert_eq!(app.plugins.status(), "Setup required");
    assert!(render(&mut app, 80, 24).text.contains("Not configured"));

    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Enter);
    replace_model(&mut app, "model-before-key");
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Enter);
    assert!(app.screen == Screen::Plugins);
    assert!(!app.plugins.key_configured);
    assert_eq!(app.plugins.model, "model-before-key");
    assert_eq!(app.plugins.status(), "Setup required");

    key(&mut app, KeyCode::Enter);
    paste(&mut app, "invalid probe");
    focus(&mut app, SettingsFocus::Save);
    key(&mut app, KeyCode::Enter);
    assert!(app.screen == Screen::PluginSettings);
    assert!(app.plugins.focus == SettingsFocus::ApiKey);
    assert!(!app.plugins.key_configured);
    assert!(
        render(&mut app, 80, 24)
            .text
            .contains("The API key cannot contain spaces.")
    );
    key(&mut app, KeyCode::Home);
    for _ in 0..app.plugins.field(true).0.chars().count() {
        key(&mut app, KeyCode::Delete);
    }
    paste(&mut app, KEY_INPUT);
    assert!(app.plugins.error.is_none());
    replace_model(&mut app, "demo-model");
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Enter);
    assert!(app.screen == Screen::Plugins);
    assert_eq!(app.plugins.status(), "Configured");
    assert_eq!(app.plugins.model, "demo-model");
    assert_eq!(app.plugins.field(true), (String::new(), 0));
    let manager = render(&mut app, 80, 24).text;
    assert!(manager.contains("Configured"));
    assert!(manager.contains("Added · not verified"));
    assert!(manager.contains("demo-model"));

    key(&mut app, KeyCode::Char(' '));
    assert_eq!(app.plugins.status(), "Disabled");
    assert!(app.plugins.key_configured);
    key(&mut app, KeyCode::Char(' '));
    assert_eq!(app.plugins.status(), "Configured");
    key(&mut app, KeyCode::Enter);
    replace_model(&mut app, "");
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.plugins.model, "openrouter/free");
    // The default is shown as `auto`, never the router's vendor name.
    let screen = render(&mut app, 80, 24).text;
    assert!(screen.contains("auto"));
    assert!(!screen.contains("openrouter/free"));
}

#[test]
fn settings_changes_and_key_removal_take_effect_only_when_saved() {
    let mut app = App::default();
    app.open_plugins();
    key(&mut app, KeyCode::Char(' '));
    configure(&mut app, "original-model");

    app.open_plugin_settings();
    paste(&mut app, "replacement_probe");
    replace_model(&mut app, "replacement-model");
    assert_eq!(app.plugins.model, "original-model");
    assert!(app.plugins.key_configured);
    focus(&mut app, SettingsFocus::Cancel);
    key(&mut app, KeyCode::Enter);
    assert!(app.screen == Screen::Plugins);
    assert_eq!(app.plugins.model, "original-model");
    assert_eq!(app.plugins.field(true), (String::new(), 0));
    app.open_plugin_settings();
    assert_eq!(app.plugins.field(false).0, "original-model");
    assert!(app.plugins.key_label().contains("Key added"));

    focus(&mut app, SettingsFocus::RemoveKey);
    key(&mut app, KeyCode::Enter);
    assert!(app.plugins.key_configured);
    assert_eq!(app.plugins.key_label(), "Key will be removed on save");
    key(&mut app, KeyCode::Esc);
    assert!(app.plugins.key_configured);
    assert_eq!(app.plugins.status(), "Configured");

    app.open_plugin_settings();
    focus(&mut app, SettingsFocus::RemoveKey);
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Up);
    assert!(app.plugins.focus == SettingsFocus::Save);
    key(&mut app, KeyCode::Enter);
    assert!(app.screen == Screen::Plugins);
    assert!(!app.plugins.key_configured);
    assert!(app.plugins.enabled);
    assert_eq!(app.plugins.model, "original-model");
    assert_eq!(app.plugins.status(), "Setup required");
    assert_eq!(app.plugins.field(true), (String::new(), 0));
    assert!(render(&mut app, 80, 24).text.contains("Not configured"));
}

#[test]
fn key_editing_masks_graphemes_and_never_exports_the_entered_text() {
    let mut app = App::default();
    app.open_plugin_settings();
    paste(&mut app, &format!("{KEY_INPUT}a👩‍💻e\u{301}界"));
    let count = KEY_INPUT.chars().count() + 4;
    assert_eq!(app.plugins.field(true), ("•".repeat(count), count * 3));
    let canvas = render(&mut app, 80, 24);
    assert!(canvas.cursor_visible);
    assert_eq!(canvas.cursor.0, 5 + count as u16);
    assert!(canvas.text.contains(&"•".repeat(count)));
    assert!(!canvas.text.contains(KEY_INPUT));
    assert!(!canvas.text.contains("👩‍💻"));
    assert!(!canvas.text.contains('界'));
    let svg = snapshot::svg(&mut app, 80, 24);
    assert!(svg.contains('•'));
    assert!(svg.contains("<animate "));
    assert!(!svg_text(&svg).contains(KEY_INPUT));
    assert!(!svg.contains("👩‍💻"));
    assert!(!svg.contains('界'));

    key(&mut app, KeyCode::Left);
    key(&mut app, KeyCode::Backspace);
    key(&mut app, KeyCode::Delete);
    assert_eq!(
        app.plugins.field(true),
        ("•".repeat(count - 2), (count - 2) * 3)
    );
    key(&mut app, KeyCode::Backspace);
    assert_eq!(
        app.plugins.field(true),
        ("•".repeat(count - 3), (count - 3) * 3)
    );
    assert_eq!(render(&mut app, 80, 24).cursor.0, 5 + (count - 3) as u16);
    focus(&mut app, SettingsFocus::Save);
    key(&mut app, KeyCode::Enter);
    assert!(app.screen == Screen::Plugins);
    assert!(app.plugins.key_configured);
    assert_eq!(app.plugins.field(true), (String::new(), 0));
    assert!(!svg_text(&snapshot::svg(&mut app, 80, 24)).contains(KEY_INPUT));
    app.open_plugin_settings();
    assert!(!render(&mut app, 80, 24).text.contains('•'));
}

#[test]
fn settings_focus_stays_visible_after_resize_and_actions_hide_the_cursor() {
    let mut app = App::default();
    app.open_plugins();
    for (width, height) in [(80, 24), (24, 12)] {
        let canvas = render(&mut app, width, height);
        assert!(canvas.text.contains("Plugins"));
        assert!(canvas.text.contains("OpenRouter BYOK"));
        assert!(!canvas.cursor_visible);
        assert!(!snapshot::svg(&mut app, width, height).contains("<animate "));
    }
    app.open_plugin_settings();
    paste(&mut app, &KEY_INPUT.repeat(4));
    let focuses = [
        (SettingsFocus::ApiKey, None),
        (SettingsFocus::Model, None),
        (SettingsFocus::TestKey, Some("Test API key")),
        (SettingsFocus::Save, Some("Save settings")),
        (SettingsFocus::RemoveKey, Some("Remove API key")),
        (SettingsFocus::Cancel, Some("Cancel")),
    ];
    for (expected, action) in focuses {
        assert!(app.plugins.focus == expected);
        if expected == SettingsFocus::Model {
            paste(&mut app, "a-long-model-name-for-the-resize-check");
        }
        let fields = (app.plugins.field(true), app.plugins.field(false));
        for (width, height) in [(80, 24), (24, 12), (80, 24)] {
            let canvas = render(&mut app, width, height);
            assert!(canvas.text.contains("OpenRouter BYOK"));
            assert!(!canvas.text.contains(KEY_INPUT));
            let svg = snapshot::svg(&mut app, width, height);
            assert!(!svg_text(&svg).contains(KEY_INPUT));
            if let Some(label) = action {
                assert!(canvas.text.contains(&format!("❯ {label}")));
                assert!(!canvas.cursor_visible);
                assert!(!svg.contains("<animate "));
            } else {
                assert!(canvas.cursor_visible);
                assert!(canvas.cursor.0 < width && canvas.cursor.1 < height);
                assert!(svg.contains("<animate "));
            }
            assert_eq!((app.plugins.field(true), app.plugins.field(false)), fields);
        }
        key(&mut app, KeyCode::Down);
    }
    assert!(app.plugins.focus == SettingsFocus::ApiKey);
    key(&mut app, KeyCode::BackTab);
    assert!(app.plugins.focus == SettingsFocus::Cancel);
    key(&mut app, KeyCode::Up);
    assert!(app.plugins.focus == SettingsFocus::RemoveKey);
    key(&mut app, KeyCode::Esc);
    assert!(app.screen == Screen::Plugins);
    assert_eq!(app.plugins.field(true), (String::new(), 0));
}
