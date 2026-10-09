//! One chat read from a computer's history observer, drawn as a Rust Native
//! transcript: messages by role with Markdown, tool rows, and a working row.
//!
//! The chat opens at its newest records, read backward from the end: the
//! first page with a row to show ends the opening read, and when it shows
//! less than a screen, the rows before it are read in the background as
//! "Load earlier" reads them. A Coder task's transcript is mostly host
//! records that show no row, so the opening read counts rows, not records.
//! [`Conversation::poll`] reads backward from the end again until it
//! reaches the newest record it has read, so a running chat grows without
//! splitting a record.
//!
//! A chat can start from a copy the phone kept ([`Cached`]), shown at once
//! while the computer is read again. A Coder task's next turn is a newer
//! source that begins with the earlier turns' messages, marked as carried;
//! the chat moves to it by reading it backward only as far as those carried
//! messages and showing its rows after the ones it has, so a follow-up's
//! reply shows after a read or two. Each source is a segment of the chat,
//! and a read for a source the chat moved past is dropped.

use coder_connect::direct::Change;
use coder_connect::protocol::Route;
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
/// Raw bytes per backward page: the route's most (`Limits`), 32 KiB
/// through a relay and 160 KiB direct. A Coder host's relay page seals
/// well inside one relay frame (at most about 77 KB on the chat load
/// benchmark's transcripts, against the relay's 128 KiB); a page that fails
/// is asked for again at [`SMALL_PAGE_BYTES`].
fn page_bytes(route: Route, small: bool) -> u32 {
    match route {
        Route::Relay if small => SMALL_PAGE_BYTES,
        _ => route.limits().page_bytes,
    }
}
/// A relay page after a full-size one failed.
const SMALL_PAGE_BYTES: u32 = 16 * 1024;
/// Backward pages read for one batch.
const BATCH_PAGES: usize = 12;
/// Raw record bytes one batch reads at most.
const BATCH_BYTES: u64 = 256 * 1024;
/// Backward pages one poll, or one read of a new turn, reads to reach what
/// it has.
const NEWER_PAGES: usize = 48;
/// Rows a batch looks for, about a screen: messages and tool rows alike.
const BATCH_ROWS: usize = 12;
/// Rows kept for one chat; older rows are not read past this.
const MAX_ROWS: usize = 240;
/// Sources one chat follows before it starts again from its newest.
const MAX_SEGMENTS: usize = 200;
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
    /// The task's turn delegated to a whole coding agent (`opencode` or
    /// `devin`): the transcript's `delegate_transcript` note, where the
    /// agent's session shows, or why its copy failed.
    Delegate {
        agent: String,
        session: String,
        error: Option<String>,
    },
}

/// The most rows of a delegate session one chat shows.
const DELEGATE_ROWS: usize = 40;
/// A delegate session's row, in a chat too large for one view.
const COMPACT_DELEGATE_ROWS: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The source the row was read from, in the order the chat followed
    /// them; with `offset` and `part`, a stable row key.
    pub segment: u8,
    /// The record's offset.
    pub offset: u64,
    /// Where the record ends.
    pub end: u64,
    pub part: u8,
    /// The record repeats an earlier turn's message at the start of a
    /// later turn's source.
    pub carried: bool,
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
            segment: 0,
            offset,
            end,
            part,
            carried: false,
            entry,
            blocks,
        }
    }
}

/// A chat as the phone last showed it, kept so it opens at once.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cached {
    /// The newest source's chat.
    pub chat: Chat,
    /// Each segment's source ID, oldest first.
    pub sources: Vec<String>,
    pub rows: Vec<CachedRow>,
    /// Where earlier records end: a segment and an offset in its source.
    pub previous: Option<(u8, u64)>,
    /// The end of the newest source's newest whole record read.
    pub through: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedRow {
    pub segment: u8,
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
    /// Each segment's source ID, oldest first; the last is read for new
    /// records.
    sources: Vec<String>,
    rows: Vec<Row>,
    /// Where earlier records end: a segment and an offset in its source.
    previous: Option<(u8, u64)>,
    /// The end of the newest whole record of the newest source read, shown
    /// or not, so a poll does not read bookkeeping records again.
    through: u64,
    /// A read of the newest source's newest records is running.
    loading: bool,
    earlier: bool,
    polling: bool,
    error: Option<String>,
    /// The newest source's newest records have not been read: the rows
    /// shown, if any, came from the phone's copy or earlier sources.
    stale: bool,
    /// Changes with the sources; a read for older ones is dropped.
    generation: u64,
    /// Reads started, and the newest one that finished without an error.
    started: u64,
    finished: u64,
    /// Changes whenever the rows do, for the phone's copy.
    version: u64,
    /// How much of a tool row's output shows: 0 all, 1 a little, 2 none,
    /// for a chat too large for one view.
    compact: u8,
    /// The read showing its pages as they arrive.
    partial: Option<u64>,
    /// The computer said the newest source grew while a read ran: read
    /// again when it finishes.
    again: bool,
    /// The rows of each delegate session the chat's notes name, by
    /// session, as read through its own reader.
    delegated: std::collections::BTreeMap<String, Vec<Row>>,
}

