use super::{History, bounded, confined, digest, encoded_len};
use crate::project::utc;
use crate::*;
use std::collections::{BTreeMap, HashSet};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

const MAX_INDEX_BYTES: u64 = 32 * 1024 * 1024;
const MAX_NOTICES: usize = 128;

pub(super) struct Source {
    pub root: usize,
    pub relative: PathBuf,
    pub id: String,
    pub harness: Harness,
    pub archived: bool,
    pub subagent: bool,
}

pub(super) fn scan(history: &History) -> Result<(Vec<Source>, Vec<Notice>), Error> {
    let mut sources = Vec::new();
    let mut notices = Vec::new();
    let mut visited = 0;
    for (root_index, root) in history.roots.iter().enumerate() {
        let starts: &[&str] = match root.harness {
            Harness::Codex => &["sessions", "archived_sessions"],
            Harness::Claude => &["projects"],
            Harness::Coder => {
                tasks(root_index, root, &mut sources, &mut notices, &mut visited)?;
                continue;
            }
        };
        for start in starts {
            let mut pending = vec![PathBuf::from(start)];
            while let Some(relative) = pending.pop() {
                let directory = match root.open_dir(&relative) {
                    Ok(file) => file,
                    Err(Error::SourceMissing) if relative == Path::new(start) => continue,
                    Err(_) => {
                        notice(
                            &mut notices,
                            "directory_unavailable",
                            Some(root.source_id(&relative)),
                        )?;
                        continue;
                    }
                };
                let names = match confined::names(&directory) {
                    Ok(names) => names,
                    Err(Error::ResourceLimit) => return Err(Error::ResourceLimit),
                    Err(_) => {
                        notice(
                            &mut notices,
                            "directory_unavailable",
                            Some(root.source_id(&relative)),
                        )?;
                        continue;
                    }
                };
                for name in names {
                    visited += 1;
                    if visited > confined::MAX_ENTRIES {
                        return Err(Error::ResourceLimit);
                    }
                    let path = relative.join(&name);
                    match confined::kind(&directory, &name) {
                        Ok(confined::Kind::Directory) => {
                            if path.components().count() >= 16 {
                                return Err(Error::ResourceLimit);
                            }
                            pending.push(path);
                        }
                        Ok(confined::Kind::File)
                            if path.extension().is_some_and(|e| e == "jsonl") =>
                        {
                            sources.push(Source {
                                root: root_index,
                                id: root.source_id(&path),
                                harness: root.harness,
                                archived: *start == "archived_sessions",
                                subagent: path.components().any(|c| c.as_os_str() == "subagents"),
                                relative: path,
                            });
                        }
                        Ok(confined::Kind::Symlink) => {
                            notice(&mut notices, "symlink_refused", Some(root.source_id(&path)))?
                        }
                        Err(_) => notice(
                            &mut notices,
                            "entry_unavailable",
                            Some(root.source_id(&path)),
                        )?,
                        _ => {}
                    }
                }
            }
        }
    }
    sources.sort_by(|a, b| a.id.cmp(&b.id));
    notices.sort_by(|a, b| (&a.code, &a.source_id).cmp(&(&b.code, &b.source_id)));
    Ok((sources, notices))
}

/// Coder's task directory is flat: each `*.atif.jsonl` directly inside it is
/// one task attempt's transcript. Other files and subdirectories are ignored.
/// A transcript of a task the owner archived is listed as archived.
fn tasks(
    root_index: usize,
    root: &confined::Root,
    sources: &mut Vec<Source>,
    notices: &mut Vec<Notice>,
    visited: &mut usize,
) -> Result<(), Error> {
    let listed = root
        .open_top()
        .and_then(|directory| confined::names(&directory).map(|names| (directory, names)));
    let (directory, names) = match listed {
        Ok(listed) => listed,
        Err(Error::ResourceLimit) => return Err(Error::ResourceLimit),
        Err(_) => {
            return notice(
                notices,
                "directory_unavailable",
                Some(root.source_id(Path::new(""))),
            );
        }
    };
    let archived = archived_tasks(root, notices)?;
    for name in names {
        *visited += 1;
        if *visited > confined::MAX_ENTRIES {
            return Err(Error::ResourceLimit);
        }
        let path = PathBuf::from(&name);
        if !name.to_str().is_some_and(|n| n.ends_with(ATIF_SUFFIX)) {
            continue;
        }
        match confined::kind(&directory, &name) {
            Ok(confined::Kind::File) => sources.push(Source {
                root: root_index,
                id: root.source_id(&path),
                harness: root.harness,
                archived: task_id(&path).is_some_and(|task| archived.contains(&task)),
                subagent: false,
                relative: path,
            }),
            Ok(confined::Kind::Symlink) => {
                notice(notices, "symlink_refused", Some(root.source_id(&path)))?
            }
            Err(_) => notice(notices, "entry_unavailable", Some(root.source_id(&path)))?,
            _ => {}
        }
    }
    Ok(())
}

