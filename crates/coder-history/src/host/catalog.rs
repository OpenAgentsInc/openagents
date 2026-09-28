use super::{History, bounded, confined, digest, encoded_len};
use crate::project::utc;
use crate::*;
use std::collections::{BTreeMap, HashSet};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

const MAX_INDEX_BYTES: u64 = 32 * 1024 * 1024;
const MAX_NOTICES: usize = 128;

#[derive(Clone)]
pub(super) struct Source {
    pub root: usize,
    pub relative: PathBuf,
    pub id: String,
    pub harness: Harness,
    pub archived: bool,
    pub subagent: bool,
    /// The file's identity, length, and last write when it was listed.
    pub stat: Option<confined::Stat>,
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
            Harness::OpenCode => {
                opencode(root_index, root, &mut sources, &mut notices, &mut visited)?;
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
                let names = match entries(root, &relative, &directory) {
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
                for (name, found) in names {
                    visited += 1;
                    if visited > confined::MAX_ENTRIES {
                        return Err(Error::ResourceLimit);
                    }
                    let path = relative.join(&name);
                    match found {
                        Ok((confined::Kind::Directory, _)) => {
                            if path.components().count() >= 16 {
                                return Err(Error::ResourceLimit);
                            }
                            pending.push(path);
                        }
                        Ok((confined::Kind::File, stat))
                            if path.extension().is_some_and(|e| e == "jsonl") =>
                        {
                            sources.push(Source {
                                stat,
                                root: root_index,
                                id: root.source_id(&path),
                                harness: root.harness,
                                archived: *start == "archived_sessions",
                                subagent: path.components().any(|c| c.as_os_str() == "subagents"),
                                relative: path,
                            });
                        }
                        Ok((confined::Kind::Symlink, _)) => {
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
    let mut places = memo(&PLACES);
    for source in &sources {
        if !places.contains_key(&source.id) {
            places.insert(
                source.id.clone(),
                (
                    history.roots[source.root].id.clone(),
                    source.relative.clone(),
                ),
            );
        }
    }
    drop(places);
    Ok((sources, notices))
}

/// A directory's entries and their kinds, as listed when its identity and
/// last change were `stat`. Adding, removing, or renaming an entry changes a
/// directory's last change, so an unchanged directory lists the same.
struct KnownDirectory {
    stat: (u64, u64, i64, i64),
    entries: Vec<(std::ffi::OsString, confined::Kind)>,
}

static DIRECTORIES: std::sync::OnceLock<Memo<(String, PathBuf), KnownDirectory>> =
    std::sync::OnceLock::new();

/// A directory's entries with their kinds, and each file's [`confined::Stat`]:
/// an unchanged directory's names and kinds come from its last listing, and
/// only its files are looked at again.
#[allow(clippy::type_complexity)]
fn entries(
    root: &confined::Root,
    relative: &Path,
    directory: &std::fs::File,
) -> Result<
    Vec<(
        std::ffi::OsString,
        Result<(confined::Kind, Option<confined::Stat>), Error>,
    )>,
    Error,
> {
    use std::os::unix::fs::MetadataExt;
    let stat = directory
        .metadata()
        .map(|m| (m.dev(), m.ino(), m.mtime(), m.mtime_nsec()))
        .map_err(|_| Error::SourceUnreadable)?;
    let key = (root.id.clone(), relative.to_path_buf());
    let known = memo(&DIRECTORIES)
        .get(&key)
        .filter(|known| known.stat == stat)
        .map(|known| known.entries.clone());
    if let Some(known) = known {
        return Ok(known
            .into_iter()
            .map(|(name, kind)| {
                let found = match kind {
                    confined::Kind::File => {
                        confined::entry(directory, &name).map(|(kind, stat)| (kind, Some(stat)))
                    }
                    kind => Ok((kind, None)),
                };
                (name, found)
            })
            .collect());
    }
    let names = confined::names(directory)?;
    let mut listed = Vec::with_capacity(names.len());
    let mut whole = true;
    for name in names {
        let found = confined::entry(directory, &name).map(|(kind, stat)| (kind, Some(stat)));
        whole &= found.is_ok();
        listed.push((name, found));
    }
    if whole {
        memo(&DIRECTORIES).insert(
            key,
            KnownDirectory {
                stat,
                entries: listed
                    .iter()
                    .filter_map(|(name, found)| {
                        found.as_ref().ok().map(|(kind, _)| (name.clone(), *kind))
                    })
                    .collect(),
            },
        );
    }
    Ok(listed)
}

/// Where each listed source ID was found: its root's ID and its path in
/// that root. Only a scan adds a place, so a place was admitted by the same
/// rules as a listing.
static PLACES: std::sync::OnceLock<Memo<String, (String, PathBuf)>> = std::sync::OnceLock::new();

/// The source `id` names, where a scan found it before; else a new scan.
pub(super) fn find(history: &History, id: &str) -> Result<Source, Error> {
    let known = memo(&PLACES).get(id).cloned();
    if let Some((root_id, relative)) = known
        && let Some(root) = history.roots.iter().position(|r| r.id == root_id)
        && history.roots[root].source_id(&relative) == id
    {
        return Ok(Source {
            root,
            id: id.to_owned(),
            harness: history.roots[root].harness,
            archived: false,
            subagent: false,
            relative,
            stat: None,
        });
    }
    let (sources, _) = scan(history)?;
    sources
        .into_iter()
        .find(|s| s.id == id)
        .ok_or(Error::SourceMissing)
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
        match confined::entry(&directory, &name) {
            Ok((confined::Kind::File, stat)) => sources.push(Source {
                stat: Some(stat),
                root: root_index,
                id: root.source_id(&path),
                harness: root.harness,
                archived: task_id(&path).is_some_and(|task| archived.contains(&task)),
                subagent: false,
                relative: path,
            }),
            Ok((confined::Kind::Symlink, _)) => {
                notice(notices, "symlink_refused", Some(root.source_id(&path)))?
            }
            Err(_) => notice(notices, "entry_unavailable", Some(root.source_id(&path)))?,
            _ => {}
        }
    }
    Ok(())
}

/// The host's OpenCode mirror is flat: each `ses_*.jsonl` directly inside it
/// is one session. The title index and every other file are not sources.
fn opencode(
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
    for name in names {
        *visited += 1;
        if *visited > confined::MAX_ENTRIES {
            return Err(Error::ResourceLimit);
        }
        let path = PathBuf::from(&name);
        if !name
            .to_str()
            .is_some_and(|n| n.starts_with("ses_") && n.ends_with(".jsonl"))
        {
            continue;
        }
        match confined::entry(&directory, &name) {
            Ok((confined::Kind::File, stat)) => sources.push(Source {
                stat: Some(stat),
                root: root_index,
                id: root.source_id(&path),
                harness: root.harness,
                archived: false,
                subagent: false,
                relative: path,
            }),
            Ok((confined::Kind::Symlink, _)) => {
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

#[derive(Clone, Default)]
struct Title {
    name: String,
    updated: Option<String>,
    /// The index says the chat is archived (OpenCode's mirror index).
    archived: bool,
}

/// A Codex title index as last read: its file's identity, length, and last
/// write, the titles, and the notices reading it left.
struct KnownTitles {
    stat: (u64, u64, u64, i64, i64),
    titles: std::sync::Arc<BTreeMap<String, Title>>,
    notices: Vec<(String, Option<String>)>,
}

static TITLES: std::sync::OnceLock<Memo<String, KnownTitles>> = std::sync::OnceLock::new();

/// A root's titles, read again only when its index file changed.
fn titles(
    root: &confined::Root,
    notices: &mut Vec<Notice>,
) -> Result<std::sync::Arc<BTreeMap<String, Title>>, Error> {
    if !matches!(root.harness, Harness::Codex | Harness::OpenCode) {
        return Ok(Default::default());
    }
    let stat = root
        .open_file(Path::new("session_index.jsonl"))
        .ok()
        .and_then(|file| file.metadata().ok())
        .map(|m| {
            use std::os::unix::fs::MetadataExt;
            (m.dev(), m.ino(), m.len(), m.mtime(), m.mtime_nsec())
        });
    if let Some(stat) = stat
        && let Some(known) = memo(&TITLES).get(&root.id)
        && known.stat == stat
    {
        for (code, source) in &known.notices {
            notice(notices, code, source.clone())?;
        }
        return Ok(known.titles.clone());
    }
    let mut own = Vec::new();
    let titles = std::sync::Arc::new(read_titles(root, &mut own)?);
    if let Some(stat) = stat {
        memo(&TITLES).insert(
            root.id.clone(),
            KnownTitles {
                stat,
                titles: titles.clone(),
                notices: own
                    .iter()
                    .map(|n| (n.code.clone(), n.source_id.clone()))
                    .collect(),
            },
        );
    }
    for n in own {
        notice(notices, &n.code, n.source_id)?;
    }
    Ok(titles)
}

fn read_titles(
    root: &confined::Root,
    notices: &mut Vec<Notice>,
) -> Result<BTreeMap<String, Title>, Error> {
    let mut result = BTreeMap::new();
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
        let archived = value
            .get("archived")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        result.insert(
            id.to_owned(),
            Title {
                name: name.to_owned(),
                updated,
                archived,
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

/// What a source's head was when last read, kept while its file is the same
/// one: `settled` when a longer file cannot change it (its first record, and
/// a Claude session's entry point, were read whole), else only while its
/// length is unchanged.
#[derive(Clone)]
struct KnownHead {
    dev: u64,
    ino: u64,
    size: u64,
    settled: bool,
    native: Option<String>,
    title: Option<String>,
    engine: bool,
    spawned: bool,
}

type Memo<K, V> = std::sync::Mutex<std::collections::HashMap<K, V>>;

fn memo<K, V>(
    cell: &'static std::sync::OnceLock<Memo<K, V>>,
) -> std::sync::MutexGuard<'static, std::collections::HashMap<K, V>> {
    let mut guard = cell
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    // Bound the memo: sources come and go, and a full memo starts over.
    if guard.len() > 4 * confined::MAX_ENTRIES {
        guard.clear();
    }
    guard
}

static HEADS: std::sync::OnceLock<Memo<(String, PathBuf), KnownHead>> = std::sync::OnceLock::new();

/// A source's head, from what was read before when its file is the same.
fn head(root: &confined::Root, source: &Source) -> Head {
    let Some(stat) = source.stat else {
        return read_head(root, source).0;
    };
    let key = (root.id.clone(), source.relative.clone());
    let modified = u64::try_from(stat.mtime).ok().map(utc);
    if stat.size == 0 {
        return Head {
            status: SourceStatus::Empty,
            modified,
            ..Head::default()
        };
    }
    if let Some(known) = memo(&HEADS).get(&key)
        && known.dev == stat.dev
        && known.ino == stat.ino
        && (known.settled && stat.size >= known.size || known.size == stat.size)
    {
        return Head {
            native: known.native.clone(),
            title: known.title.clone(),
            status: SourceStatus::Available,
            modified,
            engine: known.engine,
            spawned: known.spawned,
        };
    }
    let (head, settled) = read_head(root, source);
    if head.status == SourceStatus::Available {
        memo(&HEADS).insert(
            key,
            KnownHead {
                dev: stat.dev,
                ino: stat.ino,
                size: stat.size,
                settled,
                native: head.native.clone(),
                title: head.title.clone(),
                engine: head.engine,
                spawned: head.spawned,
            },
        );
    }
    head
}

/// Read a source's head, and whether more bytes cannot change it.
fn read_head(root: &confined::Root, source: &Source) -> (Head, bool) {
    let file = match root.open_file(&source.relative) {
        Ok(file) => file,
        Err(Error::SourceMissing) => {
            return (
                Head {
                    status: SourceStatus::Missing,
                    ..Head::default()
                },
                false,
            );
        }
        Err(_) => return (Head::default(), false),
    };
    let Ok(meta) = file.metadata() else {
        return (Head::default(), false);
    };
    let modified = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|elapsed| utc(elapsed.as_secs()));
    if meta.len() == 0 {
        return (
            Head {
                status: SourceStatus::Empty,
                modified,
                ..Head::default()
            },
            false,
        );
    }
    let mut first = Vec::new();
    let read = BufReader::new(file.take(HEADER_BYTES)).read_until(b'\n', &mut first);
    if read.is_err() {
        return (
            Head {
                modified,
                ..Head::default()
            },
            false,
        );
    }
    // A header longer than the bound never parses, however long the file.
    let mut settled = first.ends_with(b"\n") || first.len() as u64 >= HEADER_BYTES;
    // Only the fields the catalog uses are kept: a Codex header's base
    // instructions are skipped, not copied.
    #[derive(serde::Deserialize)]
    struct Payload {
        id: Option<serde_json::Value>,
        session_id: Option<serde_json::Value>,
        originator: Option<serde_json::Value>,
        source: Option<serde_json::Value>,
    }
    #[derive(serde::Deserialize)]
    #[allow(non_snake_case)]
    struct Header {
        payload: Option<Payload>,
        sessionId: Option<serde_json::Value>,
        customTitle: Option<serde_json::Value>,
        summary: Option<serde_json::Value>,
        /// The OpenCode mirror's header (`opencode.session`).
        session_id: Option<serde_json::Value>,
        parent_id: Option<serde_json::Value>,
    }
    let header = serde_json::from_slice::<Header>(&first).ok();
    let payload = header.as_ref().and_then(|h| h.payload.as_ref());
    let native = match source.harness {
        Harness::Codex => payload.and_then(|p| p.id.as_ref().or(p.session_id.as_ref())),
        Harness::Claude => header.as_ref().and_then(|h| h.sessionId.as_ref()),
        // The file name, not the header, names a Coder task.
        Harness::Coder => None,
        Harness::OpenCode => header.as_ref().and_then(|h| h.session_id.as_ref()),
    }
    .and_then(|v| v.as_str())
    .filter(|v| !v.is_empty() && v.len() <= 128)
    .map(str::to_owned);
    let title = header
        .as_ref()
        .and_then(|h| h.customTitle.as_ref().or(h.summary.as_ref()))
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let (engine, spawned) = match source.harness {
        Harness::Codex => (
            payload
                .and_then(|p| p.originator.as_ref())
                .and_then(|o| o.as_str())
                == Some(crate::engine::MARK),
            payload
                .and_then(|p| p.source.as_ref())
                .and_then(|s| s.as_object())
                .is_some_and(|s| s.contains_key("subagent")),
        ),
        Harness::Claude => {
            let (entrypoint, known) = claude_entrypoint(root, source);
            settled &= known;
            (entrypoint.as_deref() == Some(crate::engine::MARK), false)
        }
        Harness::Coder => (false, false),
        // The engine's OpenCode sessions live in their own database, which
        // the mirror never reads ([`crate::engine`]). A session with a
        // parent is a subagent's (OpenCode's `task` tool).
        Harness::OpenCode => (
            false,
            header
                .as_ref()
                .and_then(|h| h.parent_id.as_ref())
                .and_then(|p| p.as_str())
                .is_some_and(|p| !p.is_empty()),
        ),
    };
    (
        Head {
            native,
            title,
            status: SourceStatus::Available,
            modified,
            engine,
            spawned,
        },
        settled,
    )
}

/// The entry point Claude Code recorded for a session: the `entrypoint` of
/// its first record that has one, within the first
/// [`ENTRYPOINT_SCAN_RECORDS`] records and [`ENTRYPOINT_SCAN_BYTES`].
fn claude_entrypoint(root: &confined::Root, source: &Source) -> (Option<String>, bool) {
    #[derive(serde::Deserialize)]
    struct Record {
        entrypoint: Option<String>,
    }
    let Ok(file) = root.open_file(&source.relative) else {
        return (None, false);
    };
    let mut reader = BufReader::new(file.take(ENTRYPOINT_SCAN_BYTES));
    let mut line = Vec::new();
    let mut read = 0;
    for _ in 0..ENTRYPOINT_SCAN_RECORDS {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => return (None, false),
            Ok(count) => read += count as u64,
        }
        if !line.ends_with(b"\n") {
            // The scan's bound ends inside a record: nothing later counts.
            return (None, read >= ENTRYPOINT_SCAN_BYTES);
        }
        if let Ok(Record {
            entrypoint: Some(entrypoint),
        }) = serde_json::from_slice(&line)
        {
            return (Some(entrypoint), true);
        }
    }
    (None, true)
}

/// The first line of the chat's first prompt, for a chat with no title:
/// the first user message within the first 64 KiB that is not injected
/// context (text that opens with `<`, such as `<environment_context>`).
fn first_prompt(root: &confined::Root, source: &Source) -> Option<String> {
    /// Identity, length, settled, and the prompt read.
    type KnownPrompt = (u64, u64, u64, bool, Option<String>);
    static PROMPTS: std::sync::OnceLock<Memo<(String, PathBuf), KnownPrompt>> =
        std::sync::OnceLock::new();
    let key = (root.id.clone(), source.relative.clone());
    if let Some(stat) = source.stat
        && let Some((dev, ino, size, settled, prompt)) = memo(&PROMPTS).get(&key)
        && *dev == stat.dev
        && *ino == stat.ino
        && (*settled && stat.size >= *size || *size == stat.size)
    {
        return prompt.clone();
    }
    let (prompt, settled) = read_first_prompt(root, source);
    if let Some(stat) = source.stat {
        memo(&PROMPTS).insert(
            key,
            (stat.dev, stat.ino, stat.size, settled, prompt.clone()),
        );
    }
    prompt
}

/// The first prompt, and whether more bytes cannot change it.
fn read_first_prompt(root: &confined::Root, source: &Source) -> (Option<String>, bool) {
    const SCAN: u64 = 64 * 1024;
    let Ok(file) = root.open_file(&source.relative) else {
        return (None, false);
    };
    let mut reader = BufReader::new(file.take(SCAN));
    let mut line = Vec::new();
    let mut read = 0;
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => return (None, read >= SCAN),
            Ok(count) => read += count as u64,
        }
        if !line.ends_with(b"\n") {
            return (None, read >= SCAN);
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
        return (Some(bounded(first, 120).0), true);
    }
}

pub(super) fn page(
    history: &History,
    request: CatalogRequest,
    limits: Limits,
) -> Result<CatalogPage, Error> {
    if request.limit == 0 || request.limit > limits.catalog_page {
        return Err(Error::InvalidRequest);
    }
    let (sources, mut notices) = scan(history)?;
    let mut entries: Vec<(String, Chat)> = Vec::new();
    let mut untitled: BTreeMap<String, (usize, PathBuf, Option<confined::Stat>)> = BTreeMap::new();
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
                Harness::OpenCode => "Saved OpenCode chat",
            };
            let named = title.map(|t| t.name.as_str()).or(from_title.as_deref());
            if named.is_none() {
                untitled.insert(
                    source.id.clone(),
                    (root_index, source.relative.clone(), source.stat),
                );
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
                    archived: source.archived || title.is_some_and(|t| t.archived),
                    subagent: source.subagent || spawned,
                    source_id: Some(source.id.clone()),
                    status,
                },
            ));
        }
        for (native, title) in title_map
            .iter()
            .filter(|(native, _)| !present.contains(*native))
            .map(|(native, title)| (native.clone(), title.clone()))
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
                    archived: title.archived,
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
        if encoded_len(&page)? > limits.response_bytes {
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
        if let Some((root, relative, stat)) =
            chat.source_id.as_ref().and_then(|id| untitled.get(id))
            && let Some(prompt) = first_prompt(
                &history.roots[*root],
                &Source {
                    root: *root,
                    relative: relative.clone(),
                    id: String::new(),
                    harness: chat.harness,
                    archived: chat.archived,
                    subagent: chat.subagent,
                    stat: *stat,
                },
            )
        {
            chat.title = prompt;
        }
    }
    if encoded_len(&page)? > limits.response_bytes {
        return Err(Error::ResourceLimit);
    }
    Ok(page)
}
