use coder_new::{App, Mode, Screen, live::Entry, plugin_store::Store, slash, snapshot, theme, ui};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, style::Color};

fn key(app: &mut App, code: KeyCode) {
    assert!(app.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE))));
}

fn draw(terminal: &mut Terminal<TestBackend>, app: &mut App) -> Buffer {
    terminal.draw(|frame| ui::render(frame, app)).unwrap();
    terminal.backend().buffer().clone()
}

fn text(buffer: &Buffer) -> String {
    buffer.content.iter().map(|cell| cell.symbol()).collect()
}

fn temporary() -> tempfile::TempDir {
    match std::env::var_os("OPENAGENTS_SCRATCH") {
        Some(root) => tempfile::tempdir_in(root).unwrap(),
        None => tempfile::tempdir().unwrap(),
    }
}

#[test]
fn appearance_command_toggles_and_persists_without_submitting_chat() {
    let temporary = temporary();
    let store = Store::under(temporary.path());
    let mut app = App::default();
    app.load_plugin_settings(store.clone()).unwrap();
    assert!(!app.appearance.use_system_terminal_background);
    assert_eq!(slash::matches("/app"), [slash::Command::Appearance]);
    assert!(slash::help().contains("/appearance"));
    assert!(app.handle(Event::Paste("/appearance".into())));
    key(&mut app, KeyCode::Enter);
    assert!(app.screen == Screen::Appearance);
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let rendered = text(&draw(&mut terminal, &mut app));
    assert!(rendered.contains("Use System Terminal Background [ off ]"));
    let mut narrow = Terminal::new(TestBackend::new(24, 12)).unwrap();
    let rendered = text(&draw(&mut narrow, &mut app));
    assert!(rendered.contains("Space/Enter Toggle"));
    assert!(rendered.contains("Esc Back"));
    key(&mut app, KeyCode::Char(' '));
    assert!(app.appearance.use_system_terminal_background);
    let rendered = text(&draw(&mut terminal, &mut app));
    assert!(rendered.contains("Use System Terminal Background [ on ]"));

    let mut release = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
    release.kind = KeyEventKind::Release;
    assert!(app.handle(Event::Key(release)));
    assert!(app.handle(Event::Paste("ignored in appearance".into())));
    key(&mut app, KeyCode::Char('x'));
    assert!(app.appearance.use_system_terminal_background);
    assert!(app.draft.text.is_empty());
    assert!(app.messages.is_empty());
    assert!(app.request.is_none());
    let mut restarted = App::default();
    restarted.load_plugin_settings(store.clone()).unwrap();
    assert!(restarted.appearance.use_system_terminal_background);

    key(&mut app, KeyCode::Enter);
    assert!(!app.appearance.use_system_terminal_background);
    restarted.load_plugin_settings(store).unwrap();
    assert!(!restarted.appearance.use_system_terminal_background);
    key(&mut app, KeyCode::Esc);
    assert!(app.screen == Screen::Conversation);

    assert!(app.handle(Event::Paste("unsent draft".into())));
    app.open_plugins();
    app.open_appearance();
    key(&mut app, KeyCode::Esc);
    assert!(app.screen == Screen::Plugins);
    assert_eq!(app.draft.text, "unsent draft");
}

fn assert_background_switching(mut app: App, width: u16, height: u16) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let original = draw(&mut terminal, &mut app);
    assert_eq!(original[(0, height - 1)].bg, theme::BG_BASE);
    let mut expected = original.clone();
    for cell in &mut expected.content {
        cell.bg = Color::Reset;
    }
    for enabled in [true, false, true] {
        app.appearance.use_system_terminal_background = enabled;
        let actual = draw(&mut terminal, &mut app);
        assert_eq!(&actual, if enabled { &expected } else { &original });
    }
}

#[test]
fn switching_backgrounds_preserves_text_and_foreground_styles_across_screens() {
    assert_background_switching(App::default(), 110, 70);
    let mut child = App::default();
    child.selected_agent = Some(3);
    assert_background_switching(child, 110, 70);

    let mut live = App::default();
    live.set_mode(Mode::Live);
    live.live.entries = vec![
        Entry::User("A user prompt".into()),
        Entry::Assistant {
            text: "# Reply\n\n```rust\nfn main() {}\n```\n\n```diff\n-old\n+new\n```".into(),
            model: None,
            elapsed_ms: None,
        },
    ];
    assert_background_switching(live, 80, 36);

    let mut slash_menu = App::default();
    assert!(slash_menu.handle(Event::Paste("/".into())));
    assert_background_switching(slash_menu, 80, 36);

    let mut plugins = App::default();
    plugins.open_plugins();
    assert_background_switching(plugins, 80, 36);
    let mut settings = App::default();
    settings.open_plugin_settings();
    assert_background_switching(settings, 80, 36);

    let mut models = App::default();
    models.plugins.enabled = true;
    models.open_models();
    assert!(models.model_picker.is_some());
    assert_background_switching(models, 80, 36);

    let mut resume = App::default();
    resume.resume_picker = Some(coder_new::resume::Picker {
        sessions: Vec::new(),
        selected: 0,
        page: 0,
        error: None,
    });
    assert_background_switching(resume, 80, 36);
    assert_background_switching(App::default(), 23, 9);
    assert_background_switching(App::default(), 1, 1);
}

#[test]
fn damaged_preferences_show_an_error_and_keep_the_background_unchanged() {
    let temporary = temporary();
    let path = temporary.path().join("appearance.json");
    std::fs::write(&path, "invalid JSON").unwrap();
    let mut app = App::default();
    assert!(
        app.load_plugin_settings(Store::under(temporary.path()))
            .is_err()
    );
    app.open_appearance();
    key(&mut app, KeyCode::Enter);
    assert!(!app.appearance.use_system_terminal_background);
    assert_eq!(std::fs::read_to_string(path).unwrap(), "invalid JSON");
    let mut terminal = Terminal::new(TestBackend::new(110, 24)).unwrap();
    assert!(
        text(&draw(&mut terminal, &mut app)).contains("Cannot load the saved appearance settings")
    );
}

#[test]
fn exported_previews_follow_the_background_preference() {
    let mut app = App::default();
    app.open_appearance();
    let default = snapshot::svg(&mut app, 80, 24);
    assert!(default.contains(&format!(
        "<rect width=\"100%\" height=\"100%\" fill=\"#{:06x}\"/>",
        coder_ui::coder_noir::TERMINAL_BACKGROUND
    )));
    key(&mut app, KeyCode::Enter);
    let transparent = snapshot::svg(&mut app, 80, 24);
    assert!(!transparent.contains("<rect"));
    assert!(transparent.contains("Coder terminal UI preview"));
    key(&mut app, KeyCode::Enter);
    assert_eq!(snapshot::svg(&mut app, 80, 24), default);
}
