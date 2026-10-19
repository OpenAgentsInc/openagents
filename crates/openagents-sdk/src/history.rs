//! Saved account history. References carry their account and deployment; readers
//! make one bounded GET per call and never drive an engine or write a chat.
use crate::{CancellationToken, Client, Deployment, Error, Session};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use std::{collections::BTreeMap, future::Future};

macro_rules! open_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone,Debug,PartialEq,Eq)]
        pub enum $name { $($variant,)+ Other(String) }
        impl $name {
            /// The value as the service writes it.
            pub fn as_str(&self)->&str { match self {$(Self::$variant=>$wire,)+Self::Other(s)=>s} }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D:serde::Deserializer<'de>>(d:D)->Result<Self,D::Error> {
                Ok(match String::deserialize(d)?.as_str() { $($wire=>Self::$variant,)+s=>Self::Other(s.to_owned()) })
            }
        }
    }
}
open_enum!(
    /// Where a chat started. `Other` holds a value this SDK doesn't know.
    ChatSource { Web=>"web", Terminal=>"terminal", Phone=>"phone" });
open_enum!(
    /// Who wrote a saved message. `Other` holds a value this SDK doesn't know.
    MessageRole { User=>"user", Assistant=>"assistant", Tool=>"tool", Thinking=>"thinking" });
open_enum!(
    /// Whether a tool call is running, done, or failed.
    ToolStatus { Running=>"running", Done=>"done", Failed=>"failed" });
open_enum!(
    /// The type of a saved file.
    FileKind { Png=>"png", Jpeg=>"jpeg", Gif=>"gif", Webp=>"webp", Pdf=>"pdf", Text=>"text", Step=>"step", Stl=>"stl" });
open_enum!(
    /// Whether a capture's media was saved.
    CaptureState { Saved=>"saved", Unavailable=>"unavailable", NotSaved=>"not_saved" });

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
    /// The chat's ID.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// The account the chat belongs to.
    pub fn account(&self) -> &str {
        &self.context.account
    }
    /// The deployment the chat is on.
    pub fn deployment(&self) -> &Deployment {
        &self.context.deployment
    }
}

/// One chat in the account's history list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatSummary {
    pub home: chat_home::Home,
    pub links: chat_home::Links,
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
/// A project in the account's history, which may be shared.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryProject {
    context: Context,
    id: String,
    pub name: String,
    pub shared: bool,
}
impl HistoryProject {
    /// The project's ID.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// The account the project belongs to.
    pub fn account(&self) -> &str {
        &self.context.account
    }
    /// The deployment the project is on.
    pub fn deployment(&self) -> &Deployment {
        &self.context.deployment
    }
}

