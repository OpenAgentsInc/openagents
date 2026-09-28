use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "coder-history-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn write(&self, path: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }
    fn history(&self) -> History {
        History::open(Config {
            codex: Some(self.0.clone()),
            claude: None,
            coder: None,
        })
        .unwrap()
    }
    fn codex(&self, id: &str, tail: &str) -> PathBuf {
        self.write(
            &format!("sessions/2026/01/01/rollout-{id}.jsonl"),
            format!("{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{id}\"}}}}\n{tail}"),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn first(history: &History) -> Chat {
    history
        .catalog(CatalogRequest::default())
        .unwrap()
        .entries
        .remove(0)
}
fn read(
    history: &History,
    source: &str,
    cursor: Option<TranscriptCursor>,
    max: u32,
) -> TranscriptPage {
    history
        .transcript(TranscriptRequest {
            source_id: source.into(),
            cursor,
            max_bytes: max,
            end: None,
        })
        .unwrap()
}
fn collect(
    history: &History,
    source: &str,
    max: u32,
) -> (Vec<u8>, Vec<RecordChunk>, TranscriptCursor) {
    let mut bytes = Vec::new();
    let mut chunks = Vec::new();
    let mut cursor = None;
    for _ in 0..2000 {
        let page = read(history, source, cursor, max);
        assert!(encoded_len(&page).unwrap() <= MAX_RESPONSE_BYTES);
        for chunk in &page.chunks {
            assert_eq!(chunk.offset, bytes.len() as u64);
            bytes.extend(STANDARD.decode(&chunk.raw_base64).unwrap());
        }
        chunks.extend(page.chunks);
        cursor = Some(page.next);
        if !page.has_more {
            return (bytes, chunks, cursor.unwrap());
        }
    }
    panic!("synthetic transcript did not finish within its bounded pages");
}

#[test]
fn catalog_discovers_archive_titles_missing_sources_and_new_chats() {
    let fixture = Fixture::new();
    fixture.codex("active", "");
    fixture.write(
        "archived_sessions/archive.jsonl",
        b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"archive\"}}\n",
    );
    fixture.write("session_index.jsonl",b"{\"id\":\"active\",\"thread_name\":\"First title\",\"updated_at\":\"2026-01-01\"}\n{\"id\":\"active\",\"thread_name\":\"Renamed\",\"updated_at\":\"2026-01-02\"}\n{\"id\":\"missing\",\"thread_name\":\"Unavailable chat\"}\n");
    let history = fixture.history();
    let page = history.catalog(CatalogRequest::default()).unwrap();
    assert_eq!(page.entries.len(), 3);
    assert!(
        page.entries
            .iter()
            .any(|c| c.title == "Renamed" && c.native_id.as_deref() == Some("active"))
    );
    assert!(
        page.entries
            .iter()
            .any(|c| c.archived && c.native_id.as_deref() == Some("archive"))
    );
    assert!(
        page.entries
            .iter()
            .any(|c| c.status == SourceStatus::Missing && c.source_id.is_none())
    );
    fixture.codex("later", "");
    assert_eq!(
        history
            .catalog(CatalogRequest::default())
            .unwrap()
            .entries
            .len(),
        4
    );
}

#[test]
fn catalog_cursor_ignores_titles_but_refuses_changed_membership() {
    let fixture = Fixture::new();
    fixture.codex("one", "");
    fixture.codex("two", "");
    let history = fixture.history();
    let first = history
        .catalog(CatalogRequest {
            cursor: None,
            limit: 1,
        })
        .unwrap();
    fixture.write(
        "session_index.jsonl",
        b"{\"id\":\"one\",\"thread_name\":\"Updated title\",\"updated_at\":\"new\"}\n",
    );
    let next = history
        .catalog(CatalogRequest {
            cursor: first.next.clone(),
            limit: 1,
        })
        .unwrap();
    assert_eq!(next.snapshot, first.snapshot);
    assert_ne!(next.entries[0].source_id, first.entries[0].source_id);
    fixture.codex("three", "");
    assert_eq!(
        history.catalog(CatalogRequest {
            cursor: first.next,
            limit: 1
        }),
        Err(Error::CursorStale)
    );
}

#[test]
fn every_raw_byte_and_unknown_record_survives_small_pages() {
    let fixture = Fixture::new();
    let path=fixture.codex("literal","{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"café 日本語 👩🏽‍💻\"}]}}\n{\"type\":\"future_record\",\"private\":\"literal\"}\nnot-json\n");
    let before = fs::read(&path).unwrap();
    let history = fixture.history();
    let source = first(&history).source_id.unwrap();
    let (bytes, chunks, cursor) = collect(&history, &source, 7);
    assert_eq!(bytes, before);
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(chunks.iter().any(|c| {
        c.readable
            .as_ref()
            .is_some_and(|r| r.text.contains("日本語"))
    }));
    assert!(chunks.iter().any(|c| {
        c.readable
            .as_ref()
            .is_some_and(|r| r.kind == "future_record" && r.unknown)
    }));
    assert!(chunks.iter().any(|c| {
        c.readable
            .as_ref()
            .is_some_and(|r| r.kind == "invalid_json" && r.unknown)
    }));
    let eof = read(&history, &source, Some(cursor.clone()), 8);
    assert!(eof.chunks.is_empty());
    assert_eq!(eof.next, cursor);
    assert!(!eof.pending_line);
}

#[test]
fn oversized_records_are_chunked_in_full_not_discarded() {
    let fixture = Fixture::new();
    let text = "x".repeat(MAX_READABLE_RECORD_BYTES + 123);
    let path=fixture.codex("large",&format!("{{\"type\":\"response_item\",\"payload\":{{\"type\":\"function_call_output\",\"output\":\"{text}\"}}}}\n"));
    let history = fixture.history();
    let source = first(&history).source_id.unwrap();
    let (bytes, chunks, _) = collect(&history, &source, MAX_PAGE_BYTES);
    assert_eq!(bytes, fs::read(path).unwrap());
    let large: Vec<_> = chunks.iter().filter(|c| c.index == 1).collect();
    assert!(large.len() > 1);
    assert!(large.iter().all(|c| c.id == large[0].id));
    assert!(large.last().unwrap().complete);
    assert!(large.last().unwrap().oversized);
    assert!(large.last().unwrap().readable.is_none());
}

#[test]
fn incomplete_tail_is_retained_and_completed_once_after_append() {
    let fixture = Fixture::new();
    let path = fixture.codex(
        "append",
        "{\"type\":\"event_msg\",\"payload\":{\"type\":\"agent_message\",\"message\":\"hel",
    );
    let history = fixture.history();
    let source = first(&history).source_id.unwrap();
    let (mut bytes, chunks, cursor) = collect(&history, &source, MAX_PAGE_BYTES);
    assert!(!chunks.last().unwrap().complete);
    let record_id = chunks.last().unwrap().id.clone();
    let waiting = read(&history, &source, Some(cursor.clone()), MAX_PAGE_BYTES);
    assert!(waiting.pending_line);
    assert!(waiting.chunks.is_empty());
    OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"lo\"}}\n")
        .unwrap();
    let reopened = fixture.history();
    let page = read(&reopened, &source, Some(cursor), MAX_PAGE_BYTES);
    assert_eq!(page.chunks.len(), 1);
    assert_eq!(page.chunks[0].id, record_id);
    assert!(page.chunks[0].complete);
    assert_eq!(page.chunks[0].readable.as_ref().unwrap().text, "hello");
    bytes.extend(STANDARD.decode(&page.chunks[0].raw_base64).unwrap());
    assert_eq!(bytes, fs::read(path).unwrap());
}

#[test]
fn rewritten_prefix_truncation_replacement_and_forged_cursor_refuse() {
    let fixture = Fixture::new();
    let path = fixture.codex("changed", "{\"type\":\"future\"}\n");
    let history = fixture.history();
    let source = first(&history).source_id.unwrap();
    let page = read(&history, &source, None, 10);
    let original = fs::read(&path).unwrap();
    let query = |cursor| TranscriptRequest {
        source_id: source.clone(),
        cursor: Some(cursor),
        max_bytes: 8,
        end: None,
    };
    let mut changed = original.clone();
    changed[1] = b' ';
    fs::write(&path, &changed).unwrap();
    assert_eq!(
        history.transcript(query(page.next.clone())),
        Err(Error::SourceChanged)
    );
    fs::write(&path, b"x").unwrap();
    assert_eq!(
        history.transcript(query(page.next.clone())),
        Err(Error::SourceChanged)
    );
    fs::write(&path, &original).unwrap();
    let mut forged = page.next.clone();
    forged.record_offset = 1;
    assert_eq!(history.transcript(query(forged)), Err(Error::SourceChanged));
    let replacement = fixture.write("replacement.jsonl", &original);
    fs::rename(replacement, &path).unwrap();
    assert_eq!(
        history.transcript(query(page.next)),
        Err(Error::SourceChanged)
    );
}

#[test]
fn symlink_sources_and_directories_cannot_escape_selected_roots() {
    let fixture = Fixture::new();
    let outside = Fixture::new();
    outside.codex("outside", "");
    fixture.codex("inside", "");
    symlink(&outside.0, fixture.0.join("sessions/linked-directory")).unwrap();
    symlink(
        outside.0.join("sessions/2026/01/01/rollout-outside.jsonl"),
        fixture.0.join("sessions/linked.jsonl"),
    )
    .unwrap();
    let history = fixture.history();
    let page = history.catalog(CatalogRequest::default()).unwrap();
    assert_eq!(page.entries.len(), 1);
    assert_eq!(
        page.notices
            .iter()
            .filter(|n| n.code == "symlink_refused")
            .count(),
        2
    );
    for notice in page.notices {
        assert_eq!(
            history.transcript(TranscriptRequest {
                source_id: notice.source_id.unwrap(),
                cursor: None,
                max_bytes: 8,
                end: None,
            }),
            Err(Error::SourceMissing)
        );
    }
    let alias = fixture.0.join("alias");
    symlink(&outside.0, &alias).unwrap();
    assert!(matches!(
        History::open(Config {
            codex: Some(alias),
            claude: None,
            coder: None
        }),
        Err(Error::InvalidRoot)
    ));
}

#[test]
fn malformed_index_and_missing_or_empty_sources_are_explicit() {
    let fixture = Fixture::new();
    fixture.write("sessions/empty.jsonl", b"");
    fixture.write("session_index.jsonl", b"not-json\n{\"id\":\"pending");
    let page = fixture
        .history()
        .catalog(CatalogRequest::default())
        .unwrap();
    assert_eq!(page.entries[0].status, SourceStatus::Empty);
    assert!(
        page.notices
            .iter()
            .any(|n| n.code == "title_index_partial_line")
    );
    assert!(
        page.notices
            .iter()
            .any(|n| n.code == "title_index_unrecognized_records")
    );
}

#[test]
fn claude_projects_and_subagents_are_separate_read_only_sources() {
    let fixture = Fixture::new();
    fixture.write("projects/project/session.jsonl",b"{\"type\":\"assistant\",\"sessionId\":\"session\",\"uuid\":\"record\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"tool_use\",\"name\":\"Read\",\"input\":{\"file_path\":\"synthetic.rs\"}}]}}\n");
    fixture.write("projects/project/session/subagents/agent.jsonl",b"{\"type\":\"user\",\"sessionId\":\"session\",\"uuid\":\"child-record\",\"message\":{\"role\":\"user\",\"content\":\"Synthetic child\"}}\n");
    let history = History::open(Config {
        codex: None,
        claude: Some(fixture.0.clone()),
        coder: None,
    })
    .unwrap();
    let page = history.catalog(CatalogRequest::default()).unwrap();
    assert_eq!(page.entries.len(), 2);
    assert_eq!(page.entries.iter().filter(|c| c.subagent).count(), 1);
    assert!(
        page.entries
            .iter()
            .all(|c| c.harness == Harness::Claude && c.native_id.as_deref() == Some("session"))
    );
    let main = page.entries.iter().find(|c| !c.subagent).unwrap();
    let (_, chunks, _) = collect(&history, main.source_id.as_ref().unwrap(), MAX_PAGE_BYTES);
    let readable = chunks.last().unwrap().readable.as_ref().unwrap();
    assert_eq!(readable.role.as_deref(), Some("assistant"));
    assert!(readable.text.contains("Tool: Read"));
    assert_eq!(readable.tool_name.as_deref(), Some("Read"));
}

#[test]
fn many_short_and_invalid_utf8_records_keep_contiguous_bounded_pages() {
    let fixture = Fixture::new();
    let mut raw = b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"dense\"}}\n".to_vec();
    raw.extend_from_slice(&b"\n".repeat(300));
    raw.extend_from_slice(b"\xff\xfe\n");
    let path = fixture.write("sessions/dense.jsonl", &raw);
    let history = fixture.history();
    let source = first(&history).source_id.unwrap();
    let first = read(&history, &source, None, MAX_PAGE_BYTES);
    assert_eq!(first.chunks.len(), 128);
    assert!(first.has_more);
    let (bytes, chunks, _) = collect(&history, &source, MAX_PAGE_BYTES);
    assert_eq!(bytes, fs::read(path).unwrap());
    assert!(
        chunks
            .iter()
            .all(|c| STANDARD.decode(&c.raw_base64).unwrap().len() <= MAX_CHUNK_BYTES)
    );
    assert!(chunks.last().unwrap().readable.as_ref().unwrap().unknown);
}

#[test]
fn moving_to_archive_retains_chat_identity_but_requires_a_new_source_cursor() {
    let fixture = Fixture::new();
    let path = fixture.codex("moved", "");
    let history = fixture.history();
    let old = first(&history);
    let source = old.source_id.unwrap();
    let cursor = read(&history, &source, None, MAX_PAGE_BYTES).next;
    fs::create_dir(fixture.0.join("archived_sessions")).unwrap();
    fs::rename(path, fixture.0.join("archived_sessions/moved.jsonl")).unwrap();
    let new = first(&history);
    assert_eq!(old.id, new.id);
    assert!(new.archived);
    assert_ne!(Some(source.clone()), new.source_id);
    assert_eq!(
        history.transcript(TranscriptRequest {
            source_id: source,
            cursor: Some(cursor),
            max_bytes: MAX_PAGE_BYTES,
            end: None,
        }),
        Err(Error::SourceMissing)
    );
}

#[test]
fn request_and_response_bounds_are_enforced() {
    let fixture = Fixture::new();
    let path = fixture.codex("bounded", "");
    let history = fixture.history();
    let source = first(&history).source_id.unwrap();
    assert_eq!(
        history.catalog(CatalogRequest {
            cursor: None,
            limit: 0
        }),
        Err(Error::InvalidRequest)
    );
    for max in [0, MAX_PAGE_BYTES + 1] {
        assert_eq!(
            history.transcript(TranscriptRequest {
                source_id: source.clone(),
                cursor: None,
                max_bytes: max,
                end: None,
            }),
            Err(Error::InvalidRequest)
        );
    }
    OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_len(confined::MAX_SOURCE_BYTES + 1)
        .unwrap();
    assert_eq!(
        history.transcript(TranscriptRequest {
            source_id: source,
            cursor: None,
            max_bytes: 8,
            end: None,
        }),
        Err(Error::ResourceLimit)
    );
    assert!(readable_record(&vec![b' '; MAX_READABLE_RECORD_BYTES + 1]).is_none());
}

#[test]
fn unconfigured_harness_trees_and_credentials_are_never_cataloged() {
    let fixture = Fixture::new();
    fixture.codex("visible", "");
    fixture.write("auth.json", b"synthetic non-credential marker");
    fixture.write("projects/hidden.jsonl", b"{\"sessionId\":\"hidden\"}\n");
    fixture.write("unrelated/history.jsonl", b"private synthetic marker\n");
    let page = fixture
        .history()
        .catalog(CatalogRequest::default())
        .unwrap();
    assert_eq!(page.entries.len(), 1);
    assert!(matches!(
        History::open(Config::default()),
        Err(Error::InvalidRoot)
    ));
    assert!(matches!(
        History::open(Config {
            codex: Some(Path::new("relative").into()),
            claude: None,
            coder: None
        }),
        Err(Error::InvalidRoot)
    ));
}

fn back(history: &History, source: &str, end: u64, max: u32) -> TranscriptPage {
    history
        .transcript(TranscriptRequest {
            source_id: source.into(),
            cursor: None,
            max_bytes: max,
            end: Some(end),
        })
        .unwrap()
}

#[test]
fn backward_pages_walk_whole_records_from_the_newest() {
    let fixture = Fixture::new();
    let mut tail = String::new();
    for index in 0..20 {
        tail.push_str(&format!(
            "{{\"type\":\"event_msg\",\"payload\":{{\"type\":\"agent_message\",\"message\":\"reply {index:02} padding padding\"}}}}\n"
        ));
    }
    // A record still being written is never part of a backward page.
    tail.push_str("{\"type\":\"event_msg\",\"partial");
    let path = fixture.codex("019c0000-0000-7000-8000-000000000001", &tail);
    let history = fixture.history();
    let source = first(&history).source_id.unwrap();
    let whole = fs::read(&path).unwrap();
    let complete = &whole[..whole.iter().rposition(|b| *b == b'\n').unwrap() + 1];

    let mut end = NEWEST;
    let mut pages = Vec::new();
    loop {
        let page = back(&history, &source, end, 300);
        assert!(page.chunks.iter().all(|c| c.complete));
        assert!(!page.chunks.is_empty());
        let bytes: Vec<u8> = page
            .chunks
            .iter()
            .flat_map(|c| STANDARD.decode(&c.raw_base64).unwrap())
            .collect();
        assert!(bytes.len() <= 300);
        pages.push(bytes);
        match page.previous {
            Some(previous) => end = previous,
            None => break,
        }
    }
    let joined: Vec<u8> = pages.into_iter().rev().flatten().collect();
    assert_eq!(joined, complete);

    // The newest page's forward cursor continues after its last record.
    let newest = back(&history, &source, NEWEST, 300);
    assert_eq!(newest.next.offset, complete.len() as u64);
    assert!(newest.has_more, "the partial record remains to read");
    let after = read(&history, &source, Some(newest.next), MAX_PAGE_BYTES);
    assert_eq!(after.chunks.len(), 1);
    assert!(!after.chunks[0].complete);

    // A backward read takes no cursor.
    assert!(
        history
            .transcript(TranscriptRequest {
                source_id: source.clone(),
                cursor: Some(after.next),
                max_bytes: 300,
                end: Some(NEWEST),
            })
            .is_err()
    );
}

#[test]
fn catalog_lists_newest_first_with_times_and_first_prompts() {
    let fixture = Fixture::new();
    let old = fixture.codex(
        "019c0000-0000-7000-8000-00000000000a",
        "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"<environment_context>cwd</environment_context>\"}]}}\n{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"Fix the flaky test\\nmore detail\"}]}}\n",
    );
    let new = fixture.codex("019c0000-0000-7000-8000-00000000000b", "");
    let set = |path: &Path, seconds: u64| {
        OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds))
            .unwrap();
    };
    set(&old, 1_700_000_000);
    set(&new, 1_800_000_000);
    let page = fixture
        .history()
        .catalog(CatalogRequest::default())
        .unwrap();
    assert_eq!(page.entries.len(), 2);
    assert_eq!(
        page.entries[0].updated_at.as_deref(),
        Some("2027-01-15T08:00:00Z")
    );
    assert_eq!(
        page.entries[1].updated_at.as_deref(),
        Some("2023-11-14T22:13:20Z")
    );
    assert_eq!(page.entries[1].title, "Fix the flaky test");
}

