use std::time::{Duration, Instant};

use winit::keyboard::{Key as Logical, KeyCode, ModifiersState, NamedKey, SmolStr};

use super::layout::{self, Axis, Direction, GAP, Layout, Rect};
use super::pty::Program;
use super::{KeyIn, Overlay};

const AREA: Rect = Rect {
    x: 0.0,
    y: 0.0,
    w: 802.0,
    h: 602.0,
};

#[test]
fn splitting_gives_the_new_pane_focus_and_half_the_area() {
    let mut layout = Layout::new(1);
    layout.split(Axis::Columns, 2);
    assert_eq!(layout.focus(), 2);
    assert_eq!(layout.panes(), vec![1, 2]);
    let rects = layout.rects(AREA);
    assert_eq!(rects[0].1, Rect::new(0.0, 0.0, 400.0, 602.0));
    assert_eq!(rects[1].1, Rect::new(400.0 + GAP, 0.0, 400.0, 602.0));
    layout.split(Axis::Rows, 3);
    let rects = layout.rects(AREA);
    assert_eq!(rects.len(), 3);
    assert_eq!(rects[1].1, Rect::new(402.0, 0.0, 400.0, 300.0));
    assert_eq!(rects[2].1, Rect::new(402.0, 302.0, 400.0, 300.0));
    // A pane cannot be added twice.
    layout.split(Axis::Rows, 1);
    assert_eq!(layout.panes().len(), 3);
}

#[test]
fn closing_a_pane_gives_its_area_to_its_sibling() {
    let mut layout = Layout::new(1);
    layout.split(Axis::Columns, 2);
    layout.split(Axis::Rows, 3);
    assert!(layout.close(3));
    assert_eq!(layout.panes(), vec![1, 2]);
    assert_eq!(layout.focus(), 2);
    assert_eq!(layout.rects(AREA)[1].1.h, 602.0);
    assert!(layout.close(1));
    assert_eq!(layout.rects(AREA), vec![(2, AREA)]);
    // The last pane stays.
    assert!(!layout.close(2));
    assert!(!layout.close(9));
}

#[test]
fn focus_moves_to_the_nearest_pane_in_a_direction() {
    let mut layout = Layout::new(1);
    layout.split(Axis::Columns, 2);
    layout.split(Axis::Rows, 3);
    // 1 is the left half; 2 is top right; 3 is bottom right, focused.
    assert!(layout.move_focus(Direction::Up, AREA));
    assert_eq!(layout.focus(), 2);
    assert!(!layout.move_focus(Direction::Up, AREA));
    assert!(layout.move_focus(Direction::Left, AREA));
    assert_eq!(layout.focus(), 1);
    assert!(!layout.move_focus(Direction::Left, AREA));
    assert!(layout.move_focus(Direction::Right, AREA));
    assert_eq!(layout.focus(), 2);
    assert!(layout.move_focus(Direction::Down, AREA));
    assert_eq!(layout.focus(), 3);
    assert_eq!(layout.pane_at(AREA, [10.0, 500.0]), Some(1));
    assert_eq!(layout.pane_at(AREA, [700.0, 500.0]), Some(3));
}

#[test]
fn grid_sizes_follow_the_cell_size() {
    assert_eq!(layout::cells(800.0, 600.0, 10.0, 20.0), (30, 80));
    assert_eq!(layout::cells(5.0, 5.0, 10.0, 20.0), (1, 1));
    let (a, b) = layout::divide(AREA, Axis::Rows, 0.95);
    assert_eq!(a.h, (600.0_f32 * 0.9).floor());
    assert_eq!(a.h + GAP + b.h, AREA.h);
}

#[test]
fn box_drawing_and_blocks_are_shapes() {
    assert_eq!(super::draw::box_lines('─'), Some([0, 0, 1, 1]));
    assert_eq!(super::draw::box_lines('┼'), Some([1, 1, 1, 1]));
    assert_eq!(super::draw::box_lines('┏'), Some([0, 2, 0, 2]));
    assert_eq!(super::draw::box_lines('╰'), Some([1, 0, 0, 1]));
    assert_eq!(super::draw::box_lines('a'), None);
    assert!(super::draw::block('█').is_some());
    assert_eq!(super::draw::block('▒').unwrap().1, 0.5);
}

