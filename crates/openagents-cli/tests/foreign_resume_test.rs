//! `/resume`: the foreign-session picker, and what it refuses to make up.
//!
//! Three bands of test, in order of how much they can prove:
//!
//! 1. **Port fidelity.** The packet, the listing, the selection, and the four
//!    soft-failure shapes, against a fake scanner. These mirror
//!    `packages/openagents-cli/test/coder-foreign-resume.test.ts` so the two
//!    pickers cannot drift.
//! 2. **The file on disk.** A staged store where the file is present, absent,
//!    or present with a different session in it. A resume command is printed
//!    only in the first case.
//! 3. **The real machine.** The shipped `foreign_sessions` artifact, loaded and
//!    invoked for real over `~/.claude` and `~/.codex`, and every field the
//!    picker prints checked against the bytes of the session file itself —
//!    reopened and reparsed here, not taken from the scanner. Then the same
//!    real session with its id altered, which must be refused.
//!
//! Band 3 is the one that matters. A picker that returned a plausible listing
//! would pass band 1 and 2 and fail band 3, which is the failure this crate has
//! actually shipped: hardcoded sessions that scanned nothing.

use openagents_cli::foreign_resume::{
    DEFAULT_MAX_AGE_DAYS, DEFAULT_PICKER_LIMIT, ForeignResumeDeps, ForeignResumeOptions,
    ForeignScanOutput, ForeignSession, ForeignSource, OnDisk, ScanResult, build_packet,
    confirm_on_disk, foreign_resume_turn, format_age, load_scanner, normalize_scan_result,
    run_foreign_resume, scanner_invoke,
};
use openagents_cli::interactive::{CoderApp, Control};
use openagents_cli::runtime::Lane;
use openagents_cli::trace::redact_text;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc::unbounded_channel;

const DAY_MS: i64 = 86_400_000;
const HOUR_MS: i64 = 3_600_000;
const NOW_MS: i64 = 1_000_000_000_000;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("the crate sits inside the repository")
}

fn deps(selection: Option<usize>) -> ForeignResumeDeps {
    ForeignResumeDeps {
        now_ms: NOW_MS,
        cwd: "/test/cwd".to_string(),
        selection,
        home: "/Users/ada".to_string(),
        mount_roots: Vec::new(),
    }
}

/// A scanner that answers with a fixed packet and records what it was asked.
fn fixed(output: Value) -> impl Fn(&Value) -> Result<Value, String> {
    move |_packet: &Value| Ok(output.clone())
}

fn session(overrides: Value) -> Value {
    let mut base = json!({
        "source": "claude",
        "session_id": "abc-123",
        "path": "projects/abc.jsonl",
        "mtime_ms": NOW_MS,
        "size_bytes": 100,
        "record_count": 1,
        "metadata_truncated": false,
    });
    for (key, value) in overrides.as_object().expect("an object of overrides") {
        base[key] = value.clone();
    }
    base
}

// ───────────────────────────────────────────────────────────── band 1: the port

#[test]
fn the_packet_carries_the_cwd_filter_now_and_both_bounds() {
    let packet = build_packet(&deps(None), &ForeignResumeOptions::default());

    assert_eq!(packet["cwd_filter"], "/test/cwd");
    assert_eq!(packet["now_ms"], NOW_MS);
    assert_eq!(packet["max_age_days"], DEFAULT_MAX_AGE_DAYS);
    assert_eq!(packet["limit"], DEFAULT_PICKER_LIMIT as u64);
    // Sources are left unset so the guest scans its two defaults.
    assert!(packet.get("sources").is_none(), "{packet}");
}