/// A synthetic Coder task transcript in the log's spelling (`crates/atif`).
fn atif_lines() -> String {
    [
        r#"{"record":"session","schema_version":"ATIF-v1.7","at":1790570162020,"session":{"id":"TASK-1","model":"synthetic","door":"synthetic","repository":"/synthetic","directive":"","state":"","seconds":0,"version":"0.1.0"}}"#,
        r#"{"record":"step","step":{"at":1790570162024,"source":"User","message":"Summarize the README\nwith detail"}}"#,
        r#"{"record":"step","step":{"at":1790570162027,"source":"System","message":"Repository adapter admitted by the local operator.","extensions":{"admission":{}}}}"#,
        r#"{"record":"step","step":{"at":1790570162100,"source":"Agent","message":"","call":{"id":"call-1","name":"shell","arguments":{"command":"head -1 README.md"},"output":"Heading: Synthetic","outcome":"Completed","milliseconds":4}}}"#,
        r#"{"record":"step","step":{"at":1790570162200,"source":"Agent","message":"The first heading is Synthetic."}}"#,
        r#"{"record":"end","at":1790570162300,"state":"ended"}"#,
    ]
    .map(|line| format!("{line}\n"))
    .concat()
}

fn coder_history(fixture: &Fixture) -> History {
    History::open(Config {
        codex: None,
        claude: None,
        coder: Some(fixture.0.clone()),
    })
    .unwrap()
}

