use coder_new::{
    App, Mode,
    models::{self, GenerationOptions, Stage},
    plugin_store::Store,
    slash::Command,
    ui,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

fn key(app: &mut App, code: KeyCode) {
    assert!(app.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE))));
}

fn paste(app: &mut App, text: &str) {
    assert!(app.handle(Event::Paste(text.into())));
}

fn text(app: &mut App, width: u16, height: u16) -> (String, bool) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| ui::render(frame, app)).unwrap();
    let canvas = terminal.backend().buffer();
    let text = (0..height)
        .map(|y| {
            (0..width)
                .map(|x| canvas[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    (text, terminal.backend().cursor_visible())
}

#[test]
fn models_command_requires_an_enabled_provider_and_never_sends_a_chat() {
    let mut app = App::default();
    paste(&mut app, "/");
    assert!(!app.slash_hints().contains(&Command::Models));
    app.draft.text.clear();
    app.draft.cursor = 0;
    paste(&mut app, "/models");
    key(&mut app, KeyCode::Enter);
    assert!(app.model_picker.is_none());
    assert!(
        app.notice
            .as_deref()
            .unwrap()
            .contains("Turn on OpenRouter BYOK")
    );
    assert!(app.request.is_none());
    app.plugins.toggle_enabled();
    paste(&mut app, "/models");
    assert_eq!(app.slash_hints(), [Command::Models]);
    key(&mut app, KeyCode::Enter);
    assert!(app.model_picker.is_some());
    assert!(app.request.is_none());
    assert!(app.messages.is_empty());
    assert!(app.live.entries.is_empty());
}

#[test]
fn selection_is_staged_and_cancel_restores_chat_state() {
    let mut app = App::default();
    app.plugins.enabled = true;
    app.messages
        .push("A retained transcript line.\n".repeat(50));
    paste(&mut app, "unsent 🦀 draft");
    key(&mut app, KeyCode::Left);
    app.scroll = 6;
    let draft = (app.draft.text.clone(), app.draft.cursor);
    app.open_models();
    paste(&mut app, "gpt-6-luna");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.model_picker.as_ref().unwrap().stage, Stage::Reasoning);
    assert_eq!(
        app.model_picker
            .as_ref()
            .unwrap()
            .options
            .reasoning
            .as_deref(),
        Some("medium")
    );
    assert_eq!(app.plugins.model, models::DEFAULT_MODEL);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.model_picker.as_ref().unwrap().stage, Stage::Output);
    assert!(!text(&mut app, 110, 36).1);
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Esc);
    assert!(app.model_picker.is_none());
    assert_eq!((app.draft.text.clone(), app.draft.cursor), draft);
    assert_eq!(app.scroll, 6);
    assert_eq!(app.selected_agent, None);
    assert_eq!(app.plugins.model, models::DEFAULT_MODEL);
    assert_eq!(app.plugins.options, GenerationOptions::default());
}

#[test]
fn live_picker_settings_survive_restart_but_demo_choices_do_not_overwrite_them() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.load_plugin_settings(Store::under(directory.path()))
        .unwrap();
    app.plugins.toggle_enabled();
    app.open_models();
    paste(&mut app, "fable");
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Enter);
    for _ in 0..3 {
        key(&mut app, KeyCode::Down);
    }
    key(&mut app, KeyCode::Enter);
    assert!(app.model_picker.is_none());
    assert_eq!(app.plugins.model, "anthropic/claude-fable-5.1");
    assert_eq!(
        app.plugins.options,
        GenerationOptions {
            reasoning: Some("high".into()),
            max_tokens: Some(8192)
        }
    );
    assert!(app.request.is_none());
    app.set_mode(Mode::Demo);
    app.plugins.toggle_enabled();
    app.open_models();
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.model_picker.as_ref().unwrap().stage, Stage::Output);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.plugins.model, models::DEFAULT_MODEL);
    let mut restored = App::default();
    restored.set_mode(Mode::Live);
    restored
        .load_plugin_settings(Store::under(directory.path()))
        .unwrap();
    assert_eq!(restored.plugins.model, "anthropic/claude-fable-5.1");
    assert_eq!(
        restored.plugins.options,
        GenerationOptions {
            reasoning: Some("high".into()),
            max_tokens: Some(8192)
        }
    );
}

