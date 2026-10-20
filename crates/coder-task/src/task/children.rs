//! Owner-recorded child activity. Transcript requests contain no file paths.

use super::{Error, Store, Task, local, pending};
use coder_sdk::delegation as sdk;
pub use route_contract::children::*;
use route_contract::{
    digest_of,
    observation::{Observation, Scope, TRACE_KEY},
    pending::Authority,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Read as _, Seek as _, Write as _};
use std::path::{Path, PathBuf};
use std::time::Duration;

const BOOK_SCHEMA: &str = "openagents.coder.child-book.v1";
const BOOK_BYTES: u64 = 2 * 1024 * 1024;
const TRACE_BYTES: u64 = 64 * 1024 * 1024;
const LINE_BYTES: usize = 32 * 1024;

#[derive(Clone, Debug)]
pub struct Observer {
    store: PathBuf,
    scope: Scope,
    authority: Authority,
    active: pending::Service,
}
impl Observer {
    /// Mint only from a live host's existing SDK attempt.
    pub fn for_host(store: &Path, service: &pending::Service) -> Result<Self, Error> {
        service.with_active(|| {
            Ok(Self {
                store: store.into(),
                scope: service.scope().clone(),
                authority: service.authority().clone(),
                active: service.clone(),
            })
        })
    }
    pub fn run_id(&self) -> String {
        format!(
            "owner-{}",
            digest_of(&(&self.scope, &self.authority.grant))
                .as_str()
                .trim_start_matches("sha256:")
        )
    }
    pub fn lineage(&self, harness: &str) -> sdk::Lineage {
        sdk::Lineage::root(&self.run_id(), harness)
    }
    fn reference(&self, child: &sdk::Child) -> Result<Reference, Error> {
        let run = self.run_id();
        let suffix = child
            .lineage
            .id
            .strip_prefix(&run)
            .ok_or(Error::InvalidTransition)?;
        let parts: Vec<_> = suffix.split(".d").collect();
        if child.lineage.run != run
            || child.lineage.depth == 0
            || child.lineage.depth > sdk::MAX_DEPTH
            || parts.first() != Some(&"")
            || parts.len() != child.lineage.depth as usize + 1
            || parts
                .iter()
                .skip(1)
                .any(|part| part.parse::<u64>().ok().is_none_or(|part| part == 0))
            || child.lineage.chain.len() != child.lineage.depth as usize
            || !valid_id(&child.lineage.id)
            || child.target.harness.len() > 128
        {
            return Err(Error::InvalidTransition);
        }
        Ok(Reference::new(
            self.scope.clone(),
            Source::Delegation,
            child.lineage.id.clone(),
        ))
    }
    fn with_grant<T>(&self, apply: impl FnOnce() -> Result<T, Error>) -> Result<T, Error> {
        let store = Store::open(&self.store)?;
        let task = store.show(&self.scope.task)?;
        let run = run(&task, &self.scope)?;
        if task.intent_digest != self.authority.intent
            || run.admission.grant_digest != self.authority.grant
            || run.admission.workspace.to_string_lossy() != self.authority.checkout
        {
            return Err(Error::InvalidTransition);
        }
        apply()
    }
    fn start(&self, child: &sdk::Child) -> Result<(), Error> {
        let reference = self.reference(child)?;
        self.active.with_active(|| {
            self.with_grant(|| {
                update(&self.store, &self.scope.task, |book, dir| {
                    if book
                        .entries
                        .iter()
                        .any(|entry| entry.child.reference == reference)
                    {
                        return Err(Error::InvalidTransition);
                    }
                    if book.entries.len() >= MAX_CHILDREN {
                        if let Some(index) = book
                            .entries
                            .iter()
                            .position(|entry| entry.child.status != Status::Running)
                        {
                            let old = log_path(dir, &book.entries[index].child.reference);
                            if super::regular_or_absent(&old)? {
                                std::fs::remove_file(old)?;
                            }
                            book.entries.remove(index);
                            book.truncated = true;
                        } else {
                            return Err(Error::LimitExceeded);
                        }
                    }
                    let parent = child
                        .lineage
                        .id
                        .rsplit_once(".d")
                        .map(|(parent, _)| parent)
                        .filter(|parent| *parent != self.run_id())
                        .map(str::to_owned);
                    book.entries.push(Entry {
                        child: Child {
                            reference,
                            parent,
                            title: clip(&child.request.task, 512),
                            harness: clip(&child.target.harness, 128),
                            model: child.target.model.as_ref().map(|model| clip(model, 256)),
                            status: Status::Running,
                            summary: String::new(),
                            transcript: false,
                            incomplete: false,
                        },
                        authority: self.authority.clone(),
                        bytes: 0,
                        last_role: String::new(),
                    });
                    book.revision += 1;
                    Ok(())
                })
            })
        })
    }
    fn append(&self, child: &sdk::Child, value: &Value) -> Result<(), Error> {
        let reference = self.reference(child)?;
        let line = Line::from_value(value);
        self.with_grant(|| {
            update(&self.store, &self.scope.task, |book, dir| {
                let entry = book
                    .entries
                    .iter_mut()
                    .find(|entry| {
                        entry.child.reference == reference && entry.authority == self.authority
                    })
                    .ok_or(Error::NotFound)?;
                if entry.child.status != Status::Running {
                    return Ok(());
                }
                entry.child.incomplete |= line.truncated;
                if line.role == "assistant" || line.role == "assistant_delta" {
                    entry.child.summary =
                        if line.role == "assistant_delta" && entry.last_role == line.role {
                            clip(&format!("{}{}", entry.child.summary, line.text), 2048)
                        } else {
                            clip(&line.text, 2048)
                        };
                }
                if line.role == "model" && !line.text.is_empty() {
                    entry.child.model = Some(clip(&line.text, 256));
                }
                entry.last_role = line.role.clone();
                if line.role != "user" && !line.text.is_empty() {
                    entry.child.transcript = true;
                }
                let mut bytes = serde_json::to_vec(&line).map_err(|_| Error::UnsupportedSchema)?;
                bytes.push(b'\n');
                if entry.bytes.saturating_add(bytes.len()) > TRANSCRIPT_BYTES {
                    entry.child.incomplete = true;
                    book.revision += 1;
                    return Ok(());
                }
                let path = log_path(dir, &reference);
                let exists = super::regular_or_absent(&path)?;
                let mut file = super::private_open(&path, !exists, true)?;
                let actual = file.metadata()?.len() as usize;
                if actual != entry.bytes {
                    // Recover only this writer's unfinished observation copy. The
                    // SDK's original child transcript is never changed here.
                    let mut retained = Vec::new();
                    std::io::Read::by_ref(&mut file)
                        .take(entry.bytes.min(actual) as u64)
                        .read_to_end(&mut retained)?;
                    entry.bytes = retained
                        .iter()
                        .rposition(|byte| *byte == b'\n')
                        .map_or(0, |index| index + 1);
                    file.set_len(entry.bytes as u64)?;
                    entry.child.incomplete = true;
                }
                file.seek(std::io::SeekFrom::End(0))?;
                file.write_all(&bytes)?;
                file.sync_all()?;
                entry.bytes += bytes.len();
                book.revision += 1;
                Ok(())
            })
        })
    }
    fn report(
        &self,
        child: &sdk::Child,
        record: &sdk::Record,
        report: &sdk::Report,
    ) -> Result<(), Error> {
        let reference = self.reference(child)?;
        let parent = child
            .lineage
            .id
            .rsplit_once(".d")
            .map_or("", |(parent, _)| parent);
        if record.schema != sdk::RECORD_SCHEMA
            || record.run != child.lineage.run
            || record.child != child.lineage.id
            || record.parent != parent
            || record.depth != child.lineage.depth
            || record.harness != child.target.harness
            || report.child != child.lineage.id
            || report.run != child.lineage.run
            || report.depth != child.lineage.depth
        {
            return Err(Error::InvalidTransition);
        }
        self.with_grant(|| {
            update(&self.store, &self.scope.task, |book, _| {
                let entry = book
                    .entries
                    .iter_mut()
                    .find(|entry| {
                        entry.child.reference == reference && entry.authority == self.authority
                    })
                    .ok_or(Error::NotFound)?;
                let status = match report.outcome {
                    sdk::Outcome::Running => Status::Running,
                    sdk::Outcome::Done => Status::Completed,
                    sdk::Outcome::Failed => Status::Failed,
                    sdk::Outcome::Stopped => Status::Interrupted,
                };
                if entry.child.status != Status::Running && status == Status::Running {
                    return Ok(());
                }
                entry.child.status = status;
                entry.child.summary =
                    clip(report.error.as_deref().unwrap_or(&report.summary), 2048);
                entry.child.harness = clip(&report.harness, 128);
                entry.child.model = report
                    .model
                    .as_ref()
                    .map(|model| clip(model, 256))
                    .or_else(|| child.target.model.clone());
                book.revision += 1;
                Ok(())
            })
        })
    }
}
impl sdk::Observer for Observer {
    fn started(&self, child: &sdk::Child) -> Result<(), String> {
        self.start(child)
            .map_err(|_| "This child task couldn't start. Send a new message and try again.".into())
    }
    fn line(&self, child: &sdk::Child, line: &Value) {
        if self.append(child, line).is_err() {
            let _ = self.mark_incomplete(child);
        }
    }
    fn reported(&self, child: &sdk::Child, record: &sdk::Record, report: &sdk::Report) {
        if self.report(child, record, report).is_err() {
            let _ = self.mark_incomplete(child);
        }
    }
}
impl Observer {
    fn mark_incomplete(&self, child: &sdk::Child) -> Result<(), Error> {
        let reference = self.reference(child)?;
        self.with_grant(|| {
            update(&self.store, &self.scope.task, |book, _| {
                if let Some(entry) = book
                    .entries
                    .iter_mut()
                    .find(|entry| entry.child.reference == reference)
                {
                    entry.child.incomplete = true;
                    book.revision += 1;
                }
                Ok(())
            })
        })
    }
}

