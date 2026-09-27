use super::*;

fn term(rows: usize, cols: usize) -> Terminal {
    Terminal::new(rows, cols, 100)
}

fn fed(rows: usize, cols: usize, bytes: &[u8]) -> Terminal {
    let mut terminal = term(rows, cols);
    terminal.feed(bytes);
    terminal
}

fn line(terminal: &Terminal, row: usize) -> String {
    terminal.row(row).unwrap().text()
}

fn attrs(terminal: &Terminal, row: usize, col: usize) -> Attrs {
    terminal.row(row).unwrap().cells[col].attrs
}

#[test]
fn prints_text_with_carriage_return_and_linefeed() {
    let t = fed(4, 10, b"hello\r\nworld");
    assert_eq!(t.text(), "hello\nworld\n\n");
    assert_eq!(t.cursor(), (1, 5));
}

#[test]
fn a_bare_linefeed_keeps_the_column() {
    let t = fed(3, 10, b"ab\ncd");
    assert_eq!(line(&t, 1), "  cd");
}

#[test]
fn newline_mode_makes_linefeed_return_the_carriage() {
    let t = fed(3, 10, b"\x1b[20hab\ncd");
    assert_eq!(line(&t, 1), "cd");
}

#[test]
fn autowrap_defers_the_wrap_until_the_next_character() {
    let mut t = fed(3, 5, b"abcde");
    // The cursor stays on the last column with a pending wrap.
    assert_eq!(t.cursor(), (0, 4));
    assert_eq!(t.text(), "abcde\n\n");
    t.feed(b"f");
    assert_eq!(t.text(), "abcde\nf\n");
    assert!(t.row(0).unwrap().wrapped);
    // A carriage return cancels a pending wrap.
    let t = fed(3, 5, b"abcde\rX");
    assert_eq!(t.text(), "Xbcde\n\n");
}

#[test]
fn autowrap_off_overwrites_the_last_column() {
    let t = fed(2, 5, b"\x1b[?7labcdefg");
    assert_eq!(t.text(), "abcdg\n");
}

#[test]
fn output_scrolls_into_the_scrollback() {
    let t = fed(3, 10, b"1\r\n2\r\n3\r\n4\r\n5");
    assert_eq!(t.text(), "3\n4\n5");
    let scrolled: Vec<String> = t.scrollback().map(Row::text).collect();
    assert_eq!(scrolled, vec!["1", "2"]);
}

#[test]
fn the_scrollback_is_bounded() {
    let mut t = Terminal::new(2, 10, 3);
    for n in 0..10 {
        t.feed(format!("{n}\r\n").as_bytes());
    }
    assert_eq!(t.scrollback().len(), 3);
    assert_eq!(t.scrollback().next().unwrap().text(), "6");
}

#[test]
fn cursor_movement_sequences() {
    let mut t = term(10, 20);
    t.feed(b"\x1b[5;10H");
    assert_eq!(t.cursor(), (4, 9));
    t.feed(b"\x1b[2A");
    assert_eq!(t.cursor(), (2, 9));
    t.feed(b"\x1b[3B");
    assert_eq!(t.cursor(), (5, 9));
    t.feed(b"\x1b[4C");
    assert_eq!(t.cursor(), (5, 13));
    t.feed(b"\x1b[20D");
    assert_eq!(t.cursor(), (5, 0));
    t.feed(b"\x1b[7G");
    assert_eq!(t.cursor(), (5, 6));
    t.feed(b"\x1b[2d");
    assert_eq!(t.cursor(), (1, 6));
    t.feed(b"\x1b[E");
    assert_eq!(t.cursor(), (2, 0));
    t.feed(b"\x1b[5;5H\x1b[F");
    assert_eq!(t.cursor(), (3, 0));
    // Movement is clamped to the screen.
    t.feed(b"\x1b[99;99H");
    assert_eq!(t.cursor(), (9, 19));
    t.feed(b"\x1b[H");
    assert_eq!(t.cursor(), (0, 0));
}