#[test]
fn the_listing_is_numbered_in_the_order_the_scanner_gave() {
    let answer = json!({"ok": {"sessions": [
        session(json!({"source": "codex", "session_id": "codex-newest",
                       "cwd": "/Users/ada/gamma", "mtime_ms": NOW_MS - DAY_MS, "record_count": 3})),
        session(json!({"source": "claude", "session_id": "claude-older",
                       "cwd": "/Users/ada/alpha", "mtime_ms": NOW_MS - 3 * DAY_MS, "record_count": 7})),
    ]}});
    let out = run_foreign_resume(
        &deps(None),
        &fixed(answer),
        &ForeignResumeOptions::default(),
    );

    assert!(
        out.contains("Recent foreign sessions for this directory (/test/cwd):"),
        "{out}"
    );
    assert!(out.contains(" 1. "), "{out}");
    assert!(out.contains(" 2. "), "{out}");
    let newest = out.find("codex-newest").expect("the newest is listed");
    let older = out.find("claude-older").expect("the older is listed");
    assert!(
        newest < older,
        "the scanner's order was not preserved:\n{out}"
    );
    assert!(out.contains("1 day ago"), "{out}");
    assert!(out.contains("3 days ago"), "{out}");
    assert!(out.contains("Run /resume <number>"), "{out}");
}

#[test]
fn an_empty_listing_says_which_store_could_not_be_read() {
    let answer = json!({"ok": {"sessions": [], "missing_sources": ["claude", "codex"]}});
    let out = run_foreign_resume(
        &deps(None),
        &fixed(answer),
        &ForeignResumeOptions::default(),
    );

    assert!(
        out.contains("No recent foreign sessions were found"),
        "{out}"
    );
    assert!(out.contains("claude or codex state store"), "{out}");
}

#[test]
fn a_partial_scan_says_so_rather_than_reading_as_the_whole_picture() {
    let answer = json!({"ok": {
        "sessions": [session(json!({"session_id": "one"}))],
        "scan_truncated": true,
        "read_budget_exhausted": true,
    }});
    let out = run_foreign_resume(
        &deps(None),
        &fixed(answer),
        &ForeignResumeOptions::default(),
    );

    assert!(
        out.contains("The scan hit a bound and may be partial."),
        "{out}"
    );
    assert!(out.contains("file-read budget was exhausted"), "{out}");
}

#[test]
fn a_session_over_the_read_bound_is_flagged_as_metadata_only() {
    let answer = json!({"ok": {"sessions": [session(json!({
        "session_id": "huge", "record_count": null, "metadata_truncated": true,
    }))]}});
    let out = run_foreign_resume(
        &deps(None),
        &fixed(answer),
        &ForeignResumeOptions::default(),
    );

    assert!(out.contains("huge"), "{out}");
    assert!(out.contains("metadata only"), "{out}");
    assert!(out.contains("truncated"), "{out}");
    assert!(out.contains("(cwd unknown)"), "{out}");
}

#[test]
fn a_source_this_picker_cannot_resume_is_counted_rather_than_dropped_in_silence() {
    // The guest knows four stores; this manifest mounts two, and the picker
    // builds a resume command for two. A row it cannot resume is accounted for.
    let answer = json!({"ok": {"sessions": [
        session(json!({"session_id": "keep"})),
        session(json!({"source": "opencode", "session_id": "drop-1"})),
        session(json!({"source": "opencode", "session_id": "drop-2"})),
    ]}});
    let out = run_foreign_resume(
        &deps(None),
        &fixed(answer),
        &ForeignResumeOptions::default(),
    );

    assert!(out.contains("keep"), "{out}");
    assert!(
        !out.contains("drop-1"),
        "an unresumable row was listed:\n{out}"
    );
    assert!(
        out.contains("2 sessions from `opencode` were left out"),
        "the dropped rows were not accounted for:\n{out}"
    );
}

#[test]
fn an_out_of_range_pick_is_refused_and_the_list_comes_back() {
    let answer = json!({"ok": {"sessions": [
        session(json!({"session_id": "one"})),
        session(json!({"session_id": "two"})),
    ]}});
    let out = run_foreign_resume(
        &deps(Some(9)),
        &fixed(answer),
        &ForeignResumeOptions::default(),
    );

    assert!(out.contains("There is no session at 9"), "{out}");
    assert!(out.contains("Choose a number from 1 to 2."), "{out}");
    assert!(out.contains("Recent foreign sessions"), "{out}");
}

