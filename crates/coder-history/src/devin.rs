//! The host's mirror of the Devin CLI's saved sessions.
//!
//! The Devin CLI keeps its local sessions in a SQLite store,
//! `devin/cli/sessions.db` in its data directory, not in files a reader can
//! page by byte offset. The catalog and transcript readers page append-only
//! JSONL files, so the host mirrors each Devin session into one:
//! `<mirror>/<session id>.jsonl`, plus a `session_index.jsonl` of titles in
//! the shape Codex's index has. [`crate::Config::devin`] names the mirror
//! directory, and the catalog lists it as [`crate::Harness::Devin`].
//!
//! Devin stores a session's messages as a forest of nodes. The session's
//! `main_chain_id` names the newest node of the conversation, and each node
//! names its parent. When Devin compacts a conversation, the new chain starts
//! with a summary node whose `summarized_from` names the old chain's newest
//! node, and the messages it carries over are new nodes whose
//! `compact/prior_node_ids` name the old ones. The mirror reads the
//! conversation as the old chain, then the new one, and calls a carried node
//! by the first node it copies, so a compaction only appends.
//!
//! The mirror opens the store read-only and never writes it, never reads
//! Devin's credentials, and writes only inside the mirror directory. A
//! session's file is append-only while its conversation only grows:
//!
//! 1. The first line is the `devin.session` header: the session ID, its
//!    directory, its model, and when it was created.
//! 2. Each message of the conversation follows as one or more `devin.item`
//!    lines, in order: a message the owner typed (`message`, role `user`);
//!    an assistant message's `reasoning`, its reply (`message`, role
//!    `assistant`), and each `tool_call`; and a tool's `tool_result`.
//!    Devin's system prompts and the prompts it writes to itself (a
//!    compaction's request, a cache keep-alive) are left out.
//!
//! A session whose conversation no longer starts with what the file holds
//! (the owner rewound or edited it) is written again from its start, as a
//! new file, so a reader's cursor on the old one reports the source changed.
//! A session removed from the store loses its mirror file. Each file's
//! modification time is the session's last activity, so the catalog orders
//! mirrored chats by their own activity.
//!
//! Large fields are cut before they are written: a text or reasoning keeps
//! its first [`TEXT_BYTES`], a tool's result and each string of a call's
//! arguments keep their first [`TOOL_OUTPUT_BYTES`], and a line still longer
//! than [`LINE_BYTES`] keeps only its item kind. A cut item says so
//! (`openagents_clipped`).
//!
//! Sessions Coder's engine starts carry the engine mark in their `session/new`
//! `_meta`, which Devin keeps as the session's
//! `metadata.client_meta["openagents.com/engine"]`. The mirror never writes
//! such a session, so it never lists as a chat.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

/// The title index in the mirror directory, in Codex's index shape.
pub const INDEX: &str = "session_index.jsonl";
/// The mirror's own record of how far each session is written.
pub const STATE: &str = "mirror-state.json";
const STATE_SCHEMA: &str = "openagents.coder.devin-mirror.v1";
/// The most bytes of a tool's result, or of one string in a call's
/// arguments, a mirrored item keeps.
pub const TOOL_OUTPUT_BYTES: usize = 16 * 1024;
/// The most bytes of a text or reasoning a mirrored item keeps.
pub const TEXT_BYTES: usize = 64 * 1024;
/// The longest mirrored line; a longer item keeps only its kind.
pub const LINE_BYTES: usize = 200 * 1024;
/// The mirror directory under the home directory.
pub const MIRROR: &str = ".openagents/devin/mirror";

/// The owner's Devin session store: `devin/cli/sessions.db` in the data
/// directory, `$XDG_DATA_HOME`, else `~/.local/share`.
#[must_use]
pub fn database(home: &Path, xdg_data_home: Option<&Path>) -> PathBuf {
    xdg_data_home
        .filter(|path| path.is_absolute())
        .map_or_else(|| home.join(".local/share"), Path::to_path_buf)
        .join("devin")
        .join("cli")
        .join("sessions.db")
}

/// The owner's Devin session store from this process's environment.
#[must_use]
pub fn default_database() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let xdg = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from);
    Some(database(&home, xdg.as_deref()))
}

/// The mirror directory under `home`.
#[must_use]
pub fn default_mirror(home: &Path) -> PathBuf {
    home.join(MIRROR)
}

/// What one pass did.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Report {
    /// Sessions the owner started (engine sessions are not counted).
    pub sessions: usize,
    /// Sessions whose file changed.
    pub updated: usize,
    /// Lines appended.
    pub lines: usize,
    /// Sessions written again from their start.
    pub rewritten: usize,
    /// Mirror files removed with their sessions.
    pub removed: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct State {
    schema: String,
    sessions: BTreeMap<String, Progress>,
}