#[test]
fn backspace_and_tab() {
    let t = fed(2, 20, b"abc\x08\x08X\tY");
    assert_eq!(line(&t, 0), "aXc     Y");
    assert_eq!(t.cursor(), (0, 9));
    let t = fed(2, 20, b"\x1b[3g\x1b[5G\x1bH\r\tZ");
    assert_eq!(line(&t, 0), "    Z");
    let t = fed(2, 20, b"\x1b[18G\x1b[2Z*");
    assert_eq!(line(&t, 0), "        *");
}

#[test]
fn erase_in_display_and_line() {
    let rows = b"aaaaa\r\nbbbbb\r\nccccc";
    let mut t = fed(3, 5, rows);
    t.feed(b"\x1b[2;3H\x1b[K");
    assert_eq!(t.text(), "aaaaa\nbb\nccccc");
    t.feed(b"\x1b[1K");
    assert_eq!(t.text(), "aaaaa\n\nccccc");
    let mut t = fed(3, 5, rows);
    t.feed(b"\x1b[2;3H\x1b[J");
    assert_eq!(t.text(), "aaaaa\nbb\n");
    let mut t = fed(3, 5, rows);
    t.feed(b"\x1b[2;3H\x1b[1J");
    assert_eq!(t.text(), "\n   bb\nccccc");
    let mut t = fed(3, 5, rows);
    t.feed(b"\x1b[2J");
    assert_eq!(t.text(), "\n\n");
    let mut t = fed(2, 5, b"1\r\n2\r\n3\r\n4");
    assert!(t.scrollback().len() > 0);
    t.feed(b"\x1b[3J");
    assert_eq!(t.scrollback().len(), 0);
}

#[test]
fn erase_uses_the_current_background() {
    let t = fed(2, 5, b"\x1b[44m\x1b[2J");
    assert_eq!(attrs(&t, 1, 3).bg, Color::Indexed(4));
    assert_eq!(attrs(&t, 1, 3).fg, Color::Default);
}

#[test]
fn insert_delete_and_erase_characters() {
    let mut t = fed(1, 8, b"abcdef");
    t.feed(b"\x1b[3G\x1b[2@");
    assert_eq!(line(&t, 0), "ab  cdef");
    t.feed(b"\x1b[2P");
    assert_eq!(line(&t, 0), "abcdef");
    t.feed(b"\x1b[1G\x1b[3X");
    assert_eq!(line(&t, 0), "   def");
    // Insert mode shifts existing text right as it prints.
    let t = fed(1, 8, b"abc\x1b[1G\x1b[4hXY");
    assert_eq!(line(&t, 0), "XYabc");
}

#[test]
fn insert_and_delete_lines_within_the_region() {
    let mut t = fed(4, 5, b"1\r\n2\r\n3\r\n4");
    t.feed(b"\x1b[2;1H\x1b[L");
    assert_eq!(t.text(), "1\n\n2\n3");
    t.feed(b"\x1b[2M");
    assert_eq!(t.text(), "1\n3\n\n");
    // Deleted lines do not enter the scrollback.
    assert_eq!(t.scrollback().len(), 0);
}

#[test]
fn scroll_region_confines_scrolling() {
    let mut t = fed(5, 5, b"a\r\nb\r\nc\r\nd\r\ne");
    // Region rows 2 to 4; the cursor homes on setting it.
    t.feed(b"\x1b[2;4r");
    assert_eq!(t.cursor(), (0, 0));
    t.feed(b"\x1b[4;1H\nX");
    assert_eq!(t.text(), "a\nc\nd\nX\ne");
    // Region scrolling never feeds the scrollback.
    assert_eq!(t.scrollback().len(), 0);
    // Reverse index at the top margin scrolls the region down.
    t.feed(b"\x1b[2;1H\x1bMY");
    assert_eq!(t.text(), "a\nY\nc\nd\ne");
    // Explicit scroll up and down.
    t.feed(b"\x1b[S");
    assert_eq!(t.text(), "a\nc\nd\n\ne");
    t.feed(b"\x1b[2T");
    assert_eq!(t.text(), "a\n\n\nc\ne");
}