const ATIF_SUFFIX: &str = ".atif.jsonl";
/// The task store's archive record (`openagents.coder.task-archive.v1`).
const ARCHIVE_FILE: &str = "archive.json";
const ARCHIVE_SCHEMA: &str = "openagents.coder.task-archive.v1";
const MAX_ARCHIVE_BYTES: u64 = 1024 * 1024;

/// The task IDs the owner archived. A missing record archives nothing; an
/// unreadable or malformed one archives nothing and leaves a notice, so a
/// chat is never hidden by accident.
fn archived_tasks(
    root: &confined::Root,
    notices: &mut Vec<Notice>,
) -> Result<HashSet<String>, Error> {
    #[derive(serde::Deserialize)]
    struct Record {
        schema: String,
        tasks: BTreeMap<String, serde::de::IgnoredAny>,
    }
    let path = Path::new(ARCHIVE_FILE);
    let unreadable = |notices: &mut Vec<Notice>| {
        notice(notices, "archive_unavailable", Some(root.source_id(path))).map(|()| HashSet::new())
    };
    let file = match root.open_file(path) {
        Ok(file) => file,
        Err(Error::SourceMissing) => return Ok(HashSet::new()),
        Err(_) => return unreadable(notices),
    };
    let mut bytes = Vec::new();
    if file
        .take(MAX_ARCHIVE_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() as u64 > MAX_ARCHIVE_BYTES
    {
        return unreadable(notices);
    }
    match serde_json::from_slice::<Record>(&bytes) {
        Ok(record) if record.schema == ARCHIVE_SCHEMA => Ok(record.tasks.into_keys().collect()),
        _ => unreadable(notices),
    }
}

/// A Coder transcript's name without its suffix, `<task>.<attempt>`.
fn attempt(path: &Path) -> Option<&str> {
    path.file_name()?.to_str()?.strip_suffix(ATIF_SUFFIX)
}

/// The 64-hex task ID that opens a Coder transcript's file name.
fn task_id(path: &Path) -> Option<String> {
    let (task, rest) = attempt(path)?.split_once('.')?;
    (task.len() == 64
        && task.bytes().all(|b| b.is_ascii_hexdigit())
        && !rest.is_empty()
        && rest.bytes().all(|b| b.is_ascii_digit()))
    .then(|| task.to_owned())
}

fn notice(out: &mut Vec<Notice>, code: &str, source_id: Option<String>) -> Result<(), Error> {
    if out.len() == MAX_NOTICES {
        return Err(Error::ResourceLimit);
    }
    out.push(Notice {
        code: code.into(),
        source_id,
    });
    Ok(())
}

#[derive(Default)]
struct Title {
    name: String,
    updated: Option<String>,
}

fn titles(
    root: &confined::Root,
    notices: &mut Vec<Notice>,
) -> Result<BTreeMap<String, Title>, Error> {
    let mut result = BTreeMap::new();
    if root.harness != Harness::Codex {
        return Ok(result);
    }
    let path = Path::new("session_index.jsonl");
    let file = match root.open_file(path) {
        Ok(file) => file,
        Err(Error::SourceMissing) => return Ok(result),
        Err(_) => {
            notice(
                notices,
                "title_index_unavailable",
                Some(root.source_id(path)),
            )?;
            return Ok(result);
        }
    };
    let len = file.metadata().map_err(|_| Error::SourceUnreadable)?.len();
    if len > MAX_INDEX_BYTES {
        return Err(Error::ResourceLimit);
    }
    let mut bytes = Vec::new();
    file.take(MAX_INDEX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::SourceUnreadable)?;
    if bytes.len() as u64 > MAX_INDEX_BYTES {
        return Err(Error::ResourceLimit);
    }
    let mut malformed = false;
    for line in bytes.split_inclusive(|b| *b == b'\n') {
        if !line.ends_with(b"\n") {
            notice(
                notices,
                "title_index_partial_line",
                Some(root.source_id(path)),
            )?;
            break;
        }
        if line.len() > confined::MAX_PARSE_BYTES {
            malformed = true;
            continue;
        }
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(line) else {
            malformed = true;
            continue;
        };
        let Some(id) = value
            .get("id")
            .and_then(|x| x.as_str())
            .filter(|x| !x.is_empty() && x.len() <= 128)
        else {
            malformed = true;
            continue;
        };
        let Some(name) = value.get("thread_name").and_then(|x| x.as_str()) else {
            malformed = true;
            continue;
        };
        let updated = value
            .get("updated_at")
            .and_then(|x| x.as_str())
            .map(|x| bounded(x, 64).0);
        result.insert(
            id.to_owned(),
            Title {
                name: name.to_owned(),
                updated,
            },
        );
        if result.len() > confined::MAX_ENTRIES {
            return Err(Error::ResourceLimit);
        }
    }
    if malformed {
        notice(
            notices,
            "title_index_unrecognized_records",
            Some(root.source_id(path)),
        )?;
    }
    Ok(result)
}

fn uuid_suffix(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    let id = stem.get(stem.len().checked_sub(36)?..)?;
    if id.bytes().enumerate().all(|(i, b)| {
        if [8, 13, 18, 23].contains(&i) {
            b == b'-'
        } else {
            b.is_ascii_hexdigit()
        }
    }) {
        Some(id.to_owned())
    } else {
        None
    }
}

/// A source's first record: its native ID and title, when the header has
/// them, and its status. The modification time is the chat's last activity.
struct Head {
    native: Option<String>,
    title: Option<String>,
    status: SourceStatus,
    modified: Option<String>,
    /// The session records that Coder's engine started it
    /// ([`crate::engine`]), so it is not a chat of its own.
    engine: bool,
    /// Codex recorded the session as a thread another session spawned.
    spawned: bool,
}

/// An unreadable source's head, which the other cases start from.
impl Default for Head {
    fn default() -> Self {
        Self {
            native: None,
            title: None,
            status: SourceStatus::Unreadable,
            modified: None,
            engine: false,
            spawned: false,
        }
    }
}

/// How much of a source's first record the catalog reads. A Codex
/// `session_meta` header carries the session's base instructions, about
/// 20 KiB, ahead of nothing else the catalog needs, so a shorter bound
/// would lose its originator and source.
const HEADER_BYTES: u64 = crate::MAX_READABLE_RECORD_BYTES as u64;

/// How far into a Claude session [`claude_entrypoint`] looks for the first
/// record that names its entry point: the first user record carries it, and
/// an engine briefing can be long.
const ENTRYPOINT_SCAN_BYTES: u64 = 1024 * 1024;
const ENTRYPOINT_SCAN_RECORDS: usize = 16;

fn head(root: &confined::Root, source: &Source) -> Head {
    let file = match root.open_file(&source.relative) {
        Ok(file) => file,
        Err(Error::SourceMissing) => {
            return Head {
                status: SourceStatus::Missing,
                ..Head::default()
            };
        }
        Err(_) => return Head::default(),
    };
    let Ok(meta) = file.metadata() else {
        return Head::default();
    };
    let modified = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|elapsed| utc(elapsed.as_secs()));
    if meta.len() == 0 {
        return Head {
            status: SourceStatus::Empty,
            modified,
            ..Head::default()
        };
    }
    let mut first = Vec::new();
    let read = BufReader::new(file.take(HEADER_BYTES)).read_until(b'\n', &mut first);
    if read.is_err() {
        return Head {
            modified,
            ..Head::default()
        };
    }
    let value = serde_json::from_slice::<serde_json::Value>(&first).ok();
    let native = value
        .as_ref()
        .and_then(|v| match source.harness {
            Harness::Codex => v
                .get("payload")
                .and_then(|p| p.get("id").or_else(|| p.get("session_id"))),
            Harness::Claude => v.get("sessionId"),
            // The file name, not the header, names a Coder task.
            Harness::Coder => None,
        })
        .and_then(|v| v.as_str())
        .filter(|v| !v.is_empty() && v.len() <= 128)
        .map(str::to_owned);
    let title = value
        .as_ref()
        .and_then(|v| v.get("customTitle").or_else(|| v.get("summary")))
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let payload = value.as_ref().and_then(|v| v.get("payload"));
    let (engine, spawned) = match source.harness {
        Harness::Codex => (
            payload
                .and_then(|p| p.get("originator"))
                .and_then(|o| o.as_str())
                == Some(crate::engine::MARK),
            payload
                .and_then(|p| p.get("source"))
                .and_then(|s| s.as_object())
                .is_some_and(|s| s.contains_key("subagent")),
        ),
        Harness::Claude => (
            claude_entrypoint(root, source).as_deref() == Some(crate::engine::MARK),
            false,
        ),
        Harness::Coder => (false, false),
    };
    Head {
        native,
        title,
        status: SourceStatus::Available,
        modified,
        engine,
        spawned,
    }
}

