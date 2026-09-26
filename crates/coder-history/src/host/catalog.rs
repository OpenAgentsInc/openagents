use super::{History, bounded, confined, digest, encoded_len};
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

fn head(root: &confined::Root, source: &Source) -> (Option<String>, Option<String>, SourceStatus) {
    let file = match root.open_file(&source.relative) {
        Ok(file) => file,
        Err(Error::SourceMissing) => return (None, None, SourceStatus::Missing),
        Err(_) => return (None, None, SourceStatus::Unreadable),
    };
    let Ok(meta) = file.metadata() else {
        return (None, None, SourceStatus::Unreadable);
    };
    if meta.len() == 0 {
        return (None, None, SourceStatus::Empty);
    }
    let mut first = Vec::new();
    let read = BufReader::new(file.take(16 * 1024)).read_until(b'\n', &mut first);
    if read.is_err() {
        return (None, None, SourceStatus::Unreadable);
    }
    let value = serde_json::from_slice::<serde_json::Value>(&first).ok();
    let native = value
        .as_ref()
        .and_then(|v| match source.harness {
            Harness::Codex => v
                .get("payload")
                .and_then(|p| p.get("id").or_else(|| p.get("session_id"))),
            Harness::Claude => v.get("sessionId"),
        })
        .and_then(|v| v.as_str())
        .filter(|v| !v.is_empty() && v.len() <= 128)
        .map(str::to_owned);
    let title = value
        .as_ref()
        .and_then(|v| v.get("customTitle").or_else(|| v.get("summary")))
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    (native, title, SourceStatus::Available)
}

pub(super) fn page(history: &History, request: CatalogRequest) -> Result<CatalogPage, Error> {
    if request.limit == 0 || request.limit > MAX_CATALOG_PAGE {
        return Err(Error::InvalidRequest);
    }
    let (sources, mut notices) = scan(history)?;
    let mut entries: Vec<(String, Chat)> = Vec::new();
    for (root_index, root) in history.roots.iter().enumerate() {
        let title_map = titles(root, &mut notices)?;
        let mut present = HashSet::new();
        for source in sources.iter().filter(|s| s.root == root_index) {
            let (from_header, from_title, status) = head(root, source);
            let native = from_header.or_else(|| uuid_suffix(&source.relative));
            let title = native.as_ref().and_then(|id| title_map.get(id));
            if let Some(id) = &native {
                present.insert(id.clone());
            }
            let default = match root.harness {
                Harness::Codex => "Saved Codex chat",
                Harness::Claude => "Saved Claude chat",
            };
            let (name, truncated) = bounded(
                title
                    .map(|t| t.name.as_str())
                    .or(from_title.as_deref())
                    .unwrap_or(default),
                256,
            );
            let id = native
                .as_ref()
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
                    updated_at: title.and_then(|t| t.updated.clone()),
                    archived: source.archived,
                    subagent: source.subagent,
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
    // Cursor continuity covers membership/order, not mutable title metadata.
    // A running chat can update its title without starving catalog pagination.
    let membership: Vec<_> = entries
        .iter()
        .map(|(key, chat)| (key, &chat.id, &chat.source_id, chat.archived))
        .collect();
    let snapshot =
        digest(&serde_json::to_vec(&(&membership, &notices)).map_err(|_| Error::Encoding)?);
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
    if encoded_len(&page)? > MAX_RESPONSE_BYTES {
        return Err(Error::ResourceLimit);
    }
    Ok(page)
}