#[test]
fn origin_mode_addresses_relative_to_the_region() {
    let mut t = term(6, 5);
    t.feed(b"\x1b[3;5r\x1b[?6h");
    assert_eq!(t.cursor(), (2, 0));
    t.feed(b"\x1b[2;2H");
    assert_eq!(t.cursor(), (3, 1));
    t.feed(b"\x1b[9;1H");
    assert_eq!(t.cursor(), (4, 0));
    t.feed(b"\x1b[6n");
    assert_eq!(t.take_replies(), b"\x1b[3;1R");
}

#[test]
fn cursor_up_and_down_stop_at_the_margins() {
    let mut t = term(10, 5);
    t.feed(b"\x1b[3;6r\x1b[4;1H\x1b[9A");
    assert_eq!(t.cursor(), (2, 0));
    t.feed(b"\x1b[9B");
    assert_eq!(t.cursor(), (5, 0));
    // Outside the region, movement reaches the screen edge.
    t.feed(b"\x1b[8;1H\x1b[9B");
    assert_eq!(t.cursor(), (9, 0));
}

#[test]
fn save_and_restore_cursor_with_attributes() {
    let mut t = term(5, 10);
    t.feed(b"\x1b[3;4H\x1b[1m\x1b7\x1b[H\x1b[0mx\x1b8y");
    assert_eq!(line(&t, 2), "   y");
    assert!(attrs(&t, 2, 3).flags.contains(Flags::BOLD));
    t.feed(b"\x1b[5;5H\x1b[s\x1b[H\x1b[u");
    assert_eq!(t.cursor(), (4, 4));
}

#[test]
fn graphic_rendition_basic_flags() {
    let t = fed(
        1,
        20,
        b"\x1b[1;3;4;7mA\x1b[22;23;24;27mB\x1b[2;9;8mC\x1b[0mD",
    );
    let a = attrs(&t, 0, 0).flags;
    assert!(a.contains(Flags::BOLD) && a.contains(Flags::ITALIC));
    assert!(a.contains(Flags::UNDERLINE) && a.contains(Flags::INVERSE));
    assert_eq!(attrs(&t, 0, 1).flags, Flags::empty());
    let c = attrs(&t, 0, 2).flags;
    assert!(c.contains(Flags::DIM) && c.contains(Flags::STRIKE) && c.contains(Flags::HIDDEN));
    assert_eq!(attrs(&t, 0, 3), Attrs::default());
    // An empty SGR resets.
    let t = fed(1, 5, b"\x1b[1mA\x1b[mB");
    assert_eq!(attrs(&t, 0, 1), Attrs::default());
}

#[test]
fn graphic_rendition_colors() {
    let t = fed(
        1,
        20,
        b"\x1b[31;42mA\x1b[91;103mB\x1b[38;5;208mC\x1b[48;2;1;2;3mD\x1b[38:2::9:8:7mE\x1b[38:5:17mF\x1b[39;49mG",
    );
    assert_eq!(attrs(&t, 0, 0).fg, Color::Indexed(1));
    assert_eq!(attrs(&t, 0, 0).bg, Color::Indexed(2));
    assert_eq!(attrs(&t, 0, 1).fg, Color::Indexed(9));
    assert_eq!(attrs(&t, 0, 1).bg, Color::Indexed(11));
    assert_eq!(attrs(&t, 0, 2).fg, Color::Indexed(208));
    assert_eq!(attrs(&t, 0, 3).bg, Color::Rgb(1, 2, 3));
    assert_eq!(attrs(&t, 0, 4).fg, Color::Rgb(9, 8, 7));
    assert_eq!(attrs(&t, 0, 5).fg, Color::Indexed(17));
    assert_eq!(attrs(&t, 0, 6).fg, Color::Default);
    assert_eq!(attrs(&t, 0, 6).bg, Color::Default);
    // A truncated extended color changes nothing and does not panic.
    let t = fed(1, 5, b"\x1b[38;2;1mA\x1b[48;5mB");
    assert_eq!(attrs(&t, 0, 0).fg, Color::Default);
}

