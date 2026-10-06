//! Snapshot serialize and restore: a terminal restored from a snapshot at
//! any byte, then fed the rest of the output, matches one that parsed the
//! whole output without a break.

use coder_pty::ext::{
    self, Assembler, HistoryRecord, Record, RecordsFrame, StreamKind, encode_stream,
};
use coder_pty::wire::{Reason, Refusal, TerminalRef};
use coder_vt::{
    Binding, CONTINUATION_MAX, Key, Modifiers, MouseButton, MouseEvent, MouseKind, Restore,
    Terminal,
};

const SCROLLBACK: usize = 100;

fn reference() -> TerminalRef {
    TerminalRef {
        generation: "1".repeat(64),
        terminal: "2".repeat(64),
    }
}

fn binding() -> Binding {
    Binding {
        terminal: reference(),
        through: 7,
        exit: None,
    }
}

/// Everything a client can observe, with hyperlinks by target rather than
/// by the table index each terminal assigned.
fn fingerprint(t: &Terminal) -> String {
    let row = |row: &coder_vt::Row| {
        let cells: Vec<String> = row
            .cells
            .iter()
            .map(|cell| {
                format!(
                    "{}{:?}/{}/{:?}/{:?}/{}/{:?}",
                    cell.ch,
                    cell.combining,
                    cell.width,
                    cell.attrs.fg,
                    cell.attrs.bg,
                    cell.attrs.flags.bits(),
                    t.link(cell.attrs.link),
                )
            })
            .collect();
        format!("{}|{}", row.wrapped, cells.join(","))
    };
    let mut out = Vec::new();
    out.push(format!(
        "size {}x{} cursor {:?} style {:?} visible {} alternate {} app-cursor {} paste {} mouse {:?} kitty {} title {:?} epoch {} dropped {}",
        t.rows(),
        t.cols(),
        t.cursor(),
        t.cursor_style(),
        t.cursor_visible(),
        t.alternate_screen(),
        t.application_cursor(),
        t.bracketed_paste(),
        t.mouse_mode(),
        t.kitty_flags(),
        t.title(),
        t.line_epoch(),
        t.history_dropped(),
    ));
    let none = Modifiers::default();
    out.push(format!(
        "keys {:?} {:?} {:?} mouse {:?} focus {:?} paste {:?}",
        t.key(Key::Up, none),
        t.key(Key::Keypad('5'), none),
        t.key(Key::Escape, Modifiers { ctrl: true, ..none }),
        t.mouse(MouseEvent {
            kind: MouseKind::Press(MouseButton::Left),
            row: 1,
            col: 2,
            modifiers: none,
        }),
        t.focus(true),
        t.paste("p"),
    ));
    out.extend(t.scrollback().map(|r| format!("history {}", row(r))));
    out.extend(t.screen().iter().map(|r| format!("screen {}", row(r))));
    out.join("\n")
}

/// Sends `records` the way a host does, in parts, through a client's
/// stream checks.
fn transport(records: &[Record], part_max: usize) -> Result<Vec<Record>, Refusal> {
    let bytes = encode_stream(records);
    let frames = ext::frames(
        &reference(),
        &"4".repeat(64),
        &"5".repeat(64),
        &bytes,
        part_max,
    );
    let mut assembler = Assembler::new(StreamKind::Snapshot, reference());
    let mut out = Vec::new();
    for frame in &frames {
        out.extend(assembler.push(frame)?);
    }
    Ok(out)
}

/// Restores a terminal from a snapshot stream's records and attaches every
/// history page.
fn restore(records: &[Record]) -> Terminal {
    let mut restore = Restore::new(SCROLLBACK);
    let mut terminal = None;
    let mut epoch = 0;
    for record in records {
        match record {
            Record::History(page) => {
                let t: &mut Terminal = terminal.as_mut().expect("history after READY");
                t.attach_history(epoch, page).expect("the page attaches");
            }
            Record::Finish(_) => {}
            record => {
                if let Record::Terminal(binding) = record {
                    epoch = binding.epoch;
                }
                if let Some(t) = restore.push(record).expect("the record restores") {
                    terminal = Some(t);
                }
            }
        }
    }
    terminal.expect("READY arrived")
}

