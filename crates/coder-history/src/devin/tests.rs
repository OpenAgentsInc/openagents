use super::*;
use crate::{CatalogRequest, Config, Harness, History, NEWEST, TranscriptRequest};
use std::os::unix::fs::MetadataExt;
use std::sync::atomic::{AtomicU64, Ordering};

/// Rows recorded from the Devin CLI 3000.11.3 on 2026-09-28, over ACP in a
/// scratch directory: the owner's session that ran `echo mirror-fixture` in
/// bypass mode and answered, and a session Coder's engine started (its
/// `session/new` carried the engine mark) that answered `ok`. Devin's system
/// prompts are left out and the paths replaced; the rows are otherwise as
/// saved.
const CAPTURE: &str = include_str!("../../fixtures/devin/capture.rows.json");
const OWNERS: &str = "serene-crayfish";
const ENGINES: &str = "rhetorical-leopard";
const PROMPT: &str = "Run the shell command `echo mirror-fixture` and tell me what it printed, in one short sentence.";

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "coder-history-devin-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let fixture = Fixture { root };
        let db = fixture.db();
        // Devin 3000.11.3's tables, as its store declares them.
        db.execute_batch(
            "CREATE TABLE sessions (id TEXT PRIMARY KEY, working_directory TEXT NOT NULL, \
             backend_type TEXT NOT NULL, model TEXT NOT NULL, agent_mode TEXT NOT NULL, \
             created_at INTEGER NOT NULL, last_activity_at INTEGER NOT NULL, title TEXT, \
             main_chain_id INTEGER, shell_last_seen_index INTEGER DEFAULT 0, cogs_json TEXT, \
             workspace_dirs TEXT, hidden INTEGER NOT NULL DEFAULT 0, metadata TEXT);
             CREATE TABLE message_nodes (row_id INTEGER PRIMARY KEY AUTOINCREMENT, \
             session_id TEXT NOT NULL, node_id INTEGER NOT NULL, parent_node_id INTEGER, \
             chat_message TEXT NOT NULL, created_at INTEGER NOT NULL, metadata TEXT, \
             UNIQUE(session_id, node_id));",
        )
        .unwrap();
        let rows: Value = serde_json::from_str(CAPTURE).unwrap();
        for s in rows["sessions"].as_array().unwrap() {
            db.execute(
                "INSERT INTO sessions VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    s["id"].as_str(),
                    s["working_directory"].as_str(),
                    s["backend_type"].as_str(),
                    s["model"].as_str(),
                    s["agent_mode"].as_str(),
                    s["created_at"].as_i64(),
                    s["last_activity_at"].as_i64(),
                    s["title"].as_str(),
                    s["main_chain_id"].as_i64(),
                    s["shell_last_seen_index"].as_i64(),
                    s["cogs_json"].as_str(),
                    s["workspace_dirs"].as_str(),
                    s["hidden"].as_i64(),
                    s["metadata"].as_str(),
                ],
            )
            .unwrap();
        }
        for n in rows["message_nodes"].as_array().unwrap() {
            db.execute(
                "INSERT INTO message_nodes (session_id, node_id, parent_node_id, chat_message, \
                 created_at, metadata) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    n["session_id"].as_str(),
                    n["node_id"].as_i64(),
                    n["parent_node_id"].as_i64(),
                    n["chat_message"].as_str(),
                    n["created_at"].as_i64(),
                    n["metadata"].as_str(),
                ],
            )
            .unwrap();
        }
        fixture
    }

    fn database(&self) -> PathBuf {
        self.root.join("sessions.db")
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
            devin: Some(self.mirror_dir()),
            ..Config::default()
        })
        .unwrap()
    }

    fn file(&self, session: &str) -> PathBuf {
        self.mirror_dir().join(format!("{session}.jsonl"))
    }

    /// Adds a node to the owner's session and makes it the conversation's
    /// newest, as Devin does when a message ends.
    fn add(&self, node: i64, parent: Option<i64>, message: &Value, metadata: Option<&Value>) {
        let db = self.db();
        db.execute(
            "INSERT INTO message_nodes (session_id, node_id, parent_node_id, chat_message, \
             created_at, metadata) VALUES (?1, ?2, ?3, ?4, 1790633300, ?5)",
            params![
                OWNERS,
                node,
                parent,
                message.to_string(),
                metadata.map(Value::to_string)
            ],
        )
        .unwrap();
        db.execute(
            "UPDATE sessions SET main_chain_id = ?1, last_activity_at = last_activity_at + 1 \
             WHERE id = ?2",
            params![node, OWNERS],
        )
        .unwrap();
    }

    fn chat(&self, history: &History) -> crate::Chat {
        history
            .catalog(CatalogRequest::default())
            .unwrap()
            .entries
            .into_iter()
            .find(|c| c.native_id.as_deref() == Some(OWNERS))
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn said(history: &History, source: &str) -> Vec<(String, Option<String>, String)> {
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

fn user(text: &str) -> Value {
    json!({"message_id": "u", "role": "user", "content": text,
           "metadata": {"is_user_input": true}})
}

fn assistant(text: &str) -> Value {
    json!({"message_id": "a", "role": "assistant", "content": text, "tool_calls": []})
}

#[test]
fn a_recorded_session_mirrors_and_lists_as_a_devin_chat() {
    let fixture = Fixture::new();
    let report = fixture.mirror();
    assert_eq!(
        report.sessions, 1,
        "the engine's session is not the owner's"
    );
    assert_eq!(report.updated, 1);
    assert!(!fixture.file(ENGINES).exists());
    let history = fixture.history();
    let page = history.catalog(CatalogRequest::default()).unwrap();
    assert_eq!(page.entries.len(), 1, "{page:?}");
    let chat = &page.entries[0];
    assert_eq!(chat.harness, Harness::Devin);
    assert_eq!(chat.native_id.as_deref(), Some(OWNERS));
    assert_eq!(chat.title, "mirror-fixture");
    assert!(!chat.subagent && !chat.archived);
    assert_eq!(
        said(&history, chat.source_id.as_ref().unwrap()),
        vec![
            ("message".into(), Some("user".into()), PROMPT.into()),
            ("tool_call".into(), None, "echo mirror-fixture".into()),
            (
                "tool_result".into(),
                Some("tool".into()),
                "Output from command in shell a40180:\nmirror-fixture\n\n\nExit code: 0".into()
            ),
            (
                "message".into(),
                Some("assistant".into()),
                "It printed `mirror-fixture`.".into()
            ),
        ]
    );
    // The prompt was carried into two rebuilt chains; it is written once,
    // and no system prompt is written.
    let bytes = std::fs::read_to_string(fixture.file(OWNERS)).unwrap();
    assert_eq!(bytes.matches("echo mirror-fixture` and tell").count(), 1);
    assert!(!bytes.contains("system prompt"));
}

#[test]
fn a_second_pass_with_nothing_new_writes_nothing() {
    let fixture = Fixture::new();
    fixture.mirror();
    let before = std::fs::read(fixture.file(OWNERS)).unwrap();
    let report = fixture.mirror();
    assert_eq!((report.updated, report.lines), (0, 0));
    assert_eq!(std::fs::read(fixture.file(OWNERS)).unwrap(), before);
}

#[test]
fn a_next_turn_and_a_compaction_only_append() {
    let fixture = Fixture::new();
    fixture.mirror();
    let history = fixture.history();
    let source = fixture.chat(&history).source_id.unwrap();
    let inode = std::fs::metadata(fixture.file(OWNERS)).unwrap().ino();
    // The owner's next turn, and a keep-alive prompt Devin writes itself.
    fixture.add(31, Some(30), &user("Now say bye."), None);
    fixture.add(
        32,
        Some(31),
        &json!({"message_id": "k", "role": "user", "content": "continue",
                "metadata": {"is_user_input": null}}),
        None,
    );
    fixture.add(33, Some(32), &assistant("bye"), None);
    let before = std::fs::read(fixture.file(OWNERS)).unwrap();
    let report = fixture.mirror();
    assert_eq!((report.rewritten, report.lines), (0, 2));
    // Devin compacts: a new chain opens with its system prompt and a summary
    // of the old chain, then carries the last messages over as new nodes.
    fixture.add(
        40,
        None,
        &json!({"message_id": "s", "role": "system", "content": "You are Devin."}),
        None,
    );
    fixture.add(
        41,
        Some(40),
        &json!({"message_id": "c", "role": "system", "content": "You are continuing work."}),
        Some(&json!({"summarized_from": 30})),
    );
    fixture.add(
        42,
        Some(41),
        &user("Now say bye."),
        Some(&json!({"extensions": {"compact/prior_node_ids": [31]}})),
    );
    fixture.add(
        43,
        Some(42),
        &assistant("bye"),
        Some(&json!({"extensions": {"compact/prior_node_ids": [33]}})),
    );
    fixture.add(44, Some(43), &user("And once more."), None);
    let grown = std::fs::read(fixture.file(OWNERS)).unwrap();
    assert!(grown.starts_with(&before));
    let report = fixture.mirror();
    assert_eq!((report.rewritten, report.lines), (0, 1));
    assert_eq!(
        std::fs::metadata(fixture.file(OWNERS)).unwrap().ino(),
        inode
    );
    let lines = said(&fixture.history(), &source);
    assert_eq!(
        lines[lines.len() - 3..]
            .iter()
            .map(|(_, _, text)| text.as_str())
            .collect::<Vec<_>>(),
        ["Now say bye.", "bye", "And once more."]
    );
}

#[test]
fn a_rewound_session_starts_a_new_file_and_a_removed_one_leaves() {
    let fixture = Fixture::new();
    fixture.mirror();
    let inode = std::fs::metadata(fixture.file(OWNERS)).unwrap().ino();
    // The owner rewinds to the prompt and Devin answers differently.
    fixture.add(50, Some(24), &assistant("Something else."), None);
    let report = fixture.mirror();
    assert_eq!(report.rewritten, 1);
    assert_ne!(
        std::fs::metadata(fixture.file(OWNERS)).unwrap().ino(),
        inode,
        "a new file, so a reader's cursor reports the change"
    );
    let bytes = std::fs::read_to_string(fixture.file(OWNERS)).unwrap();
    assert!(!bytes.contains("It printed"));
    assert!(bytes.contains("Something else."));
    fixture
        .db()
        .execute("DELETE FROM sessions WHERE id = ?1", params![OWNERS])
        .unwrap();
    let report = fixture.mirror();
    assert_eq!(report.removed, 1);
    assert!(!fixture.file(OWNERS).exists());
}

#[test]
fn a_hidden_session_lists_as_archived_and_an_untitled_one_by_its_prompt() {
    let fixture = Fixture::new();
    fixture
        .db()
        .execute(
            "UPDATE sessions SET hidden = 1, title = NULL WHERE id = ?1",
            params![OWNERS],
        )
        .unwrap();
    fixture.mirror();
    let chat = fixture.chat(&fixture.history());
    assert_eq!(chat.title, PROMPT);
    // An untitled session has no index line, so the index cannot say it is
    // archived; a titled hidden one can.
    fixture
        .db()
        .execute(
            "UPDATE sessions SET title = 'Hidden', last_activity_at = last_activity_at + 1 \
             WHERE id = ?1",
            params![OWNERS],
        )
        .unwrap();
    fixture.mirror();
    let chat = fixture.chat(&fixture.history());
    assert!(chat.archived);
    assert_eq!(chat.title, "Hidden");
}

#[test]
fn an_engine_session_already_mirrored_leaves() {
    let fixture = Fixture::new();
    fixture
        .db()
        .execute(
            "UPDATE sessions SET metadata = NULL WHERE id = ?1",
            params![ENGINES],
        )
        .unwrap();
    fixture.mirror();
    assert!(fixture.file(ENGINES).exists());
    fixture
        .db()
        .execute(
            "UPDATE sessions SET metadata = json_object('client_meta', \
             json_object('openagents.com/engine', 'openagents-coder-engine')) WHERE id = ?1",
            params![ENGINES],
        )
        .unwrap();
    let report = fixture.mirror();
    assert_eq!(report.removed, 1);
    assert!(!fixture.file(ENGINES).exists());
}

#[test]
fn a_missing_store_writes_nothing() {
    let fixture = Fixture::new();
    let report = mirror(&fixture.root.join("absent.db"), &fixture.mirror_dir()).unwrap();
    assert_eq!(report, Report::default());
    assert!(!fixture.mirror_dir().exists());
}

#[test]
fn the_store_is_in_devins_data_directory() {
    let home = Path::new("/home/owner");
    assert_eq!(
        database(home, None),
        Path::new("/home/owner/.local/share/devin/cli/sessions.db")
    );
    assert_eq!(
        database(home, Some(Path::new("/data"))),
        Path::new("/data/devin/cli/sessions.db")
    );
    assert_eq!(
        crate::engine::DEVIN_META_KEY,
        "openagents.com/engine",
        "the key acp_client::devin sets"
    );
}

#[test]
fn a_long_result_is_cut_and_says_so() {
    let item = bounded(json!({
        "type": "devin.item", "item": "tool_result", "text": "x".repeat(TOOL_OUTPUT_BYTES + 10),
    }));
    assert_eq!(item["text"].as_str().unwrap().len(), TOOL_OUTPUT_BYTES);
    assert_eq!(item["openagents_clipped"], true);
    let call = bounded(json!({
        "type": "devin.item", "item": "tool_call", "tool": "write",
        "arguments": {"file_path": "a", "content": "y".repeat(TOOL_OUTPUT_BYTES * 2)},
    }));
    assert_eq!(
        call.pointer("/arguments/content")
            .unwrap()
            .as_str()
            .unwrap()
            .len(),
        TOOL_OUTPUT_BYTES
    );
    let small = bounded(json!({"type": "devin.item", "item": "message", "text": "hi"}));
    assert!(small.get("openagents_clipped").is_none());
}

#[test]
fn only_devin_session_ids_are_file_names() {
    assert!(crate::devin_session_id("serene-crayfish"));
    for bad in ["", "../x", "ses_A", "-x", "a/b", &"a".repeat(65)] {
        assert!(!crate::devin_session_id(bad), "{bad}");
    }
}

#[test]
fn an_engine_session_is_copied_beside_its_task_and_a_next_turn_only_appends() {
    let fixture = Fixture::new();
    let tasks = fixture.root.join("tasks");
    std::fs::create_dir_all(&tasks).unwrap();
    let task = "0b".repeat(32);
    let engine = tasks.join(crate::delegate::file_name(&task, Harness::Devin, ENGINES).unwrap());
    let owners = tasks.join(crate::delegate::file_name(&task, Harness::Devin, OWNERS).unwrap());
    // The engine's own session, which the mirror never writes, is copied.
    assert!(delegate(&fixture.database(), ENGINES, &engine).unwrap());
    assert!(delegate(&fixture.database(), OWNERS, &owners).unwrap());
    let history = History::open(Config {
        coder: Some(tasks.clone()),
        ..Config::default()
    })
    .unwrap();
    let entries = history.catalog(CatalogRequest::default()).unwrap().entries;
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|c| c.harness == Harness::Devin
        && c.subagent
        && c.native_id.as_deref() == Some(task.as_str())));
    let chat = entries
        .into_iter()
        .find(|c| c.title == format!("Devin session {OWNERS}"))
        .unwrap();
    let source = chat.source_id.unwrap();
    let (before, inode) = (
        std::fs::read(&owners).unwrap(),
        std::fs::metadata(&owners).unwrap().ino(),
    );
    fixture.add(31, Some(30), &user("Now say bye."), None);
    fixture.add(32, Some(31), &assistant("bye"), None);
    assert!(delegate(&fixture.database(), OWNERS, &owners).unwrap());
    assert!(std::fs::read(&owners).unwrap().starts_with(&before));
    assert_eq!(std::fs::metadata(&owners).unwrap().ino(), inode);
    let lines = said(&history, &source);
    assert_eq!(lines.last().unwrap().2, "bye");
    assert!(delegate(&fixture.database(), "absent-otter", &owners).is_err());
}