/// The entry point Claude Code recorded for a session: the `entrypoint` of
/// its first record that has one, within the first
/// [`ENTRYPOINT_SCAN_RECORDS`] records and [`ENTRYPOINT_SCAN_BYTES`].
fn claude_entrypoint(root: &confined::Root, source: &Source) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Record {
        entrypoint: Option<String>,
    }
    let file = root.open_file(&source.relative).ok()?;
    let mut reader = BufReader::new(file.take(ENTRYPOINT_SCAN_BYTES));
    let mut line = Vec::new();
    for _ in 0..ENTRYPOINT_SCAN_RECORDS {
        line.clear();
        if reader.read_until(b'\n', &mut line).ok()? == 0 || !line.ends_with(b"\n") {
            return None;
        }
        if let Ok(Record {
            entrypoint: Some(entrypoint),
        }) = serde_json::from_slice(&line)
        {
            return Some(entrypoint);
        }
    }
    None
}

/// The first line of the chat's first prompt, for a chat with no title:
/// the first user message within the first 64 KiB that is not injected
/// context (text that opens with `<`, such as `<environment_context>`).
fn first_prompt(root: &confined::Root, source: &Source) -> Option<String> {
    let file = root.open_file(&source.relative).ok()?;
    let mut reader = BufReader::new(file.take(64 * 1024));
    let mut line = Vec::new();
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line).ok()? == 0 || !line.ends_with(b"\n") {
            return None;
        }
        let Some(readable) = crate::readable_record(&line) else {
            continue;
        };
        if readable.role.as_deref() != Some("user") {
            continue;
        }
        let text = readable.text.trim();
        if text.is_empty()
            || text.starts_with('<')
            || text.starts_with("Caveat:")
            || text.contains("AGENTS.md instructions")
        {
            continue;
        }
        let first = text.lines().next().unwrap_or(text).trim();
        return Some(bounded(first, 120).0);
    }
}

