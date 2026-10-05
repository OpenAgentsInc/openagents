use std::time::{Duration, Instant};

use winit::keyboard::{Key as Logical, KeyCode, ModifiersState, NamedKey, SmolStr};

use super::layout::{self, Axis, Direction, GAP, Layout, Rect};
use super::pty::Program;
use super::select::{self, Point, Selection, Unit};
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
        plain: text.map(str::to_owned),
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

#[cfg(unix)]
#[test]
fn control_requests_open_split_type_and_read_panes() {
    use super::control::{self, Request, call};
    let root = tempfile::tempdir().unwrap();
    let socket = root.path().join("terminal.sock");
    let mut overlay = Overlay::with(
        root.path(),
        "/bin/sh".into(),
        Program::Command {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "echo first-pane; exec cat".into()],
            label: "first".into(),
        },
    );
    overlay.listen(&socket).unwrap();
    // Hidden, the overlay still answers.
    let status = overlay.apply(&Request::Status).unwrap();
    assert_eq!(status["open"], false);
    assert_eq!(status["panes"].as_array().unwrap().len(), 0);
    // Open starts the first pane with focus; the world is told once.
    let opened = overlay.apply(&Request::Open).unwrap();
    assert_eq!(opened["focused"], true);
    assert_eq!(overlay.focus_changed(), Some(true));
    assert_eq!(overlay.focus_changed(), None);
    let text = wait(&mut overlay, |t| t.contains("first-pane"));
    assert!(text.contains("first-pane"), "{text}");
    // A split runs a command line found on PATH or by absolute path.
    let split = overlay
        .apply(&Request::Split {
            axis: "cols".into(),
            program: vec![
                "/bin/sh".into(),
                "-c".into(),
                "echo split-pane; exec cat".into(),
            ],
        })
        .unwrap();
    assert_eq!(split["panes"].as_array().unwrap().len(), 2);
    let focused: Vec<u64> = split["panes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["focused"] == true)
        .map(|p| p["id"].as_u64().unwrap())
        .collect();
    assert_eq!(focused, vec![2]);
    let text = wait(&mut overlay, |t| t.contains("split-pane"));
    assert!(text.contains("split-pane"), "{text}");
    // Typing and a named key reach the focused pane's program.
    overlay
        .apply(&Request::Send {
            text: "typed-$((40+2))".into(),
        })
        .unwrap();
    overlay
        .apply(&Request::Key {
            name: "enter".into(),
        })
        .unwrap();
    let text = wait(&mut overlay, |t| t.matches("typed-").count() >= 2);
    assert!(text.matches("typed-").count() >= 2, "{text}");
    // Read names a pane; focus moves by direction and by id.
    let read = overlay.apply(&Request::Read { pane: Some(1) }).unwrap();
    assert!(read["text"].as_str().unwrap().contains("first-pane"));
    assert!(!read["text"].as_str().unwrap().contains("typed-"));
    overlay
        .apply(&Request::Focus {
            direction: Some("left".into()),
            pane: None,
        })
        .unwrap();
    assert!(overlay.focused_text().unwrap().contains("first-pane"));
    overlay
        .apply(&Request::Focus {
            direction: None,
            pane: Some(2),
        })
        .unwrap();
    assert!(overlay.focused_text().unwrap().contains("split-pane"));
    assert!(
        overlay
            .apply(&Request::Focus {
                direction: None,
                pane: Some(9)
            })
            .is_err()
    );
    assert!(
        overlay
            .apply(&Request::Key {
                name: "hyper-q".into()
            })
            .is_err()
    );
    assert!(
        overlay
            .apply(&Request::Split {
                axis: "cols".into(),
                program: vec!["no-such-program-verse".into()],
            })
            .is_err()
    );
    // Over the socket: a request from another thread is served by tick.
    let path = socket.clone();
    let client =
        std::thread::spawn(move || call(&path, &serde_json::json!({ "op": "read", "pane": 2 })));
    let reply = loop {
        overlay.tick();
        if client.is_finished() {
            break client.join().unwrap().unwrap();
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(reply["ok"], true);
    assert!(
        reply["text"].as_str().unwrap().contains("split-pane"),
        "{reply}"
    );
    let path = socket.clone();
    let client = std::thread::spawn(move || call(&path, &serde_json::json!({ "op": "zoom" })));
    let reply = loop {
        overlay.tick();
        if client.is_finished() {
            break client.join().unwrap().unwrap();
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(reply["tabs"][0]["zoomed"], true);
    // Hide keeps the panes; close ends the focused one.
    let hidden = overlay.apply(&Request::Hide).unwrap();
    assert_eq!(hidden["open"], false);
    assert_eq!(hidden["panes"].as_array().unwrap().len(), 2);
    overlay.apply(&Request::Close).unwrap();
    assert_eq!(overlay.panes(), 1);
    overlay.shutdown();
    drop(overlay);
    assert!(!socket.exists());

    let older = control::Listener::bind(&socket).unwrap();
    assert!(
        control::Listener::bind(&socket).is_err(),
        "a live socket stays"
    );
    std::fs::remove_file(&socket).unwrap();
    let newer = control::Listener::bind(&socket).unwrap();
    drop(older);
    assert!(
        socket.exists(),
        "an older listener must not remove a newer one's socket"
    );
    drop(newer);
    assert!(!socket.exists());
}

fn vt(rows: usize, cols: usize, bytes: &[u8]) -> coder_vt::Terminal {
    let mut vt = coder_vt::Terminal::new(rows, cols, 100);
    vt.feed(bytes);
    vt
}

fn point(vt: &coder_vt::Terminal, index: usize, col: usize) -> Point {
    Point {
        line: select::absolute(vt, index),
        col,
    }
}

#[test]
fn selected_text_keeps_wide_and_combining_characters_and_joins_wrapped_lines() {
    // A 6-column grid: "ab世界cd" wraps after the wide characters.
    let vt = vt(4, 6, "ab世界cde\u{301}\r\nnext  \r\nlast".as_bytes());
    assert_eq!(vt.line(0).unwrap().text(), "ab世界");
    assert!(vt.line(0).unwrap().wrapped);
    let all = Selection {
        anchor: point(&vt, 0, 0),
        head: point(&vt, 3, 5),
        unit: Unit::Char,
    };
    // The wrapped line joins; the next line ends with a newline and loses
    // its trailing blanks.
    assert_eq!(all.text(&vt), "ab世界cde\u{301}\nnext\nlast");
    // Starting on a wide character's right half takes the whole character.
    let half = Selection {
        anchor: point(&vt, 0, 3),
        head: point(&vt, 0, 4),
        unit: Unit::Char,
    };
    assert_eq!(half.text(&vt), "世界");
    // Backward drags read the same.
    let back = Selection {
        anchor: point(&vt, 1, 1),
        head: point(&vt, 0, 4),
        unit: Unit::Char,
    };
    assert_eq!(back.text(&vt), "界cd");
}

#[test]
fn words_and_lines_grow_from_a_click() {
    let vt = vt(3, 30, b"cargo test -p coder-vt\r\nok");
    let word = Selection::at(point(&vt, 0, 16), Unit::Word);
    assert_eq!(word.text(&vt), "coder-vt");
    let flag = Selection::at(point(&vt, 0, 11), Unit::Word);
    assert_eq!(flag.text(&vt), "-p");
    let line = Selection::at(point(&vt, 0, 3), Unit::Line);
    assert_eq!(line.text(&vt), "cargo test -p coder-vt");
    // A triple-click takes the whole logical line, wrapped rows included.
    let narrow = super::tests::vt(3, 8, b"0123456789abc\r\nz");
    let line = Selection::at(point(&narrow, 1, 2), Unit::Line);
    assert_eq!(line.text(&narrow), "0123456789abc");
    assert_eq!(
        line.columns(&narrow, select::absolute(&narrow, 0), 8),
        Some((0, 8))
    );
}

#[test]
fn a_selection_follows_its_text_into_the_scrollback() {
    let mut vt = coder_vt::Terminal::new(3, 10, 4);
    vt.feed(b"one\r\ntwo\r\n");
    let two = Selection {
        anchor: point(&vt, 1, 0),
        head: point(&vt, 1, 2),
        unit: Unit::Char,
    };
    for i in 0..5 {
        vt.feed(format!("more{i}\r\n").as_bytes());
    }
    // "two" scrolled into the scrollback, and two lines left its front.
    assert_eq!(vt.history_dropped(), 1);
    assert_eq!(two.text(&vt), "two");
    vt.feed(b"x\r\ny\r\nz\r\n");
    // Once its line is dropped, the selection reads nothing.
    assert_eq!(two.text(&vt), "");
}

#[test]
fn search_finds_matches_toward_older_and_newer_lines() {
    let vt = vt(5, 20, b"error one\r\nfine\r\nan Error two\r\nfine\r\n");
    let from = point(&vt, 4, 0);
    let (start, end) = select::search(&vt, "error", from, true).unwrap();
    assert_eq!((start, end), (point(&vt, 2, 3), point(&vt, 2, 7)));
    let (older, _) = select::search(&vt, "error", start, true).unwrap();
    assert_eq!(older, point(&vt, 0, 0));
    assert!(select::search(&vt, "error", older, true).is_none());
    // Uppercase in the query matches case.
    assert_eq!(
        select::search(&vt, "Error", from, true).unwrap().0,
        point(&vt, 2, 3)
    );
    let (newer, _) = select::search(&vt, "fine", point(&vt, 0, 0), false).unwrap();
    assert_eq!(newer, point(&vt, 1, 0));
    // A wide character's match ends on its right half.
    let wide = super::tests::vt(2, 10, "a世b".as_bytes());
    let (s, e) = select::search(&wide, "世", point(&wide, 1, 0), true).unwrap();
    assert_eq!((s.col, e.col), (1, 2));
}

fn modifiers(ctrl: bool, alt: bool, shift: bool) -> coder_vt::Modifiers {
    coder_vt::Modifiers { ctrl, alt, shift }
}

#[test]
fn option_sends_meta_on_macos_when_set() {
    let vt = vt(2, 10, b"");
    // Option+F composes "ƒ"; as Meta it sends Escape and "f".
    let mut key = char_key(KeyCode::KeyF, "ƒ");
    key.plain = Some("f".into());
    let alt = modifiers(false, true, false);
    assert_eq!(
        super::keys::encode(&key, alt, &vt, true, true).unwrap(),
        b"\x1bf"
    );
    assert_eq!(
        super::keys::encode(&key, alt, &vt, false, true).unwrap(),
        "ƒ".as_bytes()
    );
    // Shift with Option sends the shifted character.
    let shifted = modifiers(false, true, true);
    assert_eq!(
        super::keys::encode(&key, shifted, &vt, true, true).unwrap(),
        b"\x1bF"
    );
    // Without the platform's unmodified key, the physical key answers.
    key.plain = None;
    assert_eq!(
        super::keys::encode(&key, alt, &vt, true, true).unwrap(),
        b"\x1bf"
    );
    // Off macOS, Alt is always Meta.
    let mut key = char_key(KeyCode::KeyB, "b");
    key.plain = None;
    assert_eq!(
        super::keys::encode(&key, alt, &vt, false, false).unwrap(),
        b"\x1bb"
    );
    // Ctrl+Option+B: Escape and Ctrl+B.
    let ctrl_alt = modifiers(true, true, false);
    key.text = Some("∫".into());
    key.logical = Logical::Character(SmolStr::new("∫"));
    assert_eq!(
        super::keys::encode(&key, ctrl_alt, &vt, true, true).unwrap(),
        b"\x1b\x02"
    );
}

#[test]
fn named_keypad_and_function_keys_encode_as_xterm() {
    let mut vt = vt(2, 10, b"");
    let none = modifiers(false, false, false);
    let named = |n: NamedKey| key(KeyCode::F1, Logical::Named(n), None);
    let enc =
        |vt: &coder_vt::Terminal, k: &KeyIn, m| super::keys::encode(k, m, vt, true, true).unwrap();
    assert_eq!(enc(&vt, &named(NamedKey::F12), none), b"\x1b[24~");
    assert_eq!(enc(&vt, &named(NamedKey::F20), none), b"\x1b[19;2~");
    assert_eq!(enc(&vt, &named(NamedKey::Home), none), b"\x1b[H");
    assert_eq!(enc(&vt, &named(NamedKey::PageDown), none), b"\x1b[6~");
    assert_eq!(
        enc(
            &vt,
            &named(NamedKey::ArrowLeft),
            modifiers(true, false, false)
        ),
        b"\x1b[1;5D"
    );
    assert_eq!(
        enc(
            &vt,
            &named(NamedKey::ArrowRight),
            modifiers(false, true, false)
        ),
        b"\x1b[1;3C"
    );
    let pad = key(
        KeyCode::Numpad7,
        Logical::Character(SmolStr::new("7")),
        Some("7"),
    );
    assert_eq!(enc(&vt, &pad, none), b"7");
    vt.feed(b"\x1b=");
    assert_eq!(enc(&vt, &pad, none), b"\x1bOw");
    let enter = key(KeyCode::NumpadEnter, Logical::Named(NamedKey::Enter), None);
    assert_eq!(enc(&vt, &enter, none), b"\x1bOM");
}

#[test]
fn fallback_glyphs_fill_the_atlas_and_shapes_need_none() {
    let mut atlas = crate::ui::Atlas::new(16.0);
    atlas.reserve_glyphs(256).unwrap();
    let mut fallback = super::glyphs::Fallback::new();
    // Fira Mono has these beyond the prebuilt set.
    for c in ['λ', 'Ж', '→', '≠'] {
        assert!(!atlas.has_glyph(c) || c == '→', "{c} was prebuilt");
        assert!(fallback.ensure(&mut atlas, c), "{c} not rasterized");
        assert!(atlas.has_glyph(c));
    }
    // Combining marks too.
    assert!(fallback.ensure(&mut atlas, '\u{301}'));
    let mark = atlas.glyph_box('\u{301}').unwrap();
    assert!(mark.size[0] > 0.0, "{mark:?}");
    let accented = vt(1, 4, "e\u{301}".as_bytes());
    assert_eq!(accented.row(0).unwrap().cells[0].combining, ['\u{301}']);
    let mut batch = crate::ui::UiBatch::default();
    super::draw::grid(
        &mut batch,
        &atlas,
        [0.0, 0.0],
        &super::draw::Grid {
            rows: vec![accented.row(0).unwrap()],
        },
    );
    assert_eq!(batch.vertices.len(), 12, "the base and its mark");
    // Box drawing, blocks, braille, and powerline separators are shapes.
    for c in ['─', '█', '⣿', '⡀', '\u{e0b0}', '\u{e0b3}'] {
        assert!(super::draw::shaped(c), "{c} is not a shape");
    }
    assert_eq!(super::draw::braille('⣿').unwrap().len(), 8);
    assert_eq!(super::draw::braille('⡀').unwrap(), vec![(0, 3)]);
    // A private-use character no font has is remembered as missing.
    assert!(!fallback.ensure(&mut atlas, '\u{f8ff}') || atlas.has_glyph('\u{f8ff}'));
    // This computer's fonts cover CJK, symbols, and emoji.
    if cfg!(target_os = "macos") {
        for c in ['世', '界', '✓', '😀', '⎇'] {
            assert!(fallback.ensure(&mut atlas, c), "{c} not found");
        }
        let wide = atlas.glyph_box('世').unwrap();
        assert!(wide.advance > atlas.advance * 1.5);
    }
}

#[test]
fn shapes_draw_without_question_marks() {
    let atlas = crate::ui::Atlas::new(16.0);
    let row = vt(1, 6, "⣿\u{e0b0}?".as_bytes());
    let mut batch = crate::ui::UiBatch::default();
    super::draw::grid(
        &mut batch,
        &atlas,
        [0.0, 0.0],
        &super::draw::Grid {
            rows: vec![row.row(0).unwrap()],
        },
    );
    // Eight dots, one triangle, and the one real '?'.
    assert_eq!(batch.vertices.len(), 8 * 6 + 3 + 6);
    // A wide character the atlas lacks still takes two columns, and a
    // missing one shows '?'.
    let mut batch = crate::ui::UiBatch::default();
    let wide = vt(1, 6, "世".as_bytes());
    super::draw::grid(
        &mut batch,
        &atlas,
        [0.0, 0.0],
        &super::draw::Grid {
            rows: vec![wide.row(0).unwrap()],
        },
    );
    assert_eq!(batch.vertices.len(), 6);
}

#[test]
fn the_buttons_card_never_covers_the_open_overlay() {
    let size = [1280.0, 800.0];
    let overlay = Rect::new(45.0, 40.0, 1190.0, 616.0);
    let button = Rect::new(900.0, 690.0, 44.0, 44.0);
    let card = [560.0, 90.0];
    let closed = Overlay::card_for(size, button, card, None);
    assert!(closed.y + closed.h <= button.y);
    let open = Overlay::card_for(size, button, card, Some(overlay));
    assert!(open.y >= overlay.y + overlay.h, "{open:?}");
    assert!(open.x >= button.x + button.w || open.x + open.w <= button.x);
    assert!(open.x >= 0.0 && open.x + open.w <= size[0]);
    // With no room on the right, the card goes left of the button.
    let corner = Rect::new(1220.0, 690.0, 44.0, 44.0);
    let left = Overlay::card_for(size, corner, card, Some(overlay));
    assert!(left.x + left.w <= corner.x);
}

#[test]
fn only_web_mail_and_file_links_open() {
    use super::mouse::link_allowed;
    assert!(link_allowed("https://openagents.com/docs"));
    assert!(link_allowed("mailto:someone@example.com"));
    assert!(link_allowed("file:///tmp/notes.txt"));
    assert!(!link_allowed("javascript:alert(1)"));
    assert!(!link_allowed("ssh://host"));
    assert!(!link_allowed("https://x\n"));
}

fn sh(script: &str) -> Program {
    Program::Command {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        label: "sh".into(),
    }
}

/// The point at the middle of cell (row, col) of the overlay's only pane.
fn cell_point(overlay: &Overlay, row: usize, col: usize) -> [f32; 2] {
    let inner = super::draw::inner(overlay.area, overlay.cell);
    [
        inner.x + (col as f32 + 0.5) * overlay.cell[0],
        inner.y + (row as f32 + 0.5) * overlay.cell[1],
    ]
}

#[cfg(unix)]
#[test]
fn a_drag_selects_and_copy_takes_the_selection() {
    let root = tempfile::tempdir().unwrap();
    let mut overlay = Overlay::with(
        root.path(),
        "/bin/sh".into(),
        sh("printf 'alpha beta gamma\\n'; exec cat"),
    );
    overlay.open = true;
    overlay.focused = true;
    overlay.ensure_started();
    let text = wait(&mut overlay, |t| t.contains("gamma"));
    assert!(text.contains("alpha beta gamma"), "{text}");
    // A drag from "beta" to "gam".
    assert!(overlay.press(cell_point(&overlay, 0, 6)));
    overlay.pointer(cell_point(&overlay, 0, 12));
    assert!(overlay.release(cell_point(&overlay, 0, 12)));
    assert_eq!(overlay.selected_text().as_deref(), Some("beta ga"));
    overlay.modifiers(ModifiersState::SUPER | ModifiersState::CONTROL | ModifiersState::SHIFT);
    // Cmd+C on macOS, Ctrl+Shift+C elsewhere.
    if cfg!(target_os = "macos") {
        overlay.modifiers(ModifiersState::SUPER);
    } else {
        overlay.modifiers(ModifiersState::CONTROL | ModifiersState::SHIFT);
    }
    assert!(overlay.key(&char_key(KeyCode::KeyC, "c")));
    overlay.modifiers(ModifiersState::empty());
    assert_eq!(overlay.copied.as_deref(), Some("beta ga"));
    // A double-click takes a word, a triple-click the line.
    overlay.press(cell_point(&overlay, 0, 13));
    overlay.release(cell_point(&overlay, 0, 13));
    overlay.press(cell_point(&overlay, 0, 13));
    overlay.release(cell_point(&overlay, 0, 13));
    assert_eq!(overlay.selected_text().as_deref(), Some("gamma"));
    overlay.press(cell_point(&overlay, 0, 13));
    overlay.release(cell_point(&overlay, 0, 13));
    assert_eq!(overlay.selected_text().as_deref(), Some("alpha beta gamma"));
    // A single click clears it.
    std::thread::sleep(Duration::from_millis(450));
    overlay.press(cell_point(&overlay, 0, 2));
    overlay.release(cell_point(&overlay, 0, 2));
    assert_eq!(overlay.selected_text(), None);
    overlay.shutdown();
}

#[cfg(unix)]
#[test]
fn programs_that_ask_get_mouse_reports_focus_events_and_clipboard_writes() {
    let root = tempfile::tempdir().unwrap();
    // The terminal's own echo shows what reaches the program: Escape as ^[.
    let mut overlay = Overlay::with(
        root.path(),
        "/bin/sh".into(),
        sh(
            "printf '\\033[?1000h\\033[?1006h\\033[?1004h\\033]52;c;Y29waWVk\\007ready\\n'; exec cat",
        ),
    );
    overlay.open = true;
    overlay.focused = true;
    overlay.ensure_started();
    let text = wait(&mut overlay, |t| t.contains("ready"));
    assert!(text.contains("ready"), "{text}");
    // The focused pane's clipboard write was honored.
    assert_eq!(overlay.copied.as_deref(), Some("copied"));
    // Focus changes are reported once the program asked: out to the
    // world, and back in.
    overlay.modifiers(ModifiersState::CONTROL);
    overlay.key(&char_key(KeyCode::Backquote, "`"));
    overlay.tick();
    overlay.key(&char_key(KeyCode::Backquote, "`"));
    overlay.modifiers(ModifiersState::empty());
    let text = wait(&mut overlay, |t| t.contains("^[[O^[[I"));
    assert!(text.contains("^[[O^[[I"), "{text}");
    // A click is reported, not selected.
    overlay.press(cell_point(&overlay, 2, 5));
    overlay.release(cell_point(&overlay, 2, 5));
    let text = wait(&mut overlay, |t| t.contains("^[[<0;6;3m"));
    assert!(text.contains("^[[<0;6;3M^[[<0;6;3m"), "{text}");
    assert_eq!(overlay.selected_text(), None);
    // The wheel too.
    overlay.wheel(cell_point(&overlay, 1, 1), 1.0);
    let text = wait(&mut overlay, |t| t.contains("^[[<64;2;2M"));
    assert!(text.contains("^[[<64;2;2M"), "{text}");
    // Shift keeps the mouse for selection.
    overlay.modifiers(ModifiersState::SHIFT);
    overlay.press(cell_point(&overlay, 0, 0));
    overlay.pointer(cell_point(&overlay, 0, 4));
    overlay.release(cell_point(&overlay, 0, 4));
    overlay.modifiers(ModifiersState::empty());
    assert_eq!(overlay.selected_text().as_deref(), Some("ready"));
    overlay.shutdown();
}

#[cfg(unix)]
#[test]
fn a_background_pane_cannot_write_the_clipboard() {
    let root = tempfile::tempdir().unwrap();
    let mut overlay = Overlay::with(root.path(), "/bin/sh".into(), sh("exec cat"));
    overlay.open = true;
    overlay.focused = true;
    overlay.ensure_started();
    overlay.split(
        Axis::Columns,
        &sh("sleep 0.3; printf '\\033]52;c;c2VjcmV0\\007written\\n'; exec cat"),
    );
    // Focus goes back to the first pane before the second one writes.
    overlay.modifiers(ModifiersState::CONTROL);
    overlay.key(&char_key(KeyCode::KeyB, "b"));
    overlay.modifiers(ModifiersState::empty());
    overlay.key(&key(
        KeyCode::ArrowLeft,
        Logical::Named(NamedKey::ArrowLeft),
        None,
    ));
    let deadline = Instant::now() + Duration::from_secs(5);
    while overlay.notice.is_none() && Instant::now() < deadline {
        overlay.tick();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(overlay.copied, None);
    assert!(
        overlay.notice.as_deref().unwrap_or("").contains("refused"),
        "{:?}",
        overlay.notice
    );
    overlay.shutdown();
}

#[cfg(unix)]
#[test]
fn copy_mode_moves_selects_searches_and_copies() {
    let root = tempfile::tempdir().unwrap();
    let mut overlay = Overlay::with(
        root.path(),
        "/bin/sh".into(),
        sh("for i in 1 2 3 4 5 6 7 8 9; do echo line$i; done; echo needle here; exec cat"),
    );
    overlay.open = true;
    overlay.focused = true;
    overlay.ensure_started();
    wait(&mut overlay, |t| t.contains("needle"));
    let prefix = |overlay: &mut Overlay| {
        overlay.modifiers(ModifiersState::CONTROL);
        overlay.key(&char_key(KeyCode::KeyB, "b"));
        overlay.modifiers(ModifiersState::empty());
    };
    // Search for "line3", then select to the end of the word and copy.
    prefix(&mut overlay);
    overlay.key(&char_key(KeyCode::Slash, "/"));
    for c in ["l", "i", "n", "e", "3"] {
        overlay.key(&char_key(KeyCode::KeyL, c));
    }
    overlay.key(&key(KeyCode::Enter, Logical::Named(NamedKey::Enter), None));
    assert_eq!(overlay.selected_text().as_deref(), Some("line3"));
    // v starts a selection at the match; j moves down a line.
    overlay.key(&char_key(KeyCode::KeyV, "v"));
    overlay.key(&char_key(KeyCode::KeyJ, "j"));
    overlay.key(&char_key(KeyCode::KeyL, "l"));
    assert_eq!(overlay.selected_text().as_deref(), Some("line3\nli"));
    overlay.key(&char_key(KeyCode::KeyY, "y"));
    assert_eq!(overlay.copied.as_deref(), Some("line3\nli"));
    assert!(overlay.copy.is_none());
    // Typing reaches the program again.
    overlay.key(&char_key(KeyCode::KeyZ, "z"));
    let text = wait(&mut overlay, |t| t.contains("here\nz") || t.ends_with('z'));
    assert!(text.contains('z'), "{text}");
    overlay.shutdown();
}

#[cfg(unix)]
#[test]
fn a_flood_of_output_is_applied_within_the_frame_budget() {
    let root = tempfile::tempdir().unwrap();
    let mut overlay = Overlay::with(root.path(), "/bin/sh".into(), sh("exec yes flood"));
    overlay.open = true;
    overlay.focused = true;
    overlay.ensure_started();
    overlay.split(Axis::Columns, &sh("exec yes second"));
    overlay.stats.record = true;
    // Let both fill their queues, then time frames.
    std::thread::sleep(Duration::from_millis(300));
    let mut worst = Duration::ZERO;
    for _ in 0..20 {
        let started = Instant::now();
        overlay.tick();
        worst = worst.max(started.elapsed());
        overlay.frame_done(started);
    }
    // The budget plus one frame per pane and the resize checks.
    assert!(
        worst < super::UPDATE_BUDGET + Duration::from_millis(20),
        "a tick took {worst:?}"
    );
    let most = overlay.stats.frames.iter().map(|f| f.bytes).max().unwrap();
    assert!(most > 0);
    assert!(
        most <= 2 * (super::PANE_BYTES + coder_pty::wire::FRAME_MAX) as u64,
        "{most} bytes in one frame"
    );
    overlay.shutdown();
}

#[test]
fn zsh_hooks_keep_user_configuration_and_make_requests_pending() {
    let shell = std::env::var_os("OPENAGENTS_TEST_ZSH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("/bin/zsh"));
    assert!(
        shell.is_file(),
        "set OPENAGENTS_TEST_ZSH to an installed zsh"
    );
    let root = tempfile::tempdir().unwrap();
    let rc = "PROMPT='fixture> '\nalias fixture_greeting='print hello'\n";
    std::fs::write(root.path().join(".zshrc"), rc).unwrap();
    let mut overlay = Overlay::with(root.path(), shell, Program::Shell);
    overlay.open = true;
    overlay.focused = true;
    overlay.ensure_started();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !overlay
        .panes
        .values()
        .any(|pane| pane.session.blocks.at_prompt)
    {
        assert!(
            Instant::now() < deadline,
            "shell prompt did not initialize: {:?}",
            overlay.focused_text()
        );
        overlay.tick();
        std::thread::sleep(Duration::from_millis(10));
    }
    overlay.send(b"fixture_greeting; false\r");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !overlay.panes.values().any(|pane| {
        pane.session
            .blocks
            .records
            .back()
            .is_some_and(|block| block.status == Some(1))
    }) {
        assert!(
            Instant::now() < deadline,
            "command did not complete: {:?}",
            overlay.focused_text()
        );
        overlay.tick();
        std::thread::sleep(Duration::from_millis(10));
    }
    let pane = overlay.panes.values().next().unwrap();
    let block = pane.session.blocks.records.back().unwrap();
    assert_eq!(block.command, "fixture_greeting; false");
    assert!(block.output.contains("hello"), "{}", block.output);
    let before = pane.session.blocks.records.len();
    overlay.send(b"# why did that fail\r");
    let deadline = Instant::now() + Duration::from_secs(10);
    while overlay.smart.draft.is_none() {
        assert!(
            Instant::now() < deadline,
            "request hook did not initialize: {:?}",
            overlay.focused_text()
        );
        overlay.tick();
        std::thread::sleep(Duration::from_millis(10));
    }
    let draft = overlay.smart.draft.as_ref().unwrap();
    assert_eq!(draft.text, "why did that fail");
    assert_eq!(draft.context.blocks.len(), 1);
    assert!(overlay.smart.workers.is_empty());
    assert_eq!(
        overlay
            .panes
            .values()
            .next()
            .unwrap()
            .session
            .blocks
            .records
            .len(),
        before
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join(".zshrc")).unwrap(),
        rc
    );
    overlay.smart.draft = None;
    overlay.send(b"print continued\r");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !overlay
        .focused_text()
        .is_some_and(|text| text.contains("continued"))
    {
        assert!(Instant::now() < deadline);
        overlay.tick();
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn attached_context_scrubs_credentials_before_preview() {
    let text = "authorization: Bearer private\napi_key=private\nsafe\n-----BEGIN PRIVATE KEY-----\nprivate\n-----END PRIVATE KEY-----\nvalue sk-example";
    let clean = super::smart::scrub(text);
    assert!(!clean.contains("private"));
    assert!(!clean.contains("sk-example"));
    assert!(clean.contains("safe"));
}

#[test]
fn a_live_shell_proposal_waits_for_exact_enter_and_destructive_confirmation() {
    use terminal_core::proposals::{Effect, Phase, Proposal};
    let shell = std::env::var_os("OPENAGENTS_TEST_ZSH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "/bin/zsh".into());
    let root = tempfile::tempdir().unwrap();
    let mut overlay = Overlay::with(root.path(), shell, Program::Shell);
    overlay.open = true;
    overlay.focused = true;
    overlay.ensure_started();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !overlay
        .panes
        .values()
        .any(|pane| pane.session.blocks.at_prompt)
    {
        assert!(Instant::now() < deadline);
        overlay.tick();
        std::thread::sleep(Duration::from_millis(5));
    }
    let pane = overlay.focus_id().unwrap();
    let command = "print approved_once".to_owned();
    let proposal = Proposal {
        thread: "test-thread".into(),
        id: "test-proposal".into(),
        revision: 1,
        command: command.clone(),
        binding: overlay.panes[&pane]
            .session
            .binding("context".into())
            .unwrap(),
    };
    let key = overlay.smart.book.offer(proposal).unwrap();
    overlay
        .smart
        .policy
        .0
        .insert(command, Effect::Destructive("changes files".into()));
    overlay.smart.pending = Some((pane, key.clone()));
    for _ in 0..10 {
        overlay.tick();
    }
    assert!(overlay.panes[&pane].session.blocks.records.is_empty());
    let enter = KeyIn {
        code: KeyCode::Enter,
        logical: Logical::Named(NamedKey::Enter),
        text: None,
        plain: None,
        pressed: true,
    };
    assert!(overlay.key(&enter));
    assert!(matches!(
        overlay.smart.book.entries[&key].phase,
        Phase::Warned { .. }
    ));
    assert!(overlay.panes[&pane].session.blocks.records.is_empty());
    assert!(overlay.key(&enter));
    assert!(matches!(
        overlay.smart.book.entries[&key].phase,
        Phase::Warned { .. }
    ));
    let mut release = enter.clone();
    release.pressed = false;
    assert!(overlay.key(&release));
    assert!(overlay.key(&enter));
    assert!(matches!(
        overlay.smart.book.entries[&key].phase,
        Phase::Executing { .. }
    ));
    // Keep this fixture offline: verify the PTY block without starting a chat helper.
    overlay.smart.execution = None;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !overlay.panes[&pane]
        .session
        .blocks
        .records
        .back()
        .is_some_and(|block| block.end.is_some())
    {
        assert!(Instant::now() < deadline);
        overlay.tick();
        std::thread::sleep(Duration::from_millis(5));
    }
    let records = &overlay.panes[&pane].session.blocks.records;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].command, "print approved_once");
    assert_eq!(records[0].status, Some(0));
    assert!(records[0].output.contains("approved_once"));
    overlay.shutdown();
}