#[test]
fn the_four_soft_failures_each_say_what_happened() {
    let refusal = run_foreign_resume(
        &deps(None),
        &fixed(json!({"refusal": {"code": "unsupported", "reason": "unknown source"}})),
        &ForeignResumeOptions::default(),
    );
    assert!(
        refusal.contains("The scanner refused (unsupported)"),
        "{refusal}"
    );
    assert!(refusal.contains("unknown source"), "{refusal}");

    let malformed = run_foreign_resume(
        &deps(None),
        &fixed(json!({"refusal": {"code": 123}})),
        &ForeignResumeOptions::default(),
    );
    assert!(malformed.contains("malformed refusal"), "{malformed}");

    let unknown = run_foreign_resume(
        &deps(None),
        &fixed(json!({"unexpected": true})),
        &ForeignResumeOptions::default(),
    );
    assert!(unknown.contains("unrecognised packet"), "{unknown}");

    let trapped = run_foreign_resume(
        &deps(None),
        &|_packet: &Value| Err("worker trap".to_string()),
        &ForeignResumeOptions::default(),
    );
    assert!(trapped.contains("The scanner could not run"), "{trapped}");
    assert!(trapped.contains("worker trap"), "{trapped}");
}

#[test]
fn ages_read_the_way_the_typescript_reports_them() {
    assert_eq!(format_age(NOW_MS - 5 * DAY_MS, NOW_MS), "5 days ago");
    assert_eq!(format_age(NOW_MS - DAY_MS, NOW_MS), "1 day ago");
    assert_eq!(format_age(NOW_MS - 3 * HOUR_MS, NOW_MS), "3 hours ago");
    assert_eq!(format_age(NOW_MS - 1000, NOW_MS), "just now");
}

// ────────────────────────────────────────────────────── band 2: the file on disk

/// Stage a Claude-shaped store and return its root.
fn stage_claude_store(dir: &Path, relative: &str, session_id: &str, cwd: &str) -> PathBuf {
    let root = dir.join("store");
    let file = root.join(relative);
    std::fs::create_dir_all(file.parent().expect("a parent")).expect("the store is created");
    let body = format!(
        "{}\n{}\n",
        json!({"type": "queue-operation", "sessionId": session_id}),
        json!({"type": "user", "sessionId": session_id, "cwd": cwd}),
    );
    std::fs::write(&file, body).expect("the session file is written");
    root
}

fn staged_session(relative: &str, session_id: &str, cwd: Option<&str>) -> ForeignSession {
    ForeignSession {
        source: ForeignSource::Claude,
        session_id: session_id.to_string(),
        path: relative.to_string(),
        cwd: cwd.map(str::to_string),
        project_dir: None,
        mtime_ms: NOW_MS - DAY_MS,
        size_bytes: 200,
        record_count: Some(2),
        metadata_truncated: false,
    }
}

fn selection_over(root: &Path, home: &str, session: &Value) -> String {
    let mut deps = deps(Some(1));
    deps.home = home.to_string();
    deps.mount_roots = vec![root.to_path_buf()];
    run_foreign_resume(
        &deps,
        &fixed(json!({"ok": {"sessions": [session.clone()]}})),
        &ForeignResumeOptions::default(),
    )
}

#[test]
fn a_session_whose_file_carries_the_reported_id_gets_a_resume_command() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let root = stage_claude_store(
        dir.path(),
        "projects/-Users-ada-alpha/s-1.jsonl",
        "s-1",
        "/Users/ada/alpha",
    );

    let out = selection_over(
        &root,
        "/Users/ada",
        &session(json!({
            "session_id": "s-1",
            "path": "projects/-Users-ada-alpha/s-1.jsonl",
            "cwd": "/Users/ada/alpha",
        })),
    );

    assert!(out.contains("source:      claude"), "{out}");
    assert!(out.contains("session id:  s-1"), "{out}");
    assert!(out.contains("cwd:         ~/alpha"), "{out}");
    assert!(
        out.contains("the session id was read back out of this file's own records"),
        "{out}"
    );
    // `$HOME` and not `~`: inside double quotes a tilde is a literal, and a
    // `cd` to a directory named `~` is not where the session ran.
    assert!(
        out.contains("cd \"$HOME/alpha\" && claude --resume s-1"),
        "{out}"
    );
}