pub(super) fn page(history: &History, request: CatalogRequest) -> Result<CatalogPage, Error> {
    if request.limit == 0 || request.limit > MAX_CATALOG_PAGE {
        return Err(Error::InvalidRequest);
    }
    let (sources, mut notices) = scan(history)?;
    let mut entries: Vec<(String, Chat)> = Vec::new();
    let mut untitled: BTreeMap<String, (usize, PathBuf)> = BTreeMap::new();
    for (root_index, root) in history.roots.iter().enumerate() {
        let title_map = titles(root, &mut notices)?;
        let mut present = HashSet::new();
        for source in sources.iter().filter(|s| s.root == root_index) {
            let Head {
                native: from_header,
                title: from_title,
                status,
                modified,
                engine,
                spawned,
            } = head(root, source);
            let native = match root.harness {
                Harness::Coder => task_id(&source.relative),
                _ => from_header.or_else(|| uuid_suffix(&source.relative)),
            };
            let title = native.as_ref().and_then(|id| title_map.get(id));
            if let Some(id) = &native {
                present.insert(id.clone());
            }
            // A session Coder's engine started for a task is the task's
            // work, not a chat: the task's own transcript is the chat.
            if engine {
                continue;
            }
            let default = match root.harness {
                Harness::Codex => "Saved Codex chat",
                Harness::Claude => "Saved Claude chat",
                Harness::Coder => "Saved Coder chat",
            };
            let named = title.map(|t| t.name.as_str()).or(from_title.as_deref());
            if named.is_none() {
                untitled.insert(source.id.clone(), (root_index, source.relative.clone()));
            }
            let (name, truncated) = bounded(named.unwrap_or(default), 256);
            // Every attempt of a Coder task shares its task ID, so the
            // attempt, not the task, is the chat.
            let identity = match root.harness {
                Harness::Coder => native
                    .as_ref()
                    .and_then(|_| attempt(&source.relative))
                    .map(str::to_owned),
                _ => native.clone(),
            };
            let id = identity
                .map(|n| digest(format!("{}\0{n}", root.id).as_bytes()))
                .unwrap_or_else(|| source.id.clone());
            entries.push((
                source.id.clone(),
                Chat {
                    id,
                    harness: root.harness,
                    native_id: native,
                    title: name,
                    title_truncated: truncated,
                    // The file's last write is its last activity.
                    updated_at: modified.or_else(|| title.and_then(|t| t.updated.clone())),
                    archived: source.archived,
                    subagent: source.subagent || spawned,
                    source_id: Some(source.id.clone()),
                    status,
                },
            ));
        }
        for (native, title) in title_map
            .into_iter()
            .filter(|(native, _)| !present.contains(native))
        {
            let id = digest(format!("{}\0{native}", root.id).as_bytes());
            let (title_name, truncated) = bounded(&title.name, 256);
            entries.push((
                format!("missing:{id}"),
                Chat {
                    id,
                    harness: root.harness,
                    native_id: Some(native),
                    title: title_name,
                    title_truncated: truncated,
                    updated_at: title.updated,
                    archived: false,
                    subagent: false,
                    source_id: None,
                    status: SourceStatus::Missing,
                },
            ));
        }
    }
    if entries.len() > confined::MAX_ENTRIES {
        return Err(Error::ResourceLimit);
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    // Cursor continuity covers membership, not order or mutable metadata: a
    // running chat moves to the top and can update its title without
    // starving catalog pagination. Readers merge pages by chat ID.
    let membership: Vec<_> = entries
        .iter()
        .map(|(key, chat)| (key, &chat.id, &chat.source_id, chat.archived))
        .collect();
    let snapshot =
        digest(&serde_json::to_vec(&(&membership, &notices)).map_err(|_| Error::Encoding)?);
    // Newest first. Timestamps share one UTC format, so text order is time
    // order; a chat with no time goes last.
    entries.sort_by(|a, b| {
        b.1.updated_at
            .cmp(&a.1.updated_at)
            .then_with(|| a.0.cmp(&b.0))
    });
    let start = if let Some(cursor) = request.cursor {
        if cursor.snapshot != snapshot {
            return Err(Error::CursorStale);
        }
        entries
            .iter()
            .position(|e| e.0 == cursor.after)
            .ok_or(Error::InvalidRequest)?
            + 1
    } else {
        0
    };
    let mut page = CatalogPage {
        snapshot: snapshot.clone(),
        entries: Vec::new(),
        next: None,
        notices,
    };
    let mut end = start;
    while end < entries.len() && page.entries.len() < usize::from(request.limit) {
        page.entries.push(entries[end].1.clone());
        let after = entries[end].0.clone();
        page.next = if end + 1 < entries.len() {
            Some(CatalogCursor {
                snapshot: snapshot.clone(),
                after,
            })
        } else {
            None
        };
        if encoded_len(&page)? > MAX_RESPONSE_BYTES {
            page.entries.pop();
            if end == start {
                return Err(Error::ResourceLimit);
            }
            page.next = Some(CatalogCursor {
                snapshot: snapshot.clone(),
                after: entries[end - 1].0.clone(),
            });
            break;
        }
        end += 1;
    }
    // Name untitled chats on this page from their first prompt.
    for chat in &mut page.entries {
        if let Some((root, relative)) = chat.source_id.as_ref().and_then(|id| untitled.get(id))
            && let Some(prompt) = first_prompt(
                &history.roots[*root],
                &Source {
                    root: *root,
                    relative: relative.clone(),
                    id: String::new(),
                    harness: chat.harness,
                    archived: chat.archived,
                    subagent: chat.subagent,
                },
            )
        {
            chat.title = prompt;
        }
    }
    if encoded_len(&page)? > MAX_RESPONSE_BYTES {
        return Err(Error::ResourceLimit);
    }
    Ok(page)
}