/// A tool call in a saved message: its name, status, input, and output.
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
    /// The chat the attachment belongs to.
    pub fn chat(&self) -> &ChatRef {
        &self.chat
    }
    /// The attachment's ID.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// The attachment's size in bytes.
    pub fn size(&self) -> u64 {
        self.size
    }
}
/// A file attached to a saved message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedFile {
    pub id: String,
    pub name: String,
    pub kind: FileKind,
    pub size: u64,
    download: Option<AttachmentRef>,
}
impl SavedFile {
    /// The reference to download the file with, when it can be downloaded.
    pub fn download_ref(&self) -> Option<&AttachmentRef> {
        self.download.as_ref()
    }
}
/// A capture saved with a chat transcript.
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
    /// The reference to download the capture with, when it can be downloaded.
    pub fn download_ref(&self) -> Option<&AttachmentRef> {
        self.download.as_ref()
    }
}
/// A saved Lab card and the account-bound document used to read its media.
#[derive(Clone, Debug, PartialEq)]
pub struct SavedLabCard {
    pub card: lab_kit::Card,
    document: crate::labs::DocumentRef,
}
impl SavedLabCard {
    /// The document used to read the card's media.
    pub fn document_ref(&self) -> &crate::labs::DocumentRef {
        &self.document
    }
}
/// One message in a saved chat transcript.
#[derive(Clone, Debug, PartialEq)]
pub struct SavedMessage {
    pub id: String,
    pub role: MessageRole,
    pub text: String,
    pub files: Vec<SavedFile>,
    pub tool: Option<ToolDetail>,
    pub cards: Vec<SavedLabCard>,
    /// Additive content, including future generated-file/artifact objects. It is never fetched automatically.
    pub extra: BTreeMap<String, Value>,
}
/// One page of a history list.
#[derive(Clone, Debug)]
pub struct HistoryPage<T> {
    pub data: Vec<T>,
    pub has_more: bool,
    /// Computer sessions (#11897), newest first, on the first page of the
    /// account's or a shared project's chats. They are not chats in the
    /// account's store, so they never join `data` or its paging.
    pub sessions: Vec<crate::sessions::SessionSummary>,
}
/// One page of a chat transcript, with the chat's summary and captures.
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
    #[serde(default)]
    home: Option<chat_home::Home>,
    #[serde(default, flatten)]
    links: chat_home::Links,
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
        if self.links.continues.as_ref().is_some_and(|p| !p.valid())
            || self.links.continued_by.as_ref().is_some_and(|p| !p.valid())
        {
            return Err(Error::InvalidResponse);
        }
        Ok(ChatSummary {
            home: self.home.unwrap_or_else(|| {
                if matches!(self.source, ChatSource::Terminal | ChatSource::Phone) {
                    chat_home::Home::Computer(format!("legacy-{}", self.id))
                } else {
                    chat_home::Home::Cloud
                }
            }),
            links: self.links,
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
    #[serde(default)]
    sessions: Vec<Value>,
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
    #[serde(default)]
    cards: Vec<WireLabCard>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}
#[derive(Deserialize)]
struct WireLabCard {
    #[serde(flatten)]
    card: lab_kit::Card,
    #[serde(default = "personal_space")]
    space: String,
}
fn personal_space() -> String {
    "me".into()
}
impl WireLabCard {
    fn convert(mut self, client: &crate::labs::Client) -> Result<SavedLabCard, Error> {
        if self.card.schema != lab_kit::card::SCHEMA
            || self.card.version == 0
            || self.card.kind.is_empty()
        {
            return Err(Error::InvalidResponse);
        }
        self.card.view = self.card.view.drawn();
        let document = client
            .document_ref(&self.space, &self.card.document)
            .map_err(|_| Error::InvalidResponse)?;
        Ok(SavedLabCard {
            card: self.card,
            document,
        })
    }
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
    query: String,
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
            query: String::new(),
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
        if !self.query.is_empty() {
            query.push(("query", self.query.clone()));
        }
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
/// Reads the account's chat list a page at a time.
#[derive(Clone, Debug)]
pub struct ChatReader(ListReader);
impl ChatReader {
    /// Search titles across authorized history pages. Empty pages can still have
    /// a next cursor; continue paging to search the rest of the account.
    pub fn with_query(mut self, query: &str) -> Result<Self, Error> {
        let query = query.trim();
        if query.chars().count() > 200 || query.chars().any(char::is_control) {
            return Err(Error::InvalidReference);
        }
        self.0.query = query.to_owned();
        self.0.after = None;
        self.0.done = false;
        Ok(self)
    }

    /// Sets how many chats each page holds, from 1 to 200.
    pub fn with_page_size(mut self, limit: usize) -> Result<Self, Error> {
        self.0.limit = size(limit)?;
        Ok(self)
    }
    /// Reads the next page of chats. Returns `None` after the last page.
    pub async fn next_page(
        &mut self,
        client: &Client,
        session: &Session,
        cancel: &CancellationToken,
    ) -> Result<Option<HistoryPage<ChatSummary>>, Error> {
        let previous_reader = self.0.clone();
        let Some(page) = self.0.next::<WireSummary>(client, session, cancel).await? else {
            return Ok(None);
        };
        let data: Vec<ChatSummary> = page
            .data
            .into_iter()
            .map(|s| s.convert(&self.0.context))
            .collect::<Result<_, _>>()?;
        // Refuse a server that ignored the query, including an older deployment.
        // Filtering here would turn an unsupported server into a partial search.
        let query = self.0.query.to_lowercase();
        if !query.is_empty()
            && data
                .iter()
                .any(|row| !row.title.to_lowercase().contains(&query))
        {
            self.0 = previous_reader;
            return Err(Error::InvalidResponse);
        }
        if page.sessions.len() > crate::sessions::MAX_LISTED {
            self.0 = previous_reader;
            return Err(Error::InvalidResponse);
        }
        // A row this client can't read is left out rather than failing the list.
        let sessions: Vec<crate::sessions::SessionSummary> = page
            .sessions
            .iter()
            .filter_map(|row| {
                crate::sessions::rows(&Value::Array(vec![row.clone()]))
                    .ok()?
                    .pop()
            })
            .collect();
        if !query.is_empty()
            && sessions
                .iter()
                .any(|row| !row.title.to_lowercase().contains(&query))
        {
            self.0 = previous_reader;
            return Err(Error::InvalidResponse);
        }
        Ok(Some(HistoryPage {
            data,
            has_more: page.next.is_some(),
            sessions,
        }))
    }
}
/// Reads the account's projects a page at a time.
#[derive(Clone, Debug)]
pub struct ProjectReader(ListReader);
impl ProjectReader {
    /// Sets how many projects each page holds, from 1 to 200.
    pub fn with_page_size(mut self, limit: usize) -> Result<Self, Error> {
        self.0.limit = size(limit)?;
        Ok(self)
    }
    /// Reads the next page of projects. Returns `None` after the last page.
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
            sessions: Vec::new(),
        }))
    }
}

