//! One chat read from a computer's history observer, drawn as a Rust Native
//! transcript: messages by role with Markdown, tool rows, and a working row.
//!
//! The chat opens at its newest records, read backward from the end, and
//! shows each page as it arrives; "Load earlier" reads the batch before the
//! oldest row. [`Conversation::poll`] reads backward from the end again
//! until it reaches the newest record it has read, so a running chat grows
//! without splitting a record.
//!
//! A chat can start from a copy the phone kept ([`Cached`]), shown at once
//! while the computer is read again, and can move to a newer source, as a
//! Coder task's next turn; the rows it shows stay until the new source's
//! first read replaces them, and a read for an older source is dropped.

use coder_connect::{Client, Observation, Query};
use coder_history::{Chat, RecordChunk, TranscriptRequest};
use rust_native::markdown;
use rust_native::style::{Color, Style};
use rust_native::{Earlier, Element, MessageRole, Node, TextRole, ToolState};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::runtime::Handle;

use base64::Engine;

const OBSERVE_LIMIT: Duration = Duration::from_secs(20);
/// Raw bytes per backward page. A full 32 KiB page, base64-encoded with its
/// readable projections, can exceed what one sealed observer reply holds.
const PAGE_BYTES: u32 = 16 * 1024;
/// Backward pages read for one batch.
const BATCH_PAGES: usize = 12;
/// Backward pages one poll reads to reach what it has. A Coder turn writes
/// large bookkeeping records, each a page of its own.
const NEWER_PAGES: usize = 48;
/// Conversational messages a batch looks for.
const BATCH_MESSAGES: usize = 10;
/// Rows kept for one chat; older rows are not read past this.
const MAX_ROWS: usize = 240;
const MESSAGE_BYTES: usize = 6_000;
const TOOL_BYTES: usize = 1_500;
/// A tool row's output in a chat too large for one view.
const COMPACT_TOOL_BYTES: usize = 300;
const DETAIL_CHARS: usize = 100;

const WHITE: Color = Color::rgb(255, 255, 255);
const GRAY: Color = Color::rgb(153, 153, 153);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Entry {
    Message {
        role: MessageRole,
        text: String,
    },
    Tool {
        name: String,
        detail: String,
        body: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The record's offset: with `part`, a stable row key.
    pub offset: u64,
    /// Where the record ends.
    pub end: u64,
    pub part: u8,
    pub entry: Entry,
    /// A message's Markdown, parsed once when the row is made.
    pub blocks: Vec<markdown::Block>,
}

impl Row {
    fn new(offset: u64, end: u64, part: u8, entry: Entry) -> Self {
        let blocks = match &entry {
            Entry::Message { role, text } if *role != MessageRole::System => markdown::parse(text),
            _ => vec![],
        };
        Row {
            offset,
            end,
            part,
            entry,
            blocks,
        }
    }
}

/// A chat as the phone last showed it, kept so it opens at once.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cached {
    pub chat: Chat,
    pub rows: Vec<CachedRow>,
    /// Where earlier records end, when there are any.
    pub previous: Option<u64>,
    /// The end of the newest whole record read.
    pub through: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedRow {
    pub offset: u64,
    pub end: u64,
    pub part: u8,
    pub entry: Entry,
}

/// A message this device sent that the transcript does not show yet.
pub struct Pending<'a> {
    pub key: String,
    pub text: &'a str,
    pub note: Option<&'a str>,
}

#[derive(Default)]
struct Inner {
    rows: Vec<Row>,
    /// Where earlier records end, when there are any.
    previous: Option<u64>,
    /// The end of the newest whole record read, shown or not, so a poll
    /// does not read bookkeeping records again.
    through: u64,
    /// A first read of the current source is running.
    loading: bool,
    earlier: bool,
    polling: bool,
    error: Option<String>,
    /// The current source's newest batch has not been read: the rows shown,
    /// if any, came from the phone's copy or an earlier source.
    stale: bool,
    /// Changes with the source; a read for an older source is dropped.
    generation: u64,
    /// Reads started, and the newest one that finished without an error.
    started: u64,
    finished: u64,
    /// Changes whenever the rows do, for the phone's copy.
    version: u64,
    /// How much of a tool row's output shows: 0 all, 1 a little, 2 none,
    /// for a chat too large for one view.
    compact: u8,
    /// The first read that is showing its pages as they arrive: one that
    /// started with nothing to show.
    partial: Option<u64>,
}

