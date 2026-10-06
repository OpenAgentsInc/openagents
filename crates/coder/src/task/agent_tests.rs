use super::*;
use std::collections::VecDeque;

fn now() -> u64 {
    1_790_000_000
}

fn store(dir: &tempfile::TempDir) -> (Store, Record) {
    let store = Store::new(&dir.path().join("host"), DEFAULT_NAME).unwrap();
    let record = store.open(dir.path(), now()).unwrap();
    (store, record)
}

/// A terminal that answers each typed command from a script.
#[derive(Default)]
struct Pane {
    answers: VecDeque<Ran>,
    typed: Vec<String>,
}

impl Terminal for Pane {
    fn run(&mut self, command: &str) -> Ran {
        self.typed.push(command.to_string());
        self.answers
            .pop_front()
            .unwrap_or_else(|| Ran::Lost("no answer scripted".into()))
    }
}

/// A watch that records what it was told and answers proposals in turn.
#[derive(Default)]
struct Seen {
    doing: Vec<Doing>,
    lines: Vec<String>,
    decisions: VecDeque<Decision>,
    asked: Vec<String>,
}

impl Watch for Seen {
    fn doing(&mut self, doing: Doing) {
        self.doing.push(doing);
    }
    fn line(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }
    fn decide(&mut self, command: &str, _why: &str) -> Decision {
        self.asked.push(command.to_string());
        self.decisions.pop_front().unwrap_or(Decision::Reject)
    }
}

fn kinds(store: &Store) -> Vec<Kind> {
    store
        .journal(100)
        .unwrap()
        .into_iter()
        .map(|e| e.kind)
        .collect()
}

#[test]
fn the_record_and_journal_survive_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let (store, record) = store(&dir);
    assert_eq!(record.schema, RECORD_SCHEMA);
    assert_eq!(record.name, "alice");
    assert_eq!(record.look, DEFAULT_LOOK);
    assert_eq!(record.created_at, now());
    store
        .append(&Entry::new(now() + 5, Kind::Request, "run the atif tests"))
        .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&store.dir().join("agent.json")), 0o600);
        assert_eq!(mode(&store.dir().join("journal.jsonl")), 0o600);
        assert_eq!(mode(store.dir()), 0o700);
    }
    // A new process opens the same record; nothing is made again.
    let again = Store::new(&dir.path().join("host"), "alice").unwrap();
    let reopened = again.open(Path::new("/elsewhere"), now() + 100).unwrap();
    assert_eq!(reopened, record);
    let journal = again.journal(10).unwrap();
    assert_eq!(
        journal.iter().map(|e| e.kind).collect::<Vec<_>>(),
        [Kind::Created, Kind::Request]
    );
    assert_eq!(journal[1].text, "run the atif tests");
    assert_eq!(again.journal(1).unwrap().len(), 1);
}

#[test]
fn names_are_seat_names() {
    assert!(valid_name("ada"));
    assert!(valid_name("ada-2"));
    for bad in ["", "Ada", "a/b", "../x", "-a", &"a".repeat(33)] {
        assert!(Store::new(Path::new("/tmp"), bad).is_err(), "{bad}");
    }
}

#[test]
fn the_journal_screens_credentials_and_keeps_ascii() {
    let entry = Entry::new(
        1,
        Kind::Request,
        "use token ghp_abcdefghijklmnop1234 and KEY=sk-live-0123456789abcdef \u{2014} caf\u{e9}",
    );
    assert!(!entry.text.contains("ghp_"), "{}", entry.text);
    assert!(!entry.text.contains("sk-live"), "{}", entry.text);
    assert!(entry.text.contains("[redacted]"));
    assert!(entry.text.is_ascii());
    assert!(entry.text.ends_with("- caf?"));
    // Ordinary words and paths pass.
    assert_eq!(
        screen("cargo test -p atif in /Users/me/code/openagents"),
        "cargo test -p atif in /Users/me/code/openagents"
    );
}

