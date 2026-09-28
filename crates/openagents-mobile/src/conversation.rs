//! One chat read from a computer's history observer, drawn as a Rust Native
//! transcript: messages by role with Markdown, tool rows, and a working row.
//!
//! The chat opens at its newest records, read backward from the end, and
//! "Load earlier" reads the batch before the oldest row. [`Conversation::poll`]
//! reads backward from the end again until it reaches rows it has, so a
//! running chat grows without splitting a record.

use coder_connect::{Client, Observation, Query};
use coder_history::{Chat, RecordChunk, TranscriptRequest};
use rust_native::markdown;
use rust_native::style::{Color, Style};
use rust_native::{Earlier, Element, MessageRole, Node, TextRole, ToolState};
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
/// Conversational messages a batch looks for.
const BATCH_MESSAGES: usize = 10;
/// Rows kept for one chat; older rows are not read past this.
const MAX_ROWS: usize = 240;
const MESSAGE_BYTES: usize = 6_000;
const TOOL_BYTES: usize = 1_500;
const DETAIL_CHARS: usize = 100;

const WHITE: Color = Color::rgb(255, 255, 255);
const GRAY: Color = Color::rgb(153, 153, 153);

#[derive(Clone, Debug, PartialEq, Eq)]
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
}

#[derive(Default)]
struct Inner {
    rows: Vec<Row>,
    /// Where earlier records end, when there are any.
    previous: Option<u64>,
    loading: bool,
    earlier: bool,
    polling: bool,
    error: Option<String>,
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
        let conversation = Self {
            source: chat.source_id.clone().unwrap_or_default(),
            chat,
            client,
            runtime,
            inner: Arc::new(Mutex::new(Inner {
                loading: true,
                ..Inner::default()
            })),
        };
        conversation.read(coder_history::NEWEST, Read::First);
        conversation
    }

    pub fn loading(&self) -> bool {
        let inner = lock(&self.inner);
        inner.loading || inner.earlier
    }

    /// Read the batch before the oldest row.
    pub fn earlier(&self) {
        let previous = {
            let mut inner = lock(&self.inner);
            if inner.loading || inner.earlier || inner.rows.len() >= MAX_ROWS {
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

    /// Read records added since the newest row.
    pub fn poll(&self) {
        {
            let mut inner = lock(&self.inner);
            if inner.loading || inner.polling {
                return;
            }
            inner.polling = true;
        }
        self.read(coder_history::NEWEST, Read::Newer);
    }

    fn read(&self, end: u64, kind: Read) {
        let (client, source, inner) =
            (self.client.clone(), self.source.clone(), self.inner.clone());
        self.runtime.spawn(async move {
            let known = lock(&inner).rows.last().map(|row| row.end);
            let result = match kind {
                Read::Newer => newer(&client, &source, known.unwrap_or(0)).await,
                Read::First | Read::Earlier => batch(&client, &source, end).await,
            };
            let mut state = lock(&inner);
            state.loading = false;
            state.earlier = false;
            state.polling = false;
            match (result, kind) {
                (Ok((rows, previous)), Read::First) => {
                    state.rows = rows;
                    state.previous = previous;
                    state.error = None;
                }
                (Ok((mut rows, previous)), Read::Earlier) => {
                    rows.append(&mut state.rows);
                    state.rows = rows;
                    state.previous = if state.rows.len() >= MAX_ROWS {
                        None
                    } else {
                        previous
                    };
                }
                (Ok((rows, _)), Read::Newer) => {
                    let known = state.rows.last().map_or(0, |row| row.end);
                    state
                        .rows
                        .extend(rows.into_iter().filter(|row| row.offset >= known));
                    let excess = state.rows.len().saturating_sub(MAX_ROWS);
                    state.rows.drain(..excess);
                }
                (Err(error), _) => state.error = Some(error),
            }
        });
    }

    /// The chat as a transcript node. `earlier` is the intent that loads
    /// older rows; `working` adds a working row, such as "Coder is working".
    pub fn transcript<I: Clone>(&self, key: &str, earlier: I, working: Option<&str>) -> Node<I> {
        let inner = lock(&self.inner);
        let mut children: Vec<Node<I>> = vec![];
        if let Some(error) = &inner.error {
            children.push(system(&format!("{key}-error"), error));
        }
        children.extend(inner.rows.iter().map(|row| draw(row)));
        if inner.loading {
            children.push(node(
                &format!("{key}-loading"),
                Element::Working {
                    label: "Loading the chat".into(),
                },
            ));
        } else if inner.rows.is_empty() && inner.error.is_none() {
            children.push(system(&format!("{key}-empty"), "No messages yet."));
        }
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
                earlier: inner.previous.filter(|_| !inner.loading).map(|_| Earlier {
                    label: "Load earlier messages".into(),
                    loading: inner.earlier,
                    intent: earlier,
                }),
            },
        )
    }

    pub fn is_empty(&self) -> bool {
        lock(&self.inner).rows.is_empty()
    }

    /// Drop the oldest rows, for a view that grew past its bound.
    pub fn shrink(&self) {
        let mut inner = lock(&self.inner);
        let half = inner.rows.len() / 2;
        if half == 0 {
            return;
        }
        inner.rows.drain(..half);
        inner.previous = None;
    }
}

#[derive(Clone, Copy)]
enum Read {
    First,
    Earlier,
    Newer,
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

/// Up to a batch of messages ending at `end`, oldest first, and where
/// earlier records end.
async fn batch(
    client: &Client,
    source: &str,
    mut end: u64,
) -> Result<(Vec<Row>, Option<u64>), String> {
    let mut found: Vec<Row> = vec![];
    let mut previous = None;
    for _ in 0..BATCH_PAGES {
        let page = back(client, source, end).await?;
        let mut page_rows = rows(&page.chunks);
        page_rows.append(&mut found);
        found = page_rows;
        previous = page.previous;
        let conversational = found
            .iter()
            .filter(|row| matches!(row.entry, Entry::Message { .. }))
            .count();
        match page.previous {
            Some(earlier) if conversational < BATCH_MESSAGES => end = earlier,
            _ => break,
        }
    }
    Ok((found, previous))
}

/// Records that start at or after `known`, oldest first.
async fn newer(
    client: &Client,
    source: &str,
    known: u64,
) -> Result<(Vec<Row>, Option<u64>), String> {
    let mut found: Vec<Row> = vec![];
    let mut end = coder_history::NEWEST;
    for _ in 0..BATCH_PAGES {
        let page = back(client, source, end).await?;
        let start = page
            .chunks
            .first()
            .map_or(page.next.offset, |chunk| chunk.offset);
        let mut page_rows: Vec<Row> = rows(&page.chunks)
            .into_iter()
            .filter(|row| row.offset >= known)
            .collect();
        page_rows.append(&mut found);
        found = page_rows;
        match page.previous {
            Some(earlier) if start > known => end = earlier,
            _ => break,
        }
    }
    Ok((found, None))
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
            out.push(Row {
                offset: record,
                end,
                part: part as u8,
                entry,
            });
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

fn draw<I>(row: &Row) -> Node<I> {
    let key = format!("r{}-{}", row.offset, row.part);
    match &row.entry {
        Entry::Message {
            role: MessageRole::System,
            text,
        } => system(&key, text),
        Entry::Message { role, text } => node(
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
                        blocks: markdown::parse(text),
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
                children: vec![Node {
                    key: format!("{key}-body"),
                    style: Style {
                        foreground: Some(GRAY),
                        ..Style::default()
                    },
                    element: Element::Text {
                        value: body.clone(),
                        role: TextRole::Code,
                    },
                }],
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