pub struct Conversation {
    pub chat: Chat,
    source: String,
    client: Arc<Client>,
    runtime: Handle,
    inner: Arc<Mutex<Inner>>,
}

fn lock(inner: &Mutex<Inner>) -> MutexGuard<'_, Inner> {
    inner.lock().unwrap_or_else(|poison| poison.into_inner())
}

impl Conversation {
    /// Open `chat` and read its newest batch in the background.
    pub fn open(runtime: Handle, client: Arc<Client>, chat: Chat) -> Self {
        Self::resume(runtime, client, chat, None)
    }

    /// Open `chat` showing the phone's copy of it, then read what changed:
    /// the records after the copy's newest when it is of the same source,
    /// else the source's newest batch, which replaces the copy.
    pub fn resume(
        runtime: Handle,
        client: Arc<Client>,
        chat: Chat,
        cached: Option<Cached>,
    ) -> Self {
        let source = chat.source_id.clone().unwrap_or_default();
        let mut inner = Inner {
            stale: true,
            ..Inner::default()
        };
        if let Some(cached) = cached {
            inner.rows = cached
                .rows
                .into_iter()
                .map(|row| Row::new(row.offset, row.end, row.part, row.entry))
                .collect();
            if cached.chat.source_id.as_deref() == Some(source.as_str()) {
                inner.previous = cached.previous;
                inner.through = cached.through;
                inner.stale = false;
            }
        }
        let conversation = Self {
            chat,
            source,
            client,
            runtime,
            inner: Arc::new(Mutex::new(inner)),
        };
        conversation.poll();
        conversation
    }

    /// Move to `chat`, a newer source of the same conversation, such as a
    /// Coder task's next turn. The rows shown stay until its newest batch
    /// replaces them.
    pub fn switch(&mut self, chat: Chat) {
        self.source = chat.source_id.clone().unwrap_or_default();
        self.chat = chat;
        {
            let mut inner = lock(&self.inner);
            inner.generation += 1;
            inner.stale = true;
            inner.loading = false;
            inner.polling = false;
            inner.earlier = false;
            inner.previous = None;
            inner.through = 0;
        }
        self.poll();
    }

    pub fn loading(&self) -> bool {
        let inner = lock(&self.inner);
        inner.loading || inner.earlier
    }

    /// Read the batch before the oldest row.
    pub fn earlier(&self) {
        let previous = {
            let mut inner = lock(&self.inner);
            if inner.loading || inner.earlier || inner.stale || inner.rows.len() >= MAX_ROWS {
                return;
            }
            let Some(previous) = inner.previous else {
                return;
            };
            inner.earlier = true;
            previous
        };
        self.read(previous, Read::Earlier);
    }

    /// Read records added since the newest one read, or, while the source's
    /// newest batch has not been read, as after a failed first read or a
    /// move to a newer source, that batch. Returns whether a read started.
    pub fn poll(&self) -> bool {
        let first = {
            let mut inner = lock(&self.inner);
            if inner.loading || inner.polling {
                return false;
            }
            if inner.stale {
                inner.loading = true;
            } else {
                inner.polling = true;
            }
            inner.stale
        };
        if first {
            self.read(coder_history::NEWEST, Read::First);
        } else {
            self.read(coder_history::NEWEST, Read::Newer);
        }
        true
    }

    /// The source's newest batch was not read, as after a failed first
    /// read: the chat needs another read even when its task has ended.
    pub fn failed(&self) -> bool {
        let inner = lock(&self.inner);
        inner.stale && !inner.loading
    }

    /// How many reads have started. A later [`Conversation::read_since`]
    /// with this number says whether a read that started after now has
    /// finished.
    pub fn reads(&self) -> u64 {
        lock(&self.inner).started
    }

    /// Whether a read that started after [`Conversation::reads`] returned
    /// `ticket` finished without an error, with the source's newest batch
    /// read.
    pub fn read_since(&self, ticket: u64) -> bool {
        let inner = lock(&self.inner);
        inner.finished > ticket && !inner.stale
    }

    /// Changes whenever the rows do.
    pub fn version(&self) -> u64 {
        lock(&self.inner).version
    }

