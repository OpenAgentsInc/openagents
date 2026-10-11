use coder_new::{App, Mode, ui};
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn working_spinner_starts_at_the_left_edge() {
    for width in [24, 80] {
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app.live.busy = true;
        let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
        terminal.draw(|frame| ui::render(frame, &mut app)).unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(
            buffer[(0, 0)].symbol(),
            coder_new::tools::spinner(app.animation_frame)
        );
        assert_eq!(buffer[(1, 0)].symbol(), " ");
        assert_eq!(buffer[(2, 0)].symbol(), "W");
    }
}