fn snapshot_and_restore(host: &mut Terminal) -> Terminal {
    let records = host.snapshot(&binding()).expect("the snapshot fits");
    restore(&transport(&records, 512).expect("the stream checks"))
}

/// Output exercising split UTF-8, CSI, OSC, DCS, the alternate screen,
/// mouse and key modes, charsets, regions, tabs, and scrolling.
fn session() -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice("héllo 世界 e\u{301} 🙂\r\n".as_bytes());
    out.extend_from_slice(b"\x1b[1;31mred\x1b[0m \x1b[38;2;1;2;3mrgb\x1b[48;5;200mbg\x1b[m\r\n");
    // A line feed inside a CSI executes at once; replaying the sequence
    // must not run it twice.
    out.extend_from_slice(b"ab\x1b[1\r\n;3Hc");
    out.extend_from_slice(
        b"\x1b]0;my title\x07\x1b]8;;https://example.test/a\x1b\\link\x1b]8;;\x1b\\\r\n",
    );
    out.extend_from_slice(b"\x1bP1$qm\x1b\\after dcs\r\n");
    out.extend_from_slice(b"\x1b(0lqk\x1b(B \x1b)0\x0eqx\x0f plain\r\n");
    out.extend_from_slice(b"\x1b[3g\x1b[5G\x1bH\r\tT\r\n");
    for n in 0..12 {
        out.extend_from_slice(format!("line {n}\r\n").as_bytes());
    }
    out.extend_from_slice(
        b"\x1b[?1049h\x1b[?1h\x1b=\x1b[?1002h\x1b[?1006h\x1b[?2004h\x1b[?1004h\x1b[>1u",
    );
    out.extend_from_slice(b"\x1b[2;4r\x1b[?6h\x1b[2;3Hfull\x1b[5 q\x1b[?25l\x1b[4h\x1b[20hins\n");
    out.extend_from_slice("\x1b[1;1H世界\x1b[7m wide 1234567890abcdefghij".as_bytes());
    out.extend_from_slice(b"\x1b[?6l\x1b[r\x1b[?1049l\x1b[4l\x1b[20l");
    out.extend_from_slice("tail ü\r\n".as_bytes());
    out
}

#[test]
fn a_restore_at_every_byte_continues_as_uninterrupted_parsing() {
    let bytes = session();
    for split in 0..=bytes.len() {
        // `vte` can parse UTF-8 differently across feed boundaries, so the
        // uninterrupted reference takes the same pieces, without a
        // snapshot between them.
        let mut whole = Terminal::new(6, 20, SCROLLBACK);
        whole.feed(&bytes[..split]);
        let mut host = Terminal::new(6, 20, SCROLLBACK);
        host.feed(&bytes[..split]);
        let mut client = snapshot_and_restore(&mut host);
        assert_eq!(
            fingerprint(&client),
            fingerprint(&host),
            "restored state differs at byte {split}"
        );
        // The rest of the output in small pieces, as live frames arrive.
        for chunk in bytes[split..].chunks(7) {
            whole.feed(chunk);
            host.feed(chunk);
            client.feed(chunk);
        }
        assert_eq!(
            fingerprint(&client),
            fingerprint(&whole),
            "live output after a restore at byte {split} differs"
        );
        assert_eq!(fingerprint(&host), fingerprint(&whole));
    }
}