#[test]
fn coder_tasks_list_flat_atif_attempts_newest_first() {
    let fixture = Fixture::new();
    let task = "ab".repeat(32);
    let older = fixture.write(&format!("{task}.1.atif.jsonl"), atif_lines());
    let newer = fixture.write(
        &format!("{task}.2.atif.jsonl"),
        r#"{"record":"session","at":1,"session":{"id":"x"}}"#.to_owned() + "\n",
    );
    // Neither task bookkeeping, other JSONL, nor nested transcripts are chats.
    fixture.write("tasks.json", b"{}");
    fixture.write(&format!("repository-launch-{task}.jsonl"), b"{}\n");
    fixture.write(&format!("nested/{task}.3.atif.jsonl"), atif_lines());
    let set = |path: &Path, seconds: u64| {
        OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds))
            .unwrap();
    };
    set(&older, 1_700_000_000);
    set(&newer, 1_800_000_000);
    let history = coder_history(&fixture);
    let page = history.catalog(CatalogRequest::default()).unwrap();
    assert_eq!(page.entries.len(), 2);
    let (first, second) = (&page.entries[0], &page.entries[1]);
    assert!(page.entries.iter().all(|c| c.harness == Harness::Coder
        && c.native_id.as_deref() == Some(task.as_str())
        && !c.archived
        && !c.subagent
        && c.status == SourceStatus::Available));
    assert_ne!(first.id, second.id, "each attempt is its own chat");
    assert_eq!(first.title, "Saved Coder chat");
    assert_eq!(first.updated_at.as_deref(), Some("2027-01-15T08:00:00Z"));
    assert_eq!(second.title, "Summarize the README");
    assert_eq!(second.updated_at.as_deref(), Some("2023-11-14T22:13:20Z"));
    // A second catalog rescans the same directory rather than resuming it.
    assert_eq!(history.catalog(CatalogRequest::default()).unwrap(), page);
}