#[derive(Serialize, Deserialize)]
struct Entry {
    child: Child,
    authority: Authority,
    bytes: usize,
    last_role: String,
}
#[derive(Serialize, Deserialize)]
struct Book {
    schema: String,
    task: String,
    revision: u64,
    entries: Vec<Entry>,
    truncated: bool,
}
impl Book {
    fn new(task: &str) -> Self {
        Self {
            schema: BOOK_SCHEMA.into(),
            task: task.into(),
            revision: 0,
            entries: vec![],
            truncated: false,
        }
    }
    fn valid(&self, task: &str) -> bool {
        self.schema == BOOK_SCHEMA
            && self.task == task
            && self.entries.len() <= MAX_CHILDREN
            && self.entries.iter().enumerate().all(|(index, entry)| {
                entry.child.valid()
                    && !self.entries[..index]
                        .iter()
                        .any(|old| old.child.reference.id == entry.child.reference.id)
                    && entry.child.reference.scope.host == local::LOCAL_HOST
                    && entry.child.reference.source == Source::Delegation
                    && entry.child.reference.scope.task == task
                    && entry.bytes <= TRANSCRIPT_BYTES
                    && entry.last_role.len() <= 32
            })
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            schema: SCHEMA.into(),
            task: self.task.clone(),
            revision: self.revision,
            children: self
                .entries
                .iter()
                .map(|entry| entry.child.clone())
                .collect(),
            truncated: self.truncated,
        }
    }
}
fn directory(store: &Path, task: &str) -> Result<PathBuf, Error> {
    let root = store.join("children");
    super::prepare_directory(&root)?;
    let dir = root.join(digest_of(&task).as_str().trim_start_matches("sha256:"));
    super::prepare_directory(&dir)?;
    Ok(dir)
}
fn log_path(dir: &Path, reference: &Reference) -> PathBuf {
    dir.join(format!(
        "{}.jsonl",
        reference.id.as_str().trim_start_matches("sha256:")
    ))
}
fn update<T>(
    store: &Path,
    task: &str,
    apply: impl FnOnce(&mut Book, &Path) -> Result<T, Error>,
) -> Result<T, Error> {
    let dir = directory(store, task)?;
    let lock = super::open_lock(&dir.join("book.lock"))?;
    super::take_lock(&lock, Duration::from_secs(1))?;
    super::verify_same_file(&dir.join("book.lock"), &lock)?;
    let path = dir.join("book.json");
    let mut original = Vec::new();
    let mut book = if super::regular_or_absent(&path)? {
        super::private_open(&path, false, false)?
            .take(BOOK_BYTES + 1)
            .read_to_end(&mut original)?;
        if original.len() as u64 > BOOK_BYTES {
            return Err(Error::LimitExceeded);
        }
        let book: Book = serde_json::from_slice(&original)
            .map_err(|_| Error::Corrupt("child activity could not be read"))?;
        if !book.valid(task) {
            return Err(Error::Corrupt("child activity has an invalid scope"));
        }
        book
    } else {
        Book::new(task)
    };
    let result = apply(&mut book, &dir);
    if !book.valid(task) {
        return Err(Error::UnsupportedSchema);
    }
    let bytes = serde_json::to_vec(&book).map_err(|_| Error::UnsupportedSchema)?;
    if bytes.len() as u64 > BOOK_BYTES {
        return Err(Error::LimitExceeded);
    }
    if bytes != original {
        super::write_new(&dir.join("book.next"), &bytes)?;
        super::regular_or_absent(&path)?;
        std::fs::rename(dir.join("book.next"), path)?;
        super::sync_directory(&dir)?;
    }
    result
}
fn run<'a>(task: &'a Task, scope: &Scope) -> Result<&'a super::owner::Run, Error> {
    if scope.host != local::LOCAL_HOST
        || scope.task != task.task_id
        || scope.turn == 0
        || scope.generation == 0
    {
        return Err(Error::InvalidTransition);
    }
    let file = task.trace_file(scope.turn);
    task.run
        .iter()
        .chain(task.earlier.iter())
        .find(|run| run.admission.trace_file == file)
        .ok_or(Error::NotFound)
}