#[test]
fn resizes_before_and_after_a_restore_match() {
    let bytes = session();
    let (head, tail) = bytes.split_at(bytes.len() / 2);
    let mut whole = Terminal::new(6, 20, SCROLLBACK);
    whole.feed(head);
    whole.resize(4, 13);
    let mut host = Terminal::new(6, 20, SCROLLBACK);
    host.feed(head);
    host.resize(4, 13);
    let mut client = snapshot_and_restore(&mut host);
    assert_eq!(fingerprint(&client), fingerprint(&host));
    for t in [&mut whole, &mut client] {
        t.feed(tail);
        t.resize(9, 31);
        t.feed(b"after\x1b[2;2Hx");
    }
    assert_eq!(fingerprint(&client), fingerprint(&whole));
}

#[test]
fn a_continuation_holds_exactly_the_unfinished_input() {
    let mut t = Terminal::new(3, 10, 0);
    t.feed(b"ok\x1b[1;");
    assert_eq!(t.continuation().as_deref(), Some(&b"\x1b[1;"[..]));
    t.feed(b"2H");
    assert_eq!(t.continuation().as_deref(), Some(&b""[..]));
    t.feed(&[0xe4, 0xb8]);
    assert_eq!(t.continuation().as_deref(), Some(&[0xe4, 0xb8][..]));
    t.feed(&[0x96]);
    assert_eq!(t.continuation().as_deref(), Some(&b""[..]));
    t.feed(b"\x1b]0;title");
    assert_eq!(t.continuation().as_deref(), Some(&b"\x1b]0;title"[..]));
}

/// Feeds `head` to a host, snapshots it, restores a client, and feeds
/// `tail` to both, which must then agree; answers the continuation sent.
fn across_a_snapshot(head: &[u8], tail: &[u8]) -> (Terminal, Vec<u8>) {
    let mut host = Terminal::new(3, 30, SCROLLBACK);
    host.feed(head);
    let records = host.snapshot(&binding()).unwrap();
    let sent = records
        .iter()
        .find_map(|record| match record {
            Record::Continuation(bytes) => Some(bytes.clone()),
            _ => None,
        })
        .unwrap_or_default();
    assert!(sent.len() <= CONTINUATION_MAX);
    let mut client = restore(&transport(&records, 4096).unwrap());
    for piece in tail.chunks(5) {
        host.feed(piece);
        client.feed(piece);
    }
    assert_eq!(fingerprint(&client), fingerprint(&host));
    (client, sent)
}

#[test]
fn a_long_sequence_resumes_as_an_equivalent_short_one() {
    let long = |prefix: &[u8], fill: u8| {
        let mut bytes = prefix.to_vec();
        bytes.resize(CONTINUATION_MAX + 100, fill);
        bytes
    };
    // A CSI with too many parameters, an escape with many intermediates,
    // a long DCS, an SOS string, and an escape followed by many line
    // feeds: each is ignored the same way on both sides.
    for (head, tail, shown) in [
        (long(b"\x1b[1", b'0'), &b"m;1Hplain\x1b[1mbold"[..], "bold"),
        (long(b"\x1b[", b';'), b"\n2mafter", "after"),
        (long(b"\x1b ", b' '), b"Gnext", "next"),
        (long(b"\x1bP1;2", b'3'), b"q#data\x1b\\done", "done"),
        (long(b"\x1bPq", b'#'), b"\x9c#\x1b\\done", "done"),
        (long(b"\x1bX", b'x'), b"\x1b\\done", "done"),
        (long(b"\x1b", b'\n'), b"Mup", "up"),
    ] {
        let (client, sent) = across_a_snapshot(&head, tail);
        assert!(sent.len() < 64, "{sent:?}");
        assert!(client.text().contains(shown), "{:?}", client.text());
    }
}

#[test]
fn an_oversized_osc_string_is_abandoned_on_both_sides() {
    let mut long = b"\x1b]0;".to_vec();
    long.resize(CONTINUATION_MAX + 10, b'a');
    let (client, sent) = across_a_snapshot(&long, b"rest\x07after");
    assert_eq!(sent, b"\x1b]999999;");
    assert_eq!(client.title(), "");
    assert!(client.text().starts_with("after"), "{:?}", client.text());
}

