//! No network, owner home, or engine. The fixture models host-owned attachments.
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
use terminal_core::{
    bridge,
    pty::{Attachment, Event as Output, Program, Sessions, Transport},
};
use terminal_mux::Mux;
#[derive(Default)]
struct State {
    input: Mutex<Vec<Vec<u8>>>,
    events: Mutex<VecDeque<Output>>,
    available: AtomicBool,
    closes: Mutex<usize>,
}
struct Fake(Arc<State>);
impl Attachment for Fake {
    fn input_available(&self) -> bool {
        self.0.available.load(Ordering::SeqCst)
    }
    fn host_grid(&self) -> bool {
        true
    }
    fn host_answers(&self) -> bool {
        true
    }
    fn input(&self, b: &[u8]) {
        self.0.input.lock().unwrap().push(b.to_vec());
    }
    fn resize(&self, _: u16, _: u16) {}
    fn close(&self) {
        *self.0.closes.lock().unwrap() += 1;
    }
    fn poll(&mut self) -> Option<Output> {
        self.0.events.lock().unwrap().pop_front()
    }
    fn target(&self) -> Option<terminal_core::proposals::Binding> {
        None
    }
    fn directory(&self) -> Option<String> {
        None
    }
}
impl Transport for Fake {
    fn shell(&self) -> &Path {
        Path::new("scratch")
    }
    fn open(&self, _: &Program, _: u16, _: u16) -> Result<Box<dyn Attachment>, String> {
        Ok(Box::new(Fake(self.0.clone())))
    }
    fn shutdown(&self) {}
    fn thread_program(&self) -> Option<Program> {
        None
    }
    fn resolve(&self, _: &str) -> Option<PathBuf> {
        None
    }
    fn request(&self, _: &bridge::Request) -> Result<bridge::Connection, String> {
        Err("Unavailable resource".into())
    }
    fn git_summary(&self, _: u64, _: String) -> mpsc::Receiver<(u64, String, String)> {
        mpsc::channel().1
    }
    fn open_link(&self, _: &str) -> Result<(), String> {
        Err("Text only".into())
    }
    fn clipboard(&self) -> Option<String> {
        None
    }
    fn copy(&self, _: &str) -> Result<(), String> {
        Err("Text only".into())
    }
}
fn fixture(n: usize) -> (Mux, Vec<Arc<State>>) {
    let states = (0..n)
        .map(|_| {
            let s = Arc::new(State::default());
            s.available.store(true, Ordering::SeqCst);
            s
        })
        .collect::<Vec<_>>();
    let mux = Mux::attach(
        states
            .iter()
            .map(|s| Sessions(Arc::new(Fake(s.clone()))))
            .collect(),
    )
    .unwrap();
    (mux, states)
}
fn key(m: &mut Mux, c: KeyCode, mods: KeyModifiers) {
    m.event(Event::Key(KeyEvent::new(c, mods)), Rect::new(0, 0, 80, 24));
}
fn prefix(m: &mut Mux, c: char) {
    key(m, KeyCode::Char('b'), KeyModifiers::CONTROL);
    key(m, KeyCode::Char(c), KeyModifiers::NONE);
}
#[test]
fn escaped_prefix_tabs_splits_and_detach_never_close() {
    let (mut m, s) = fixture(2);
    key(&mut m, KeyCode::Char('b'), KeyModifiers::CONTROL);
    key(&mut m, KeyCode::Char('b'), KeyModifiers::CONTROL);
    assert_eq!(*s[0].input.lock().unwrap(), vec![vec![2]]);
    prefix(&mut m, 'n');
    key(&mut m, KeyCode::Char('x'), KeyModifiers::NONE);
    assert_eq!(*s[1].input.lock().unwrap(), vec![b"x".to_vec()]);
    prefix(&mut m, '%');
    assert!(m.split.is_some());
    prefix(&mut m, 'd');
    assert!(m.detached);
    drop(m);
    assert_eq!(*s[0].closes.lock().unwrap(), 0);
    assert_eq!(*s[1].closes.lock().unwrap(), 0);
}
#[test]
fn lost_typist_input_is_discarded_and_host_queries_are_not_answered_twice() {
    let (mut m, s) = fixture(1);
    s[0].events
        .lock()
        .unwrap()
        .push_back(Output::Output(b"\x1b[6n".to_vec()));
    m.pump();
    assert!(s[0].input.lock().unwrap().is_empty());
    s[0].available.store(false, Ordering::SeqCst);
    key(&mut m, KeyCode::Char('x'), KeyModifiers::NONE);
    m.event(Event::Paste("bad\n".into()), Rect::default());
    assert!(m.notice.contains("never queued"));
    s[0].available.store(true, Ordering::SeqCst);
    m.pump();
    assert!(s[0].input.lock().unwrap().is_empty());
    s[0].events
        .lock()
        .unwrap()
        .push_back(Output::End("Host restarted: original terminal lost".into()));
    m.pump();
    key(&mut m, KeyCode::Enter, KeyModifiers::NONE);
    assert!(s[0].input.lock().unwrap().is_empty());
}
#[test]
fn paste_and_application_keys_preserve_modes() {
    let (mut m, s) = fixture(1);
    s[0].events
        .lock()
        .unwrap()
        .push_back(Output::Output(b"\x1b[?2004h\x1b[?1h".to_vec()));
    m.pump();
    m.event(Event::Paste("a\nb".into()), Rect::default());
    key(&mut m, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(
        *s[0].input.lock().unwrap(),
        vec![b"\x1b[200~a\rb\x1b[201~".to_vec(), b"\x1bOA".to_vec()]
    );
}
#[test]
fn redraw_is_cropped_and_output_budget_yields() {
    let (mut m, s) = fixture(2);
    for _ in 0..100 {
        s[0].events
            .lock()
            .unwrap()
            .push_back(Output::Output(vec![b'x'; 4096]));
    }
    let used = m.pump();
    assert!(used <= terminal_mux::FRAME_BYTES as u64);
    assert!(!s[0].events.lock().unwrap().is_empty());
    let mut terminal = Terminal::new(TestBackend::new(16, 8)).unwrap();
    prefix(&mut m, '%');
    terminal.draw(|f| m.draw(f)).unwrap();
    assert_eq!(terminal.backend().buffer().content.len(), 128);
}
#[test]
fn mouse_positions_are_relative_to_the_inner_pane() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let (mut m, s) = fixture(1);
    s[0].events
        .lock()
        .unwrap()
        .push_back(Output::Output(b"\x1b[?1000h\x1b[?1006h".to_vec()));
    m.pump();
    m.event(
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 3,
            row: 4,
            modifiers: KeyModifiers::NONE,
        }),
        Rect::new(0, 0, 80, 24),
    );
    assert_eq!(*s[0].input.lock().unwrap(), vec![b"\x1b[<0;3;3M".to_vec()]);
}
#[test]
fn guard_restores_modes_on_unwind_once() {
    use coder_terminal::guard::{Console, Guard, Step};
    struct ConsoleFixture(Arc<Mutex<Vec<(bool, Step)>>>);
    impl Console for ConsoleFixture {
        fn apply(&mut self, s: Step) -> std::io::Result<()> {
            self.0.lock().unwrap().push((true, s));
            Ok(())
        }
        fn undo(&mut self, s: Step) -> std::io::Result<()> {
            self.0.lock().unwrap().push((false, s));
            Ok(())
        }
    }
    let events = Arc::new(Mutex::new(Vec::new()));
    let cloned = events.clone();
    let _ = std::panic::catch_unwind(move || {
        let _guard = Guard::enter(
            ConsoleFixture(cloned),
            &[
                Step::RawMode,
                Step::AlternateScreen,
                Step::MouseCapture,
                Step::BracketedPaste,
            ],
        )
        .unwrap();
        panic!("scratch unwind")
    });
    assert_eq!(
        events
            .lock()
            .unwrap()
            .iter()
            .filter(|(apply, _)| !*apply)
            .count(),
        4
    );
    assert_eq!(
        events.lock().unwrap().last().unwrap(),
        &(false, Step::RawMode)
    );
}