#[test]
fn runs_group_cells_by_attributes() {
    let t = fed(1, 8, b"ab\x1b[1mcd\x1b[0mef");
    let runs = t.row(0).unwrap().runs();
    let texts: Vec<&str> = runs.iter().map(|r| r.text.as_str()).collect();
    assert_eq!(texts, vec!["ab", "cd", "ef  "]);
    let columns: usize = runs.iter().map(|r| r.columns).sum();
    assert_eq!(columns, 8);
    // A wide character counts two columns in its run.
    let t = fed(1, 4, "漢x".as_bytes());
    let runs = t.row(0).unwrap().runs();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].text, "漢x ");
    assert_eq!(runs[0].columns, 4);
}

#[test]
fn alternate_screen_saves_and_restores_the_primary() {
    let mut t = fed(3, 10, b"shell$ vim");
    t.feed(b"\x1b[?1049h");
    assert!(t.alternate_screen());
    assert_eq!(t.text(), "\n\n");
    t.feed(b"\x1b[Hfull screen");
    assert_eq!(line(&t, 0), "full scree");
    t.feed(b"\x1b[?1049l");
    assert!(!t.alternate_screen());
    assert_eq!(t.text(), "shell$ vim\n\n");
    assert_eq!(t.cursor(), (0, 9));
    // Scrolling the alternate screen never feeds the scrollback.
    let mut t = term(2, 5);
    t.feed(b"\x1b[?1049h1\r\n2\r\n3\r\n4");
    assert_eq!(t.scrollback().len(), 0);
    // Mode 47 switches without clearing.
    t.feed(b"\x1b[?1049l\x1b[?47h");
    assert_eq!(t.text(), "3\n4");
    t.feed(b"\x1b[?47l\x1b[?1047h");
    assert_eq!(t.text(), "\n");
}

#[test]
fn modes_the_client_reads() {
    let mut t = term(2, 5);
    assert!(t.cursor_visible());
    t.feed(b"\x1b[?25l\x1b[?1h\x1b[?2004h");
    assert!(!t.cursor_visible());
    assert!(t.application_cursor());
    assert!(t.bracketed_paste());
    assert_eq!(t.key(Key::Up, Modifiers::NONE), b"\x1bOA");
    assert_eq!(t.paste("ls\n"), b"\x1b[200~ls\r\x1b[201~");
    t.feed(b"\x1b[?25h\x1b[?1l\x1b[?2004l");
    assert!(t.cursor_visible());
    assert_eq!(t.key(Key::Up, Modifiers::NONE), b"\x1b[A");
    assert_eq!(t.paste("ls\n"), b"ls\r");
}

#[test]
fn replies_to_status_and_attribute_requests() {
    let mut t = term(5, 10);
    t.feed(b"\x1b[3;7H\x1b[6n\x1b[5n\x1b[c\x1b[>c");
    assert_eq!(t.take_replies(), b"\x1b[3;7R\x1b[0n\x1b[?1;2c\x1b[>0;0;0c");
    assert!(t.take_replies().is_empty());
}

#[test]
fn replies_are_bounded() {
    let mut t = term(2, 2);
    for _ in 0..2000 {
        t.feed(b"\x1b[5n");
    }
    assert!(t.take_replies().len() <= MAX_REPLIES);
}

#[test]
fn utf8_wide_and_combining_characters() {
    let mut t = term(2, 6);
    // A character split across two feeds.
    let bytes = "é漢".as_bytes();
    t.feed(&bytes[..1]);
    t.feed(&bytes[1..]);
    assert_eq!(line(&t, 0), "é漢");
    assert_eq!(t.cursor(), (0, 3));
    assert_eq!(t.row(0).unwrap().cells[2].width, 0);
    // A combining accent joins the previous character.
    t.feed("e\u{301}".as_bytes());
    assert_eq!(line(&t, 0), "é漢e\u{301}");
    assert_eq!(t.cursor(), (0, 4));
    // A wide character that does not fit wraps to the next line.
    let t = fed(2, 5, "abcd漢".as_bytes());
    assert_eq!(t.text(), "abcd\n漢");
    // Overwriting half of a wide character blanks the other half.
    let t = fed(1, 6, "漢\x1b[2GX".as_bytes());
    assert_eq!(line(&t, 0), " X");
    // Invalid UTF-8 becomes a replacement character.
    let t = fed(1, 6, b"a\xffb");
    assert_eq!(line(&t, 0), "a\u{fffd}b");
}