#[test]
fn a_long_run_of_broken_utf8_starts_both_parsers_fresh() {
    let mut garbage = Vec::new();
    for _ in 0..CONTINUATION_MAX {
        garbage.extend_from_slice(&[0xc0, 0xff]);
    }
    garbage.push(0xe4);
    let (_, sent) = across_a_snapshot(&garbage, "\u{4e16}ok".as_bytes());
    assert!(sent.is_empty());
}

#[test]
fn an_endless_osc_string_is_bounded_and_dropped() {
    let mut t = Terminal::new(3, 20, 10);
    t.feed(b"\x1b]0;");
    let chunk = vec![b'z'; 64 * 1024];
    for _ in 0..40 {
        t.feed(&chunk);
    }
    // Past the bound the string is abandoned; its rest is dropped up to
    // the terminator, and output after it prints.
    assert_eq!(t.continuation().as_deref(), Some(&b"\x1b]999999;"[..]));
    t.feed(b"zzz\x07shown");
    assert_eq!(t.title(), "");
    assert_eq!(t.text().lines().next(), Some("shown"));
    // A clipboard write at its bound still arrives.
    let mut write = b"\x1b]52;c;".to_vec();
    write.extend(std::iter::repeat_n(b'A', coder_vt::MAX_CLIPBOARD));
    write.push(0x07);
    t.feed(&write);
    assert!(t.take_clipboard().is_some());
}

fn scrolled(lines: usize) -> Terminal {
    let mut t = Terminal::new(4, 12, SCROLLBACK);
    for n in 0..lines {
        t.feed(format!("old {n}\r\n").as_bytes());
    }
    t
}

#[test]
fn ready_alone_draws_the_screen_and_resumes_parsing() {
    let mut host = scrolled(30);
    host.feed(b"\x1b[1;3");
    let records = host.snapshot(&binding()).unwrap();
    let ready = records
        .iter()
        .position(|record| matches!(record, Record::Ready))
        .unwrap();
    let mut restore = Restore::new(SCROLLBACK);
    let mut client = None;
    for record in &records[..=ready] {
        client = restore.push(record).unwrap();
    }
    let mut client = client.expect("READY yields a terminal");
    assert_eq!(client.text(), host.text());
    assert_eq!(client.scrollback_len(), 0);
    // Screen row 0 keeps its absolute line before any history arrives.
    assert_eq!(
        client.history_dropped(),
        host.history_dropped() + host.scrollback_len() as u64
    );
    host.feed(b"1mbold");
    client.feed(b"1mbold");
    assert_eq!(client.screen(), host.screen());
    // History rows past READY are no business of the restore.
    let history = records[ready + 1..]
        .iter()
        .find(|record| matches!(record, Record::History(_)))
        .unwrap();
    assert!(restore.push(history).is_err());
}