#[test]
fn a_codex_session_gets_the_codex_verb() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let root = dir.path().join("store");
    let file = root.join("sessions/2026/08/26/rollout-x.jsonl");
    std::fs::create_dir_all(file.parent().expect("a parent")).expect("the store is created");
    std::fs::write(
        &file,
        format!(
            "{}\n",
            json!({"type": "session_meta", "payload": {"id": "roll-9", "cwd": "/srv/build"}})
        ),
    )
    .expect("the rollout is written");

    let out = selection_over(
        &root,
        "/Users/ada",
        &session(json!({
            "source": "codex",
            "session_id": "roll-9",
            "path": "sessions/2026/08/26/rollout-x.jsonl",
            "cwd": "/srv/build",
        })),
    );

    assert!(
        out.contains("cd \"/srv/build\" && codex resume roll-9"),
        "{out}"
    );
}

#[test]
fn a_session_whose_file_is_gone_is_refused_by_name_with_the_path() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let root = stage_claude_store(
        dir.path(),
        "projects/p/s-1.jsonl",
        "s-1",
        "/Users/ada/alpha",
    );

    let out = selection_over(
        &root,
        "/Users/ada",
        &session(json!({
            "session_id": "s-2",
            "path": "projects/p/s-2.jsonl",
            "cwd": "/Users/ada/alpha",
        })),
    );

    assert!(
        out.contains("projects/p/s-2.jsonl"),
        "the path was not named:\n{out}"
    );
    assert!(out.contains("no mounted store holds it"), "{out}");
    assert!(
        out.contains("Nothing is resumed from a path that is not there."),
        "{out}"
    );
    assert!(
        !out.contains("claude --resume"),
        "a command was printed for a file that is not there:\n{out}"
    );
}

#[test]
fn a_file_that_holds_a_different_session_does_not_become_a_resume_command() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    // The file exists at the reported path and holds someone else's session.
    let root = stage_claude_store(
        dir.path(),
        "projects/p/s-1.jsonl",
        "other-session",
        "/Users/ada/alpha",
    );

    let out = selection_over(
        &root,
        "/Users/ada",
        &session(json!({
            "session_id": "s-1",
            "path": "projects/p/s-1.jsonl",
            "cwd": "/Users/ada/alpha",
        })),
    );

    assert!(
        out.contains("does not carry the session id the scanner reported"),
        "{out}"
    );
    assert!(out.contains("s-1"), "{out}");
    assert!(
        !out.contains("claude --resume"),
        "a command was printed for a session the file does not hold:\n{out}"
    );
}

#[test]
fn a_reported_path_cannot_climb_out_of_the_mounted_store() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let root = dir.path().join("store");
    std::fs::create_dir_all(&root).expect("the store is created");
    std::fs::write(
        dir.path().join("outside.jsonl"),
        "{\"sessionId\":\"s-1\"}\n",
    )
    .expect("the outside file is written");

    let escaped = staged_session("../outside.jsonl", "s-1", Some("/Users/ada/alpha"));
    match confirm_on_disk(&escaped, std::slice::from_ref(&root)) {
        OnDisk::Missing { relative, .. } => assert_eq!(relative, "../outside.jsonl"),
        other => panic!("a path that leaves the store was resolved: {other:?}"),
    }

    let absolute = staged_session("/etc/passwd", "s-1", None);
    assert!(matches!(
        confirm_on_disk(&absolute, &[root]),
        OnDisk::Missing { .. }
    ));
}

#[test]
fn an_oversized_file_says_the_id_came_from_its_name() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    // The scanner never opened this one, so the id is the stem and the body is
    // deliberately not a match for it.
    let root = stage_claude_store(
        dir.path(),
        "projects/p/big.jsonl",
        "unrelated",
        "/Users/ada/x",
    );

    let out = selection_over(
        &root,
        "/Users/ada",
        &session(json!({
            "session_id": "big",
            "path": "projects/p/big.jsonl",
            "cwd": null,
            "record_count": null,
            "metadata_truncated": true,
        })),
    );

    assert!(out.contains("metadata:    truncated"), "{out}");
    assert!(
        out.contains("the session id is its file name and nothing was read from inside it"),
        "{out}"
    );
    assert!(out.contains("records:     (unknown)"), "{out}");
    assert!(out.contains("claude --resume big"), "{out}");
    assert!(
        !out.contains("cd \""),
        "a cwd was invented for a file nobody read:\n{out}"
    );
}