impl Inner {
    fn newest(&self) -> (u8, String) {
        let segment = self.sources.len().saturating_sub(1);
        (
            u8::try_from(segment).unwrap_or(u8::MAX),
            self.sources.last().cloned().unwrap_or_default(),
        )
    }

    /// Show `found`, read from the newest records of `segment`: the first
    /// segment's rows replace every row; a later one's follow the rows of
    /// the segments before it, leaving out the messages it carries.
    fn show(&mut self, segment: u8, found: &[Row], previous: Option<u64>) {
        let found = found.iter().map(|row| Row {
            segment,
            ..row.clone()
        });
        if segment == 0 {
            self.rows = found.collect();
            self.previous = previous.map(|end| (0, end));
        } else {
            self.rows.retain(|row| row.segment != segment);
            self.rows.extend(found.filter(|row| !row.carried));
        }
        self.version += 1;
    }
}

pub struct Conversation {
    pub chat: Chat,
    client: Arc<Client>,
    runtime: Handle,
    inner: Arc<Mutex<Inner>>,
    /// Reads again when the computer says the newest source grew.
    watch: tokio::task::AbortHandle,
}

impl Drop for Conversation {
    fn drop(&mut self) {
        self.watch.abort();
    }
}

fn lock(inner: &Mutex<Inner>) -> MutexGuard<'_, Inner> {
    inner.lock().unwrap_or_else(|poison| poison.into_inner())
}

impl Conversation {
    /// Open `chat` and read its newest batch in the background.
    pub fn open(runtime: Handle, client: Arc<Client>, chat: Chat) -> Self {
        Self::resume(runtime, client, chat, None)
    }

    /// Open `chat` showing the phone's copy of a chat, then read what
    /// changed: the records after the copy's newest when its newest source
    /// is `chat`'s, else `chat`'s newest batch, which replaces the copy.
    pub fn resume(
        runtime: Handle,
        client: Arc<Client>,
        chat: Chat,
        cached: Option<Cached>,
    ) -> Self {
        let source = chat.source_id.clone().unwrap_or_default();
        let mut inner = Inner {
            sources: vec![source.clone()],
            stale: true,
            ..Inner::default()
        };
        if let Some(cached) = cached {
            inner.rows = cached
                .rows
                .into_iter()
                .map(|row| Row {
                    segment: row.segment,
                    ..Row::new(row.offset, row.end, row.part, row.entry)
                })
                .collect();
            if cached.sources.last() == Some(&source) {
                inner.sources = cached.sources;
                inner.previous = cached.previous;
                inner.through = cached.through;
                inner.stale = false;
            } else {
                // The copy is of another source: it shows as the first
                // segment's rows until this source's newest batch replaces
                // them.
                for row in &mut inner.rows {
                    row.segment = 0;
                }
            }
        }
        let inner = Arc::new(Mutex::new(inner));
        let watch = watch(&runtime, &client, &inner);
        let conversation = Self {
            chat,
            client,
            runtime,
            inner,
            watch,
        };
        conversation.poll();
        conversation
    }