    /// The chat as shown, for the phone's copy, once its source was read.
    pub fn cached(&self) -> Option<Cached> {
        let inner = lock(&self.inner);
        if inner.stale || inner.rows.is_empty() {
            return None;
        }
        Some(Cached {
            chat: self.chat.clone(),
            rows: inner
                .rows
                .iter()
                .map(|row| CachedRow {
                    offset: row.offset,
                    end: row.end,
                    part: row.part,
                    entry: row.entry.clone(),
                })
                .collect(),
            previous: inner.previous,
            through: inner.through,
        })
    }

    /// How many of the user's messages with exactly `text` show.
    pub fn sent(&self, text: &str) -> usize {
        let text = bounded(text.trim(), MESSAGE_BYTES);
        lock(&self.inner)
            .rows
            .iter()
            .filter(|row| {
                matches!(&row.entry, Entry::Message { role: MessageRole::User, text: shown } if *shown == text)
            })
            .count()
    }

    fn read(&self, end: u64, kind: Read) {
        let (client, source, inner) =
            (self.client.clone(), self.source.clone(), self.inner.clone());
        let (generation, ticket, through) = {
            let mut state = lock(&inner);
            state.started += 1;
            if matches!(kind, Read::First) && state.rows.is_empty() {
                state.partial = Some(state.started);
            }
            (state.generation, state.started, state.through)
        };
        self.runtime.spawn(async move {
            let result = match kind {
                Read::Newer => newer(&client, &source, through).await,
                Read::First => {
                    // Show each page as it arrives: the newest messages
                    // first, then the rest of the batch.
                    let shown = inner.clone();
                    let show = move |rows: &[Row], previous: Option<u64>, through: u64| {
                        let mut state = lock(&shown);
                        if state.generation == generation
                            && state.partial == Some(ticket)
                            && !rows.is_empty()
                        {
                            state.rows = rows.to_vec();
                            state.previous = previous;
                            state.through = state.through.max(through);
                            state.version += 1;
                        }
                    };
                    batch(&client, &source, end, &show).await
                }
                Read::Earlier => batch(&client, &source, end, &|_, _, _| {}).await,
            };
            finish(&mut lock(&inner), kind, generation, ticket, result);
        });
    }