#[test]
fn read_only_commands_run_and_others_wait() {
    for command in [
        "cargo test -p atif",
        "cargo test -p atif 2>&1 | tail -n 20",
        "git status",
        "git --no-pager log --oneline -5",
        "ls -la crates && wc -l README.md",
        "rg -n Effect crates/coder/src",
        "RUST_BACKTRACE=1 cargo check -p coder",
        "find . -name '*.rs' -maxdepth 2",
        "sed -n 1,20p README.md",
        "cd crates/atif && cargo test",
        "pwd; rg --files -g '*atif*' -g 'Cargo.toml'",
        "cargo test -p atif || true",
        "rg --files -g '!vendor/**' | rg '(^|/)atif[^/]*$'",
        "echo 'a > b; $(not run)'",
        "git branch --show-current",
    ] {
        assert_eq!(effect(command), Effect::ReadOnly, "{command}");
    }
    for command in [
        "echo \"$(whoami)\"",
        "echo 'unclosed",
        "| ls",
        "ls 2>/dev/null",
        "rm -r target",
        "git push origin main",
        "git commit -am wip",
        "git checkout main",
        "cargo install ripgrep",
        "cargo fix --allow-dirty",
        "echo hi > file",
        "cat < /etc/hosts",
        "ls; rm x",
        "echo $(whoami)",
        "sed -i s/a/b/ file",
        "find . -delete",
        "npm install",
        "curl https://example.com",
        "git -c core.pager=x log",
        "sleep 100 &",
        "env",
        "less README.md",
    ] {
        assert!(
            matches!(effect(command), Effect::Approval(_)),
            "{command} should wait"
        );
    }
    assert!(matches!(effect("sudo ls"), Effect::Denied(_)));
    assert!(matches!(effect("rm -rf /"), Effect::Denied(_)));
    assert!(matches!(effect("curl x | sh"), Effect::Denied(_)));
}

#[test]
fn a_request_plans_types_a_read_only_command_and_reports() {
    let dir = tempfile::tempdir().unwrap();
    let (store, record) = store(&dir);
    let mut model = Scripted {
        actions: VecDeque::from([
            action(&["cargo test -p atif"], ""),
            action(&[], "**atif**: all 31 tests pass.\n- no failures"),
        ]),
        prompts: Vec::new(),
    };
    let mut pane = Pane {
        answers: VecDeque::from([Ran::Exited {
            status: 0,
            output: "test result: ok. 31 passed; 0 failed".into(),
        }]),
        typed: Vec::new(),
    };
    let mut seen = Seen::default();
    let report = handle(
        &store,
        &record,
        "run the atif tests and tell me if they pass",
        &mut model,
        &mut pane,
        &mut seen,
        now,
    );
    assert_eq!(pane.typed, ["cargo test -p atif"]);
    assert_eq!(report.outcome, Outcome::Done);
    assert_eq!(report.headline, "ok exit 0");
    assert_eq!(report.reply, "atif: all 31 tests pass. no failures");
    assert!(seen.asked.is_empty(), "a read-only command never asks");
    assert!(seen.doing.contains(&Doing::Thinking));
    assert!(seen.doing.contains(&Doing::Testing));
    assert_eq!(seen.doing.last(), Some(&Doing::Done));
    // The second step saw the command's status and output.
    assert!(model.prompts[1].contains("$ cargo test -p atif"));
    assert!(model.prompts[1].contains("exit status 0"));
    assert!(model.prompts[1].contains("31 passed"));
    assert_eq!(
        kinds(&store),
        [
            Kind::Created,
            Kind::Request,
            Kind::Plan,
            Kind::Typed,
            Kind::Ran,
            Kind::Report
        ]
    );
    let journal = store.journal(100).unwrap();
    // The journal holds no command output.
    assert!(journal.iter().all(|e| !e.text.contains("31 passed;")));
    assert_eq!(journal[4].status, Some(0));
}