// ────────────────────────────────────────────────────────── band 2b: redaction

#[derive(serde::Deserialize)]
struct PlantedSecret {
    label: String,
    credential: bool,
    raw: String,
    leak: String,
}

#[derive(serde::Deserialize)]
struct PlantedSecrets {
    secrets: Vec<PlantedSecret>,
}

/// The one place a token family is written down. Restating the patterns here is
/// how `oa_pat_` leaked out of the TypeScript redactor in the first place.
fn planted_credentials() -> Vec<PlantedSecret> {
    let path = repo_root().join("fixtures/redaction/planted-secrets.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "the shared redaction fixture is unreadable at {}: {error}",
            path.display()
        )
    });
    let parsed: PlantedSecrets = serde_json::from_str(&text).expect("the fixture is JSON");
    parsed
        .secrets
        .into_iter()
        .filter(|s| s.credential)
        .collect()
}

#[test]
fn no_planted_credential_survives_into_the_listing_or_the_selection() {
    let credentials = planted_credentials();
    assert!(
        credentials.len() >= 16,
        "the fixture yielded {} credentials, too few to be the real file",
        credentials.len()
    );

    let dir = tempfile::tempdir().expect("a temporary directory");
    let mut survivors: Vec<String> = Vec::new();

    for entry in &credentials {
        // A foreign session file is untrusted content: the working directory
        // and the session id are whatever some other agent wrote there.
        let planted = session(json!({
            "session_id": "s-1",
            "path": "projects/p/s-1.jsonl",
            "cwd": entry.raw,
        }));
        let root = stage_claude_store(dir.path(), "projects/p/s-1.jsonl", "s-1", &entry.raw);

        let mut listing_deps = deps(None);
        listing_deps.home = "/Users/octavia".to_string();
        listing_deps.mount_roots = vec![root.clone()];
        let listing = run_foreign_resume(
            &listing_deps,
            &fixed(json!({"ok": {"sessions": [planted.clone()]}})),
            &ForeignResumeOptions::default(),
        );
        let selection = selection_over(&root, "/Users/octavia", &planted);

        if listing.contains(&entry.leak) || selection.contains(&entry.leak) {
            survivors.push(entry.label.clone());
        }
        // And a working directory the rules had to touch never becomes a `cd`.
        assert!(
            !selection.contains("cd \""),
            "{} produced a `cd` out of a redacted working directory:\n{selection}",
            entry.label
        );
    }

    assert!(
        survivors.is_empty(),
        "these planted credentials reached the transcript through /resume: {}",
        survivors.join(", ")
    );
}

#[test]
fn a_working_directory_that_would_run_a_command_never_reaches_the_command_line() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let hostile = "/tmp/x\"; rm -rf ~; #";
    let root = stage_claude_store(dir.path(), "projects/p/s-1.jsonl", "s-1", hostile);

    let out = selection_over(
        &root,
        "/Users/ada",
        &session(json!({"session_id": "s-1", "path": "projects/p/s-1.jsonl", "cwd": hostile})),
    );

    assert!(out.contains("cannot be safely quoted"), "{out}");
    // The recorded value is still shown — as an escaped literal, so it reads as
    // data rather than as the line above it, which says `cd`.
    assert!(
        out.contains(r#"cwd:         "/tmp/x\"; rm -rf ~; #""#),
        "the recorded working directory was not shown as a literal:\n{out}"
    );
    // Exactly one line in the output is something to run, and it is the one
    // built from fields that passed the shape checks.
    let runnable: Vec<&str> = out
        .lines()
        .filter(|line| {
            line.starts_with("  ") && (line.contains("cd ") || line.contains("--resume"))
        })
        .collect();
    assert_eq!(
        runnable,
        vec!["  claude --resume s-1"],
        "a shell payload reached a printed command:\n{out}"
    );
}

// ─────────────────────────────────────────────────── band 3: this real machine