    /// The chat as a transcript node. `earlier` is the intent that loads
    /// older rows; `pending` are messages this device sent that do not show
    /// yet; `working` adds a working row, such as "Coder is working".
    pub fn transcript<I: Clone>(
        &self,
        key: &str,
        earlier: I,
        pending: &[Pending<'_>],
        working: Option<&str>,
    ) -> Node<I> {
        transcript(&lock(&self.inner), key, earlier, pending, working)
    }

    /// A transcript with no rows read yet: `pending` messages this device
    /// sent, and a `working` row.
    pub fn pending_transcript<I>(
        key: &str,
        pending: &[Pending<'_>],
        working: Option<&str>,
    ) -> Node<I> {
        let mut children: Vec<Node<I>> = pending.iter().map(sent).collect();
        if let Some(label) = working {
            children.push(node(
                &format!("{key}-working"),
                Element::Working {
                    label: label.into(),
                },
            ));
        }
        node(
            key,
            Element::Transcript {
                label: "Messages".into(),
                children,
                earlier: None,
            },
        )
    }

    /// Make the chat fit one view: first show less of each tool row's
    /// output, then drop the oldest rows, which "Load earlier" reads again.
    /// Returns whether anything changed.
    pub fn shrink(&self) -> bool {
        shrink(&mut lock(&self.inner))
    }
}

/// Show less of each tool row's output, then drop the oldest half of the
/// rows, keeping them loadable. Returns whether anything changed.
fn shrink(inner: &mut Inner) -> bool {
    if inner.compact < 2 {
        inner.compact += 1;
        return true;
    }
    let half = inner.rows.len() / 2;
    if half == 0 {
        return false;
    }
    inner.rows.drain(..half);
    inner.previous = inner.rows.first().map(|row| row.offset);
    inner.version += 1;
    true
}

/// What a finished read changes. A read for an older source changes
/// nothing.
fn finish(
    state: &mut Inner,
    kind: Read,
    generation: u64,
    ticket: u64,
    result: Result<Found, String>,
) {
    if state.generation != generation {
        return;
    }
    match kind {
        Read::First => state.loading = false,
        Read::Earlier => state.earlier = false,
        Read::Newer => state.polling = false,
    }
    match (result, kind) {
        (Ok(read), Read::First) => {
            state.rows = read.rows;
            state.previous = read.previous;
            state.through = read.through;
            state.error = None;
            state.stale = false;
            state.partial = None;
            state.version += 1;
            state.finished = state.finished.max(ticket);
        }
        (Ok(mut read), Read::Earlier) => {
            read.rows.append(&mut state.rows);
            state.rows = read.rows;
            state.previous = if state.rows.len() >= MAX_ROWS {
                None
            } else {
                read.previous
            };
            state.version += 1;
        }
        (Ok(read), Read::Newer) => {
            state.error = None;
            let known = state.rows.last().map_or(0, |row| row.end);
            let before = state.rows.len();
            state
                .rows
                .extend(read.rows.into_iter().filter(|row| row.offset >= known));
            let excess = state.rows.len().saturating_sub(MAX_ROWS);
            state.rows.drain(..excess);
            if excess > 0 {
                state.previous = state.rows.first().map(|row| row.offset);
            }
            if state.rows.len() != before || excess > 0 {
                state.version += 1;
            }
            state.through = state.through.max(read.through);
            state.finished = state.finished.max(ticket);
        }
        // A missed poll is retried by the next one; it is not the reader's
        // problem.
        (Err(_), Read::Newer) => {}
        // A failed first read keeps what shows and says why only when
        // nothing does; the next poll reads again.
        (Err(error), _) => {
            state.partial = None;
            state.error = Some(error);
        }
    }
}

/// The chat in `inner` as a transcript node.
fn transcript<I: Clone>(
    inner: &Inner,
    key: &str,
    earlier: I,
    pending: &[Pending<'_>],
    working: Option<&str>,
) -> Node<I> {
    let mut children: Vec<Node<I>> = vec![];
    if let Some(error) = inner.error.as_ref().filter(|_| inner.rows.is_empty()) {
        children.push(system(&format!("{key}-error"), error));
    }
    children.extend(inner.rows.iter().map(|row| draw(row, inner.compact)));
    children.extend(pending.iter().map(sent));
    // One working row at most: the task's own state when it has one,
    // since it says more than the read that is still loading.
    if let Some(label) = working {
        children.push(node(
            &format!("{key}-working"),
            Element::Working {
                label: label.into(),
            },
        ));
    } else if inner.loading && inner.rows.is_empty() {
        children.push(node(
            &format!("{key}-loading"),
            Element::Working {
                label: "Loading the chat".into(),
            },
        ));
    } else if inner.rows.is_empty() && inner.error.is_none() && pending.is_empty() {
        children.push(system(&format!("{key}-empty"), "No messages yet."));
    }
    node(
        key,
        Element::Transcript {
            label: "Messages".into(),
            children,
            earlier: inner
                .previous
                .filter(|_| !inner.stale && inner.rows.len() < MAX_ROWS)
                .map(|_| Earlier {
                    label: "Load earlier messages".into(),
                    loading: inner.earlier,
                    intent: earlier,
                }),
        },
    )
}

#[derive(Clone, Copy)]
enum Read {
    First,
    Earlier,
    Newer,
}

/// What a read found: rows oldest first, where earlier records end, and
/// the end of the newest whole record it read.
struct Found {
    rows: Vec<Row>,
    previous: Option<u64>,
    through: u64,
}

async fn observe(client: &Client, query: Query) -> Result<Observation, String> {
    tokio::time::timeout(OBSERVE_LIMIT, client.observe(query))
        .await
        .map_err(|_| "The computer did not answer.".to_string())?
        .map_err(|error| error.to_string())
}

async fn back(
    client: &Client,
    source: &str,
    end: u64,
) -> Result<coder_history::TranscriptPage, String> {
    let request = TranscriptRequest {
        source_id: source.to_owned(),
        cursor: None,
        max_bytes: PAGE_BYTES,
        end: Some(end),
    };
    match observe(client, Query::Page(request)).await? {
        Observation::Page(page) => Ok(page),
        Observation::Catalog(_) => Err("The computer answered with the wrong page.".into()),
    }
}

/// The end of the newest whole record on a page, if it holds one.
fn page_through(page: &coder_history::TranscriptPage) -> u64 {
    page.chunks
        .iter()
        .filter(|chunk| chunk.complete)
        .map(|chunk| chunk.end_offset)
        .max()
        .unwrap_or(0)
}

/// Sees a batch's rows, where earlier records end, and the newest record's
/// end after each page.
type Show = dyn Fn(&[Row], Option<u64>, u64) + Send + Sync;

/// Up to a batch of messages ending at `end`, oldest first, and where
/// earlier records end. `show` sees the rows found after each page.
async fn batch(client: &Client, source: &str, mut end: u64, show: &Show) -> Result<Found, String> {
    let mut found = Found {
        rows: vec![],
        previous: None,
        through: 0,
    };
    for _ in 0..BATCH_PAGES {
        let page = back(client, source, end).await?;
        let mut page_rows = rows(&page.chunks);
        page_rows.append(&mut found.rows);
        found.rows = page_rows;
        found.previous = page.previous;
        found.through = found.through.max(page_through(&page));
        show(&found.rows, found.previous, found.through);
        let conversational = found
            .rows
            .iter()
            .filter(|row| matches!(row.entry, Entry::Message { .. }))
            .count();
        match page.previous {
            Some(earlier) if conversational < BATCH_MESSAGES => end = earlier,
            _ => break,
        }
    }
    Ok(found)
}

/// Records that start at or after `known`, the end of the newest record
/// already read, oldest first.
async fn newer(client: &Client, source: &str, known: u64) -> Result<Found, String> {
    let mut found = Found {
        rows: vec![],
        previous: None,
        through: known,
    };
    let mut end = coder_history::NEWEST;
    for _ in 0..NEWER_PAGES {
        let page = back(client, source, end).await?;
        found.through = found.through.max(page_through(&page));
        let start = page
            .chunks
            .first()
            .map_or(page.next.offset, |chunk| chunk.offset);
        let mut page_rows: Vec<Row> = rows(&page.chunks)
            .into_iter()
            .filter(|row| row.offset >= known)
            .collect();
        page_rows.append(&mut found.rows);
        found.rows = page_rows;
        match page.previous {
            Some(earlier) if start > known => end = earlier,
            _ => break,
        }
    }
    Ok(found)
}

fn bounded(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

fn first_line(text: &str) -> String {
    let line = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    let mut out: String = line.chars().take(DETAIL_CHARS).collect();
    if line.chars().count() > DETAIL_CHARS {
        out.push('…');
    }
    out
}

/// Rows from whole records on a page. A record split across pages uses its
/// readable preview.
pub fn rows(chunks: &[RecordChunk]) -> Vec<Row> {
    let mut out = vec![];
    let mut index = 0;
    while index < chunks.len() {
        let start = index;
        let record = chunks[start].record_offset;
        while index < chunks.len() && chunks[index].record_offset == record {
            index += 1;
        }
        let group = &chunks[start..index];
        let end = group.last().map_or(record, |c| c.end_offset);
        let whole = group[0].offset == record && group.last().is_some_and(|c| c.complete);
        let full = whole
            .then(|| {
                let mut bytes = vec![];
                for chunk in group {
                    bytes.extend(
                        base64::engine::general_purpose::STANDARD
                            .decode(&chunk.raw_base64)
                            .ok()?,
                    );
                }
                coder_history::readable_record_full(&bytes)
            })
            .flatten();
        let Some(readable) = full.or_else(|| group.iter().rev().find_map(|c| c.readable.clone()))
        else {
            continue;
        };
        for (part, entry) in entries(&readable).into_iter().enumerate() {
            out.push(Row::new(record, end, part as u8, entry));
        }
    }
    out
}

/// What one record shows: messages by role, tool calls and results, or
/// nothing for bookkeeping records.
fn entries(readable: &coder_history::Readable) -> Vec<Entry> {
    let kind = readable.kind.as_str();
    let text = readable.text.trim();
    if readable.unknown
        || text.is_empty()
        || matches!(
            kind,
            "reasoning"
                | "session"
                | "session_meta"
                | "turn_context"
                | "token_count"
                | "task_started"
                | "task_complete"
                | "summary"
                | "compacted"
        )
    {
        return vec![];
    }
    let tool_kind = matches!(
        kind,
        "function_call"
            | "function_call_output"
            | "custom_tool_call"
            | "custom_tool_call_output"
            | "tool_use"
            | "tool_result"
            | "tool_call"
    );
    let role = readable.role.as_deref();
    // A tool result: a record that answers a call, or a tool role.
    if (role == Some("user") && readable.call_id.is_some() && readable.tool_name.is_none())
        || role == Some("tool")
        || kind.ends_with("_output")
        || kind == "tool_result"
    {
        return vec![Entry::Tool {
            name: "Result".into(),
            detail: first_line(text),
            body: bounded(text, TOOL_BYTES),
        }];
    }
    // A reply that calls a tool: its prose, then the call.
    if let Some(at) = text.find("Tool: ").filter(|_| role == Some("assistant")) {
        let prose = text[..at].trim();
        let call = &text[at + "Tool: ".len()..];
        let (name, arguments) = call.split_once('\n').unwrap_or((call, ""));
        let mut out = vec![];
        if !prose.is_empty() {
            out.push(Entry::Message {
                role: MessageRole::Assistant,
                text: bounded(prose, MESSAGE_BYTES),
            });
        }
        out.push(Entry::Tool {
            name: name.trim().to_owned(),
            detail: first_line(arguments),
            body: bounded(arguments.trim(), TOOL_BYTES),
        });
        return out;
    }
    if tool_kind || readable.tool_name.is_some() {
        return vec![Entry::Tool {
            name: readable.tool_name.clone().unwrap_or_else(|| "Tool".into()),
            detail: first_line(text),
            body: bounded(text, TOOL_BYTES),
        }];
    }
    match role {
        // Injected context, such as `<environment_context>`, is not the
        // person's words.
        Some("user") if text.starts_with('<') => vec![],
        Some("user") => vec![Entry::Message {
            role: MessageRole::User,
            text: bounded(text, MESSAGE_BYTES),
        }],
        Some("assistant") => vec![Entry::Message {
            role: MessageRole::Assistant,
            text: bounded(text, MESSAGE_BYTES),
        }],
        Some("system") | None if kind == "turn_aborted" => vec![Entry::Message {
            role: MessageRole::System,
            text: "The turn was stopped.".into(),
        }],
        Some("system") => vec![Entry::Message {
            role: MessageRole::System,
            text: bounded(text, MESSAGE_BYTES),
        }],
        _ => vec![],
    }
}

fn node<I>(key: &str, element: Element<I>) -> Node<I> {
    Node {
        key: key.into(),
        style: Style::default(),
        element,
    }
}

fn system<I>(key: &str, text: &str) -> Node<I> {
    node(
        key,
        Element::Message {
            role: MessageRole::System,
            note: None,
            children: vec![Node {
                key: format!("{key}-text"),
                style: Style {
                    foreground: Some(GRAY),
                    ..Style::default()
                },
                element: Element::Text {
                    value: text.into(),
                    role: TextRole::Status,
                },
            }],
        },
    )
}

/// A message this device sent that the transcript does not show yet.
fn sent<I>(pending: &Pending<'_>) -> Node<I> {
    node(
        &pending.key,
        Element::Message {
            role: MessageRole::User,
            note: pending.note.map(str::to_owned),
            children: vec![Node {
                key: format!("{}-md", pending.key),
                style: Style {
                    foreground: Some(WHITE),
                    ..Style::default()
                },
                element: Element::Markdown {
                    blocks: markdown::parse(pending.text),
                },
            }],
        },
    )
}

fn draw<I>(row: &Row, compact: u8) -> Node<I> {
    let key = format!("r{}-{}", row.offset, row.part);
    match &row.entry {
        Entry::Message {
            role: MessageRole::System,
            text,
        } => system(&key, text),
        Entry::Message { role, .. } => node(
            &key,
            Element::Message {
                role: *role,
                note: None,
                children: vec![Node {
                    key: format!("{key}-md"),
                    style: Style {
                        foreground: Some(WHITE),
                        ..Style::default()
                    },
                    element: Element::Markdown {
                        blocks: row.blocks.clone(),
                    },
                }],
            },
        ),
        Entry::Tool { name, detail, body } => node(
            &key,
            Element::Tool {
                name: name.clone(),
                detail: detail.clone(),
                state: ToolState::Done,
                children: match compact {
                    0 => Some(body.clone()),
                    1 => Some(bounded(body, COMPACT_TOOL_BYTES)),
                    _ => None,
                }
                .map(|body| Node {
                    key: format!("{key}-body"),
                    style: Style {
                        foreground: Some(GRAY),
                        ..Style::default()
                    },
                    element: Element::Text {
                        value: body,
                        role: TextRole::Code,
                    },
                })
                .into_iter()
                .collect(),
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(offset: u64, role: MessageRole, text: &str) -> Row {
        Row::new(
            offset,
            offset + 10,
            0,
            Entry::Message {
                role,
                text: text.into(),
            },
        )
    }

    fn found(rows: Vec<Row>, previous: Option<u64>) -> Found {
        let through = rows.last().map_or(0, |row| row.end);
        Found {
            rows,
            previous,
            through,
        }
    }

    fn kinds(node: &Node<()>) -> Vec<String> {
        let Element::Transcript { children, .. } = &node.element else {
            panic!("not a transcript");
        };
        children
            .iter()
            .map(|child| match &child.element {
                Element::Working { label } => format!("working:{label}"),
                Element::Message { role, note, .. } => {
                    format!("{role:?}:{}", note.as_deref().unwrap_or(""))
                }
                Element::Tool { children, .. } => format!("tool:{}", children.len()),
                _ => "other".into(),
            })
            .collect()
    }

    #[test]
    fn one_working_row_shows_and_the_task_state_stands_for_loading() {
        let loading = Inner {
            loading: true,
            stale: true,
            ..Inner::default()
        };
        let queued = transcript(&loading, "t", (), &[], Some("Queued"));
        assert_eq!(kinds(&queued), ["working:Queued"]);
        let unknown = transcript(&loading, "t", (), &[], None);
        assert_eq!(kinds(&unknown), ["working:Loading the chat"]);
        // Rows already show while a newer source loads: no loading row.
        let shown = Inner {
            rows: vec![message(0, MessageRole::User, "Hi")],
            ..loading
        };
        assert_eq!(kinds(&transcript(&shown, "t", (), &[], None)), ["User:"]);
    }

    #[test]
    fn a_sent_message_shows_before_the_working_row_with_its_state() {
        let inner = Inner {
            rows: vec![message(0, MessageRole::User, "Hi")],
            ..Inner::default()
        };
        let pending = [Pending {
            key: "sent-1".into(),
            text: "And the docs.",
            note: Some("Queued"),
        }];
        let node = transcript(&inner, "t", (), &pending, Some("Coder is working"));
        assert_eq!(
            kinds(&node),
            ["User:", "User:Queued", "working:Coder is working"]
        );
        let empty = Conversation::pending_transcript::<()>("t", &pending, Some("Queued"));
        assert_eq!(kinds(&empty), ["User:Queued", "working:Queued"]);
    }

    #[test]
    fn a_read_for_an_older_source_changes_nothing() {
        let mut inner = Inner {
            rows: vec![message(0, MessageRole::User, "Turn one")],
            generation: 2,
            stale: true,
            loading: true,
            ..Inner::default()
        };
        let old = found(vec![message(0, MessageRole::Assistant, "Old")], None);
        finish(&mut inner, Read::First, 1, 1, Ok(old));
        assert!(inner.loading && inner.stale);
        assert_eq!(inner.rows.len(), 1);
    }

    #[test]
    fn a_new_turn_replaces_the_rows_once_read_and_earlier_stays_loadable() {
        // The phone's copy of turn one shows while turn two is read.
        let mut inner = Inner {
            rows: vec![message(0, MessageRole::User, "Turn one")],
            stale: true,
            loading: true,
            generation: 3,
            started: 5,
            ..Inner::default()
        };
        assert!(inner.finished <= 4);
        let turn = found(
            vec![
                message(500, MessageRole::User, "Turn one"),
                message(510, MessageRole::Assistant, "Asked."),
                message(520, MessageRole::User, "Pear."),
            ],
            Some(500),
        );
        finish(&mut inner, Read::First, 3, 5, Ok(turn));
        assert!(!inner.stale && !inner.loading);
        assert_eq!(inner.finished, 5);
        assert_eq!(inner.through, 530);
        let node = transcript(&inner, "t", (), &[], None);
        let Element::Transcript { earlier, .. } = &node.element else {
            unreachable!()
        };
        assert!(earlier.is_some());
        // A poll adds only records after the newest one read.
        let polled = found(
            vec![
                message(520, MessageRole::User, "Pear."),
                message(530, MessageRole::Assistant, "pear"),
            ],
            None,
        );
        inner.polling = true;
        let version = inner.version;
        finish(&mut inner, Read::Newer, 3, 6, Ok(polled));
        assert_eq!(inner.rows.len(), 4);
        assert!(inner.version > version);
        assert_eq!(inner.finished, 6);
        // A failed poll changes nothing and is not a finished read.
        inner.polling = true;
        finish(&mut inner, Read::Newer, 3, 7, Err("late".into()));
        assert_eq!((inner.rows.len(), inner.finished), (4, 6));
    }

    #[test]
    fn a_failed_first_read_keeps_what_shows() {
        let mut inner = Inner {
            rows: vec![message(0, MessageRole::User, "Kept")],
            stale: true,
            loading: true,
            ..Inner::default()
        };
        finish(&mut inner, Read::First, 0, 1, Err("late".into()));
        assert!(inner.stale && !inner.loading);
        assert_eq!(kinds(&transcript(&inner, "t", (), &[], None)), ["User:"]);
    }

    #[test]
    fn too_large_a_chat_shows_less_tool_output_before_it_drops_rows() {
        let tool = |offset| {
            Row::new(
                offset,
                offset + 10,
                0,
                Entry::Tool {
                    name: "shell".into(),
                    detail: "ls".into(),
                    body: "x".repeat(1_000),
                },
            )
        };
        let mut inner = Inner {
            rows: vec![tool(0), tool(10), tool(20), tool(30)],
            ..Inner::default()
        };
        let body = |inner: &Inner| match &transcript(inner, "t", (), &[], None).element {
            Element::Transcript { children, .. } => match &children[0].element {
                Element::Tool { children, .. } => {
                    children.first().map(|child| match &child.element {
                        Element::Text { value, .. } => value.len(),
                        _ => 0,
                    })
                }
                _ => None,
            },
            _ => None,
        };
        assert_eq!(body(&inner), Some(1_000));
        assert!(shrink(&mut inner));
        assert!(body(&inner).is_some_and(|len| len < 400));
        assert!(shrink(&mut inner));
        assert_eq!(body(&inner), None);
        assert_eq!(inner.rows.len(), 4);
        // Then the oldest half goes, and stays loadable.
        assert!(shrink(&mut inner));
        assert_eq!(inner.rows.len(), 2);
        assert_eq!(inner.previous, Some(20));
        inner.rows.clear();
        assert!(!shrink(&mut inner));
    }

    fn readable(kind: &str, role: Option<&str>, text: &str) -> coder_history::Readable {
        coder_history::Readable {
            kind: kind.into(),
            native_id: None,
            role: role.map(str::to_owned),
            timestamp: None,
            tool_name: None,
            call_id: None,
            text: text.into(),
            text_truncated: false,
            unknown: false,
        }
    }

    #[test]
    fn records_become_messages_and_tool_rows() {
        assert_eq!(
            entries(&readable("user", Some("user"), "Fix it")),
            vec![Entry::Message {
                role: MessageRole::User,
                text: "Fix it".into()
            }]
        );
        assert!(entries(&readable("user", Some("user"), "<environment_context>x")).is_empty());
        assert!(entries(&readable("reasoning", None, "hmm")).is_empty());
        let split = entries(&readable(
            "assistant",
            Some("assistant"),
            "Checking.\nTool: Bash\n{\"command\":\"ls\"}",
        ));
        assert_eq!(split.len(), 2);
        assert!(matches!(&split[1], Entry::Tool { name, .. } if name == "Bash"));
        let mut result = readable("user", Some("user"), "total 3\nfile");
        result.call_id = Some("call-1".into());
        assert!(
            matches!(&entries(&result)[0], Entry::Tool { name, detail, .. } if name == "Result" && detail == "total 3")
        );
        let mut call = readable("function_call", None, "{\"cmd\":\"ls\"}");
        call.tool_name = Some("shell".into());
        assert!(matches!(&entries(&call)[0], Entry::Tool { name, .. } if name == "shell"));
    }
}