#[test]
fn a_task_the_owner_archived_lists_as_an_archived_chat() {
    let fixture = Fixture::new();
    let (kept, archived) = ("ab".repeat(32), "cd".repeat(32));
    fixture.write(&format!("{kept}.1.atif.jsonl"), atif_lines());
    fixture.write(&format!("{archived}.1.atif.jsonl"), atif_lines());
    let history = coder_history(&fixture);
    let flags = |history: &History| {
        let page = history.catalog(CatalogRequest::default()).unwrap();
        let mut flags: Vec<(String, bool)> = page
            .entries
            .iter()
            .map(|c| (c.native_id.clone().unwrap(), c.archived))
            .collect();
        flags.sort();
        (flags, page.notices)
    };
    // Without a record nothing is archived.
    assert_eq!(
        flags(&history).0,
        [(kept.clone(), false), (archived.clone(), false)]
    );
    fixture.write(
        "archive.json",
        format!(
            r#"{{"schema":"openagents.coder.task-archive.v1","tasks":{{"{archived}":{{"at":5,"reason":"Test chat","by":{{"kind":"owner"}}}}}}}}"#
        ),
    );
    let (listed, notices) = flags(&history);
    assert_eq!(listed, [(kept.clone(), false), (archived.clone(), true)]);
    assert!(notices.is_empty());
    // A malformed record hides nothing and says so.
    fixture.write("archive.json", b"{\"schema\":\"other\",\"tasks\":{}}");
    let (listed, notices) = flags(&history);
    assert_eq!(listed, [(kept, false), (archived, false)]);
    assert_eq!(notices[0].code, "archive_unavailable");
}