fn key(code: KeyCode, logical: Logical, text: Option<&str>) -> KeyIn {
    KeyIn {
        code,
        logical,
        text: text.map(str::to_owned),
        pressed: true,
    }
}

fn char_key(code: KeyCode, c: &str) -> KeyIn {
    key(code, Logical::Character(SmolStr::new(c)), Some(c))
}

#[test]
fn a_focused_overlay_takes_every_key_and_a_hidden_one_none() {
    let mut overlay = Overlay::new();
    let w = char_key(KeyCode::KeyW, "w");
    assert!(!overlay.key(&w));
    overlay.toggle();
    assert!(overlay.open && overlay.focused);
    for code in [KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyD] {
        assert!(overlay.key(&char_key(code, "w")));
    }
    assert!(overlay.key(&key(
        KeyCode::Space,
        Logical::Named(NamedKey::Space),
        Some(" ")
    )));
    // Escape goes to the program, for vim.
    assert!(overlay.key(&key(
        KeyCode::Escape,
        Logical::Named(NamedKey::Escape),
        None
    )));
    assert!(overlay.focused);
    // Ctrl+` gives focus to the world, which then gets keys again.
    overlay.modifiers(ModifiersState::CONTROL);
    assert!(overlay.key(&char_key(KeyCode::Backquote, "`")));
    overlay.modifiers(ModifiersState::empty());
    assert!(!overlay.focused && overlay.open);
    assert!(!overlay.key(&w));
    // So does the prefix, then Escape.
    overlay.focused = true;
    overlay.modifiers(ModifiersState::CONTROL);
    assert!(overlay.key(&char_key(KeyCode::KeyB, "b")));
    overlay.modifiers(ModifiersState::empty());
    assert!(overlay.key(&key(
        KeyCode::Escape,
        Logical::Named(NamedKey::Escape),
        None
    )));
    assert!(!overlay.focused);
    overlay.toggle();
    assert!(!overlay.open);
}

#[test]
fn presses_inside_focus_and_outside_release() {
    let mut overlay = Overlay::new();
    assert!(!overlay.press([10.0, 10.0]));
    overlay.toggle();
    overlay.focused = false;
    let inside = [overlay.area.x + 20.0, overlay.area.y + 20.0];
    assert!(overlay.press(inside));
    assert!(overlay.focused);
    assert!(!overlay.press([overlay.area.x + overlay.area.w + 100.0, 2000.0]));
    assert!(!overlay.focused);
}

fn wait(overlay: &mut Overlay, done: impl Fn(&str) -> bool) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        overlay.tick();
        let text = overlay.focused_text().unwrap_or_default();
        if done(&text) || Instant::now() > deadline {
            return text;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(unix)]
#[test]
fn a_pane_runs_a_program_on_a_pty_and_shows_its_output() {
    let root = tempfile::tempdir().unwrap();
    let mut overlay = Overlay::with(
        root.path(),
        "/bin/sh".into(),
        Program::Command {
            program: "/bin/sh".into(),
            args: vec![
                "-c".into(),
                "printf 'cols=%s\\n' \"$(tput cols 2>/dev/null || stty size)\"; echo terminal-$((40+2)); exec cat".into(),
            ],
            label: "sh".into(),
        },
    );
    overlay.open = true;
    overlay.focused = true;
    overlay.ensure_started();
    assert_eq!(overlay.panes(), 1);
    let text = wait(&mut overlay, |t| t.contains("terminal-42"));
    assert!(text.contains("terminal-42"), "{text}");
    // Typing reaches the program: the line discipline echoes it and cat
    // prints it back.
    for c in ["h", "i"] {
        overlay.key(&char_key(KeyCode::KeyH, c));
    }
    overlay.key(&key(KeyCode::Enter, Logical::Named(NamedKey::Enter), None));
    let text = wait(&mut overlay, |t| t.matches("hi").count() >= 2);
    assert!(text.matches("hi").count() >= 2, "{text}");
    // A split opens a second pane with the shell, and both survive hiding.
    overlay.split(Axis::Columns, &Program::Shell);
    assert_eq!(overlay.panes(), 2);
    overlay.toggle();
    overlay.tick();
    assert_eq!(overlay.panes(), 2);
    // Closing the focused pane ends its program.
    overlay.close_focused();
    assert_eq!(overlay.panes(), 1);
    overlay.shutdown();
    assert_eq!(overlay.panes(), 0);
}
