//! Behaviour that landed on `main` while the markdown port was in flight, and
//! that the port's rewrite of `tui.rs` / `interactive.rs` could quietly undo.
//!
//! Each test names the commit it defends. These are cheap and they are here
//! because "it still compiles" is not evidence that a feature survived a
//! conflict resolution.

use openagents_cli::coder::export::git_info;
use openagents_cli::coder::tui::{CoderUi, Entry, Role, ToolCall, now_ms};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn draw(ui: &mut CoderUi, w: u16, h: u16) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal
        .draw(|f| {
            let area = f.area();
            ui.render(f, area);
        })
        .unwrap();
    let buf = terminal.backend().buffer().clone();
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf.cell((x, y)).unwrap().symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

/// The fields `/export` serializes off one entry.
type ExportPayload = (Role, String, Option<String>, u64, Option<String>);

fn payload(entry: &Entry) -> ExportPayload {
    (
        entry.role.clone(),
        entry.text.clone(),
        entry.output.clone(),
        entry.at,
        entry.tool.as_ref().map(|t| t.call_id.clone()),
    )
}

fn delegate_call() -> ToolCall {
    ToolCall {
        call_id: "call-7".to_string(),
        function_name: "delegate".to_string(),
        arguments: serde_json::json!({ "agent": "devin", "task": "Read src/main.rs" }),
        output: None,
        error: None,
        done: false,
    }
}

// ── 727ab02ece: Remove leading spaces from Notice/Reasoning bullet ──

#[test]
fn notice_and_reasoning_bullets_sit_flush_left() {
    let mut ui = CoderUi::new();
    ui.entries
        .push(Entry::new(Role::Notice, "found ACP agents: cursor, devin"));
    ui.entries
        .push(Entry::new(Role::Reasoning, "thinking about it"));
    let lines = draw(&mut ui, 80, 24);

    for role in ["found ACP agents", "thinking about it"] {
        let line = lines
            .iter()
            .find(|l| l.contains(role))
            .unwrap_or_else(|| panic!("{role:?} was not rendered: {lines:?}"));
        assert!(
            line.starts_with("⏺ "),
            "the bullet is indented again; 727ab02ece put it at column 0: {line:?}"
        );
    }
}

#[test]
fn a_wrapped_notice_continues_at_the_two_column_indent() {
    // 727ab02ece moved the continuation indent from five columns to two along
    // with the bullet. A revert shows up here as a wider hanging indent.
    let mut ui = CoderUi::new();
    ui.entries.push(Entry::new(
        Role::Notice,
        "alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima",
    ));
    let lines = draw(&mut ui, 30, 24);
    let first = lines.iter().position(|l| l.starts_with("⏺ ")).unwrap();
    let second = &lines[first + 1];
    assert!(
        second.starts_with("  ") && !second.starts_with("   "),
        "continuation indent changed: {second:?}"
    );
}

// ── 2185306d80: Port /export command from TypeScript TUI ──

#[test]
fn entries_built_by_the_constructors_carry_what_export_reads() {
    // `/export` reads `at`, `tool`, `output`, `text` and `role` off each
    // entry. The port replaced every struct literal with a constructor, so
    // this is where a dropped field would show up.
    let before = now_ms();

    let you = Entry::new(Role::You, "explain rust");
    let assistant = Entry::new(Role::Assistant, "Rust is a systems language.");
    let mut tool = Entry::tool_call("delegate devin: Read src/main.rs");
    tool.tool = Some(delegate_call());
    tool.output = Some("Reading file...\nDone".to_string());

    for entry in [&you, &assistant, &tool] {
        assert!(
            entry.at >= before,
            "a constructor left `at` unstamped, so every export step would \
             carry the epoch: {}",
            entry.at
        );
    }
    assert!(you.tool.is_none() && assistant.tool.is_none());
    assert_eq!(tool.tool.as_ref().unwrap().call_id, "call-7");
    assert_eq!(tool.tool.as_ref().unwrap().function_name, "delegate");
    assert_eq!(tool.output.as_deref(), Some("Reading file...\nDone"));
}

#[test]
fn rendering_a_transcript_does_not_disturb_its_export_payload() {
    // `render_entry` now takes `&mut Entry` so the streaming renderer can live
    // on the entry. It must not touch anything `/export` serializes.
    let mut ui = CoderUi::new();
    ui.entries.push(Entry::new(Role::You, "explain rust"));
    ui.entries.push(Entry::new(
        Role::Assistant,
        "Rust is a **systems** language.",
    ));
    let mut tool = Entry::tool_call("delegate devin: Read src/main.rs");
    tool.tool = Some(delegate_call());
    tool.output = Some("Reading file...\nDone".to_string());
    ui.entries.push(tool);

    let snapshot: Vec<ExportPayload> = ui.entries.iter().map(payload).collect();

    for _ in 0..3 {
        let _ = draw(&mut ui, 80, 24);
    }

    let after: Vec<ExportPayload> = ui.entries.iter().map(payload).collect();

    assert_eq!(snapshot, after, "rendering mutated the export payload");
}

#[test]
fn git_info_still_resolves_the_repository() {
    // `/export` labels the document with these. They are read once at startup
    // in `run_tui`, which the port also touched.
    let (repo, branch) = git_info().expect("git_info returned nothing inside a git checkout");
    assert!(!repo.is_empty());
    assert!(!branch.is_empty());
}

// ── c9a6c21bbd: Surface ACP delegate errors in the TUI ──

#[test]
fn a_delegate_error_is_visible_in_the_tool_box() {
    // `runtime.rs` sends `ToolTitle("error")` then `ToolText(<message>)`. Both
    // land on the entry's header and output box, which the port re-rendered.
    let mut ui = CoderUi::new();
    let mut tool = Entry::tool_call("delegate devin: error");
    tool.output = Some("acp child exited with status 127: command not found".to_string());
    ui.entries.push(tool);

    let lines = draw(&mut ui, 80, 24);
    let screen = lines.join("\n");
    assert!(
        screen.contains("delegate devin: error"),
        "the error header vanished:\n{screen}"
    );
    assert!(
        screen.contains("command not found"),
        "the delegate error message was swallowed:\n{screen}"
    );
    assert!(
        lines.iter().any(|l| l.starts_with("│ ")),
        "the output box border is gone:\n{screen}"
    );
}

#[test]
fn a_long_delegate_error_sweeps_to_and_keeps_its_last_lines() {
    // The box starts at the top, then settles on the trailing four lines,
    // which is where a stack trace's actual cause lives.
    let mut ui = CoderUi::new();
    let mut tool = Entry::tool_call("delegate devin: error");
    tool.output = Some(
        (1..=9)
            .map(|i| format!("frame {i}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    ui.entries.push(tool);

    let first_screen = draw(&mut ui, 80, 24).join("\n");
    assert!(
        first_screen.contains("frame 1"),
        "the sweep did not start at the top:\n{first_screen}"
    );

    for _ in 0..24 {
        draw(&mut ui, 80, 24);
    }
    let screen = draw(&mut ui, 80, 24).join("\n");
    assert!(screen.contains("frame 9"), "lost the last line:\n{screen}");
    assert!(screen.contains("frame 6"), "lost the window:\n{screen}");
    assert!(
        !screen.contains("frame 1\n"),
        "the box grew past four lines:\n{screen}"
    );
}
