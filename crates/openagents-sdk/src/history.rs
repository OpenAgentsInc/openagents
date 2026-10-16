//! Saved account history. References carry their account and deployment; readers
//! make one bounded GET per call and never drive an engine or write a chat.
use crate::{CancellationToken, Client, Deployment, Error, Session};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use std::{collections::BTreeMap, future::Future};

macro_rules! open_enum {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        #[derive(Clone,Debug,PartialEq,Eq)]
        pub enum $name { $($variant,)+ Other(String) }
        impl $name { pub fn as_str(&self)->&str { match self {$(Self::$variant=>$wire,)+Self::Other(s)=>s} } }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D:serde::Deserializer<'de>>(d:D)->Result<Self,D::Error> {
                Ok(match String::deserialize(d)?.as_str() { $($wire=>Self::$variant,)+s=>Self::Other(s.to_owned()) })
            }
        }
    }
}
open_enum!(ChatSource { Web=>"web", Terminal=>"terminal", Phone=>"phone" });
open_enum!(MessageRole { User=>"user", Assistant=>"assistant", Tool=>"tool", Thinking=>"thinking" });
open_enum!(ToolStatus { Running=>"running", Done=>"done", Failed=>"failed" });
open_enum!(FileKind { Png=>"png", Jpeg=>"jpeg", Gif=>"gif", Webp=>"webp", Pdf=>"pdf", Text=>"text", Step=>"step", Stl=>"stl" });
open_enum!(CaptureState { Saved=>"saved", Unavailable=>"unavailable", NotSaved=>"not_saved" });

