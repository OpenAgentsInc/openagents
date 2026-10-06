//! The host's block journal: commands and outcomes from shell marks,
//! without their output, paged newest first.

use coder_pty::emulator::Emulator;
use coder_pty::ext::{BlockPage, BlockState, Origin, SeqRange};
use coder_pty::wire::{Reason, Size};
use coder_vt::Authority;
use coder_vt::journal::JOURNAL_MAX;

fn hex(text: &str) -> String {
    text.bytes().map(|byte| format!("{byte:02x}")).collect()
}

/// The marks a hooked shell prints around one command, in three output
/// frames: the prompt, the command and its output, and the end.
fn command(
    authority: &mut Authority,
    seq: &mut u64,
    reported: Option<&str>,
    typed: &str,
    output: &str,
    end: &str,
) {
    let mut frame = |bytes: Vec<u8>| {
        *seq += 1;
        authority.output(&bytes, *seq);
    };
    frame(
        format!("\x1b]7;file://host/work/dir\x07\x1b]133;A\x07$ \x1b]133;B\x07{typed}\r\n")
            .into_bytes(),
    );
    let mut start = String::new();
    if let Some(reported) = reported {
        start.push_str(&format!(
            "\x1b]777;openagents;command;{}\x07",
            hex(reported)
        ));
    }
    start.push_str("\x1b]133;C\x07");
    start.push_str(output);
    frame(start.into_bytes());
    frame(end.as_bytes().to_vec());
}

fn page(authority: &Authority, before: Option<u64>, limit: u16) -> BlockPage {
    authority.blocks(before, limit).unwrap().unwrap()
}

#[test]
fn a_finished_command_is_a_block_without_its_output() {
    let mut authority = Authority::new(Size::new(24, 80), 100);
    let mut seq = 0;
    command(
        &mut authority,
        &mut seq,
        None,
        "make test",
        "lots of output\r\n",
        "\x1b]133;D;2\x07",
    );
    let page = page(&authority, None, 8);
    assert_eq!(
        (page.newest, page.oldest, page.more),
        (Some(1), Some(1), false)
    );
    let block = &page.blocks[0];
    assert_eq!(block.block, 1);
    assert_eq!(block.origin, Origin::Unattributed);
    // The typed text, read from between the input and output marks.
    assert_eq!(block.command, "make test");
    assert_eq!(block.dir, "/work/dir");
    assert_eq!(block.status, Some(2));
    assert_eq!(block.state, BlockState::Finished);
    assert_eq!(block.output, Some(SeqRange { from: 2, to: 3 }));
    assert!(block.started.is_some() && block.ended.is_some());
    let lines = block.lines.unwrap();
    assert!(lines.end > lines.start);
    // The record holds no output text.
    assert!(
        !serde_json::to_string(&page)
            .unwrap()
            .contains("lots of output")
    );
}

#[test]
fn the_hook_reported_command_wins_and_marks_without_an_end_are_abandoned() {
    let mut authority = Authority::new(Size::new(24, 80), 100);
    let mut seq = 0;
    command(&mut authority, &mut seq, Some("git status"), "gst", "", "");
    // A new prompt without an end mark abandons the running block.
    command(
        &mut authority,
        &mut seq,
        None,
        "true",
        "",
        "\x1b]133;D;0\x07",
    );
    let page = page(&authority, None, 8);
    assert_eq!(page.blocks.len(), 2);
    assert_eq!(page.blocks[1].command, "git status");
    assert_eq!(page.blocks[1].state, BlockState::Abandoned);
    assert_eq!(page.blocks[1].status, None);
    assert_eq!(page.blocks[0].state, BlockState::Finished);
}

#[test]
fn a_full_screen_command_has_no_output_range() {
    let mut authority = Authority::new(Size::new(24, 80), 100);
    let mut seq = 0;
    command(
        &mut authority,
        &mut seq,
        None,
        "vim",
        "\x1b[?1049hediting",
        "\x1b[?1049l\x1b]133;D;0\x07",
    );
    let block = &page(&authority, None, 8).blocks[0];
    assert!(block.alternate);
    assert_eq!(block.output, None);
    assert_eq!(block.state, BlockState::Finished);
}

#[test]
fn pages_run_newest_first_and_old_blocks_leave_the_journal() {
    let mut authority = Authority::new(Size::new(24, 80), 100);
    let mut seq = 0;
    for n in 0..JOURNAL_MAX + 10 {
        let typed = format!("echo {n}");
        command(
            &mut authority,
            &mut seq,
            None,
            &typed,
            "",
            "\x1b]133;D;0\x07",
        );
    }
    let newest = page(&authority, None, 4);
    let numbers: Vec<u64> = newest.blocks.iter().map(|block| block.block).collect();
    let last = (JOURNAL_MAX + 10) as u64;
    assert_eq!(numbers, vec![last, last - 1, last - 2, last - 3]);
    assert!(newest.more);
    assert_eq!(newest.oldest, Some(11));
    let older = page(&authority, Some(last - 3), 2);
    assert_eq!(older.blocks[0].block, last - 4);
    // The blocks below the oldest kept one are gone.
    let refusal = authority.blocks(Some(11), 4).unwrap().unwrap_err();
    assert_eq!(refusal.reason, Reason::ContentUnavailable);
    let oldest = page(&authority, Some(12), 4);
    assert_eq!(oldest.blocks.len(), 1);
    assert!(!oldest.more);
    assert!(authority.blocks(None, 0).unwrap().is_err());
}

#[test]
fn a_command_line_is_bounded_and_cleaned() {
    let mut authority = Authority::new(Size::new(24, 200), 100);
    let mut seq = 0;
    let long = "x".repeat(3000);
    command(
        &mut authority,
        &mut seq,
        Some(&long),
        "",
        "",
        "\x1b]133;D\x07",
    );
    let block = &page(&authority, None, 1).blocks[0];
    assert_eq!(block.command.len(), coder_pty::ext::BLOCK_TEXT_MAX);
    assert!(block.command_truncated);
    assert_eq!(block.status, None);
}