#[test]
fn history_pages_attach_newest_first_while_live_output_scrolls() {
    let mut host = scrolled(300);
    let records = host.snapshot(&binding()).unwrap();
    let epoch = host.line_epoch();
    let mut restore = Restore::new(SCROLLBACK);
    let mut client = None;
    let mut pages: Vec<HistoryRecord> = Vec::new();
    for record in &records {
        match record {
            Record::History(page) => pages.push(page.clone()),
            Record::Finish(finish) => assert!(finish.complete),
            record => {
                if let Some(t) = restore.push(record).unwrap() {
                    client = Some(t);
                }
            }
        }
    }
    let mut client = client.unwrap();
    // Small pages, to interleave them with live output.
    let mut small: Vec<HistoryRecord> = Vec::new();
    for page in &pages {
        for (offset, chunk) in page.rows.chunks(16).enumerate().rev() {
            small.push(HistoryRecord {
                first: page.first + (offset * 16) as u64,
                rows: chunk.to_vec(),
            });
        }
    }
    // A page that does not adjoin, or from another epoch, changes nothing.
    let before = fingerprint(&client);
    assert_eq!(
        client.attach_history(epoch, &small[1]).unwrap_err().reason,
        Reason::Malformed
    );
    assert_eq!(
        client
            .attach_history(epoch + 1, &small[0])
            .unwrap_err()
            .reason,
        Reason::Stale
    );
    assert_eq!(fingerprint(&client), before);
    for (index, page) in small.iter().enumerate() {
        if index % 2 == 0 {
            // Live output scrolls rows into history between pages.
            let line = format!("new {index}\r\n");
            host.feed(line.as_bytes());
            client.feed(line.as_bytes());
        }
        // Once the scrollback is full, older pages have nowhere to go.
        let full = client.scrollback_len() >= SCROLLBACK;
        let attached = client.attach_history(epoch, page);
        if !full {
            attached.unwrap();
        }
    }
    assert_eq!(client.scrollback_len(), SCROLLBACK);
    assert_eq!(fingerprint(&client), fingerprint(&host));
}

#[test]
fn a_reset_after_ready_makes_pending_history_stale() {
    let mut host = scrolled(30);
    let records = host.snapshot(&binding()).unwrap();
    let epoch = host.line_epoch();
    let page = records
        .iter()
        .find_map(|record| match record {
            Record::History(page) => Some(page.clone()),
            _ => None,
        })
        .unwrap();
    let mut client = restore(
        &records
            .iter()
            .filter(|record| !matches!(record, Record::History(_) | Record::Finish(_)))
            .cloned()
            .collect::<Vec<_>>(),
    );
    client.feed(b"\x1bcfresh");
    assert_eq!(client.line_epoch(), epoch + 1);
    let screen = client.text();
    assert_eq!(
        client.attach_history(epoch, &page).unwrap_err().reason,
        Reason::Stale
    );
    assert_eq!(client.text(), screen);
    assert_eq!(client.scrollback_len(), 0);
}

#[test]
fn corrupt_streams_refuse_before_any_state_is_trusted() {
    let mut host = scrolled(20);
    host.feed(b"\x1b[2");
    let records = host.snapshot(&binding()).unwrap();
    let bytes = encode_stream(&records);
    let frames =
        |bytes: &[u8]| ext::frames(&reference(), &"4".repeat(64), &"5".repeat(64), bytes, 8192);
    let push_all = |frames: &[RecordsFrame]| -> Result<Vec<Record>, Refusal> {
        let mut assembler = Assembler::new(StreamKind::Snapshot, reference());
        let mut out = Vec::new();
        for frame in frames {
            out.extend(assembler.push(frame)?);
        }
        Ok(out)
    };
    // A flipped payload byte fails its checksum.
    let mut corrupt = bytes.clone();
    corrupt[ext::RECORD_HEADER + 3] ^= 0x01;
    assert!(push_all(&frames(&corrupt)).is_err());
    // An oversized length.
    let mut oversized = bytes.clone();
    oversized[2..6].copy_from_slice(&((ext::PAYLOAD_MAX + 1) as u32).to_le_bytes());
    assert!(push_all(&frames(&oversized)).is_err());
    // A truncated stream ends inside a record.
    assert!(push_all(&frames(&bytes[..bytes.len() - 3])).is_err());
    // An unsupported format and another generation.
    let rebind = |change: &dyn Fn(&mut ext::TerminalRecord)| {
        let mut records = records.clone();
        if let Record::Terminal(binding) = &mut records[0] {
            change(binding);
        }
        records
    };
    let unsupported = rebind(&|b| b.format = 2);
    assert_eq!(
        push_all(&frames(&encode_stream(&unsupported)))
            .unwrap_err()
            .reason,
        Reason::UnsupportedVersion
    );
    assert_eq!(
        Restore::new(10).push(&unsupported[0]).unwrap_err().reason,
        Reason::UnsupportedVersion
    );
    let other = rebind(&|b| b.generation = "3".repeat(64));
    assert_eq!(
        push_all(&frames(&encode_stream(&other)))
            .unwrap_err()
            .reason,
        Reason::IdentityMismatch
    );
    // A restore that fails yields no terminal and takes nothing more.
    let mut restore = Restore::new(10);
    restore.push(&records[0]).unwrap();
    restore.push(&records[1]).unwrap();
    let mut bad = records[2].clone();
    if let Record::Rows(page) = &mut bad {
        page.rows[0].runs.push(ext::Run {
            text: "xy".into(),
            cells: 5,
            style: ext::Style::plain(),
        });
    }
    assert!(restore.push(&bad).is_err());
    assert!(restore.push(&records[2]).is_err());
    // Records out of order refuse.
    let mut restore = Restore::new(10);
    assert!(restore.push(&records[1]).is_err());
}