/// The id and working directory a session file itself records, parsed here
/// rather than taken from the scanner. Claude writes `sessionId` on a
/// top-level record; Codex writes `payload.id` on a `session_meta` first line.
fn read_back(path: &Path) -> (Option<String>, Option<String>) {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut id = None;
    let mut cwd = None;
    for line in text.lines().take(20) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if id.is_none() {
            if let Some(found) = value.get("sessionId").and_then(Value::as_str) {
                id = Some(found.to_string());
            } else if value.get("type").and_then(Value::as_str) == Some("session_meta") {
                id = value
                    .get("payload")
                    .and_then(|p| p.get("id"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
        }
        if cwd.is_none() {
            if let Some(found) = value.get("cwd").and_then(Value::as_str) {
                cwd = Some(found.to_string());
            } else if let Some(found) = value
                .get("payload")
                .and_then(|p| p.get("cwd"))
                .and_then(Value::as_str)
            {
                cwd = Some(found.to_string());
            }
        }
        if id.is_some() && cwd.is_some() {
            break;
        }
    }
    (id, cwd)
}

/// One real scan of this machine's stores: the raw packet and the parsed one.
///
/// `None` when the artifact is not beside the crate (a published crate carries
/// no `plugins/`) or the stores are not on this machine.
fn real_scan(home: &Path) -> Option<(Value, ForeignScanOutput, Vec<PathBuf>)> {
    let root = repo_root();
    if !root
        .join("plugins/foreign-sessions/manifest.json")
        .is_file()
    {
        return None;
    }
    let plugin = match load_scanner(&root) {
        Ok(plugin) => plugin,
        // A machine with no `~/.claude` or `~/.codex` refuses the mount at
        // load. There is nothing to check, rather than something to fail.
        Err(_) => return None,
    };
    let deps = ForeignResumeDeps {
        now_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_millis() as i64),
        // No filter: every session either store holds, newest first.
        cwd: String::new(),
        selection: None,
        home: home.to_string_lossy().into_owned(),
        mount_roots: plugin.mounts.clone(),
    };
    let options = ForeignResumeOptions {
        max_age_days: Some(36_500.0),
        limit: Some(10),
    };
    let raw = scanner_invoke(&plugin)(&build_packet(&deps, &options))
        .expect("the shipped scanner answers with a packet");
    let output = match normalize_scan_result(&raw) {
        ScanResult::Ok(output) => *output,
        other => panic!("the shipped scanner did not answer with a scan: {other:?}"),
    };
    Some((raw, output, plugin.mounts.clone()))
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
}

/// The newest reported session the scanner actually opened, and its 1-based
/// pick number.
///
/// The newest session on a working machine is often the one being written right
/// now, which is over the per-file read bound and carries only its file name.
/// The content checks below need one whose id and cwd came out of the records,
/// so they ask for that rather than settling for whichever row is first.
fn newest_readable(output: &ForeignScanOutput) -> Option<(usize, ForeignSession)> {
    output
        .sessions
        .iter()
        .enumerate()
        .find(|(_, session)| !session.metadata_truncated)
        .map(|(index, session)| (index + 1, session.clone()))
}

#[test]
fn the_real_scanner_reports_sessions_that_are_actually_on_this_machine() {
    let home = home();
    let Some((_, output, roots)) = real_scan(&home) else {
        eprintln!("skipped: no shipped scanner or no foreign store on this machine");
        return;
    };
    if output.sessions.is_empty() {
        eprintln!("skipped: this machine's foreign stores hold no sessions");
        return;
    }

    // Every reported session resolves to a file that is there and holds it.
    for session in &output.sessions {
        match confirm_on_disk(session, &roots) {
            OnDisk::Confirmed { path } => {
                let (id, _) = read_back(&path);
                assert_eq!(
                    id.as_deref(),
                    Some(session.session_id.as_str()),
                    "{} does not record the id the scanner reported",
                    path.display()
                );
            }
            OnDisk::FromFileName { path } => {
                let name = path
                    .file_name()
                    .expect("a name")
                    .to_string_lossy()
                    .into_owned();
                assert_eq!(
                    name.strip_suffix(".jsonl").unwrap_or(&name),
                    session.session_id,
                    "an oversized session's id is not its file name"
                );
            }
            other => {
                panic!("the scanner reported a session that is not on disk as reported: {other:?}")
            }
        }
    }
}

#[test]
fn resuming_a_real_session_prints_what_that_session_file_actually_holds() {
    let home = home();
    let Some((raw, output, roots)) = real_scan(&home) else {
        eprintln!("skipped: no shipped scanner or no foreign store on this machine");
        return;
    };
    let Some((pick, first)) = newest_readable(&output) else {
        eprintln!("skipped: no session on this machine was inside the scanner's read bound");
        return;
    };

    // The renderer runs against the packet the real scan produced, so the
    // scan is not repeated — but the file it names is opened for real below.
    let picked = ForeignResumeDeps {
        now_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_millis() as i64),
        cwd: String::new(),
        selection: Some(pick),
        home: home.to_string_lossy().into_owned(),
        mount_roots: roots.clone(),
    };
    let rendered = run_foreign_resume(
        &picked,
        &fixed(raw.clone()),
        &ForeignResumeOptions {
            max_age_days: Some(36_500.0),
            limit: Some(10),
        },
    );

    let path = match confirm_on_disk(&first, &roots) {
        OnDisk::Confirmed { path } => path,
        other => panic!("a session the scanner read is not confirmed on disk: {other:?}"),
    };
    let (file_id, file_cwd) = read_back(&path);
    let home_text = home.to_string_lossy().into_owned();

    eprintln!("--- /resume {pick} against this machine ---\n{rendered}\n---");
    assert!(rendered.starts_with("Resume context:"), "{rendered}");
    assert!(
        rendered.contains(&format!("session id:  {}", first.session_id)),
        "the printed id is not the reported one:\n{rendered}"
    );
    assert!(
        rendered.contains(&format!(
            "file:        {}",
            redact_text(&path.to_string_lossy(), &home_text).text
        )),
        "the printed file is not the file that was confirmed:\n{rendered}"
    );

    assert!(
        rendered.contains("the session id was read back out of this file's own records"),
        "{rendered}"
    );

    let file_id = file_id.expect("a confirmed session file records its id");
    assert_eq!(
        file_id, first.session_id,
        "the scanner's id and the file's own id disagree"
    );
    assert!(
        rendered.contains(&format!("{} {file_id}", first.source.resume_verb())),
        "the resume command does not name the id the file records:\n{rendered}"
    );

    let file_cwd = file_cwd.expect("a confirmed session file records its cwd");
    assert_eq!(
        first.cwd.as_deref(),
        Some(file_cwd.as_str()),
        "the scanner's cwd and the file's own cwd disagree"
    );
    // The rendered cwd is the file's, redacted. Assert on the last path segment
    // too, which the rules leave alone, so this is not just the redactor
    // agreeing with itself.
    assert!(
        rendered.contains(&format!(
            "cwd:         {}",
            redact_text(&file_cwd, &home_text).text
        )),
        "the printed cwd is not the file's own:\n{rendered}"
    );
    if let Some(last) = file_cwd.rsplit('/').next().filter(|s| !s.is_empty()) {
        if redact_text(last, &home_text).total == 0 {
            assert!(
                rendered.contains(last),
                "the printed cwd dropped the directory the session ran in ({last}):\n{rendered}"
            );
        }
    }
    // And the `cd` is the one that lands back in that directory.
    let expected_cd = if file_cwd.starts_with(&home_text) && !home_text.is_empty() {
        format!("cd \"$HOME{}\"", &file_cwd[home_text.len()..])
    } else {
        format!("cd \"{file_cwd}\"")
    };
    assert!(
        rendered.contains(&expected_cd),
        "the printed `cd` does not lead to the directory the file records ({expected_cd}):\n{rendered}"
    );

    // Nothing from the file's body leaked into a metadata-only rendering.
    let body = std::fs::read_to_string(&path).unwrap_or_default();
    for line in body.lines().skip(20).take(50) {
        if line.len() > 80 {
            assert!(
                !rendered.contains(line),
                "a transcript record reached the picker's output:\n{line}"
            );
        }
    }
}