#[test]
fn coder_steps_project_as_user_system_tool_and_assistant_records() {
    let fixture = Fixture::new();
    fixture.write(&format!("{}.1.atif.jsonl", "0".repeat(64)), atif_lines());
    let history = coder_history(&fixture);
    let source = first(&history).source_id.unwrap();
    let page = read(&history, &source, None, MAX_PAGE_BYTES);
    let views: Vec<_> = page
        .chunks
        .iter()
        .map(|c| c.readable.clone().unwrap())
        .collect();
    assert_eq!(views.len(), 6);
    assert!(views.iter().all(|v| !v.unknown));
    fn shape(v: &Readable) -> (&str, Option<&str>, Option<&str>) {
        (v.kind.as_str(), v.role.as_deref(), v.tool_name.as_deref())
    }
    assert_eq!(shape(&views[0]), ("session", None, None));
    assert_eq!(views[0].native_id.as_deref(), Some("TASK-1"));
    assert_eq!(views[0].text, "");
    assert_eq!(
        views[0].timestamp.as_deref(),
        Some("2026-09-28T04:36:02.020Z")
    );
    assert_eq!(shape(&views[1]), ("message", Some("user"), None));
    assert_eq!(views[1].text, "Summarize the README\nwith detail");
    assert_eq!(shape(&views[2]), ("adapter", None, None));
    assert_eq!(views[2].text, "");
    assert_eq!(shape(&views[3]), ("tool_call", None, Some("shell")));
    assert_eq!(views[3].call_id.as_deref(), Some("call-1"));
    assert_eq!(views[3].text, "head -1 README.md\n\nHeading: Synthetic");
    assert_eq!(shape(&views[4]), ("message", Some("assistant"), None));
    assert_eq!(views[4].text, "The first heading is Synthetic.");
    assert_eq!(shape(&views[5]), ("end", None, None));
}

