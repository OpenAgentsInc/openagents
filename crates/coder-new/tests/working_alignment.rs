use coder_new::{App, Mode, ui};
use ratatui::{Terminal, backend::TestBackend};

/// The whole screen as rows of text.
fn rows(app: &mut App, width: u16, height: u16) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| ui::render(frame, app)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| (0..width).map(|x| buffer[(x, y)].symbol()).collect())
        .collect()
}

/// The label a rule carries, without its line and corners.
fn label(line: &str) -> &str {
    line.trim_matches(['─', '┌', '┐', '└', '┘']).trim()
}

#[test]
fn working_spinner_rides_the_inputs_top_left_rail() {
    for width in [24, 80] {
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app.live.runs_on = Some(coder_new::harness_plugin::Harness::CoderLoop);
        app.live.busy = true;
        let rows = rows(&mut app, width, 24);
        let top = rows
            .iter()
            .position(|line| line.starts_with('┌'))
            .expect("the input's top rail");
        let spinner = coder_new::tools::spinner(app.animation_frame);
        // The spinner is the top-left rail's own label, still paired with
        // `Working`, and the agent keeps the right end of the same rule
        // where there is room for both.
        let rail = label(&rows[top]);
        assert!(rail.starts_with(&format!("{spinner} Working")), "{rail:?}");
        if width >= 80 {
            assert!(rail.ends_with("Coder (auto)"), "{rail:?}");
        }
        // The transcript never repeats it, and no second spinner draws.
        let screen = rows.join("\n");
        assert_eq!(screen.matches(spinner).count(), 1, "{screen}");
        assert_eq!(screen.matches("Working").count(), 1, "{screen}");
    }
}

#[test]
fn an_idle_input_rail_carries_no_working_label() {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.live.runs_on = Some(coder_new::harness_plugin::Harness::CoderLoop);
    let rows = rows(&mut app, 80, 24);
    let top = rows
        .iter()
        .position(|line| line.starts_with('┌'))
        .expect("the input's top rail");
    // What Auto routed this chat to (#11524).
    assert_eq!(label(&rows[top]), "Auto · Coder (auto)");
    assert!(!rows.join("\n").contains("Working"));
}

#[test]
fn thinking_rides_the_rail_right_of_working_while_it_streams() {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.live.runs_on = Some(coder_new::harness_plugin::Harness::CoderLoop);
    app.live.busy = true;
    app.live.thinking = true;
    let shown = rows(&mut app, 80, 24);
    let top = shown
        .iter()
        .position(|line| line.starts_with('┌'))
        .expect("the input's top rail");
    let rail = label(&shown[top]);
    let working = rail.find("Working").expect("Working on the rail");
    let thinking = rail.find("Thinking").expect("Thinking on the rail");
    assert!(working < thinking, "{rail:?}");

    app.live.thinking = false;
    assert!(!rows(&mut app, 80, 24).join("\n").contains("Thinking"));
}