/// The composer's `/resume` reaches the runtime as the message the actor runs.
///
/// Without this the picker could be perfect and unreachable, which is the state
/// this port was left in twice: a module with nowhere to hang.
#[test]
fn the_composer_turns_slash_resume_into_the_message_the_actor_runs() {
    let (tx, mut rx) = unbounded_channel::<Control>();
    let mut app = CoderApp::new("openagents coder", &Lane::default());

    app.submit("/resume".to_string(), &tx);
    assert!(matches!(rx.try_recv(), Ok(Control::ForeignResume(None))));

    app.submit("/resume 3".to_string(), &tx);
    assert!(matches!(rx.try_recv(), Ok(Control::ForeignResume(Some(3)))));

    // A mistyped pick is refused rather than quietly re-listing, because a
    // listing is what a picked session that failed to parse looks like.
    app.submit("/resume later".to_string(), &tx);
    assert!(
        rx.try_recv().is_err(),
        "a mistyped pick reached the runtime"
    );
    assert!(
        app.transcript().contains("`later` is not one"),
        "{}",
        app.transcript()
    );

    app.submit("/resume 0".to_string(), &tx);
    assert!(rx.try_recv().is_err(), "a zero pick reached the runtime");

    // And `/help` advertises it, so it is not a hidden command.
    app.submit("/help".to_string(), &tx);
    assert!(
        app.transcript()
            .contains("/resume — recent Claude Code and Codex sessions"),
        "{}",
        app.transcript()
    );
}