#[derive(Serialize, Deserialize)]
struct Line {
    role: String,
    text: String,
    truncated: bool,
}
impl Line {
    fn from_value(value: &Value) -> Self {
        let role = clip(value["role"].as_str().unwrap_or("event"), 32);
        let (text, truncated) =
            if let Some(text) = value["text"].as_str().or_else(|| value["model"].as_str()) {
                (clip(text, LINE_BYTES), text.len() > LINE_BYTES)
            } else {
                let mut bounded = Limited {
                    bytes: vec![],
                    truncated: false,
                };
                let _ = serde_json::to_writer_pretty(&mut bounded, value);
                let mut bytes = bounded.bytes;
                let end = std::str::from_utf8(&bytes)
                    .err()
                    .map_or(bytes.len(), |error| error.valid_up_to());
                bytes.truncate(end);
                (
                    String::from_utf8(bytes).unwrap_or_default(),
                    bounded.truncated,
                )
            };
        Self {
            role,
            text,
            truncated,
        }
    }
}
struct Limited {
    bytes: Vec<u8>,
    truncated: bool,
}
impl std::io::Write for Limited {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let remaining = LINE_BYTES.saturating_sub(self.bytes.len());
        self.bytes
            .extend_from_slice(&bytes[..bytes.len().min(remaining)]);
        if bytes.len() > remaining {
            self.truncated = true;
            Err(std::io::Error::other("child observation limit"))
        } else {
            Ok(bytes.len())
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn clip(value: &str, max: usize) -> String {
    openagents_chat::children::clip(value, max)
}
fn render_sdk(bytes: &[u8]) -> (String, bool) {
    let mut entries: Vec<(String, String)> = vec![];
    let mut active: Option<usize> = None;
    let mut incomplete = false;
    for raw in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let Ok(line) = serde_json::from_slice::<Line>(raw) else {
            incomplete = true;
            continue;
        };
        if line.text.len() > LINE_BYTES || line.role.len() > 32 {
            incomplete = true;
            continue;
        }
        incomplete |= line.truncated;
        match line.role.as_str() {
            "assistant_delta" => {
                let index = if let Some(index) = active {
                    index
                } else {
                    entries.push(("Agent".into(), String::new()));
                    let index = entries.len() - 1;
                    active = Some(index);
                    index
                };
                entries[index].1.push_str(&line.text);
            }
            "assistant" => {
                if let Some(index) = active.take() {
                    entries[index].1 = line.text;
                } else {
                    entries.push(("Agent".into(), line.text));
                }
            }
            other => {
                active = None;
                entries.push((
                    match other {
                        "user" => "Task",
                        "tool" => "Tool",
                        "model" => "Model",
                        _ => "Update",
                    }
                    .into(),
                    line.text,
                ));
            }
        }
    }
    let mut text = String::new();
    for (role, value) in entries {
        if text.len() + role.len() + value.len() + 4 > TRANSCRIPT_BYTES {
            incomplete = true;
            break;
        }
        text.push_str(&format!("{role}:\n{value}\n\n"));
    }
    (text, incomplete)
}
fn page(read: &Read, text: &str, truncated: bool, status: Status) -> Result<Page, Error> {
    if !read.reference.valid() {
        return Err(Error::InvalidTransition);
    }
    let offset = read.cursor.as_ref().map_or(0, |cursor| cursor.offset);
    if offset > text.len() || offset > TRANSCRIPT_BYTES || !text.is_char_boundary(offset) {
        return Err(Error::InvalidTransition);
    }
    if let Some(cursor) = &read.cursor
        && cursor.prefix != route_contract::Digest::of_bytes(&text.as_bytes()[..offset])
    {
        return Err(Error::InvalidTransition);
    }
    let mut end = (offset + PAGE_BYTES).min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let next = (end < text.len()).then(|| Cursor {
        offset: end,
        prefix: route_contract::Digest::of_bytes(&text.as_bytes()[..end]),
    });
    Ok(Page {
        reference: read.reference.id.clone(),
        offset,
        text: text[offset..end].into(),
        next,
        truncated,
        running: status == Status::Running,
    })
}

impl local::Local {
    pub fn sdk_children(&self, task: &str) -> Result<Snapshot, String> {
        if local::record(self.store(), task).is_none() {
            return Err("This task is not on this computer.".into());
        }
        let store = Store::open(self.store()).map_err(|_| "This task is unavailable.")?;
        let current = store.show(task).map_err(|_| "This task is unavailable.")?;
        update(self.store(), task, |book, _| {
            let mut snapshot = book.snapshot();
            snapshot
                .children
                .retain(|child| run(&current, &child.reference.scope).is_ok());
            Ok(snapshot)
        })
        .map_err(|_| "Child activity is unavailable. Try again.".into())
    }
    pub fn child_transcript(&self, task: &str, read: &Read) -> Result<Page, String> {
        self.child_read(task, read).map_err(|_| {
            "This child transcript changed or is unavailable. Refresh it and try again.".into()
        })
    }
    fn child_read(&self, task: &str, read: &Read) -> Result<Page, Error> {
        if !read.reference.valid()
            || read.reference.scope.task != task
            || read.reference.scope.host != local::LOCAL_HOST
            || local::record(self.store(), task).is_none()
        {
            return Err(Error::InvalidTransition);
        }
        let store = Store::open(self.store())?;
        let current = store.show(task)?;
        let owned = run(&current, &read.reference.scope)?;
        match read.reference.source {
            Source::Delegation => update(self.store(), task, |book, dir| {
                let entry = book
                    .entries
                    .iter()
                    .find(|entry| {
                        entry.child.reference == read.reference
                            && entry.authority.intent == current.intent_digest
                            && entry.authority.grant == owned.admission.grant_digest
                    })
                    .ok_or(Error::NotFound)?;
                let path = log_path(dir, &entry.child.reference);
                let mut bytes = Vec::new();
                if entry.bytes > 0 {
                    super::private_open(&path, false, false)?
                        .take(entry.bytes as u64)
                        .read_to_end(&mut bytes)?;
                }
                let (text, mut incomplete) = render_sdk(&bytes);
                incomplete |= entry.child.incomplete || bytes.len() != entry.bytes;
                page(read, &text, incomplete, entry.child.status)
            }),
            Source::Native => {
                let file = Path::new(&owned.admission.trace_file);
                if file.components().count() != 1 {
                    return Err(Error::UnsafePath);
                }
                let mut bytes = Vec::new();
                super::private_open(&self.store().join(file), false, false)?
                    .take(TRACE_BYTES + 1)
                    .read_to_end(&mut bytes)?;
                if bytes.len() as u64 > TRACE_BYTES {
                    return Err(Error::LimitExceeded);
                }
                let observations: Vec<Observation> = bytes
                    .split(|byte| *byte == b'\n')
                    .filter_map(|line| serde_json::from_slice::<Value>(line).ok())
                    .filter(|line| {
                        line["record"] == "step"
                            && matches!(line["step"]["source"].as_str(), Some("System" | "system"))
                    })
                    .filter_map(|line| {
                        serde_json::from_value(line["step"]["extensions"][TRACE_KEY].clone()).ok()
                    })
                    .collect();
                let (text, incomplete, status) =
                    openagents_chat::children::transcript(&observations, &read.reference)
                        .ok_or(Error::NotFound)?;
                page(read, &text, incomplete, status)
            }
        }
    }
}