#[derive(Clone, Debug, PartialEq, Eq)]
struct Context {
    deployment: Deployment,
    account: String,
}
impl Context {
    fn of(session: &Session) -> Self {
        Self {
            deployment: session.deployment().clone(),
            account: session.account().id.clone(),
        }
    }
    fn check(&self, client: &Client, session: &Session) -> Result<(), Error> {
        client.check_session(session)?;
        if self.deployment != *client.deployment() {
            return Err(Error::WrongDeployment);
        }
        if self.account != session.account().id {
            return Err(Error::WrongAccount);
        }
        Ok(())
    }
}
/// A saved chat ID in one account. Construct with `Client::chat` or a summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatRef {
    context: Context,
    id: String,
}
impl ChatRef {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn account(&self) -> &str {
        &self.context.account
    }
    pub fn deployment(&self) -> &Deployment {
        &self.context.deployment
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatSummary {
    pub chat: ChatRef,
    pub title: String,
    pub revision: u64,
    pub updated_unix: u64,
    pub pinned_unix: Option<u64>,
    pub archived_unix: Option<u64>,
    pub project: Option<String>,
    pub source: ChatSource,
    pub messages: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryProject {
    context: Context,
    id: String,
    pub name: String,
    pub shared: bool,
}
impl HistoryProject {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn account(&self) -> &str {
        &self.context.account
    }
    pub fn deployment(&self) -> &Deployment {
        &self.context.deployment
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ToolDetail {
    pub name: String,
    pub summary: String,
    pub status: ToolStatus,
    pub input: String,
    pub output: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum DownloadKind {
    File,
    Capture,
}
/// Only the SDK constructs these references, from saved metadata. No URL or bearer is embedded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttachmentRef {
    chat: ChatRef,
    id: String,
    kind: DownloadKind,
    size: u64,
}
impl AttachmentRef {
    pub fn chat(&self) -> &ChatRef {
        &self.chat
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn size(&self) -> u64 {
        self.size
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedFile {
    pub id: String,
    pub name: String,
    pub kind: FileKind,
    pub size: u64,
    download: Option<AttachmentRef>,
}
impl SavedFile {
    pub fn download_ref(&self) -> Option<&AttachmentRef> {
        self.download.as_ref()
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedCapture {
    pub id: String,
    pub name: String,
    pub state: CaptureState,
    pub size: Option<u64>,
    pub media: Option<String>,
    download: Option<AttachmentRef>,
}
impl SavedCapture {
    pub fn download_ref(&self) -> Option<&AttachmentRef> {
        self.download.as_ref()
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedMessage {
    pub id: String,
    pub role: MessageRole,
    pub text: String,
    pub files: Vec<SavedFile>,
    pub tool: Option<ToolDetail>,
    /// Additive content, including future generated-file/artifact objects. It is never fetched automatically.
    pub extra: BTreeMap<String, Value>,
}
#[derive(Clone, Debug)]
pub struct HistoryPage<T> {
    pub data: Vec<T>,
    pub has_more: bool,
}
#[derive(Clone, Debug)]
pub struct TranscriptPage {
    pub thread: ChatSummary,
    pub revision: u64,
    pub data: Vec<SavedMessage>,
    pub captures: Vec<SavedCapture>,
    pub has_more: bool,
}

#[derive(Deserialize)]
struct WireSummary {
    id: String,
    title: String,
    revision: u64,
    updated_unix: u64,
    pinned_unix: Option<u64>,
    archived_unix: Option<u64>,
    project: Option<String>,
    source: ChatSource,
    messages: usize,
}
impl WireSummary {
    fn convert(self, context: &Context) -> Result<ChatSummary, Error> {
        if !chat_id(&self.id) || self.revision == 0 {
            return Err(Error::InvalidResponse);
        }
        Ok(ChatSummary {
            chat: ChatRef {
                context: context.clone(),
                id: self.id,
            },
            title: self.title,
            revision: self.revision,
            updated_unix: self.updated_unix,
            pinned_unix: self.pinned_unix,
            archived_unix: self.archived_unix,
            project: self.project,
            source: self.source,
            messages: self.messages,
        })
    }
}
#[derive(Deserialize)]
struct WireProject {
    id: String,
    name: String,
    shared: bool,
}
trait Identity {
    fn id(&self) -> &str;
    fn valid(&self) -> bool {
        true
    }
}
impl Identity for WireSummary {
    fn valid(&self) -> bool {
        self.revision > 0
    }
    fn id(&self) -> &str {
        &self.id
    }
}
impl Identity for WireProject {
    fn id(&self) -> &str {
        &self.id
    }
}
#[derive(Deserialize)]
struct WireList<T> {
    data: Vec<T>,
    next: Option<String>,
}
#[derive(Deserialize)]
struct WireFile {
    id: String,
    name: String,
    kind: FileKind,
    size: u64,
}
#[derive(Deserialize)]
struct WireCapture {
    id: String,
    name: String,
    state: CaptureState,
    size: Option<u64>,
    media: Option<String>,
}
#[derive(Deserialize)]
struct WireMessage {
    id: String,
    role: MessageRole,
    #[serde(default)]
    text: String,
    #[serde(default)]
    files: Vec<WireFile>,
    #[serde(default)]
    tool: Option<ToolDetail>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}
#[derive(Deserialize)]
struct WireTranscript {
    thread: WireSummary,
    revision: u64,
    data: Vec<WireMessage>,
    next: Option<String>,
    #[serde(default)]
    captures: Vec<WireCapture>,
}

fn chat_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}
fn project_id(id: &str) -> bool {
    id.starts_with("prj_")
        && id.len() <= 128
        && id.len() > 4
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
fn file_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn history_error(error: Error) -> Error {
    match error {
        Error::Refused { status: 404, .. } => Error::NotFound,
        Error::Refused {
            status: 409,
            code: Some("changed"),
        } => Error::Changed,
        Error::Refused {
            status: 400,
            code: Some("invalid_cursor"),
        } => Error::InvalidCursor,
        error => error,
    }
}
async fn cancellable<T>(
    cancel: &CancellationToken,
    f: impl Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    tokio::select! {biased; _=cancel.cancelled()=>Err(Error::Cancelled),result=f=>result.map_err(history_error)}
}
fn size(n: usize) -> Result<usize, Error> {
    if (1..=200).contains(&n) {
        Ok(n)
    } else {
        Err(Error::InvalidCursor)
    }
}

#[derive(Clone, Debug)]
struct ListReader {
    context: Context,
    project: Option<String>,
    projects: bool,
    after: Option<String>,
    limit: usize,
    done: bool,
}
impl ListReader {
    fn new(session: &Session, project: Option<String>, projects: bool) -> Self {
        Self {
            context: Context::of(session),
            project,
            projects,
            after: None,
            limit: 100,
            done: false,
        }
    }
    async fn next<T: DeserializeOwned + Identity>(
        &mut self,
        client: &Client,
        session: &Session,
        cancel: &CancellationToken,
    ) -> Result<Option<WireList<T>>, Error> {
        self.context.check(client, session)?;
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if self.done {
            return Ok(None);
        }
        let mut query = vec![("limit", self.limit.to_string())];
        if let Some(after) = &self.after {
            query.push(("after", after.clone()));
        }
        if let Some(project) = &self.project {
            query.push(("project", project.clone()));
        }
        let path = if self.projects {
            vec!["history", "projects"]
        } else {
            vec!["history"]
        };
        let page: WireList<T> =
            cancellable(cancel, client.history_json(&path, &query, session.token())).await?;
        let valid = |id: &str| {
            if self.projects {
                project_id(id)
            } else {
                chat_id(id)
            }
        };
        if page.data.len() > self.limit {
            return Err(Error::InvalidResponse);
        }
        let mut previous = self.after.as_deref();
        for row in &page.data {
            if !row.valid() || !valid(row.id()) || previous.is_some_and(|p| row.id() <= p) {
                return Err(Error::InvalidResponse);
            }
            previous = Some(row.id());
        }
        if let Some(next) = page.next.as_deref() {
            if !valid(next)
                || self.after.as_deref().is_some_and(|a| next <= a)
                || previous.is_some_and(|last| next < last)
            {
                return Err(Error::InvalidCursor);
            }
        }
        self.after = page.next.clone();
        self.done = page.next.is_none();
        Ok(Some(page))
    }
}
#[derive(Clone, Debug)]
pub struct ChatReader(ListReader);
impl ChatReader {
    pub fn with_page_size(mut self, limit: usize) -> Result<Self, Error> {
        self.0.limit = size(limit)?;
        Ok(self)
    }
    pub async fn next_page(
        &mut self,
        client: &Client,
        session: &Session,
        cancel: &CancellationToken,
    ) -> Result<Option<HistoryPage<ChatSummary>>, Error> {
        let Some(page) = self.0.next::<WireSummary>(client, session, cancel).await? else {
            return Ok(None);
        };
        let data = page
            .data
            .into_iter()
            .map(|s| s.convert(&self.0.context))
            .collect::<Result<_, _>>()?;
        Ok(Some(HistoryPage {
            data,
            has_more: page.next.is_some(),
        }))
    }
}
#[derive(Clone, Debug)]
pub struct ProjectReader(ListReader);
impl ProjectReader {
    pub fn with_page_size(mut self, limit: usize) -> Result<Self, Error> {
        self.0.limit = size(limit)?;
        Ok(self)
    }
    pub async fn next_page(
        &mut self,
        client: &Client,
        session: &Session,
        cancel: &CancellationToken,
    ) -> Result<Option<HistoryPage<HistoryProject>>, Error> {
        let Some(page) = self.0.next::<WireProject>(client, session, cancel).await? else {
            return Ok(None);
        };
        Ok(Some(HistoryPage {
            data: page
                .data
                .into_iter()
                .map(|p| HistoryProject {
                    context: self.0.context.clone(),
                    id: p.id,
                    name: p.name,
                    shared: p.shared,
                })
                .collect(),
            has_more: page.next.is_some(),
        }))
    }
}

#[derive(Clone, Debug)]
pub struct TranscriptReader {
    chat: ChatRef,
    revision: Option<u64>,
    at: usize,
    limit: usize,
    done: bool,
    failed: Option<Error>,
}
impl TranscriptReader {
    pub fn with_page_size(mut self, limit: usize) -> Result<Self, Error> {
        self.limit = size(limit)?;
        Ok(self)
    }
    pub fn chat(&self) -> &ChatRef {
        &self.chat
    }
    pub async fn next_page(
        &mut self,
        client: &Client,
        session: &Session,
        cancel: &CancellationToken,
    ) -> Result<Option<TranscriptPage>, Error> {
        self.chat.context.check(client, session)?;
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        if self.done {
            return Ok(None);
        }
        let mut query = vec![("limit", self.limit.to_string())];
        if let Some(revision) = self.revision {
            query.push(("revision", revision.to_string()));
        }
        if self.at > 0 {
            query.push(("after", format!("msg_{}", self.at - 1)));
        }
        let result = cancellable(
            cancel,
            client.history_json::<WireTranscript>(
                &["history", self.chat.id(), "messages"],
                &query,
                session.token(),
            ),
        )
        .await;
        let page = match result {
            Ok(p) => p,
            Err(e) => {
                if matches!(e, Error::Changed | Error::InvalidCursor) {
                    self.failed = Some(e.clone());
                }
                return Err(e);
            }
        };
        if page.revision == 0 || self.revision.is_some_and(|r| r != page.revision) {
            self.failed = Some(Error::Changed);
            return Err(Error::Changed);
        }
        let thread = page.thread.convert(&self.chat.context)?;
        if thread.chat != self.chat
            || thread.revision != page.revision
            || page.data.len() > self.limit
            || page.captures.len() > 8
        {
            return Err(Error::InvalidResponse);
        }
        let end = self
            .at
            .checked_add(page.data.len())
            .ok_or(Error::InvalidResponse)?;
        if end > thread.messages
            || (page.next.is_none() && end != thread.messages)
            || (page.next.is_some()
                && (page.data.is_empty()
                    || end >= thread.messages
                    || page.next.as_deref() != Some(format!("msg_{}", end - 1).as_str())))
        {
            return Err(Error::InvalidCursor);
        }
        let mut data = Vec::new();
        for (n, m) in page.data.into_iter().enumerate() {
            if m.id != format!("msg_{}", self.at + n) {
                return Err(Error::InvalidCursor);
            }
            let files = m
                .files
                .into_iter()
                .map(|f| {
                    let download = (file_id(&f.id) && f.size > 0 && f.size <= 10 * 1024 * 1024)
                        .then(|| AttachmentRef {
                            chat: self.chat.clone(),
                            id: f.id.clone(),
                            kind: DownloadKind::File,
                            size: f.size,
                        });
                    SavedFile {
                        id: f.id,
                        name: f.name,
                        kind: f.kind,
                        size: f.size,
                        download,
                    }
                })
                .collect();
            data.push(SavedMessage {
                id: m.id,
                role: m.role,
                text: m.text,
                files,
                tool: m.tool,
                extra: m.extra,
            });
        }
        let captures = page
            .captures
            .into_iter()
            .map(|c| {
                let download = (c.state == CaptureState::Saved && chat_id(&c.id))
                    .then_some(c.size)
                    .flatten()
                    .filter(|s| *s <= 15 * 1024 * 1024)
                    .map(|size| AttachmentRef {
                        chat: self.chat.clone(),
                        id: c.id.clone(),
                        kind: DownloadKind::Capture,
                        size,
                    });
                SavedCapture {
                    id: c.id,
                    name: c.name,
                    state: c.state,
                    size: c.size,
                    media: c.media,
                    download,
                }
            })
            .collect();
        self.at = end;
        self.revision = Some(page.revision);
        self.done = page.next.is_none();
        Ok(Some(TranscriptPage {
            thread,
            revision: page.revision,
            data,
            captures,
            has_more: !self.done,
        }))
    }
}

impl Client {
    /// Bind an explicit saved ID to this account; the service checks access on each read.
    pub fn chat(&self, session: &Session, id: &str) -> Result<ChatRef, Error> {
        self.check_session(session)?;
        if !chat_id(id) {
            return Err(Error::InvalidReference);
        }
        Ok(ChatRef {
            context: Context::of(session),
            id: id.to_owned(),
        })
    }
    pub fn history(&self, session: &Session) -> Result<ChatReader, Error> {
        self.check_session(session)?;
        Ok(ChatReader(ListReader::new(session, None, false)))
    }
    pub fn history_projects(&self, session: &Session) -> Result<ProjectReader, Error> {
        self.check_session(session)?;
        Ok(ProjectReader(ListReader::new(session, None, true)))
    }
    pub fn shared_history(
        &self,
        session: &Session,
        project: &HistoryProject,
    ) -> Result<ChatReader, Error> {
        project.context.check(self, session)?;
        if !project.shared || !project_id(&project.id) {
            return Err(Error::InvalidReference);
        }
        Ok(ChatReader(ListReader::new(
            session,
            Some(project.id.clone()),
            false,
        )))
    }
    pub fn transcript(&self, session: &Session, chat: &ChatRef) -> Result<TranscriptReader, Error> {
        chat.context.check(self, session)?;
        Ok(TranscriptReader {
            chat: chat.clone(),
            revision: None,
            at: 0,
            limit: 100,
            done: false,
            failed: None,
        })
    }
    /// Fetch only this deployment's authorized saved bytes. No redirects, files, hosts, or external URLs are followed.
    pub async fn download(
        &self,
        session: &Session,
        file: &AttachmentRef,
        cancel: &CancellationToken,
    ) -> Result<Vec<u8>, Error> {
        file.chat.context.check(self, session)?;
        let kind = match file.kind {
            DownloadKind::File => "files",
            DownloadKind::Capture => "captures",
        };
        let bytes = cancellable(
            cancel,
            self.history_bytes(
                &["history", file.chat.id(), kind, &file.id],
                session.token(),
                file.size as usize,
            ),
        )
        .await?;
        if bytes.len() as u64 != file.size {
            return Err(Error::InvalidResponse);
        }
        Ok(bytes)
    }
}

/// Optional in-memory merge for owned/shared pages in one account. Replace it on
/// access refresh; it does not grant authority to read or download cached rows.
#[derive(Clone, Debug)]
pub struct HistoryIndex {
    context: Context,
    rows: BTreeMap<String, ChatSummary>,
}
impl HistoryIndex {
    pub fn extend(&mut self, rows: impl IntoIterator<Item = ChatSummary>) -> Result<(), Error> {
        let rows: Vec<_> = rows.into_iter().collect();
        for row in &rows {
            if row.chat.context.deployment != self.context.deployment {
                return Err(Error::WrongDeployment);
            }
            if row.chat.context.account != self.context.account {
                return Err(Error::WrongAccount);
            }
        }
        for row in rows {
            if self
                .rows
                .get(row.chat.id())
                .is_none_or(|old| old.revision <= row.revision)
            {
                self.rows.insert(row.chat.id.clone(), row);
            }
        }
        Ok(())
    }
    pub fn len(&self) -> usize {
        self.rows.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub fn clear(&mut self) {
        self.rows.clear();
    }
    pub fn get(&self, id: &str) -> Option<&ChatSummary> {
        self.rows.get(id)
    }
    pub fn sorted(&self, archived: bool) -> Vec<&ChatSummary> {
        let mut rows: Vec<_> = self
            .rows
            .values()
            .filter(|r| r.archived_unix.is_some() == archived)
            .collect();
        rows.sort_by(|a, b| {
            a.pinned_unix
                .is_none()
                .cmp(&b.pinned_unix.is_none())
                .then_with(|| b.pinned_unix.cmp(&a.pinned_unix))
                .then_with(|| b.updated_unix.cmp(&a.updated_unix))
                .then_with(|| a.chat.id.cmp(&b.chat.id))
        });
        rows
    }
}
impl Client {
    pub fn history_index(&self, session: &Session) -> Result<HistoryIndex, Error> {
        self.check_session(session)?;
        Ok(HistoryIndex {
            context: Context::of(session),
            rows: BTreeMap::new(),
        })
    }
}