#[test]
fn line_drawing_character_set() {
    let t = fed(1, 10, b"\x1b(0lqk\x1b(Bx");
    assert_eq!(line(&t, 0), "┌─┐x");
    // Shift out selects G1.
    let t = fed(1, 10, b"\x1b)0a\x0eq\x0fq");
    assert_eq!(line(&t, 0), "a─q");
}

#[test]
fn repeat_the_last_character() {
    let t = fed(1, 10, b"-\x1b[4b");
    assert_eq!(line(&t, 0), "-----");
}

#[test]
fn title_bell_and_ignored_commands() {
    let mut t = term(2, 10);
    t.feed(b"\x1b]0;my title\x07\x07\x1b]2;other\x1b\\");
    assert_eq!(t.title(), "other");
    assert_eq!(t.bells(), 1);
    // A clipboard request (OSC 52) and a DCS string change nothing visible.
    t.feed(b"\x1b]52;c;aGVsbG8=\x07\x1bPq#0;2;0;0;0\x1b\\ok");
    assert_eq!(line(&t, 0), "ok");
    // Titles are bounded.
    let long = format!("\x1b]0;{}\x07", "x".repeat(1000));
    t.feed(long.as_bytes());
    assert_eq!(t.title().chars().count(), MAX_TITLE);
}

#[test]
fn sequences_split_across_feeds_continue() {
    let mut t = term(2, 10);
    t.feed(b"\x1b[");
    t.feed(b"1;3");
    t.feed(b"1mR");
    assert_eq!(attrs(&t, 0, 0).fg, Color::Indexed(1));
    assert!(attrs(&t, 0, 0).flags.contains(Flags::BOLD));
}

#[test]
fn a_marker_starts_on_its_own_line_and_resets_the_parser() {
    let mut t = term(4, 20);
    // Output cut off in the middle of a control sequence.
    t.feed(b"$ make\x1b[3");
    t.mark("[output lost]");
    t.feed(b"1mdone");
    assert_eq!(t.text(), "$ make\n[output lost]\n1mdone\n");
    assert!(attrs(&t, 1, 0).flags.contains(Flags::MARKER));
    assert!(!attrs(&t, 2, 0).flags.contains(Flags::MARKER));
    // At the start of a line, no blank line is added.
    let mut t = term(3, 20);
    t.mark("gap");
    assert_eq!(t.text(), "gap\n\n");
}

#[test]
fn resize_keeps_the_cursor_line_and_clamps() {
    let mut t = fed(4, 10, b"1\r\n2\r\n3\r\n4");
    t.resize(2, 10);
    assert_eq!(t.text(), "3\n4");
    assert_eq!(t.cursor(), (1, 1));
    let scrolled: Vec<String> = t.scrollback().map(Row::text).collect();
    assert_eq!(scrolled, vec!["1", "2"]);
    t.resize(3, 3);
    assert_eq!(t.text(), "3\n4\n");
    assert_eq!(t.cols(), 3);
    t.resize(3, 6);
    t.feed(b"\x1b[1;6Hz");
    assert_eq!(line(&t, 0), "3    z");
    // The size is clamped.
    t.resize(0, 5000);
    assert_eq!((t.rows(), t.cols()), (1, MAX_SIZE));
}

#[test]
fn resize_resets_the_scroll_region_and_cuts_split_wide_characters() {
    let mut t = fed(5, 4, "ab漢".as_bytes());
    t.feed(b"\x1b[2;3r");
    t.resize(5, 3);
    assert_eq!(line(&t, 0), "ab");
    t.feed(b"\x1b[5;1H\n");
    // The region is the whole screen again, so the top line scrolls away.
    assert_eq!(t.scrollback().len(), 1);
}

#[test]
fn full_and_soft_reset() {
    let mut t = fed(3, 10, b"\x1b[1;31mtext\x1b[?25l\x1b[2;3r");
    t.feed(b"\x1b[!p");
    assert!(t.cursor_visible());
    assert_eq!(line(&t, 0), "text");
    t.feed(b"x");
    assert_eq!(attrs(&t, 0, 4), Attrs::default());
    t.feed(b"\x07\x1bc");
    assert_eq!(t.text(), "\n\n");
    assert_eq!(t.cursor(), (0, 0));
    assert_eq!(t.bells(), 1);
}