/// Reads a chat's messages a page at a time.
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
    /// Sets how many messages each page holds, from 1 to 200.
    pub fn with_page_size(mut self, limit: usize) -> Result<Self, Error> {
        self.limit = size(limit)?;
        Ok(self)
    }
    /// Start at an indexed message in a known revision, for loading recent history first.
    pub fn starting_at(mut self, index: usize, revision: u64) -> Result<Self, Error> {
        if revision == 0 {
            return Err(Error::InvalidCursor);
        }
        self.at = index;
        self.revision = Some(revision);
        self.done = false;
        self.failed = None;
        Ok(self)
    }
    /// The chat being read.
    pub fn chat(&self) -> &ChatRef {
        &self.chat
    }
    /// Reads the next page of messages. Returns `None` after the last page.
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
            if m.cards.len() > 16 {
                return Err(Error::InvalidResponse);
            }
            let labs = client.labs(session).map_err(|_| Error::InvalidResponse)?;
            let cards = m
                .cards
                .into_iter()
                .map(|card| card.convert(&labs))
                .collect::<Result<Vec<_>, _>>()?;
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
                cards,
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
    /// Starts reading the account's chat list.
    pub fn history(&self, session: &Session) -> Result<ChatReader, Error> {
        self.check_session(session)?;
        Ok(ChatReader(ListReader::new(session, None, false)))
    }
    /// Starts reading the account's projects.
    pub fn history_projects(&self, session: &Session) -> Result<ProjectReader, Error> {
        self.check_session(session)?;
        Ok(ProjectReader(ListReader::new(session, None, true)))
    }
    /// Starts reading the chats in a shared project. Refuses a project that isn't shared.
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
    /// Starts reading a chat's messages from the first one.
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
    /// Adds rows to the index, keeping the newest revision of each chat. Refuses rows
    /// from another account or deployment.
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
    /// The number of chats in the index.
    pub fn len(&self) -> usize {
        self.rows.len()
    }
    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    /// Removes every chat from the index.
    pub fn clear(&mut self) {
        self.rows.clear();
    }
    /// The chat with this ID, if the index holds it.
    pub fn get(&self, id: &str) -> Option<&ChatSummary> {
        self.rows.get(id)
    }
    /// The archived or unarchived chats, pinned first, then the most recently updated.
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
    /// Creates an empty history index for the signed-in account.
    pub fn history_index(&self, session: &Session) -> Result<HistoryIndex, Error> {
        self.check_session(session)?;
        Ok(HistoryIndex {
            context: Context::of(session),
            rows: BTreeMap::new(),
        })
    }
}

