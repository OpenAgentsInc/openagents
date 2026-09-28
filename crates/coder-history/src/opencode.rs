//! The host's mirror of OpenCode's saved sessions.
//!
//! OpenCode (1.2 and later) keeps its sessions in a SQLite database,
//! `opencode.db` in its data directory, not in files a reader can page by
//! byte offset. The catalog and transcript readers page append-only JSONL
//! files, so the host mirrors each OpenCode session into one:
//! `<mirror>/<session id>.jsonl`, plus a `session_index.jsonl` of titles in
//! the shape Codex's index has. [`crate::Config::opencode`] names the mirror
//! directory, and the catalog lists it as [`crate::Harness::OpenCode`].
//!
//! The mirror opens the database read-only and never writes it, never reads
//! OpenCode's credentials, and writes only inside the mirror directory. A
//! session's file is append-only while the session only grows:
//!
//! 1. The first line is the `opencode.session` header: the session ID, its
//!    parent (a subagent's session has one), its directory, and when it was
//!    created.
//! 2. Each finished part follows as an `opencode.part` line, in OpenCode's
//!    order (messages by creation, parts by ID). A part is finished when its
//!    text or reasoning has ended, its tool call completed or failed, or its
//!    message completed. The mirror stops at the first unfinished part, so
//!    a running reply appears part by part and never out of order.
//! 3. An assistant message that ended in an error adds an `opencode.error`
//!    line after its parts.
//!
//! Large fields are cut before they are written: a tool's output keeps its
//! first [`TOOL_OUTPUT_BYTES`], a text keeps its first [`TEXT_BYTES`], a
//! tool's metadata and attachments and a file part's inline data are left
//! out, and a line still longer than [`LINE_BYTES`] keeps only the part's
//! type. A cut part says so (`openagents_clipped`).
//!
//! A session whose recorded place no longer exists in the database (OpenCode
//! reverted or removed messages) is written again from its start, as a new
//! file, so a reader's cursor on the old one reports the source changed. A
//! session removed from the database loses its mirror file. Each file's
//! modification time is the session's last update, so the catalog orders
//! mirrored chats by their own activity.
//!
//! Sessions Coder's engine starts are saved in the engine's own database
//! ([`crate::engine::OPENCODE_DATABASE`], beside the owner's), which the
//! mirror never reads, so they never list as chats.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension as _, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The title index in the mirror directory, in Codex's index shape.
pub const INDEX: &str = "session_index.jsonl";
/// The mirror's own record of how far each session is written.
pub const STATE: &str = "mirror-state.json";
const STATE_SCHEMA: &str = "openagents.coder.opencode-mirror.v1";
/// The most bytes of a tool's output a mirrored part keeps.
pub const TOOL_OUTPUT_BYTES: usize = 16 * 1024;
/// The most bytes of a text or reasoning part a mirrored part keeps.
pub const TEXT_BYTES: usize = 64 * 1024;
/// The longest mirrored line; a longer part keeps only its type.
pub const LINE_BYTES: usize = 200 * 1024;
/// The mirror directory under the home directory.
pub const MIRROR: &str = ".openagents/opencode/mirror";

/// The owner's OpenCode database: `opencode.db` in OpenCode's data
/// directory, `$XDG_DATA_HOME/opencode`, else `~/.local/share/opencode`.
#[must_use]
pub fn database(home: &Path, xdg_data_home: Option<&Path>) -> PathBuf {
    xdg_data_home
        .filter(|path| path.is_absolute())
        .map_or_else(|| home.join(".local/share"), Path::to_path_buf)
        .join("opencode")
        .join("opencode.db")
}

/// The owner's OpenCode database from this process's environment.
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
    /// Sessions in the database.
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
    /// The session's `time_updated` when it was last written through.
    updated: i64,
    /// The message the file has reached, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    at: Option<Place>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Place {
    /// The message's creation time and ID: OpenCode's message order.
    created: i64,
    message: String,
    /// The last part written; none yet when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    part: Option<String>,
    /// Every part of the message and its error line are written.
    closed: bool,
}

struct Session {
    id: String,
    parent: Option<String>,
    directory: String,
    title: String,
    version: String,
    created: i64,
    updated: i64,
    archived: bool,
}

/// Brings the mirror in `dir` up to date with `database`. A missing
/// database is an empty one: the pass writes nothing.
///
/// # Errors
///
/// When the database can't be read or the mirror can't be written.
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
    let sessions = sessions(&connection)?;
    let mut state = read_state(dir);
    let mut report = Report {
        sessions: sessions.len(),
        ..Report::default()
    };
    for session in &sessions {
        let path = dir.join(format!("{}.jsonl", session.id));
        let known = state.sessions.get(&session.id).cloned();
        if known.as_ref().is_some_and(|k| k.updated == session.updated) && path.is_file() {
            continue;
        }
        let progress = known.filter(|_| path.is_file()).unwrap_or_default();
        let (progress, lines, rewritten) = sync(&connection, session, &path, progress)?;
        report.updated += 1;
        report.lines += lines;
        report.rewritten += usize::from(rewritten);
        state.sessions.insert(session.id.clone(), progress);
    }
    let present: std::collections::BTreeSet<&str> =
        sessions.iter().map(|s| s.id.as_str()).collect();
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