#[test]
fn picker_search_and_all_stages_render_at_small_and_large_terminal_sizes() {
    let mut app = App::default();
    app.plugins.enabled = true;
    app.open_models();
    for (width, height) in [(24, 12), (44, 16), (80, 24), (110, 36)] {
        let (canvas, cursor) = text(&mut app, width, height);
        assert!(canvas.contains("Pick model"));
        assert!(cursor);
    }
    paste(&mut app, "no model exists 🦀");
    assert!(text(&mut app, 110, 36).0.contains("No matching models"));
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.model_picker.as_ref().unwrap().stage, Stage::Models);
    app.model_picker.as_mut().unwrap().query = Default::default();
    paste(&mut app, "fable");
    key(&mut app, KeyCode::Enter);
    assert!(text(&mut app, 110, 36).0.contains("High"));
    key(&mut app, KeyCode::Enter);
    for (width, height) in [(24, 12), (80, 24), (110, 36)] {
        assert!(!text(&mut app, width, height).1);
    }
}

#[test]
fn plugin_qualified_identity_and_refresh_preserve_the_selection() {
    let mut catalog = models::openrouter_catalog();
    let mut alternative = catalog[1].clone();
    alternative.plugin = "another-provider".into();
    alternative.provider = "Another provider".into();
    catalog.insert(0, alternative);
    let mut picker = models::Picker::new(
        catalog.clone(),
        models::OPENROUTER_PLUGIN,
        "openai/gpt-6-luna",
        GenerationOptions::default(),
        false,
    );
    assert_eq!(
        picker.matching()[picker.selected].plugin,
        models::OPENROUTER_PLUGIN
    );
    catalog.reverse();
    picker.refresh(catalog, None);
    assert_eq!(
        picker.matching()[picker.selected].plugin,
        models::OPENROUTER_PLUGIN
    );
    assert_eq!(picker.matching()[picker.selected].id, "openai/gpt-6-luna");
    let mut app = App::default();
    assert!(
        !app.plugins.set_model(
            &picker
                .models
                .iter()
                .find(|model| model.plugin == "another-provider")
                .unwrap()
                .clone(),
            GenerationOptions::default()
        )
    );
    assert_eq!(app.plugins.model, models::DEFAULT_MODEL);
}

#[test]
fn choosing_free_router_clears_previous_reasoning_settings() {
    let mut app = App::default();
    app.plugins.enabled = true;
    app.plugins.model = "anthropic/claude-fable-5.1".into();
    app.plugins.options.reasoning = Some("high".into());
    app.open_models();
    paste(&mut app, "openrouter/free");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.model_picker.as_ref().unwrap().stage, Stage::Output);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.plugins.model, models::DEFAULT_MODEL);
    assert_eq!(app.plugins.options, GenerationOptions::default());
}

#[test]
fn metadata_refresh_reconciles_staged_capabilities_before_saving() {
    let mut app = App::default();
    app.plugins.enabled = true;
    app.open_models();
    paste(&mut app, "gpt-6-luna");
    key(&mut app, KeyCode::Enter);
    let mut catalog = models::openrouter_catalog();
    let model = catalog
        .iter_mut()
        .find(|model| model.id == "openai/gpt-6-luna")
        .unwrap();
    model.context_length = Some(128_000);
    model.max_output_tokens = Some(64_000);
    app.model_picker
        .as_mut()
        .unwrap()
        .refresh(catalog.clone(), None);
    assert_eq!(app.model_picker.as_ref().unwrap().stage, Stage::Reasoning);
    assert_eq!(
        app.model_picker
            .as_ref()
            .unwrap()
            .pending
            .as_ref()
            .unwrap()
            .context_length,
        Some(128_000)
    );
    key(&mut app, KeyCode::Enter);
    let model = catalog
        .iter_mut()
        .find(|model| model.id == "openai/gpt-6-luna")
        .unwrap();
    model.efforts.clear();
    model.supports_output_limit = false;
    app.model_picker.as_mut().unwrap().refresh(catalog, None);
    assert_eq!(app.model_picker.as_ref().unwrap().stage, Stage::Models);
    assert!(app.model_picker.as_ref().unwrap().pending.is_none());
    assert!(
        app.model_picker
            .as_ref()
            .unwrap()
            .error
            .as_deref()
            .unwrap()
            .contains("Select the model again")
    );
    assert_eq!(app.plugins.model, models::DEFAULT_MODEL);
    key(&mut app, KeyCode::Enter);
    assert!(app.model_picker.is_none());
    assert_eq!(app.plugins.model, "openai/gpt-6-luna");
    assert_eq!(app.plugins.options, GenerationOptions::default());
}
