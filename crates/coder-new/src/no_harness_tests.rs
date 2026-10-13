//! #11548: Auto with no harness runs the Coder loop on Gemini 4 Argon.

use coder_sdk::auto_policy::{ARGON, AutoPolicy, NoHarnessModel};

use crate::harness_plugin::{Exclusion, Facts, Harness, candidates};
use crate::{App, Mode};

fn live_app() -> App {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.cwd = Some(std::env::temp_dir());
    app
}

fn screen(app: &mut App) -> String {
    use ratatui::{Terminal, backend::TestBackend};
    let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
    terminal
        .draw(|frame| crate::ui::render(frame, app))
        .unwrap();
    let buffer = terminal.backend().buffer();
    (0..20)
        .map(|y| {
            (0..100)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn forced() -> AutoPolicy {
    AutoPolicy {
        no_harness: NoHarnessModel::default(),
        force_no_harness: true,
    }
}

#[test]
fn with_no_harness_every_call_runs_on_argon() {
    crate::harness_plugin::sign_in_claude(false);
    crate::no_harness::set_test(Some(AutoPolicy::default()), Some("fixture-gemini-key"));
    let app = live_app();
    let turn = app.no_harness_turn().expect("no harness can run here");
    assert_eq!(turn.model(), ARGON);
    assert!(turn.gemini_key.is_some());
    assert_eq!(
        crate::routing_record::no_harness_choice(&turn).model,
        "google/gemini-4-argon"
    );
    // A model the person chose keeps its own path.
    let mut chosen = live_app();
    chosen.live.pinned_model = Some("google/gemini-3.8-flash".into());
    assert!(chosen.no_harness_turn().is_none());
    crate::no_harness::set_test(None, None);
}

#[test]
fn a_usable_harness_keeps_auto_on_the_harnesses_unless_forced() {
    crate::harness_plugin::sign_in_claude(true);
    crate::no_harness::set_test(Some(AutoPolicy::default()), None);
    let mut app = live_app();
    app.plugins.bundled.claude = true;
    assert!(
        candidates(&Facts::read(&app))
            .admitted
            .contains(&Harness::ClaudeCode)
    );
    assert!(app.no_harness_turn().is_none());
    // Forced: Claude Code counts as off for the run, Shift+Tab offers only
    // Auto, and the turn runs on Argon; the person's switch is untouched.
    crate::no_harness::set_test(Some(forced()), None);
    let found = candidates(&Facts::read(&app));
    assert_eq!(found.admitted, [Harness::CoderLoop]);
    assert!(
        found
            .excluded
            .contains(&(Harness::ClaudeCode, Exclusion::PluginOff))
    );
    assert_eq!(
        crate::harness_pick::local(&app),
        crate::harness_pick::Local::default()
    );
    assert!(app.plugins.bundled.claude);
    assert!(!app.plugins.execution_settings(std::env::temp_dir()).claude);
    assert_eq!(
        app.no_harness_turn().map(|turn| turn.model().to_owned()),
        Some(ARGON.into())
    );
    crate::harness_plugin::sign_in_claude(false);
    crate::no_harness::set_test(None, None);
}

#[test]
fn the_rail_names_argon_and_its_million_token_window() {
    crate::no_harness::set_test(Some(forced()), None);
    let mut app = live_app();
    app.live.runs_on = Some(Harness::CoderLoop);
    app.no_harness_model = Some(ARGON.into());
    app.live
        .entries
        .push(crate::live::Entry::User("Fix the bug".into()));
    let shown = screen(&mut app);
    assert!(shown.contains("Auto → Coder (Gemini 4 Argon)"), "{shown}");
    // Its context figure is over Argon's 1,048,576 tokens, as the Gemini
    // API names the model.
    app.live.round(crate::bundled_runtime::RoundUsage {
        model: "gemini-4-argon".into(),
        prompt_tokens: 104_858,
        ..Default::default()
    });
    assert_eq!(
        crate::long_session::context_line(&app).as_deref(),
        Some("Context 10%")
    );
    crate::no_harness::set_test(None, None);
}