/// The error the newest message of `session` in `database` ended with,
/// as OpenCode saved it (`name`, and `data` with the provider's
/// `statusCode` and `responseHeaders` for an `APIError`), or `None` when the
/// newest message has none or the database can't be read. Opened read-only.
///
/// OpenCode's ACP server reports only the error's name and message when a
/// prompt fails; the engine reads the status and headers here, from its own
/// database, to tell a rate limit from any other failure.
#[must_use]
pub fn last_error(database: &Path, session: &str) -> Option<Value> {
    if !database.is_file() {
        return None;
    }
    let connection = Connection::open_with_flags(
        database,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .ok()?;
    let data: String = connection
        .query_row(
            "SELECT data FROM message WHERE session_id = ?1 \
             ORDER BY time_created DESC, id DESC LIMIT 1",
            params![session],
            |row| row.get(0),
        )
        .optional()
        .ok()??;
    let mut data: Value = serde_json::from_str(&data).ok()?;
    data.get_mut("error").map(Value::take)
}

fn sessions(connection: &Connection) -> Result<Vec<Session>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, parent_id, directory, title, version, time_created, time_updated, \
             time_archived FROM session ORDER BY id",
        )
        .map_err(|e| format!("cannot list OpenCode sessions: {e}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok(Session {
                id: row.get(0)?,
                parent: row.get(1)?,
                directory: row.get(2)?,
                title: row.get(3)?,
                version: row.get(4)?,
                created: row.get(5)?,
                updated: row.get(6)?,
                archived: row.get::<_, Option<i64>>(7)?.is_some(),
            })
        })
        .map_err(|e| format!("cannot list OpenCode sessions: {e}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map(|all| all.into_iter().filter(|s| safe_id(&s.id)).collect())
        .map_err(|e| format!("cannot read an OpenCode session: {e}"))
}

/// An ID that is safe as a file name: OpenCode's `ses_` and base62.
fn safe_id(id: &str) -> bool {
    id.starts_with("ses_") && id.len() <= 64 && id[4..].bytes().all(|b| b.is_ascii_alphanumeric())
}

struct Message {
    id: String,
    created: i64,
    data: Value,
}

/// Writes one session's new lines. Returns its progress, the lines written,
/// and whether the file was started again.
fn sync(
    connection: &Connection,
    session: &Session,
    path: &Path,
    mut progress: Progress,
) -> Result<(Progress, usize, bool), String> {
    let mut rewritten = false;
    // A place whose message is gone means OpenCode rewrote the session.
    if let Some(at) = &progress.at {
        let exists = connection
            .query_row(
                "SELECT 1 FROM message WHERE id = ?1 AND session_id = ?2",
                params![at.message, session.id],
                |_| Ok(()),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .is_some();
        if !exists {
            progress = Progress::default();
            rewritten = true;
        }
    }
    let mut lines: Vec<Vec<u8>> = Vec::new();
    let fresh = !path.is_file() || rewritten;
    if fresh {
        progress = Progress::default();
        lines.push(line(&json!({
            "type": "opencode.session",
            "session_id": session.id,
            "parent_id": session.parent,
            "directory": session.directory,
            "version": session.version,
            "time": session.created,
        })));
    }
    let messages = messages_from(connection, &session.id, progress.at.as_ref())?;
    let mut complete = true;
    'messages: for message in messages {
        let mut place = match &progress.at {
            Some(at) if at.message == message.id => at.clone(),
            Some(at) if !at.closed => break,
            _ => Place {
                created: message.created,
                message: message.id.clone(),
                part: None,
                closed: false,
            },
        };
        if place.closed {
            continue;
        }
        let role = message
            .data
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned();
        let finished = role == "user"
            || message
                .data
                .pointer("/time/completed")
                .is_some_and(|t| !t.is_null())
            || message.data.get("error").is_some_and(|e| !e.is_null());
        let model = match role.as_str() {
            "assistant" => pair(message.data.get("providerID"), message.data.get("modelID")),
            _ => pair(
                message.data.pointer("/model/providerID"),
                message.data.pointer("/model/modelID"),
            ),
        };
        for (part_id, part) in parts_after(connection, &message.id, place.part.as_deref())? {
            if !finished && !part_finished(&part) {
                progress.at = Some(place);
                complete = false;
                break 'messages;
            }
            lines.push(line(&json!({
                "type": "opencode.part",
                "session_id": session.id,
                "message_id": message.id,
                "part_id": part_id,
                "role": role,
                "model": model,
                "time": message.created,
                "part": clip(part),
            })));
            place.part = Some(part_id);
        }
        if !finished {
            progress.at = Some(place);
            complete = false;
            break;
        }
        if let Some(error) = message.data.get("error").filter(|e| !e.is_null()) {
            lines.push(line(&json!({
                "type": "opencode.error",
                "session_id": session.id,
                "message_id": message.id,
                "time": message.created,
                "error": {
                    "name": error.get("name"),
                    "data": {
                        "message": error.pointer("/data/message"),
                        "statusCode": error.pointer("/data/statusCode"),
                    },
                },
            })));
        }
        place.closed = true;
        progress.at = Some(place);
    }
    let written = lines.len();
    if fresh {
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
    touch(path, session.updated);
    // A session with an unfinished part is looked at again next pass.
    progress.updated = if complete { session.updated } else { -1 };
    Ok((progress, written, rewritten))
}

fn pair(provider: Option<&Value>, model: Option<&Value>) -> Value {
    match (
        provider.and_then(Value::as_str),
        model.and_then(Value::as_str),
    ) {
        (Some(provider), Some(model)) => json!(format!("{provider}/{model}")),
        _ => Value::Null,
    }
}

/// The session's messages from `at`'s message on, in OpenCode's order.
fn messages_from(
    connection: &Connection,
    session: &str,
    at: Option<&Place>,
) -> Result<Vec<Message>, String> {
    let (created, id) = at.map_or((i64::MIN, String::new()), |at| {
        (at.created, at.message.clone())
    });
    let mut statement = connection
        .prepare(
            "SELECT id, time_created, data FROM message WHERE session_id = ?1 \
             AND (time_created > ?2 OR (time_created = ?2 AND id >= ?3)) \
             ORDER BY time_created, id",
        )
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map(params![session, created, id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        let (id, created, data) = row.map_err(|e| e.to_string())?;
        out.push(Message {
            id,
            created,
            data: serde_json::from_str(&data).unwrap_or(Value::Null),
        });
    }
    Ok(out)
}

/// A message's parts after `after`, by ID.
fn parts_after(
    connection: &Connection,
    message: &str,
    after: Option<&str>,
) -> Result<Vec<(String, Value)>, String> {
    let mut statement = connection
        .prepare("SELECT id, data FROM part WHERE message_id = ?1 AND id > ?2 ORDER BY id")
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map(params![message, after.unwrap_or("")], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        let (id, data) = row.map_err(|e| e.to_string())?;
        out.push((id, serde_json::from_str(&data).unwrap_or(Value::Null)));
    }
    Ok(out)
}

/// Whether a part of a running message is finished.
fn part_finished(part: &Value) -> bool {
    match part.get("type").and_then(Value::as_str) {
        Some("text" | "reasoning") => part.pointer("/time/end").is_some_and(|t| !t.is_null()),
        Some("tool") => matches!(
            part.pointer("/state/status").and_then(Value::as_str),
            Some("completed" | "error")
        ),
        _ => true,
    }
}

/// A part with its large fields cut; see the module documentation.
fn clip(mut part: Value) -> Value {
    let mut clipped = false;
    let mut cut = |value: Option<&mut Value>, max: usize| {
        if let Some(Value::String(text)) = value
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
    match part.get("type").and_then(Value::as_str) {
        Some("text" | "reasoning") => {
            cut(part.get_mut("text"), TEXT_BYTES);
            if let Some(map) = part.as_object_mut() {
                map.remove("metadata");
            }
        }
        Some("tool") => {
            if let Some(state) = part.get_mut("state") {
                cut(state.get_mut("output"), TOOL_OUTPUT_BYTES);
                cut(state.get_mut("error"), TOOL_OUTPUT_BYTES);
                if let Some(map) = state.as_object_mut() {
                    clipped |= map.remove("metadata").is_some();
                    clipped |= map.remove("attachments").is_some();
                    clipped |= map.remove("raw").is_some();
                }
            }
            if let Some(map) = part.as_object_mut() {
                map.remove("metadata");
            }
        }
        Some("file") => {
            if part
                .get("url")
                .and_then(Value::as_str)
                .is_some_and(|url| url.starts_with("data:"))
                && let Some(map) = part.as_object_mut()
            {
                map.remove("url");
                clipped = true;
            }
        }
        _ => {}
    }
    if serde_json::to_vec(&part).map_or(0, |b| b.len()) > LINE_BYTES {
        part = json!({ "type": part.get("type") });
        clipped = true;
    }
    if clipped && let Some(map) = part.as_object_mut() {
        map.insert("openagents_clipped".into(), json!(true));
    }
    part
}

fn line(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

/// OpenCode's placeholder titles, which name no chat.
fn named(title: &str) -> bool {
    !title.trim().is_empty()
        && !title.starts_with("New session - ")
        && !title.starts_with("Child session - ")
}

fn write_index(dir: &Path, sessions: &[Session]) -> Result<(), String> {
    let mut bytes = Vec::new();
    for session in sessions.iter().filter(|s| named(&s.title)) {
        bytes.extend(line(&json!({
            "id": session.id,
            "thread_name": session.title,
            "updated_at": crate::project::utc(u64::try_from(session.updated / 1000).unwrap_or(0)),
            "archived": session.archived,
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

/// Sets the file's modification time to `millis`, the session's last update.
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