/// How far one session is written.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Progress {
    /// The session's `last_activity_at` and `main_chain_id` when it was last
    /// written through.
    activity: i64,
    head: i64,
    /// How many messages the file holds, and the digest of their node IDs
    /// in order.
    messages: usize,
    digest: String,
}

struct Session {
    id: String,
    directory: String,
    model: String,
    title: Option<String>,
    created: i64,
    activity: i64,
    head: Option<i64>,
    hidden: bool,
    engine: bool,
}

/// Brings the mirror in `dir` up to date with `database`. A missing store is
/// an empty one: the pass writes nothing.
///
/// # Errors
///
/// When the store can't be read or the mirror can't be written.
pub fn mirror(database: &Path, dir: &Path) -> Result<Report, String> {
    static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _one = ONE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if !database.is_file() {
        return Ok(Report::default());
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let connection = Connection::open_with_flags(
        database,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("cannot open {}: {e}", database.display()))?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let sessions: Vec<Session> = sessions(&connection)?
        .into_iter()
        .filter(|s| !s.engine)
        .collect();
    let mut state = read_state(dir);
    let mut report = Report {
        sessions: sessions.len(),
        ..Report::default()
    };
    for session in &sessions {
        let Some(head) = session.head else { continue };
        let path = dir.join(format!("{}.jsonl", session.id));
        let known = state
            .sessions
            .get(&session.id)
            .cloned()
            .filter(|_| path.is_file());
        if known
            .as_ref()
            .is_some_and(|k| k.activity == session.activity && k.head == head)
        {
            continue;
        }
        let (progress, lines, rewritten) =
            sync(&connection, session, head, &path, known.unwrap_or_default())?;
        report.updated += usize::from(lines > 0 || rewritten);
        report.lines += lines;
        report.rewritten += usize::from(rewritten);
        state.sessions.insert(session.id.clone(), progress);
    }
    let present: BTreeSet<&str> = sessions
        .iter()
        .filter(|s| s.head.is_some())
        .map(|s| s.id.as_str())
        .collect();
    let gone: Vec<String> = state
        .sessions
        .keys()
        .filter(|id| !present.contains(id.as_str()))
        .cloned()
        .collect();
    for id in gone {
        state.sessions.remove(&id);
        if std::fs::remove_file(dir.join(format!("{id}.jsonl"))).is_ok() {
            report.removed += 1;
        }
    }
    write_index(dir, &sessions)?;
    state.schema = STATE_SCHEMA.into();
    write_atomic(
        &dir.join(STATE),
        &serde_json::to_vec(&state).map_err(|e| e.to_string())?,
    )?;
    Ok(report)
}

fn sessions(connection: &Connection) -> Result<Vec<Session>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, working_directory, model, title, created_at, last_activity_at, \
             main_chain_id, hidden, metadata FROM sessions ORDER BY id",
        )
        .map_err(|e| format!("cannot list Devin sessions: {e}"))?;
    let rows = statement
        .query_map([], |row| {
            let metadata: Option<String> = row.get(8)?;
            let engine = metadata
                .as_deref()
                .and_then(|m| serde_json::from_str::<Value>(m).ok())
                .and_then(|m| {
                    m.pointer("/client_meta")
                        .and_then(|c| c.get(crate::engine::DEVIN_META_KEY))
                        .and_then(Value::as_str)
                        .map(|mark| mark == crate::engine::MARK)
                })
                .unwrap_or(false);
            Ok(Session {
                id: row.get(0)?,
                directory: row.get(1)?,
                model: row.get(2)?,
                title: row.get(3)?,
                created: row.get(4)?,
                activity: row.get(5)?,
                head: row.get(6)?,
                hidden: row.get::<_, Option<i64>>(7)?.unwrap_or(0) != 0,
                engine,
            })
        })
        .map_err(|e| format!("cannot list Devin sessions: {e}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map(|all| all.into_iter().filter(|s| safe_id(&s.id)).collect())
        .map_err(|e| format!("cannot read a Devin session: {e}"))
}

/// An ID that is safe as a file name: Devin's lowercase words and hyphens.
fn safe_id(id: &str) -> bool {
    crate::devin_session_id(id)
}

/// One node as the walk needs it: no message text.
struct Node {
    parent: Option<i64>,
    /// A compaction's summary node names the old chain's newest node.
    summarized_from: Option<i64>,
    /// A carried node names the node it copies.
    prior: Option<i64>,
    /// A message the mirror writes: the owner's own, the assistant's, or a
    /// tool's.
    written: bool,
}

fn nodes(connection: &Connection, session: &str) -> Result<HashMap<i64, Node>, String> {
    let mut statement = connection
        .prepare(
            "SELECT node_id, parent_node_id, metadata, json_extract(chat_message, '$.role'), \
             json_extract(chat_message, '$.metadata.is_user_input') \
             FROM message_nodes WHERE session_id = ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map(params![session], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<i64>>(4)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut out = HashMap::new();
    for row in rows {
        let (id, parent, metadata, role, typed) = row.map_err(|e| e.to_string())?;
        let metadata = metadata
            .as_deref()
            .and_then(|m| serde_json::from_str::<Value>(m).ok())
            .unwrap_or(Value::Null);
        let written = match role.as_deref() {
            Some("assistant" | "tool") => true,
            Some("user") => typed == Some(1),
            _ => false,
        };
        out.insert(
            id,
            Node {
                parent,
                summarized_from: metadata.get("summarized_from").and_then(Value::as_i64),
                prior: metadata
                    .pointer("/extensions/compact~1prior_node_ids/0")
                    .and_then(Value::as_i64),
                written,
            },
        );
    }
    Ok(out)
}

/// The conversation ending at `head`, oldest first: each written node with
/// the node it is known by (a carried node by the first node it copies).
fn conversation(nodes: &HashMap<i64, Node>, head: i64) -> Vec<(i64, i64)> {
    fn walk(nodes: &HashMap<i64, Node>, head: i64, heads: &mut HashSet<i64>, out: &mut Vec<i64>) {
        if !heads.insert(head) {
            return;
        }
        let mut chain = Vec::new();
        let mut seen = HashSet::new();
        let mut at = Some(head);
        while let Some(id) = at {
            let Some(node) = nodes.get(&id).filter(|_| seen.insert(id)) else {
                break;
            };
            chain.push(id);
            at = node.parent;
        }
        for id in chain.into_iter().rev() {
            let node = &nodes[&id];
            if let Some(earlier) = node.summarized_from {
                walk(nodes, earlier, heads, out);
            }
            if node.written {
                out.push(id);
            }
        }
    }
    let known_as = |mut id: i64| {
        let mut seen = HashSet::new();
        while seen.insert(id)
            && let Some(prior) = nodes.get(&id).and_then(|n| n.prior)
        {
            id = prior;
        }
        id
    };
    let mut ids = Vec::new();
    walk(nodes, head, &mut HashSet::new(), &mut ids);
    let mut named = HashSet::new();
    ids.into_iter()
        .map(|id| (id, known_as(id)))
        .filter(|(_, name)| named.insert(*name))
        .collect()
}

fn digest(names: &[(i64, i64)]) -> String {
    let mut hash = Sha256::new();
    for (_, name) in names {
        hash.update(name.to_le_bytes());
    }
    hash.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Writes one session's new lines. Returns its progress, the lines written,
/// and whether the file was started again.
fn sync(
    connection: &Connection,
    session: &Session,
    head: i64,
    path: &Path,
    known: Progress,
) -> Result<(Progress, usize, bool), String> {
    let nodes = nodes(connection, &session.id)?;
    let conversation = conversation(&nodes, head);
    let exists = path.is_file();
    let continues = exists
        && known.messages <= conversation.len()
        && digest(&conversation[..known.messages]) == known.digest;
    let rewritten = exists && !continues;
    let from = if continues { known.messages } else { 0 };
    let mut lines: Vec<Vec<u8>> = Vec::new();
    if !continues {
        lines.push(line(&json!({
            "type": "devin.session",
            "session_id": session.id,
            "directory": session.directory,
            "model": session.model,
            "time": session.created.saturating_mul(1000),
        })));
    }
    let mut message = connection
        .prepare(
            "SELECT chat_message, created_at FROM message_nodes \
             WHERE session_id = ?1 AND node_id = ?2",
        )
        .map_err(|e| e.to_string())?;
    for (id, name) in &conversation[from..] {
        let (raw, created): (String, i64) = message
            .query_row(params![session.id, id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .map_err(|e| e.to_string())?;
        let value: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
        for item in items(&value) {
            let mut entry = json!({
                "type": "devin.item",
                "session_id": session.id,
                "node_id": name,
                "time": created.saturating_mul(1000),
            });
            if let (Some(map), Value::Object(item)) = (entry.as_object_mut(), item) {
                map.extend(item);
            }
            lines.push(line(&bounded(entry)));
        }
    }
    let written = lines.len();
    if !continues {
        let mut bytes = Vec::new();
        for line in &lines {
            bytes.extend_from_slice(line);
        }
        write_atomic(path, &bytes)?;
    } else if !lines.is_empty() {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .map_err(|e| format!("cannot append to {}: {e}", path.display()))?;
        for line in &lines {
            file.write_all(line)
                .map_err(|e| format!("cannot append to {}: {e}", path.display()))?;
        }
    }
    touch(path, session.activity.saturating_mul(1000));
    Ok((
        Progress {
            activity: session.activity,
            head,
            messages: conversation.len(),
            digest: digest(&conversation),
        },
        written,
        rewritten,
    ))
}

/// A message's items, in order, each with its `item` kind; see the module
/// documentation.
fn items(message: &Value) -> Vec<Value> {
    let text = |name: &str| {
        message
            .get(name)
            .and_then(Value::as_str)
            .filter(|t| !t.trim().is_empty())
    };
    let mut out = Vec::new();
    match message.get("role").and_then(Value::as_str) {
        Some("user") => {
            out.push(json!({"item": "message", "role": "user", "text": text("content").unwrap_or_default()}));
        }
        Some("assistant") => {
            if let Some(thinking) = message
                .pointer("/thinking/thinking")
                .and_then(Value::as_str)
                .filter(|t| !t.trim().is_empty())
            {
                out.push(json!({"item": "reasoning", "text": thinking}));
            }
            if let Some(reply) = text("content") {
                out.push(json!({"item": "message", "role": "assistant", "text": reply}));
            }
            for call in message
                .get("tool_calls")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                out.push(json!({
                    "item": "tool_call",
                    "tool": call.get("name"),
                    "call_id": call.get("id"),
                    "arguments": call.get("arguments"),
                }));
            }
        }
        Some("tool") => {
            out.push(json!({
                "item": "tool_result",
                "call_id": message.get("tool_call_id"),
                "text": message.get("content").and_then(Value::as_str).unwrap_or_default(),
            }));
        }
        _ => {}
    }
    out
}

/// An item with its large fields cut; see the module documentation.
fn bounded(mut entry: Value) -> Value {
    let mut clipped = false;
    let mut cut = |value: &mut Value, max: usize| {
        if let Value::String(text) = value
            && text.len() > max
        {
            let mut end = max;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
            clipped = true;
        }
    };
    let max = if entry.get("item").and_then(Value::as_str) == Some("tool_result") {
        TOOL_OUTPUT_BYTES
    } else {
        TEXT_BYTES
    };
    if let Some(text) = entry.get_mut("text") {
        cut(text, max);
    }
    if let Some(Value::Object(arguments)) = entry.get_mut("arguments") {
        for value in arguments.values_mut() {
            cut(value, TOOL_OUTPUT_BYTES);
        }
    }
    if serde_json::to_vec(&entry).map_or(0, |b| b.len()) > LINE_BYTES {
        entry = json!({
            "type": entry.get("type"),
            "session_id": entry.get("session_id"),
            "node_id": entry.get("node_id"),
            "time": entry.get("time"),
            "item": entry.get("item"),
        });
        clipped = true;
    }
    if clipped && let Some(map) = entry.as_object_mut() {
        map.insert("openagents_clipped".into(), json!(true));
    }
    entry
}

fn line(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

fn write_index(dir: &Path, sessions: &[Session]) -> Result<(), String> {
    let mut bytes = Vec::new();
    for session in sessions.iter().filter(|s| s.head.is_some()) {
        let Some(title) = session.title.as_deref().filter(|t| !t.trim().is_empty()) else {
            continue;
        };
        bytes.extend(line(&json!({
            "id": session.id,
            "thread_name": title,
            "updated_at": crate::project::utc(u64::try_from(session.activity).unwrap_or(0)),
            "archived": session.hidden,
        })));
    }
    let path = dir.join(INDEX);
    if std::fs::read(&path).is_ok_and(|old| old == bytes) {
        return Ok(());
    }
    write_atomic(&path, &bytes)
}

fn read_state(dir: &Path) -> State {
    std::fs::read(dir.join(STATE))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<State>(&bytes).ok())
        .filter(|state| state.schema == STATE_SCHEMA)
        .unwrap_or_default()
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&temporary, bytes)
        .and_then(|()| std::fs::rename(&temporary, path))
        .map_err(|e| {
            let _ = std::fs::remove_file(&temporary);
            format!("cannot write {}: {e}", path.display())
        })
}

/// Sets the file's modification time to `millis`, the session's last
/// activity.
fn touch(path: &Path, millis: i64) {
    let Ok(millis) = u64::try_from(millis) else {
        return;
    };
    let when = std::time::UNIX_EPOCH + std::time::Duration::from_millis(millis);
    if let Ok(file) = std::fs::OpenOptions::new().append(true).open(path) {
        let _ = file.set_modified(when);
    }
}

#[cfg(test)]
mod tests;