#[test]
fn hostile_state_refuses_instead_of_panicking() {
    let mut host = Terminal::new(3, 8, 10);
    let records = host.snapshot(&binding()).unwrap();
    let with_state = |change: &dyn Fn(&mut ext::StateRecord)| {
        let mut records = records.clone();
        if let Record::State(state) = &mut records[1] {
            change(state);
        }
        let mut restore = Restore::new(10);
        records
            .iter()
            .take_while(|record| !matches!(record, Record::History(_) | Record::Finish(_)))
            .try_for_each(|record| restore.push(record).map(drop))
    };
    assert!(with_state(&|_| {}).is_ok());
    assert!(with_state(&|s| s.cursor.row = 3).is_err());
    assert!(with_state(&|s| s.scroll.bottom = 3).is_err());
    assert!(with_state(&|s| s.tabs = vec![8]).is_err());
    assert!(with_state(&|s| s.charsets.shift = 2).is_err());
    assert!(with_state(&|s| s.keyboard.primary = vec![1; 17]).is_err());
    assert!(with_state(&|s| s.pen.flags = 0x100).is_err());
    assert!(with_state(&|s| s.title = "a\u{7}".into()).is_err());
}

#[test]
fn a_one_row_terminal_round_trips() {
    let mut host = Terminal::new(1, 5, 10);
    host.feed(b"abc\r\ndef");
    let mut client = snapshot_and_restore(&mut host);
    assert_eq!(fingerprint(&client), fingerprint(&host));
    client.feed(b"\r\nz");
    host.feed(b"\r\nz");
    assert_eq!(fingerprint(&client), fingerprint(&host));
}

#[test]
fn a_history_read_answers_older_rows_or_refuses() {
    let host = scrolled(50);
    let epoch = host.line_epoch();
    let first = host.history_dropped();
    let end = first + host.scrollback_len() as u64;
    let records = host
        .history_stream(&binding(), epoch, end - 10, 20)
        .unwrap();
    let bytes = encode_stream(&records);
    let frames = ext::frames(&reference(), &"4".repeat(64), &"5".repeat(64), &bytes, 8192);
    let mut assembler = Assembler::new(
        StreamKind::History {
            epoch,
            before: end - 10,
            rows: 20,
        },
        reference(),
    );
    for frame in &frames {
        assembler.push(frame).unwrap();
    }
    assert!(assembler.finished());
    let Some(Record::History(page)) = records.get(1) else {
        panic!("a history page");
    };
    assert_eq!(page.first, end - 30);
    assert_eq!(page.rows.len(), 20);
    let refuse = |epoch, before| {
        host.history_stream(&binding(), epoch, before, 20)
            .unwrap_err()
            .reason
    };
    assert_eq!(refuse(epoch + 1, end), Reason::Stale);
    assert_eq!(refuse(epoch, end + 1), Reason::Malformed);
    assert_eq!(refuse(epoch, first), Reason::ContentUnavailable);
}
