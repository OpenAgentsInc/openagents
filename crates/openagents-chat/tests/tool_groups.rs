//! Tool-call grouping over real runs (#10117).
//!
//! Each fixture is the event stream of one real Coder run on a scratch
//! repository, recorded on 2026-10-01 with `openagents chat send --local
//! --json` on Grok Build, Codex, and Claude Code (home paths replaced with
//! `/Users/me`). The run read three files, searched, listed `src`, built,
//! and read the log, without changing anything. Each test folds the stream
//! the way every surface does and checks what a condensed view shows.

use openagents_chat::coder_events::{CoderEvent, Line, Verb};
use openagents_chat::tool_groups::{Item, Stretch};

const GROK: &str = include_str!("../fixtures/coder-events/tools-grok.ndjson");
const CODEX: &str = include_str!("../fixtures/coder-events/tools-codex.ndjson");
const CLAUDE: &str = include_str!("../fixtures/coder-events/tools-claude.ndjson");

fn lines(text: &str) -> Vec<Line> {
    text.lines()
        .map(|line| serde_json::from_str(line).expect("a coder event line"))
        .collect()
}

/// The run's stretches of tool activity, split where any other event
/// comes, as the surfaces split them.
fn stretches(lines: &[Line]) -> Vec<Stretch> {
    let mut out = Vec::new();
    let mut open = Stretch::default();
    for line in lines {
        if matches!(line.event, CoderEvent::Progress(_)) {
            continue;
        }
        if !open.push(line.seq, &line.event) && !open.is_empty() {
            out.push(std::mem::take(&mut open));
            // The event that ended a stretch may start the next one.
            open.push(line.seq, &line.event);
        }
        if line.event.ends_turn() {
            open.settle();
        }
    }
    if !open.is_empty() {
        out.push(open);
    }
    out
}

/// A condensed view, one string a row.
fn condensed(text: &str) -> Vec<String> {
    stretches(&lines(text))
        .iter()
        .flat_map(|stretch| {
            stretch
                .items(true)
                .into_iter()
                .map(|item| match item {
                    Item::Group { label, .. } => format!("◈ {}", label.line()),
                    Item::Call(shown) => format!("◆ {}", shown.line()),
                    Item::Thought { text, .. } => format!("· {text}"),
                    Item::More { label, .. } => format!("◈ {}", label.line()),
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn grok_builds_looking_calls_fold_under_one_label() {
    assert_eq!(
        condensed(GROK),
        [
            "· Continuing the OpenAgents conversation.",
            "◈ Read 3 files, Listed 1 dir, Searched 2 patterns",
            "◆ Run Build crate and show git history",
            "· I will summarize the crate's purpose in one sentence.",
        ]
    );
    // Every call came typed; none read from its words.
    let lines = lines(GROK);
    let stretch = &stretches(&lines)[1];
    let verbs: Vec<Verb> = stretch.calls().map(|shown| shown.call.verb).collect();
    assert_eq!(
        verbs,
        [
            Verb::Read,
            Verb::Read,
            Verb::Read,
            Verb::List,
            Verb::Search,
            Verb::Search,
            Verb::Run
        ]
    );
    // Expanded, the group lists its calls and keeps what each returned.
    let Item::Group { members, .. } = &stretch.items(false)[0] else {
        panic!("a group first");
    };
    assert_eq!(members.len(), 6);
    let read = stretch.calls().find(|shown| shown.call.target == "lib.rs");
    assert!(read.is_some_and(|shown| shown.output.contains("pub fn add")));
}

#[test]
fn codex_commands_stand_alone_with_the_thoughts_between_them() {
    let rows = condensed(CODEX);
    assert_eq!(rows.len(), 5, "{rows:#?}");
    assert!(rows[0].starts_with("· Inspect the requested files"));
    assert_eq!(
        rows[1],
        "◆ Run pwd; git status --short; ls -la src; rg -n TODO src Cargo.toml || test \"$?\" -eq 1"
    );
    assert!(rows[2].starts_with("· Build an unchanged temporary copy"));
    // A multi-line command shows its first line.
    assert_eq!(
        rows[3],
        "◆ Run build_dir=$(mktemp -d \"${TMPDIR:-/tmp}/demo-build.XXXXXX\")"
    );
    assert!(rows[4].starts_with("· All requested inspection"));
    // Each command got its own output, matched by the command.
    let lines = lines(CODEX);
    for stretch in stretches(&lines) {
        for shown in stretch.calls() {
            assert!(!shown.running && !shown.failed());
            assert!(!shown.output.starts_with("exit 0 in"), "{shown:?}");
        }
    }
}

#[test]
fn claude_codes_batch_of_commands_matches_each_output() {
    assert_eq!(
        condensed(CLAUDE),
        [
            "· Run the read-only inspection commands requested; view the three files.",
            "◆ Run grep -rn TODO --exclude-dir=target --exclude-dir=.git . || echo 'no TODO found'",
            "◆ Run ls -la src",
            "◆ Run cargo build 2>&1 | tail -30",
            "◆ Run git log --oneline | head -30",
            "◆ Run git status --short",
            "· All requested inspection done; report findings.",
        ]
    );
    let lines = lines(CLAUDE);
    let stretch = &stretches(&lines)[0];
    let log = stretch
        .calls()
        .find(|shown| shown.call.target.starts_with("git log"))
        .unwrap();
    assert_eq!(log.output, "45f98b7 init");
    let status = stretch
        .calls()
        .find(|shown| shown.call.target == "git status --short")
        .unwrap();
    assert!(status.output.contains("?? Cargo.lock"), "{status:?}");
}

/// Whatever the engine, a stretch never folds a command into a group, and
/// the label counts exactly the looking calls it holds.
#[test]
fn every_engine_groups_only_looking_calls() {
    for text in [GROK, CODEX, CLAUDE] {
        let lines = lines(text);
        for stretch in stretches(&lines) {
            for item in stretch.items(false) {
                if let Item::Group { members, .. } = item {
                    for member in members {
                        if let openagents_chat::tool_groups::Entry::Call(shown) = member {
                            assert!(openagents_chat::tool_groups::looks(shown.call.verb));
                        }
                    }
                }
            }
        }
    }
}