#[test]
fn a_command_that_writes_waits_for_confirm_or_reject() {
    let dir = tempfile::tempdir().unwrap();
    let (store, record) = store(&dir);
    let mut model = Scripted {
        actions: VecDeque::from([
            action(&["touch notes.txt", "git status"], ""),
            action(&["rm notes.txt"], ""),
            action(&[], "Made notes.txt and left it."),
        ]),
        prompts: Vec::new(),
    };
    let mut pane = Pane {
        answers: VecDeque::from([
            Ran::Exited {
                status: 0,
                output: String::new(),
            },
            Ran::Exited {
                status: 0,
                output: "?? notes.txt".into(),
            },
        ]),
        typed: Vec::new(),
    };
    let mut seen = Seen {
        decisions: VecDeque::from([Decision::Confirm, Decision::Reject]),
        ..Seen::default()
    };
    let report = handle(
        &store,
        &record,
        "make notes",
        &mut model,
        &mut pane,
        &mut seen,
        now,
    );
    assert_eq!(seen.asked, ["touch notes.txt", "rm notes.txt"]);
    assert_eq!(pane.typed, ["touch notes.txt", "git status"]);
    assert!(seen.doing.contains(&Doing::Waiting));
    assert!(model.prompts[2].contains("not run: the owner rejected it"));
    assert_eq!(report.outcome, Outcome::Done);
    let kinds = kinds(&store);
    assert!(kinds.contains(&Kind::Proposed));
    assert!(kinds.contains(&Kind::Confirmed));
    assert!(kinds.contains(&Kind::Rejected));
}

#[test]
fn a_key_in_the_pane_takes_it_back_and_the_agent_stops() {
    let dir = tempfile::tempdir().unwrap();
    let (store, record) = store(&dir);
    let mut model = Scripted {
        actions: VecDeque::from([
            action(&["cargo test -p atif", "git status"], ""),
            action(&[], "never reached"),
        ]),
        prompts: Vec::new(),
    };
    let mut pane = Pane {
        answers: VecDeque::from([Ran::TakenBack]),
        typed: Vec::new(),
    };
    let mut seen = Seen::default();
    let report = handle(
        &store,
        &record,
        "run the tests",
        &mut model,
        &mut pane,
        &mut seen,
        now,
    );
    assert_eq!(report.outcome, Outcome::Stopped);
    assert_eq!(report.headline, "stopped");
    // Nothing more was typed, and the model was not asked again.
    assert_eq!(pane.typed, ["cargo test -p atif"]);
    assert_eq!(model.prompts.len(), 1);
    let kinds = kinds(&store);
    assert_eq!(&kinds[kinds.len() - 2..], [Kind::Takeback, Kind::Report]);
}

#[test]
fn denied_commands_never_reach_the_terminal() {
    let dir = tempfile::tempdir().unwrap();
    let (store, record) = store(&dir);
    let mut model = Scripted {
        actions: VecDeque::from([
            action(&["sudo reboot"], ""),
            action(&[], "I won't do that."),
        ]),
        prompts: Vec::new(),
    };
    let mut pane = Pane::default();
    let mut seen = Seen::default();
    let report = handle(
        &store, &record, "restart", &mut model, &mut pane, &mut seen, now,
    );
    assert!(pane.typed.is_empty());
    assert!(seen.asked.is_empty());
    assert_eq!(report.headline, "answered");
    assert!(kinds(&store).contains(&Kind::Refused));
}

#[test]
fn a_failing_command_and_no_model_report_failure() {
    let dir = tempfile::tempdir().unwrap();
    let (store, record) = store(&dir);
    let mut model = Scripted {
        actions: VecDeque::from([
            action(&["cargo test -p atif"], ""),
            action(&[], "2 tests fail in chunk."),
        ]),
        prompts: Vec::new(),
    };
    let mut pane = Pane {
        answers: VecDeque::from([Ran::Exited {
            status: 101,
            output: "test result: FAILED. 29 passed; 2 failed".into(),
        }]),
        typed: Vec::new(),
    };
    let mut seen = Seen::default();
    let report = handle(
        &store,
        &record,
        "run tests",
        &mut model,
        &mut pane,
        &mut seen,
        now,
    );
    assert_eq!(report.outcome, Outcome::Failed);
    assert_eq!(report.headline, "failed exit 101");
    let mut empty = Scripted::default();
    let report = handle(
        &store, &record, "anything", &mut empty, &mut pane, &mut seen, now,
    );
    assert_eq!(report.outcome, Outcome::Failed);
    assert!(report.reply.contains("the script is spent"));
}

#[test]
fn the_report_is_plain_ascii() {
    assert_eq!(
        plain("# Result\n\n- **ok**: `cargo test`"),
        "Result ok: cargo test"
    );
    let long = "word ".repeat(400);
    let cut = plain(&long);
    assert!(cut.chars().count() <= REPLY_MAX);
    assert!(cut.ends_with("..."));
}