#[test]
fn exported_atif_tool_calls_and_observations_project_and_unknown_records_stay_unknown() {
    let project = |value: serde_json::Value| readable_record(value.to_string().as_bytes()).unwrap();
    let both = project(serde_json::json!({"record": "step", "step": {
        "at": 0, "source": "Agent", "message": "Listing files.",
        "tool_calls": [{"tool_call_id": "c1", "function_name": "shell", "arguments": {"command": "ls"}}],
        "observation": {"results": [{"source_call_id": "c1", "content": "README.md"}]}
    }}));
    assert_eq!(both.kind, "message");
    assert_eq!(both.role.as_deref(), Some("assistant"));
    assert_eq!(both.tool_name.as_deref(), Some("shell"));
    assert_eq!(both.text, "Listing files.\n\nTool: shell ls");
    let result = project(serde_json::json!({"record": "step", "step": {
        "at": 0, "source": "Agent", "message": "",
        "observation": {"results": [{"source_call_id": "c1", "content": "README.md"}]}
    }}));
    assert_eq!(result.kind, "tool_result");
    assert_eq!(result.role.as_deref(), Some("tool"));
    assert_eq!(result.call_id.as_deref(), Some("c1"));
    assert_eq!(result.text, "README.md");
    for unknown in [
        serde_json::json!({"record": "surprise"}),
        serde_json::json!({"record": "step", "step": {"source": "Robot", "message": "x"}}),
        serde_json::json!({"record": "step", "step": "not an object"}),
    ] {
        assert!(project(unknown).unknown);
    }
    assert!(readable_record(b"{\"record\":\"step\"").unwrap().unknown);
}

#[test]
fn coder_transcripts_page_backward_over_whole_records() {
    let fixture = Fixture::new();
    let path = fixture.write(&format!("{}.1.atif.jsonl", "f".repeat(64)), atif_lines());
    let history = coder_history(&fixture);
    let source = first(&history).source_id.unwrap();
    let mut end = NEWEST;
    let mut pages = Vec::new();
    loop {
        let page = back(&history, &source, end, 300);
        assert!(
            page.chunks
                .iter()
                .all(|c| c.complete && c.readable.is_some())
        );
        pages.push(
            page.chunks
                .iter()
                .flat_map(|c| STANDARD.decode(&c.raw_base64).unwrap())
                .collect::<Vec<_>>(),
        );
        match page.previous {
            Some(previous) => end = previous,
            None => break,
        }
    }
    assert!(pages.len() > 1);
    let joined: Vec<u8> = pages.into_iter().rev().flatten().collect();
    assert_eq!(joined, fs::read(&path).unwrap());
    let newest = back(&history, &source, NEWEST, 300);
    assert_eq!(
        newest
            .chunks
            .last()
            .unwrap()
            .readable
            .as_ref()
            .unwrap()
            .kind,
        "end"
    );
}