#[test]
fn the_generation_advances_on_changes() {
    let mut t = term(2, 5);
    let start = t.generation();
    t.feed(b"");
    assert_eq!(t.generation(), start);
    t.feed(b"a");
    assert!(t.generation() > start);
    let before = t.generation();
    t.resize(2, 5);
    assert_eq!(t.generation(), before);
    t.resize(3, 5);
    assert!(t.generation() > before);
}

#[test]
fn decaln_fills_the_screen() {
    let t = fed(2, 3, b"\x1b#8");
    assert_eq!(t.text(), "EEE\nEEE");
}

/// A shell session's worth of typical output, including a prompt that
/// redraws its line, a colored `ls`, and a full-screen program's frame.
#[test]
fn a_typical_session_renders_as_expected() {
    let mut t = term(6, 30);
    t.feed(b"\x1b]0;user@host: ~\x07\x1b[01;32muser@host\x1b[00m:\x1b[01;34m~\x1b[00m$ ");
    t.feed(b"ls\r\n\x1b[0m\x1b[01;34mdocs\x1b[0m  README.md\r\n");
    t.feed(b"\x1b[01;32muser@host\x1b[00m:\x1b[01;34m~\x1b[00m$ ");
    assert_eq!(t.title(), "user@host: ~");
    assert_eq!(
        t.text(),
        "user@host:~$ ls\ndocs  README.md\nuser@host:~$\n\n\n"
    );
    assert_eq!(attrs(&t, 1, 0).fg, Color::Indexed(4));
    assert!(attrs(&t, 1, 0).flags.contains(Flags::BOLD));
    // A full-screen program draws a box on the alternate screen and leaves.
    t.feed(b"\x1b[?1049h\x1b[H\x1b[2J\x1b(0lqqk\x1b(B\x1b[2;1H\x1b(0x\x1b(Bhi\x1b(0x\x1b(B");
    assert_eq!(line(&t, 0), "┌──┐");
    assert_eq!(line(&t, 1), "│hi│");
    t.feed(b"\x1b[?1049l");
    assert_eq!(line(&t, 2), "user@host:~$");
    assert_eq!(t.cursor(), (2, 13));
}

/// Arbitrary output never panics and never breaks the grid's shape, on
/// grids down to one cell, across resizes.
#[test]
fn arbitrary_output_keeps_the_grid_well_formed() {
    let pieces: [&[u8]; 24] = [
        b"\x1b[",
        b"\x1b[?",
        b"1049h",
        b"1049l",
        b"m",
        b";",
        b"38;5;",
        b"H",
        b"J",
        b"K",
        b"r",
        b"@",
        b"P",
        b"L",
        b"M",
        b"\x1b7",
        b"\x1b8",
        b"\x1bM",
        "漢".as_bytes(),
        "e\u{301}".as_bytes(),
        b"\r\n",
        b"\t",
        b"\x08",
        b"\x1b(0q",
    ];
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for (rows, cols) in [(1, 1), (1, 2), (2, 1), (3, 7), (24, 80)] {
        let mut t = Terminal::new(rows, cols, 10);
        for step in 0..4000 {
            let pick = next();
            if pick % 7 == 0 {
                let byte = (next() % 256) as u8;
                t.feed(&[byte]);
            } else if pick % 11 == 0 {
                let digits = format!("{}", next() % 2000);
                t.feed(digits.as_bytes());
            } else {
                t.feed(pieces[(pick % pieces.len() as u64) as usize]);
            }
            if step % 997 == 0 {
                t.resize((next() % 30) as usize + 1, (next() % 90) as usize + 1);
            }
            let (row, col) = t.cursor();
            assert!(row < t.rows() && col < t.cols());
            assert_eq!(t.screen().len(), t.rows());
            for line in t.screen() {
                assert_eq!(line.cells.len(), t.cols());
                let columns: usize = line.runs().iter().map(|r| r.columns).sum();
                assert_eq!(columns, t.cols());
            }
        }
        let _ = t.take_replies();
    }
}