#[test]
fn command_navigation_uses_the_current_host_epoch() {
    use coder_pty::ext::{Block, BlockPage, BlockState, Lines, Origin};
    let (mut mux, states) = fixture(1);
    states[0].events.lock().unwrap().push_back(Output::Output(
        (0..60)
            .map(|i| format!("line-{i:02}\r\n"))
            .collect::<String>()
            .into_bytes(),
    ));
    mux.pump();
    let epoch = mux.panes[0].session.vt.line_epoch();
    let block = |epoch| Block {
        block: 1,
        origin: Origin::Typed,
        command: "retained command".into(),
        command_truncated: false,
        dir: "/scratch".into(),
        started: Some(1),
        ended: Some(2),
        status: Some(0),
        state: BlockState::Finished,
        alternate: false,
        output: None,
        retained: false,
        lines: Some(Lines {
            epoch,
            start: 5,
            end: 10,
        }),
    };
    states[0]
        .events
        .lock()
        .unwrap()
        .push_back(Output::Blocks(BlockPage {
            newest: Some(1),
            oldest: Some(1),
            blocks: vec![block(epoch)],
            more: false,
        }));
    mux.pump();
    prefix(&mut mux, 'k');
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|frame| mux.draw(frame)).unwrap();
    let line = (1..20)
        .map(|x| terminal.backend().buffer()[(x, 2)].symbol())
        .collect::<String>();
    assert!(line.contains("line-05"), "{line}");
    prefix(&mut mux, 'r');
    // Stale epoch anchors are discarded by the shared projection.
    states[0]
        .events
        .lock()
        .unwrap()
        .push_back(Output::Blocks(BlockPage {
            newest: Some(1),
            oldest: Some(1),
            blocks: vec![block(epoch + 1)],
            more: false,
        }));
    mux.pump();
    assert!(mux.panes[0].session.blocks.records.is_empty());
}