#[test]
fn coder_loop_events_project_as_replies_commands_and_endings() {
    let system = |extensions: serde_json::Value| {
        let line = serde_json::json!({"record": "step", "step": {
            "at": 1_790_570_162_027u64, "source": "System",
            "message": "Microcoder loop observation.", "extensions": extensions
        }});
        readable_record(line.to_string().as_bytes()).unwrap()
    };
    let event = |event: serde_json::Value| {
        system(serde_json::json!({"microcoder": {"seconds": 1.5, "event": event}}))
    };
    let view = |r: &Readable| {
        (
            r.kind.clone(),
            r.role.clone(),
            r.tool_name.clone(),
            r.text.clone(),
            r.unknown,
        )
    };
    let limit = "{\"error\":{\"type\":\"usage_limit_reached\",\"message\":\"The usage limit has been reached\",\"plan_type\":\"pro\",\"resets_at\":1791050824,\"eligible_promo\":null,\"limit_window_minutes\":10080,\"resets_in_seconds\":480645}}";
    let error = format!("the provider returned HTTP 429: {limit}");
    let generated = |action: serde_json::Value| {
        serde_json::json!({"event": "generated", "step": 1, "prompt_chars": 1464, "generated": {
            "action": action, "model": "synthetic", "prompt_tokens": 0, "completion_tokens": 0,
            "usd": 0.0, "known_usd": 0.0, "cost_unknown": null, "usd_upper": 0.0,
            "cost_basis": "list_price", "milliseconds": 16844
        }})
    };
    let s = |text: &str| Some(text.to_owned());

    // The shapes of a real run that failed on a provider usage limit.
    let failed = view(&event(generated(serde_json::json!({"Err": error}))));
    assert_eq!(
        (&failed.0, &failed.1, failed.4),
        (&"message".to_owned(), &s("system"), false)
    );
    assert!(
        failed
            .3
            .starts_with("The model call failed: the provider returned HTTP 429: {")
    );
    assert!(failed.3.len() <= "The model call failed: ".len() + 300 + "…".len());
    assert_eq!(
        view(&event(serde_json::json!({"event": "ended", "outcome": {
            "ending": {"reason": "bad_replies", "detail": error}, "steps": 3, "seconds": 48.7
        }}))),
        (
            "message".into(),
            s("system"),
            None,
            format!("Coder stopped: the provider returned HTTP 429: {limit}"),
            false
        )
    );

    // A run that worked, built from microcoder's structs.
    assert_eq!(
        view(&event(generated(serde_json::json!({"Ok": {
            "rationale": "Read the README heading.", "commands": ["head -1 README.md"],
            "view": [], "freeze_tests": false, "expand": [], "finished": false
        }})))),
        (
            "message".into(),
            s("assistant"),
            None,
            "Read the README heading.".into(),
            false
        )
    );
    assert_eq!(
        view(&event(generated(serde_json::json!({"Ok": {
            "rationale": "The heading is Synthetic.", "commands": [], "finished": true
        }}))))
        .3,
        "The heading is Synthetic.\nFinished."
    );
    let ran = |exit: serde_json::Value, timed_out: bool| {
        view(&event(
            serde_json::json!({"event": "ran", "step": 1, "result": {
                "command": "head -1 README.md", "exit": exit, "timed_out": timed_out,
                "seconds": 0.01, "output": "# Synthetic\n"
            }}),
        ))
    };
    assert_eq!(
        ran(serde_json::json!(0), false),
        (
            "tool_call".into(),
            None,
            s("shell"),
            "head -1 README.md\n\n# Synthetic\n".into(),
            false
        )
    );
    assert_eq!(
        ran(serde_json::json!(2), false).3,
        "head -1 README.md\nexit 2\n\n# Synthetic\n"
    );
    assert_eq!(
        ran(serde_json::Value::Null, true).3,
        "head -1 README.md\ntimed out\n\n# Synthetic\n"
    );
    assert_eq!(
        view(&event(
            serde_json::json!({"event": "tested", "step": 2, "froze": true, "results": [
                {"command": "cargo test\n# more", "exit": 0, "timed_out": false, "seconds": 1.0, "output": ""},
                {"command": "./check.sh", "exit": 1, "timed_out": false, "seconds": 1.0, "output": "no"}
            ]})
        )),
        (
            "tool_call".into(),
            None,
            s("tests"),
            "cargo test: exit 0\n./check.sh: exit 1".into(),
            false
        )
    );
    let ended = |ending: serde_json::Value, steps: u64| {
        view(&event(serde_json::json!({"event": "ended", "outcome": {
            "ending": ending, "steps": steps, "seconds": 9.0
        }})))
        .3
    };
    assert_eq!(
        ended(serde_json::json!({"reason": "finished"}), 2),
        "Coder finished in 2 steps."
    );
    assert_eq!(
        ended(serde_json::json!({"reason": "finished"}), 1),
        "Coder finished in 1 step."
    );
    assert_eq!(
        ended(serde_json::json!({"reason": "step_limit"}), 24),
        "Coder stopped: step limit"
    );
    assert_eq!(
        ended(serde_json::json!({"reason": "tests_held"}), 9),
        "Coder stopped: tests held"
    );

    // Host evidence and the loop's other events are adapter records.
    for evidence in [
        system(
            serde_json::json!({"admission": {"grant": {}}, "controller": {}, "capabilities": {}}),
        ),
        system(
            serde_json::json!({"effect_result": {"sequence": 3, "kind": "codex_request",
            "result": {"error": error}}}),
        ),
        system(serde_json::json!({"decision_response": {"model": "jev", "status": 200}})),
        system(serde_json::json!({"adapter_summary": {}, "host_fault": null})),
        event(serde_json::json!({"event": "judged", "step": 1, "judgment": {}})),
        event(serde_json::json!({"event": "gated", "step": 1, "checked": {}})),
    ] {
        assert_eq!(
            view(&evidence),
            ("adapter".into(), None, None, String::new(), false)
        );
    }
    // A system step without host evidence is still a system message.
    let plain = readable_record(
        br#"{"record":"step","step":{"at":1,"source":"System","message":"Note."}}"#,
    )
    .unwrap();
    assert_eq!(
        view(&plain),
        ("message".into(), s("system"), None, "Note.".into(), false)
    );
}

