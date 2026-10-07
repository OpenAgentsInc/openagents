use super::*;

fn now() -> u64 {
    1_790_000_000
}

fn store(dir: &tempfile::TempDir) -> (Store, Record) {
    let store = Store::new(&dir.path().join("host"), DEFAULT_NAME).unwrap();
    let record = store.open(dir.path(), now()).unwrap();
    (store, record)
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