/// Confirmed saved-pin order in one account and deployment.
#[derive(Clone, Debug)]
pub struct SavedPinOrder {
    context: Context,
    revision: u64,
    ids: Vec<String>,
}
impl SavedPinOrder {
    /// The order's revision; a write names it to replace this order.
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// The pinned conversation ids, first to last.
    pub fn ids(&self) -> &[String] {
        &self.ids
    }
}
#[derive(Deserialize)]
struct WirePinOrder {
    revision: u64,
    ids: Vec<String>,
}
impl WirePinOrder {
    fn convert(self, session: &Session) -> Result<SavedPinOrder, Error> {
        let mut seen = std::collections::HashSet::new();
        if self.ids.len() > 4096 || self.ids.iter().any(|id| !chat_id(id) || !seen.insert(id)) {
            return Err(Error::InvalidResponse);
        }
        Ok(SavedPinOrder {
            context: Context::of(session),
            revision: self.revision,
            ids: self.ids,
        })
    }
}
impl Client {
    /// Read an owner-scoped order. No chat is pinned or created by this call.
    pub async fn saved_pin_order(
        &self,
        session: &Session,
        cancel: &CancellationToken,
    ) -> Result<SavedPinOrder, Error> {
        self.check_session(session)?;
        let wire: WirePinOrder = cancellable(
            cancel,
            self.history_json(&["history", "pins"], &[], session.token()),
        )
        .await?;
        wire.convert(session)
    }
    /// Move an owned saved pin into an exact gap using one revisioned server CAS.
    pub async fn move_saved_pin(
        &self,
        session: &Session,
        order: &SavedPinOrder,
        chat: &ChatRef,
        after: Option<&ChatRef>,
        before: Option<&ChatRef>,
        cancel: &CancellationToken,
    ) -> Result<SavedPinOrder, Error> {
        order.context.check(self, session)?;
        for chat in std::iter::once(chat).chain(after).chain(before) {
            chat.context.check(self, session)?;
            if !order.ids.iter().any(|id| id == chat.id()) {
                return Err(Error::InvalidReference);
            }
        }
        if after.is_some_and(|a| a.id() == chat.id()) || before.is_some_and(|b| b.id() == chat.id())
        {
            return Err(Error::InvalidReference);
        }
        let wire: WirePinOrder = cancellable(cancel, self.request(
            reqwest::Method::PATCH, &["history", "pins"], &[],
            Some(serde_json::json!({"revision": order.revision, "id": chat.id(), "after": after.map(ChatRef::id), "before": before.map(ChatRef::id)})),
            Some(session.token()),
        )).await?;
        let next = wire.convert(session)?;
        if order.revision.checked_add(1) != Some(next.revision) {
            return Err(Error::InvalidResponse);
        }
        let at = next
            .ids
            .iter()
            .position(|id| id == chat.id())
            .ok_or(Error::InvalidResponse)?;
        if at.checked_sub(1).map(|at| next.ids[at].as_str()) != after.map(ChatRef::id)
            || next.ids.get(at + 1).map(String::as_str) != before.map(ChatRef::id)
        {
            return Err(Error::InvalidResponse);
        }
        Ok(next)
    }
}