    /// Move to `chat`, a newer source of the same conversation, such as a
    /// Coder task's next turn: its records after the messages it carries
    /// show after the rows shown now.
    pub fn switch(&mut self, chat: Chat) {
        let source = chat.source_id.clone().unwrap_or_default();
        self.chat = chat;
        {
            let mut inner = lock(&self.inner);
            inner.generation += 1;
            if inner.sources.len() >= MAX_SEGMENTS {
                inner.sources.clear();
                inner.rows.clear();
                inner.previous = None;
            }
            inner.sources.push(source);
            inner.stale = true;
            inner.loading = false;
            inner.polling = false;
            inner.earlier = false;
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
        earlier(&self.client, &self.runtime, &self.inner);
    }

    /// Read records added since the newest one read, or, while the newest
    /// source's newest records have not been read, as after a failed read
    /// or a move to a newer source, those. Returns whether a read started.
    pub fn poll(&self) -> bool {
        poll(&self.client, &self.runtime, &self.inner, false)
    }

    /// The newest source's newest records were not read, as after a failed
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
    /// `ticket` finished without an error, with the newest source's newest
    /// records read.
    pub fn read_since(&self, ticket: u64) -> bool {
        let inner = lock(&self.inner);
        inner.finished > ticket && !inner.stale
    }

    /// Changes whenever the rows do.
    pub fn version(&self) -> u64 {
        lock(&self.inner).version
    }

    /// The chat as shown, for the phone's copy, once its newest source was
    /// read.
    pub fn cached(&self) -> Option<Cached> {
        let inner = lock(&self.inner);
        if inner.stale || inner.rows.is_empty() {
            return None;
        }
        Some(Cached {
            chat: self.chat.clone(),
            sources: inner.sources.clone(),
            rows: inner
                .rows
                .iter()
                .map(|row| CachedRow {
                    segment: row.segment,
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

    /// The delegate sessions the chat's notes name, as agent and session,
    /// each once.
    pub fn delegates(&self) -> Vec<(String, String)> {
        let mut found: Vec<(String, String)> = vec![];
        for row in &lock(&self.inner).rows {
            if let Entry::Delegate {
                agent,
                session,
                error: None,
            } = &row.entry
                && !found.iter().any(|(_, known)| known == session)
            {
                found.push((agent.clone(), session.clone()));
            }
        }
        found
    }

    /// Show `rows`, a delegate session's, under the note that names
    /// `session`.
    pub fn delegated(&self, session: &str, rows: Vec<Row>) {
        let mut inner = lock(&self.inner);
        if inner.delegated.get(session) != Some(&rows) {
            inner.delegated.insert(session.to_owned(), rows);
        }
    }

    /// The rows read so far.
    pub fn rows(&self) -> Vec<Row> {
        lock(&self.inner).rows.clone()
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
                source: None,
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

/// Start a read of `source` in the background.
fn read(
    client: &Arc<Client>,
    runtime: &Handle,
    inner: &Arc<Mutex<Inner>>,
    (kind, segment, source, end): (Read, u8, String, u64),
) {
    let (client, inner) = (client.clone(), inner.clone());
    let (generation, ticket, through) = {
        let mut state = lock(&inner);
        state.started += 1;
        if matches!(kind, Read::Head) && (segment > 0 || state.rows.is_empty()) {
            state.partial = Some(state.started);
        }
        (state.generation, state.started, state.through)
    };
    runtime.spawn(async move {
        let result = match kind {
            Read::Newer => newer(&client, &source, through).await,
            Read::Head => {
                // Show each page as it arrives: the newest messages
                // first, then the rest.
                let shown = inner.clone();
                let show = move |rows: &[Row], previous: Option<u64>| {
                    let mut state = lock(&shown);
                    if state.generation == generation
                        && state.partial == Some(ticket)
                        && !rows.is_empty()
                    {
                        state.show(segment, rows, previous);
                        drop(state);
                        crate::wake::ring();
                    }
                };
                let until = if segment == 0 {
                    Until::Shown
                } else {
                    Until::Carried
                };
                batch(&client, &source, segment, end, until, &show).await
            }
            Read::Earlier => {
                let until = if segment == 0 {
                    Until::Screen
                } else {
                    Until::Carried
                };
                batch(&client, &source, segment, end, until, &|_, _| {}).await
            }
        };
        let (again, fill, changed) = {
            let mut state = lock(&inner);
            let (version, error) = (state.version, state.error.clone());
            let failed = result.is_err();
            finish(&mut state, kind, segment, generation, ticket, result);
            // Ring only for what a screen shows: new rows, a new error, or
            // an earlier batch's end. A poll that found nothing new, or a
            // read that failed as the last one did, would otherwise start
            // the next read at once, in a loop.
            let changed =
                state.version != version || state.error != error || matches!(kind, Read::Earlier);
            let fill = matches!(kind, Read::Head)
                && segment == 0
                && !failed
                && state.generation == generation
                && state.rows.len() < BATCH_ROWS;
            (std::mem::take(&mut state.again), fill, changed)
        };
        if fill {
            earlier(&client, &Handle::current(), &inner);
        }
        if changed {
            crate::wake::ring();
        }
        if again {
            poll(&client, &Handle::current(), &inner, false);
        }
    });
}

/// Start a read of the batch before the oldest row, unless a read of the
/// newest records or an earlier batch runs or the chat holds its most rows.
fn earlier(client: &Arc<Client>, runtime: &Handle, inner: &Arc<Mutex<Inner>>) {
    let (segment, source, previous) = {
        let mut state = lock(inner);
        if state.loading || state.earlier || state.rows.len() >= MAX_ROWS {
            return;
        }
        let Some((segment, previous)) = state.previous else {
            return;
        };
        let Some(source) = state.sources.get(usize::from(segment)).cloned() else {
            return;
        };
        state.earlier = true;
        (segment, source, previous)
    };
    read(
        client,
        runtime,
        inner,
        (Read::Earlier, segment, source, previous),
    );
}

/// Start a read of the newest source (see [`Conversation::poll`]). A nudge
/// that arrives while a read runs reads again once it finishes, since the
/// running read may have started before the source grew.
fn poll(client: &Arc<Client>, runtime: &Handle, inner: &Arc<Mutex<Inner>>, nudged: bool) -> bool {
    let (kind, segment, source) = {
        let mut state = lock(inner);
        if state.loading || state.polling {
            state.again |= nudged;
            return false;
        }
        let kind = if state.stale {
            state.loading = true;
            Read::Head
        } else {
            state.polling = true;
            Read::Newer
        };
        let (segment, source) = state.newest();
        (kind, segment, source)
    };
    read(
        client,
        runtime,
        inner,
        (kind, segment, source, coder_history::NEWEST),
    );
    true
}

/// Follow the client's nudges for the chat's newest source.
fn watch(
    runtime: &Handle,
    client: &Arc<Client>,
    inner: &Arc<Mutex<Inner>>,
) -> tokio::task::AbortHandle {
    use tokio::sync::broadcast::error::RecvError;
    let mut changes = client.changes();
    let (client, weak) = (client.clone(), Arc::downgrade(inner));
    runtime
        .spawn(async move {
            loop {
                let source = match changes.recv().await {
                    Ok(Change::Source(source)) => source,
                    Ok(Change::Catalog) | Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => return,
                };
                let Some(inner) = weak.upgrade() else { return };
                if lock(&inner).newest().1 == source {
                    poll(&client, &Handle::current(), &inner, true);
                }
            }
        })
        .abort_handle()
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
    inner.previous = inner.rows.first().map(|row| (row.segment, row.offset));
    inner.version += 1;
    true
}

/// What a finished read changes. A read for sources the chat moved past
/// changes nothing.
fn finish(
    state: &mut Inner,
    kind: Read,
    segment: u8,
    generation: u64,
    ticket: u64,
    result: Result<Found, String>,
) {
    if state.generation != generation {
        return;
    }
    match kind {
        Read::Head => state.loading = false,
        Read::Earlier => state.earlier = false,
        Read::Newer => state.polling = false,
    }
    match (result, kind) {
        (Ok(read), Read::Head) => {
            state.show(segment, &read.rows, read.previous);
            state.through = read.through;
            state.error = None;
            state.stale = false;
            state.partial = None;
            state.finished = state.finished.max(ticket);
        }
        (Ok(mut read), Read::Earlier) => {
            for row in &mut read.rows {
                row.segment = segment;
            }
            read.rows.append(&mut state.rows);
            state.rows = read.rows;
            state.previous = if state.rows.len() >= MAX_ROWS {
                None
            } else {
                read.previous.map(|end| (segment, end))
            };
            state.version += 1;
        }
        (Ok(read), Read::Newer) => {
            state.error = None;
            let known = state
                .rows
                .iter()
                .rev()
                .find(|row| row.segment == segment)
                .map_or(0, |row| row.end);
            let before = state.rows.len();
            state.rows.extend(
                read.rows
                    .into_iter()
                    .filter(|row| row.offset >= known && !row.carried),
            );
            let excess = state.rows.len().saturating_sub(MAX_ROWS);
            state.rows.drain(..excess);
            if excess > 0 {
                state.previous = state.rows.first().map(|row| (row.segment, row.offset));
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
        // A failed read keeps what shows and says why only when nothing
        // does; the next poll reads again.
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
    children.extend(
        inner
            .rows
            .iter()
            .map(|row| draw(row, inner.compact, &inner.delegated)),
    );
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
                .filter(|_| inner.rows.len() < MAX_ROWS)
                .map(|_| Earlier {
                    label: "Load earlier messages".into(),
                    loading: inner.earlier,
                    intent: earlier,
                }),
            source: None,
        },
    )
}

#[derive(Clone, Copy)]
enum Read {
    /// The newest source's newest records.
    Head,
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

/// One backward page ending at `end`; after a failure, once more as a
/// smaller relay page.
async fn back(
    client: &Client,
    source: &str,
    end: u64,
) -> Result<coder_history::TranscriptPage, String> {
    match back_within(client, source, end, false).await {
        Ok(page) => Ok(page),
        Err(_) => back_within(client, source, end, true).await,
    }
}

async fn back_within(
    client: &Client,
    source: &str,
    end: u64,
    small: bool,
) -> Result<coder_history::TranscriptPage, String> {
    let read = client.observe_with(|route| {
        Query::Page(TranscriptRequest {
            source_id: source.to_owned(),
            cursor: None,
            max_bytes: page_bytes(route, small),
            end: Some(end),
        })
    });
    let observed = tokio::time::timeout(OBSERVE_LIMIT, read)
        .await
        .map_err(|_| "The computer did not answer.".to_string())?
        .map_err(|error| error.to_string())?;
    match observed {
        Observation::Page(page) => Ok(page),
        Observation::Catalog(_) => Err("Couldn't load this chat. Try again.".into()),
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

/// Sees a read's rows and where earlier records end after each page.
type Show = dyn Fn(&[Row], Option<u64>) + Send + Sync;

/// Where a backward batch stops.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Until {
    /// A page with a row to show: a chat's first read.
    Shown,
    /// [`BATCH_ROWS`] rows, or [`BATCH_BYTES`] read: the batch before the
    /// oldest row.
    Screen,
    /// The messages a later turn's source carries from the turns before.
    Carried,
}

impl Until {
    fn pages(self) -> usize {
        match self {
            Self::Shown | Self::Screen => BATCH_PAGES,
            Self::Carried => NEWER_PAGES,
        }
    }

    fn reached(self, rows: &[Row], bytes: u64) -> bool {
        match self {
            Self::Shown => !rows.is_empty() || bytes >= BATCH_BYTES,
            Self::Screen => rows.len() >= BATCH_ROWS || bytes >= BATCH_BYTES,
            Self::Carried => rows.iter().any(|row| row.carried),
        }
    }
}

/// Raw record bytes a page carries.
fn page_size(page: &coder_history::TranscriptPage) -> u64 {
    page.chunks
        .iter()
        .map(|chunk| chunk.end_offset.saturating_sub(chunk.offset))
        .sum()
}

/// Rows ending at `end`, oldest first, and where earlier records end,
/// read backward a page at a time `until` it has enough. `show` sees the
/// rows found after each page.
async fn batch(
    client: &Client,
    source: &str,
    segment: u8,
    mut end: u64,
    until: Until,
    show: &Show,
) -> Result<Found, String> {
    let mut found = Found {
        rows: vec![],
        previous: None,
        through: 0,
    };
    let mut bytes = 0;
    for _ in 0..until.pages() {
        let page = back(client, source, end).await?;
        bytes += page_size(&page);
        let mut page_rows = rows(&page.chunks);
        for row in &mut page_rows {
            row.segment = segment;
        }
        page_rows.append(&mut found.rows);
        found.rows = page_rows;
        found.previous = page.previous;
        found.through = found.through.max(page_through(&page));
        show(&found.rows, found.previous);
        let enough = until.reached(&found.rows, bytes);
        match page.previous {
            Some(earlier) if !enough => end = earlier,
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
        let bytes = whole
            .then(|| {
                let mut bytes = vec![];
                for chunk in group {
                    bytes.extend(
                        base64::engine::general_purpose::STANDARD
                            .decode(&chunk.raw_base64)
                            .ok()?,
                    );
                }
                Some(bytes)
            })
            .flatten();
        let full = bytes
            .as_deref()
            .and_then(coder_history::readable_record_full);
        let carried = bytes.as_deref().is_some_and(carried);
        if let Some(entry) = bytes.as_deref().and_then(delegate) {
            out.push(Row::new(record, end, 0, entry));
            continue;
        }
        let Some(readable) = full.or_else(|| group.iter().rev().find_map(|c| c.readable.clone()))
        else {
            continue;
        };
        for (part, entry) in entries(&readable).into_iter().enumerate() {
            out.push(Row {
                carried,
                ..Row::new(record, end, part as u8, entry)
            });
        }
    }
    out
}

/// Whether a Coder transcript record repeats an earlier turn's message:
/// a step whose extensions name the turn it was carried from.
fn carried(bytes: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .is_some_and(|record| record.pointer("/step/extensions/carried_from").is_some())
}

/// The delegate note a Coder transcript record carries: a step whose
/// `delegate_transcript` extension names the agent and its session, with
/// the copy's file or why it failed.
fn delegate(bytes: &[u8]) -> Option<Entry> {
    let record = serde_json::from_slice::<serde_json::Value>(bytes).ok()?;
    let note = record.pointer(&format!(
        "/step/extensions/{}",
        coder_history::delegate::NOTE
    ))?;
    let agent = note["agent"]
        .as_str()
        .filter(|a| matches!(*a, "opencode" | "devin"))?;
    let session = note["session"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 64)?;
    Some(Entry::Delegate {
        agent: agent.to_owned(),
        session: session.to_owned(),
        error: note["error"].as_str().map(|error| bounded(error, 300)),
    })
}

/// A delegate agent's name as the chat shows it.
fn agent_label(agent: &str) -> &'static str {
    match agent {
        "devin" => "Devin",
        _ => "OpenCode",
    }
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
                // A decision-model call's record stays in the trajectory,
                // never the transcript (#10073).
                | "decision_call"
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
    // A handoff shows the person's message once, with where it came from
    // as the note, not pasted in front of it (#10076).
    let request = crate::basic_chats::handoff_request(pending.text);
    let note = pending.note.map(str::to_owned).or_else(|| {
        request
            .is_some()
            .then(|| crate::basic_chats::HANDOFF_NOTE.to_owned())
    });
    node(
        &pending.key,
        Element::Message {
            role: MessageRole::User,
            note,
            children: vec![Node {
                key: format!("{}-md", pending.key),
                style: Style {
                    foreground: Some(WHITE),
                    ..Style::default()
                },
                element: Element::Markdown {
                    blocks: markdown::parse(request.as_deref().unwrap_or(pending.text)),
                },
            }],
        },
    )
}

/// Render parsed task rows with the same semantics on every native adapter.
pub fn project_rows<I>(rows: &[Row]) -> Vec<Node<I>> {
    let delegated = std::collections::BTreeMap::new();
    rows.iter().map(|row| draw(row, 0, &delegated)).collect()
}

fn draw<I>(
    row: &Row,
    compact: u8,
    delegated: &std::collections::BTreeMap<String, Vec<Row>>,
) -> Node<I> {
    let key = format!("r{}-{}-{}", row.segment, row.offset, row.part);
    match &row.entry {
        // A compact row that opens to the delegate session, read-only.
        Entry::Delegate {
            agent,
            session,
            error,
        } => {
            let label = agent_label(agent);
            let rows = delegated.get(session).map_or(&[][..], Vec::as_slice);
            let (shown, bytes) = match compact {
                0 => (DELEGATE_ROWS, 600),
                1 => (COMPACT_DELEGATE_ROWS, 200),
                _ => (0, 0),
            };
            let skip = rows.len().saturating_sub(shown);
            let children = rows
                .iter()
                .skip(skip)
                .enumerate()
                .filter_map(|(index, row)| {
                    let (value, role, color) = match &row.entry {
                        Entry::Message { role, text } => (
                            format!(
                                "{}: {}",
                                match role {
                                    MessageRole::User => "Coder",
                                    MessageRole::Assistant => label,
                                    MessageRole::System => "Note",
                                },
                                bounded(text, bytes)
                            ),
                            TextRole::Body,
                            WHITE,
                        ),
                        Entry::Tool { name, detail, .. } => (
                            bounded(&format!("{name} {detail}"), bytes),
                            TextRole::Code,
                            GRAY,
                        ),
                        Entry::Delegate { .. } => return None,
                    };
                    Some(Node {
                        key: format!("{key}-d{index}"),
                        style: Style {
                            foreground: Some(color),
                            ..Style::default()
                        },
                        element: Element::Text { value, role },
                    })
                })
                .collect();
            node(
                &key,
                Element::Tool {
                    name: format!("Delegated to {label}"),
                    detail: error.clone().unwrap_or_else(|| session.clone()),
                    state: if error.is_some() {
                        ToolState::Failed
                    } else {
                        ToolState::Done
                    },
                    children,
                },
            )
        }
        Entry::Message {
            role: MessageRole::System,
            text,
        } => system(&key, text),
        Entry::Message { role, text } => {
            // A handoff from the phone is the person's message, once: not
            // the conversation pasted back, and not prefixed with where it
            // came from, which is the note (#10076).
            let request =
                crate::basic_chats::handoff_request(text).filter(|_| *role == MessageRole::User);
            node(
                &key,
                Element::Message {
                    role: *role,
                    note: request
                        .is_some()
                        .then(|| crate::basic_chats::HANDOFF_NOTE.to_owned()),
                    children: vec![Node {
                        key: format!("{key}-md"),
                        style: Style {
                            foreground: Some(WHITE),
                            ..Style::default()
                        },
                        element: Element::Markdown {
                            blocks: match request {
                                Some(request) => markdown::parse(&request),
                                None => row.blocks.clone(),
                            },
                        },
                    }],
                },
            )
        }
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

    /// A Coder transcript is mostly records with no row: the opening read
    /// ends at the first page with a row, and the background read before it
    /// at a screen of rows, messages and tools alike, or its byte bound.
    #[test]
    fn the_opening_read_ends_at_a_row_and_the_fill_at_a_screen() {
        let tool = |offset| {
            Row::new(
                offset,
                offset + 10,
                0,
                Entry::Tool {
                    name: "shell".into(),
                    detail: "ls".into(),
                    body: String::new(),
                },
            )
        };
        assert!(!Until::Shown.reached(&[], 40_000));
        assert!(Until::Shown.reached(&[tool(0)], 100));
        assert!(Until::Shown.reached(&[], BATCH_BYTES));
        let some: Vec<Row> = (0..BATCH_ROWS as u64 - 1).map(tool).collect();
        assert!(!Until::Screen.reached(&some, 100));
        let screen: Vec<Row> = (0..BATCH_ROWS as u64).map(tool).collect();
        assert!(Until::Screen.reached(&screen, 100));
        assert!(Until::Screen.reached(&some, BATCH_BYTES));
        // A relay page asks for the relay's most, and a smaller one after a
        // failure; a direct page is always the direct most.
        assert_eq!(
            page_bytes(Route::Relay, false),
            coder_history::MAX_PAGE_BYTES
        );
        assert_eq!(page_bytes(Route::Relay, true), SMALL_PAGE_BYTES);
        assert_eq!(
            page_bytes(Route::Direct, true),
            coder_history::Limits::DIRECT.page_bytes
        );
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
            sources: vec!["a".into(), "b".into()],
            rows: vec![message(0, MessageRole::User, "Turn one")],
            generation: 2,
            stale: true,
            loading: true,
            ..Inner::default()
        };
        let old = found(vec![message(0, MessageRole::Assistant, "Old")], None);
        finish(&mut inner, Read::Head, 1, 1, 1, Ok(old));
        assert!(inner.loading && inner.stale);
        assert_eq!(inner.rows.len(), 1);
    }

    /// A follow-up's turn is a new source that begins with the earlier
    /// turns' messages: its rows after them follow the rows shown, and a
    /// poll adds only its records after the newest one read.
    #[test]
    fn a_new_turn_adds_its_own_rows_after_the_ones_shown() {
        let mut inner = Inner {
            sources: vec!["turn-1".into(), "turn-2".into()],
            rows: vec![
                message(0, MessageRole::User, "Ask me."),
                message(10, MessageRole::Assistant, "Apple or pear?"),
            ],
            previous: Some((0, 0)),
            stale: true,
            loading: true,
            generation: 3,
            started: 5,
            ..Inner::default()
        };
        let carried = |offset, role, text| Row {
            carried: true,
            ..message(offset, role, text)
        };
        let turn = found(
            vec![
                carried(100, MessageRole::User, "Ask me."),
                carried(110, MessageRole::Assistant, "Apple or pear?"),
                message(120, MessageRole::User, "Pear."),
            ],
            Some(100),
        );
        finish(&mut inner, Read::Head, 1, 3, 5, Ok(turn));
        assert!(!inner.stale && !inner.loading);
        assert_eq!(inner.finished, 5);
        assert_eq!(inner.through, 130);
        let texts: Vec<&str> = inner
            .rows
            .iter()
            .map(|row| match &row.entry {
                Entry::Message { text, .. } => text.as_str(),
                Entry::Tool { .. } | Entry::Delegate { .. } => "",
            })
            .collect();
        assert_eq!(texts, ["Ask me.", "Apple or pear?", "Pear."]);
        assert_eq!(inner.rows[2].segment, 1);
        // Earlier records still come from the first turn's source.
        assert_eq!(inner.previous, Some((0, 0)));
        let polled = found(
            vec![
                message(120, MessageRole::User, "Pear."),
                message(130, MessageRole::Assistant, "pear"),
            ],
            None,
        );
        inner.polling = true;
        let version = inner.version;
        finish(&mut inner, Read::Newer, 1, 3, 6, Ok(polled));
        assert_eq!(inner.rows.len(), 4);
        assert!(inner.version > version);
        assert_eq!(inner.finished, 6);
        // A failed poll changes nothing and is not a finished read.
        inner.polling = true;
        finish(&mut inner, Read::Newer, 1, 3, 7, Err("late".into()));
        assert_eq!((inner.rows.len(), inner.finished), (4, 6));
        // Row keys stay distinct across sources.
        let node = transcript(&inner, "t", (), &[], None);
        let Element::Transcript { children, .. } = &node.element else {
            unreachable!()
        };
        let mut keys: Vec<&str> = children.iter().map(|child| child.key.as_str()).collect();
        keys.dedup();
        assert_eq!(keys.len(), 4);
    }

    #[test]
    fn a_first_read_replaces_the_copy_of_another_source() {
        let mut inner = Inner {
            sources: vec!["turn-2".into()],
            rows: vec![message(0, MessageRole::User, "Kept")],
            stale: true,
            loading: true,
            ..Inner::default()
        };
        finish(&mut inner, Read::Head, 0, 0, 1, Err("late".into()));
        assert!(inner.stale && !inner.loading);
        assert_eq!(kinds(&transcript(&inner, "t", (), &[], None)), ["User:"]);
        inner.loading = true;
        let read = found(
            vec![
                message(0, MessageRole::User, "Kept"),
                message(10, MessageRole::Assistant, "Reply"),
            ],
            Some(0),
        );
        finish(&mut inner, Read::Head, 0, 0, 2, Ok(read));
        assert_eq!(inner.rows.len(), 2);
        assert_eq!(inner.previous, Some((0, 0)));
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
        assert_eq!(inner.previous, Some((0, 20)));
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

    /// A decision-model call's record (a Jev judgment the loop asked)
    /// shows no row on the phone or the desktop; a real tool call does
    /// (#10073).
    #[test]
    fn a_decision_call_shows_no_row() {
        let mut judged = readable(
            "decision_call",
            None,
            "{\"model\":\"jev-1.13.0\",\"state\":{}}",
        );
        judged.tool_name = Some("openagents.microcoder.judge.v1".into());
        assert!(entries(&judged).is_empty());
        let mut tool = readable("tool_call", None, "ls");
        tool.tool_name = Some("shell".into());
        assert_eq!(entries(&tool).len(), 1);
    }

    /// A task transcript's delegate note becomes a row that opens to the
    /// delegate session's rows, read-only; a failed copy says why.
    #[test]
    fn a_delegate_note_shows_the_delegated_session_under_it() {
        let record = serde_json::json!({"step": {"source": "system", "message": "",
            "extensions": {"delegate_transcript": {"agent": "opencode",
                "session": "ses_abc", "file": "t.delegate.opencode.ses_abc.jsonl"}}}});
        let bytes = format!("{record}\n");
        let chunk = RecordChunk {
            id: "c".into(),
            index: 0,
            record_offset: 0,
            offset: 0,
            end_offset: bytes.len() as u64,
            raw_base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
            complete: true,
            oversized: false,
            readable: None,
        };
        let found = rows(&[chunk]);
        assert_eq!(
            found
                .iter()
                .map(|row| row.entry.clone())
                .collect::<Vec<_>>(),
            [Entry::Delegate {
                agent: "opencode".into(),
                session: "ses_abc".into(),
                error: None,
            }]
        );
        let mut inner = Inner {
            rows: found,
            ..Inner::default()
        };
        inner.delegated.insert(
            "ses_abc".into(),
            vec![
                message(0, MessageRole::User, "Fix the test"),
                message(10, MessageRole::Assistant, "Fixed."),
            ],
        );
        let view = transcript(&inner, "t", (), &[], None);
        let Element::Transcript { children, .. } = &view.element else {
            panic!("a transcript");
        };
        let Element::Tool {
            name,
            detail,
            state,
            children,
        } = &children[0].element
        else {
            panic!("a delegate row");
        };
        assert_eq!(name, "Delegated to OpenCode");
        assert_eq!(detail, "ses_abc");
        assert_eq!(*state, ToolState::Done);
        let lines: Vec<&str> = children
            .iter()
            .map(|child| match &child.element {
                Element::Text { value, .. } => value.as_str(),
                _ => "",
            })
            .collect();
        assert_eq!(lines, ["Coder: Fix the test", "OpenCode: Fixed."]);
        let failed = Row::new(
            0,
            1,
            0,
            Entry::Delegate {
                agent: "devin".into(),
                session: "calm-river".into(),
                error: Some("the store was locked".into()),
            },
        );
        let view: Node<()> = draw(&failed, 0, &inner.delegated);
        assert!(matches!(
            view.element,
            Element::Tool { ref name, state: ToolState::Failed, ref detail, .. }
                if name == "Delegated to Devin" && detail == "the store was locked"
        ));
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