/// The whole turn the actor runs, on this machine, with nothing stubbed.
#[test]
fn the_whole_turn_runs_against_this_machine_and_names_what_it_read() {
    let root = repo_root();
    if !root
        .join("plugins/foreign-sessions/manifest.json")
        .is_file()
    {
        eprintln!("skipped: no shipped scanner beside this crate");
        return;
    }
    let home = home();
    let out = foreign_resume_turn(&root, &home, None);
    eprintln!("--- /resume in {} ---\n{out}\n---", root.display());

    if out.contains("Nothing was scanned.") {
        eprintln!("skipped: this machine has no foreign store to mount");
        return;
    }
    assert!(
        out.starts_with("Recent foreign sessions for this directory"),
        "{out}"
    );
    // Read access is disclosed, not implied, and named as what was mounted.
    assert!(out.contains("Read read-only from:"), "{out}");
    assert!(out.contains(".claude") && out.contains(".codex"), "{out}");
    // The home path never reaches the transcript raw.
    assert!(
        !out.contains(&home.to_string_lossy().into_owned()),
        "the invoking user's home path was printed unredacted:\n{out}"
    );
}

#[test]
fn a_real_session_with_the_wrong_id_on_it_is_refused_against_the_file() {
    let home = home();
    let Some((raw, output, roots)) = real_scan(&home) else {
        eprintln!("skipped: no shipped scanner or no foreign store on this machine");
        return;
    };
    let Some((pick, _)) = newest_readable(&output) else {
        eprintln!("skipped: no session on this machine was inside the scanner's read bound");
        return;
    };

    // Same real packet, same real file, one field altered: the exact shape of a
    // scanner that reported a session the store does not hold.
    let mut tampered = raw.clone();
    tampered["ok"]["sessions"][pick - 1]["session_id"] =
        Value::String("00000000-0000-4000-8000-000000000000".to_string());

    let deps = ForeignResumeDeps {
        now_ms: NOW_MS,
        cwd: String::new(),
        selection: Some(pick),
        home: home.to_string_lossy().into_owned(),
        mount_roots: roots,
    };
    let rendered = run_foreign_resume(&deps, &fixed(tampered), &ForeignResumeOptions::default());

    assert!(
        rendered.contains("does not carry the session id the scanner reported"),
        "an unbacked session id was accepted against a real file:\n{rendered}"
    );
    assert!(
        !rendered.contains("--resume") && !rendered.contains("codex resume"),
        "a resume command was printed for an id the file does not hold:\n{rendered}"
    );
    // And the honest one still works, so the refusal is not the only outcome.
    let honest = run_foreign_resume(&deps, &fixed(raw), &ForeignResumeOptions::default());
    assert!(honest.starts_with("Resume context:"), "{honest}");
}
