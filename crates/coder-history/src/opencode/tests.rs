use super::*;
use crate::{CatalogRequest, Config, Harness, History, NEWEST, TranscriptRequest};
use std::os::unix::fs::MetadataExt;
use std::sync::atomic::{AtomicU64, Ordering};

/// Rows recorded from a real OpenCode 1.18.26 run on 2026-09-28 with
/// `OPENCODE_DB` set to a scratch database: one session that ran `cat` and
/// answered, and one whose provider refused with HTTP 403.
const CAPTURE: &str = include_str!("../../fixtures/opencode/capture.rows.json");
const ANSWERED: &str = "ses_f160cfbc3ffeFQ1TJ4BLDKGbNo";
const REFUSED: &str = "ses_f160e45b3ffePgqXHjHfkkpEUY";
/// The recorded prompt, as `opencode run` saved it: the CLI quotes a
/// positional message that has spaces.
const PROMPT: &str = "\"Use the bash tool to run: cat note.txt . Then reply with exactly: done\"";

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "coder-history-opencode-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let fixture = Fixture { root };
        let db = fixture.db();
        db.execute_batch(
            "CREATE TABLE session (id text PRIMARY KEY, project_id text NOT NULL, parent_id text, \
             slug text NOT NULL, directory text NOT NULL, title text NOT NULL, version text NOT NULL, \
             time_created integer NOT NULL, time_updated integer NOT NULL, time_archived integer);
             CREATE TABLE message (id text PRIMARY KEY, session_id text NOT NULL, \
             time_created integer NOT NULL, time_updated integer NOT NULL, data text NOT NULL);
             CREATE TABLE part (id text PRIMARY KEY, message_id text NOT NULL, session_id text NOT NULL, \
             time_created integer NOT NULL, time_updated integer NOT NULL, data text NOT NULL);",
        )
        .unwrap();
        let rows: Value = serde_json::from_str(CAPTURE).unwrap();
        for s in rows["session"].as_array().unwrap() {
            db.execute(
                "INSERT INTO session VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    s["id"].as_str(),
                    s["project_id"].as_str(),
                    s["parent_id"].as_str(),
                    s["slug"].as_str(),
                    s["directory"].as_str(),
                    s["title"].as_str(),
                    s["version"].as_str(),
                    s["time_created"].as_i64(),
                    s["time_updated"].as_i64(),
                    s["time_archived"].as_i64(),
                ],
            )
            .unwrap();
        }
        for m in rows["message"].as_array().unwrap() {
            db.execute(
                "INSERT INTO message VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    m["id"].as_str(),
                    m["session_id"].as_str(),
                    m["time_created"].as_i64(),
                    m["time_updated"].as_i64(),
                    m["data"].as_str(),
                ],
            )
            .unwrap();
        }
        for p in rows["part"].as_array().unwrap() {
            db.execute(
                "INSERT INTO part VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    p["id"].as_str(),
                    p["message_id"].as_str(),
                    p["session_id"].as_str(),
                    p["time_created"].as_i64(),
                    p["time_updated"].as_i64(),
                    p["data"].as_str(),
                ],
            )
            .unwrap();
        }
        fixture
    }

    fn database(&self) -> PathBuf {
        self.root.join("opencode.db")
    }

    fn mirror_dir(&self) -> PathBuf {
        self.root.join("mirror")
    }

    fn db(&self) -> Connection {
        Connection::open(self.database()).unwrap()
    }

    fn mirror(&self) -> Report {
        mirror(&self.database(), &self.mirror_dir()).unwrap()
    }

    fn history(&self) -> History {
        History::open(Config {
            opencode: Some(self.mirror_dir()),
            ..Config::default()
        })
        .unwrap()
    }

    fn file(&self, session: &str) -> PathBuf {
        self.mirror_dir().join(format!("{session}.jsonl"))
    }

    /// Bumps a session's last update, as OpenCode does when it writes.
    fn touch(&self, session: &str) {
        self.db()
            .execute(
                "UPDATE session SET time_updated = time_updated + 1000 WHERE id = ?1",
                params![session],
            )
            .unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn texts(history: &History, source: &str) -> Vec<(String, Option<String>, String)> {
    let page = history
        .transcript(TranscriptRequest {
            source_id: source.to_owned(),
            cursor: None,
            max_bytes: crate::MAX_PAGE_BYTES,
            end: Some(NEWEST),
        })
        .unwrap();
    page.chunks
        .iter()
        .filter_map(|chunk| chunk.readable.clone())
        .filter(|r| !r.text.is_empty() && r.kind != "reasoning")
        .map(|r| (r.kind, r.role, r.text))
        .collect()
}

#[test]
fn a_recorded_session_mirrors_and_lists_as_an_opencode_chat() {
    let fixture = Fixture::new();
    let report = fixture.mirror();
    assert_eq!(report.sessions, 2);
    assert_eq!(report.updated, 2);
    let history = fixture.history();
    let page = history.catalog(CatalogRequest::default()).unwrap();
    assert_eq!(page.entries.len(), 2, "{page:?}");
    let answered = page
        .entries
        .iter()
        .find(|c| c.native_id.as_deref() == Some(ANSWERED))
        .unwrap();
    assert_eq!(answered.harness, Harness::OpenCode);
    assert_eq!(answered.title, "Reading note.txt contents");
    assert!(!answered.subagent && !answered.archived);
    let source = answered.source_id.clone().unwrap();
    assert_eq!(
        texts(&history, &source),
        vec![
            ("message".into(), Some("user".into()), PROMPT.into()),
            ("tool_call".into(), None, "cat note.txt\n\nhello\n".into()),
            ("message".into(), Some("assistant".into()), "done".into()),
        ]
    );
    // The engine's large, opaque tool metadata is not mirrored.
    let bytes = std::fs::read_to_string(fixture.file(ANSWERED)).unwrap();
    assert!(!bytes.contains("thoughtSignature"));
    let refused = page
        .entries
        .iter()
        .find(|c| c.native_id.as_deref() == Some(REFUSED))
        .unwrap();
    let said = texts(&history, refused.source_id.as_ref().unwrap());
    assert_eq!(
        said.last().unwrap(),
        &(
            "message".to_owned(),
            Some("system".to_owned()),
            "OpenCode stopped: Upstream request failed: Model access is disabled".to_owned()
        )
    );
}

#[test]
fn a_second_pass_with_nothing_new_writes_nothing() {
    let fixture = Fixture::new();
    fixture.mirror();
    let before = std::fs::read(fixture.file(ANSWERED)).unwrap();
    let report = fixture.mirror();
    assert_eq!((report.updated, report.lines), (0, 0));
    assert_eq!(std::fs::read(fixture.file(ANSWERED)).unwrap(), before);
}

#[test]
fn a_running_reply_is_appended_part_by_part_and_never_out_of_order() {
    let fixture = Fixture::new();
    let db = fixture.db();
    // The last reply is still running: its message has not completed and
    // its text has not ended.
    db.execute(
        "UPDATE message SET data = json_remove(data, '$.time.completed') \
         WHERE id = 'msg_0e9f319b70015sWvmtp3aB6Dcs'",
        [],
    )
    .unwrap();
    db.execute(
        "UPDATE part SET data = json_remove(data, '$.time.end') \
         WHERE id = 'prt_0e9f31f270019DTBmacaQ0jH3L'",
        [],
    )
    .unwrap();
    fixture.mirror();
    let history = fixture.history();
    let chat = history
        .catalog(CatalogRequest::default())
        .unwrap()
        .entries
        .into_iter()
        .find(|c| c.native_id.as_deref() == Some(ANSWERED))
        .unwrap();
    let source = chat.source_id.unwrap();
    let said = texts(&history, &source);
    assert_eq!(said.len(), 2, "the running text waits: {said:?}");
    let before = std::fs::read(fixture.file(ANSWERED)).unwrap();
    let inode = std::fs::metadata(fixture.file(ANSWERED)).unwrap().ino();
    // The text ends and the message completes.
    db.execute(
        "UPDATE part SET data = json_set(data, '$.time.end', 1790631419697) \
         WHERE id = 'prt_0e9f31f270019DTBmacaQ0jH3L'",
        [],
    )
    .unwrap();
    db.execute(
        "UPDATE message SET data = json_set(data, '$.time.completed', 1790631419741) \
         WHERE id = 'msg_0e9f319b70015sWvmtp3aB6Dcs'",
        [],
    )
    .unwrap();
    fixture.touch(ANSWERED);
    let report = fixture.mirror();
    assert_eq!(report.rewritten, 0);
    let after = std::fs::read(fixture.file(ANSWERED)).unwrap();
    assert!(after.starts_with(&before), "the file only grew");
    assert_eq!(
        std::fs::metadata(fixture.file(ANSWERED)).unwrap().ino(),
        inode
    );
    let said = texts(&fixture.history(), &source);
    assert_eq!(said.last().unwrap().2, "done");
    assert_eq!(said.len(), 3);
}

#[test]
fn a_rewritten_session_starts_a_new_file_and_a_removed_one_leaves() {
    let fixture = Fixture::new();
    fixture.mirror();
    let inode = std::fs::metadata(fixture.file(ANSWERED)).unwrap().ino();
    // OpenCode reverted the last reply: its message is gone.
    let db = fixture.db();
    db.execute(
        "DELETE FROM part WHERE message_id = 'msg_0e9f319b70015sWvmtp3aB6Dcs'",
        [],
    )
    .unwrap();
    db.execute(
        "DELETE FROM message WHERE id = 'msg_0e9f319b70015sWvmtp3aB6Dcs'",
        [],
    )
    .unwrap();
    fixture.touch(ANSWERED);
    let report = fixture.mirror();
    assert_eq!(report.rewritten, 1);
    assert_ne!(
        std::fs::metadata(fixture.file(ANSWERED)).unwrap().ino(),
        inode,
        "a new file, so a reader's cursor reports the change"
    );
    let bytes = std::fs::read_to_string(fixture.file(ANSWERED)).unwrap();
    assert!(!bytes.contains("\"text\":\"done\""));
    db.execute("DELETE FROM session WHERE id = ?1", params![REFUSED])
        .unwrap();
    let report = fixture.mirror();
    assert_eq!(report.removed, 1);
    assert!(!fixture.file(REFUSED).exists());
}

#[test]
fn a_subagent_session_lists_as_a_subagent_and_an_archived_one_as_archived() {
    let fixture = Fixture::new();
    let db = fixture.db();
    db.execute(
        "UPDATE session SET parent_id = ?1 WHERE id = ?2",
        params![ANSWERED, REFUSED],
    )
    .unwrap();
    db.execute(
        "UPDATE session SET time_archived = time_updated WHERE id = ?1",
        params![ANSWERED],
    )
    .unwrap();
    fixture.mirror();
    let page = fixture
        .history()
        .catalog(CatalogRequest::default())
        .unwrap();
    let find = |id: &str| {
        page.entries
            .iter()
            .find(|c| c.native_id.as_deref() == Some(id))
            .unwrap()
            .clone()
    };
    assert!(find(REFUSED).subagent);
    assert!(find(ANSWERED).archived);
    assert!(!find(REFUSED).archived);
}

#[test]
fn a_placeholder_title_names_the_chat_by_its_first_prompt() {
    let fixture = Fixture::new();
    fixture
        .db()
        .execute(
            "UPDATE session SET title = 'New session - 2026-09-28T21:36:50.000Z' WHERE id = ?1",
            params![ANSWERED],
        )
        .unwrap();
    fixture.mirror();
    let page = fixture
        .history()
        .catalog(CatalogRequest::default())
        .unwrap();
    let chat = page
        .entries
        .iter()
        .find(|c| c.native_id.as_deref() == Some(ANSWERED))
        .unwrap();
    assert_eq!(chat.title, PROMPT);
}

#[test]
fn a_missing_database_writes_nothing() {
    let fixture = Fixture::new();
    let report = mirror(&fixture.root.join("absent.db"), &fixture.mirror_dir()).unwrap();
    assert_eq!(report, Report::default());
    assert!(!fixture.mirror_dir().exists());
}

#[test]
fn the_engine_database_is_never_the_owners() {
    let home = Path::new("/home/owner");
    let data = database(home, None).parent().unwrap().to_path_buf();
    assert_ne!(
        crate::engine::opencode_database(&data),
        database(home, None),
        "the mirror reads the owner's database; the engine writes its own"
    );
    assert_eq!(
        database(home, Some(Path::new("/data"))),
        Path::new("/data/opencode/opencode.db")
    );
    assert_eq!(
        database(home, None),
        Path::new("/home/owner/.local/share/opencode/opencode.db")
    );
}

#[test]
fn a_long_tool_output_is_cut_and_says_so() {
    let part = clip(json!({
        "type": "tool",
        "tool": "bash",
        "state": {"status": "completed", "output": "x".repeat(TOOL_OUTPUT_BYTES + 10),
                  "metadata": {"output": "y"}, "title": "t"},
    }));
    assert_eq!(
        part.pointer("/state/output")
            .unwrap()
            .as_str()
            .unwrap()
            .len(),
        TOOL_OUTPUT_BYTES
    );
    assert!(part.pointer("/state/metadata").is_none());
    assert_eq!(part["openagents_clipped"], true);
    let small = clip(json!({"type": "text", "text": "hi", "time": {"start": 1, "end": 2}}));
    assert!(small.get("openagents_clipped").is_none());
}