#[test]
fn a_codex_session_the_engine_started_is_not_a_chat_and_a_spawned_thread_is_a_subagent() {
    let fixture = Fixture::new();
    let mark = crate::engine::MARK;
    fixture.codex("typed", "");
    fixture.write(
        "sessions/2026/01/01/rollout-engine.jsonl",
        format!(
            "{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"engine\",\"originator\":\"{mark}\",\"source\":\"exec\"}}}}\n"
        ),
    );
    fixture.write(
        "sessions/2026/01/01/rollout-exec.jsonl",
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"exec\",\"originator\":\"codex_exec\",\"source\":\"exec\"}}\n",
    );
    fixture.write(
        "sessions/2026/01/01/rollout-thread.jsonl",
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"thread\",\"originator\":\"codex-tui\",\"source\":{\"subagent\":{\"thread_spawn\":{\"parent_thread_id\":\"typed\"}}}}}\n",
    );
    // A title in the index does not bring the engine's session back as a
    // missing chat.
    fixture.write(
        "session_index.jsonl",
        "{\"id\":\"engine\",\"thread_name\":\"Engine work\"}\n",
    );
    let page = fixture
        .history()
        .catalog(CatalogRequest::default())
        .unwrap();
    let mut listed: Vec<(String, bool)> = page
        .entries
        .iter()
        .map(|c| (c.native_id.clone().unwrap(), c.subagent))
        .collect();
    listed.sort();
    assert_eq!(
        listed,
        [
            ("exec".to_owned(), false),
            ("thread".to_owned(), true),
            ("typed".to_owned(), false),
        ]
    );
}

#[test]
fn a_claude_session_the_engine_started_is_not_a_chat() {
    let fixture = Fixture::new();
    let mark = crate::engine::MARK;
    // Claude Code's first records can carry no entry point, and the first
    // user record, which does, holds the whole briefing.
    let queued =
        "{\"type\":\"queue-operation\",\"operation\":\"enqueue\",\"sessionId\":\"{id}\"}\n";
    let user = |id: &str, entrypoint: &str| {
        format!(
            "{}{{\"type\":\"user\",\"sessionId\":\"{id}\",\"message\":{{\"role\":\"user\",\"content\":\"{}\"}},\"entrypoint\":\"{entrypoint}\"}}\n",
            queued.replace("{id}", id),
            "x".repeat(200 * 1024)
        )
    };
    fixture.write("projects/p/engine.jsonl", user("engine", mark));
    fixture.write("projects/p/print.jsonl", user("print", "sdk-cli"));
    fixture.write("projects/p/typed.jsonl", user("typed", "cli"));
    let history = History::open(Config {
        codex: None,
        claude: Some(fixture.0.clone()),
        coder: None,
    })
    .unwrap();
    let page = history.catalog(CatalogRequest::default()).unwrap();
    let mut listed: Vec<String> = page
        .entries
        .iter()
        .map(|c| c.native_id.clone().unwrap())
        .collect();
    listed.sort();
    assert_eq!(listed, ["print", "typed"]);
}
