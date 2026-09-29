//! The phone's chat-loading algorithms, driven through the real
//! `coder_connect::Client`.
//!
//! `crates/openagents-mobile` is its own Cargo workspace (Breez's SQLite),
//! so this crate cannot link it. These functions mirror its read loops and
//! constants exactly; keep them in step:
//!
//! - [`catalog`] is `catalog_pages` and `catalog_page` in
//!   `crates/openagents-mobile/src/chats.rs` (`CATALOG_PAGES`,
//!   `CATALOG_WANTED`, `head_limit`, `shown`).
//! - [`open`] is `batch` and `back` in
//!   `crates/openagents-mobile/src/conversation.rs` (`PAGE_BYTES`,
//!   `page_bytes`, `BATCH_PAGES`, `BATCH_MESSAGES`), and [`rows`] is its
//!   `rows` and `entries`, reduced to what the layout needs.

use coder_connect::protocol::Route;
use coder_connect::{Client, Observation, Query};
use coder_history::{CatalogRequest, Chat, RecordChunk, TranscriptRequest};
use rust_native::markdown;
use rust_native::style::Style;
use rust_native::view::ToolState;
use rust_native::{Element, MessageRole, Node};
use std::time::{Duration, Instant};

/// `OBSERVE_LIMIT` in `chats.rs` and `conversation.rs`.
pub const OBSERVE_LIMIT: Duration = Duration::from_secs(20);
/// `CATALOG_PAGES` in `chats.rs`.
pub const CATALOG_PAGES: usize = 8;
/// `CATALOG_WANTED` in `chats.rs`.
pub const CATALOG_WANTED: usize = 60;
/// `PAGE_BYTES` in `conversation.rs`: raw bytes per backward relay page.
pub const PAGE_BYTES: u32 = 16 * 1024;
/// `BATCH_PAGES` in `conversation.rs`.
pub const BATCH_PAGES: usize = 12;
/// `BATCH_MESSAGES` in `conversation.rs`.
pub const BATCH_MESSAGES: usize = 10;
const MESSAGE_BYTES: usize = 6_000;
const TOOL_BYTES: usize = 1_500;

/// `shown` in `chats.rs`: a Coder task's chat, not archived.
pub fn shown(chat: &Chat) -> bool {
    chat.harness == coder_history::Harness::Coder && !chat.archived
}

async fn observe(
    client: &Client,
    make: &(dyn Fn(Route) -> Query + Sync),
) -> Result<Observation, String> {
    tokio::time::timeout(OBSERVE_LIMIT, client.observe_with(make))
        .await
        .map_err(|_| "the computer did not answer".to_string())?
        .map_err(|error| error.to_string())
}

/// What one chat-list load did.
#[derive(Debug)]
pub struct CatalogLoad {
    pub chats: Vec<Chat>,
    pub pages: usize,
    /// When the first page arrived: the earliest the list could show.
    pub first_page: Duration,
    /// When the loop ended: when the list shows today.
    pub total: Duration,
}

/// `catalog_pages` in `chats.rs`: read pages newest first until
/// `CATALOG_WANTED` chats would show, at most `CATALOG_PAGES`.
///
/// # Errors
/// When the first page fails.
pub async fn catalog(client: &Client) -> Result<CatalogLoad, String> {
    let started = Instant::now();
    let mut first_page = None;
    let mut chats: Vec<Chat> = vec![];
    let mut cursor = None;
    let mut pages = 0;
    for _ in 0..CATALOG_PAGES {
        let after = cursor.take();
        let make = |route: Route| {
            Query::Catalog(CatalogRequest {
                cursor: after.clone(),
                limit: route.limits().catalog_page,
            })
        };
        let page = match observe(client, &make).await {
            Ok(Observation::Catalog(page)) => page,
            Ok(_) => return Err("the computer answered with the wrong page".into()),
            Err(_) if !chats.is_empty() => break,
            Err(error) => return Err(error),
        };
        pages += 1;
        first_page.get_or_insert_with(|| started.elapsed());
        for chat in page.entries {
            if !chats.iter().any(|known| known.id == chat.id) {
                chats.push(chat);
            }
        }
        if chats.iter().filter(|chat| shown(chat)).count() >= CATALOG_WANTED {
            break;
        }
        match page.next {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    Ok(CatalogLoad {
        chats,
        pages,
        first_page: first_page.unwrap_or_default(),
        total: started.elapsed(),
    })
}

/// `page_bytes` in `conversation.rs`.
pub fn page_bytes(route: Route) -> u32 {
    match route {
        Route::Relay => PAGE_BYTES,
        Route::Direct => route.limits().page_bytes,
    }
}

/// One transcript row, as the phone draws it.
#[derive(Clone, Debug)]
pub enum Row {
    Message { role: MessageRole, text: String },
    Tool { name: String, body: String },
}

/// What opening one chat did.
#[derive(Debug)]
pub struct Opened {
    pub rows: Vec<Row>,
    pub pages: usize,
    /// When the first page's rows could show.
    pub first_page: Duration,
    /// When the batch ended.
    pub total: Duration,
    /// Raw source bytes the pages carried.
    pub bytes: u64,
}

/// `batch` in `conversation.rs` for a chat's first segment: backward pages
/// from the newest record until `BATCH_MESSAGES` messages, at most
/// `BATCH_PAGES`.
///
/// # Errors
/// When a page fails.
pub async fn open(client: &Client, source: &str) -> Result<Opened, String> {
    let started = Instant::now();
    let mut first_page = None;
    let mut found: Vec<Row> = vec![];
    let mut end = coder_history::NEWEST;
    let mut pages = 0;
    let mut bytes = 0;
    for _ in 0..BATCH_PAGES {
        let make = |route: Route| {
            Query::Page(TranscriptRequest {
                source_id: source.to_owned(),
                cursor: None,
                max_bytes: page_bytes(route),
                end: Some(end),
            })
        };
        let page = match observe(client, &make).await? {
            Observation::Page(page) => page,
            Observation::Catalog(_) => {
                return Err("the computer answered with the wrong page".into());
            }
        };
        pages += 1;
        bytes += page
            .chunks
            .iter()
            .map(|c| c.end_offset - c.offset)
            .sum::<u64>();
        let mut page_rows = rows(&page.chunks);
        page_rows.append(&mut found);
        found = page_rows;
        first_page.get_or_insert_with(|| started.elapsed());
        let enough = found
            .iter()
            .filter(|row| matches!(row, Row::Message { .. }))
            .count()
            >= BATCH_MESSAGES;
        match page.previous {
            Some(earlier) if !enough => end = earlier,
            _ => break,
        }
    }
    Ok(Opened {
        rows: found,
        pages,
        first_page: first_page.unwrap_or_default(),
        total: started.elapsed(),
        bytes,
    })
}

fn bounded(text: &str, max: usize) -> String {
    let mut end = text.len().min(max);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

/// `rows` and `entries` in `conversation.rs`, reduced to messages and tool
/// rows.
pub fn rows(chunks: &[RecordChunk]) -> Vec<Row> {
    use base64::Engine;
    let mut out = vec![];
    let mut index = 0;
    while index < chunks.len() {
        let start = index;
        let record = chunks[start].record_offset;
        while index < chunks.len() && chunks[index].record_offset == record {
            index += 1;
        }
        let group = &chunks[start..index];
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
        let Some(readable) = full.or_else(|| group.iter().rev().find_map(|c| c.readable.clone()))
        else {
            continue;
        };
        let text = readable.text.trim();
        let kind = readable.kind.as_str();
        if readable.unknown
            || text.is_empty()
            || matches!(
                kind,
                "reasoning" | "session" | "session_meta" | "turn_context" | "token_count"
            )
        {
            continue;
        }
        let role = readable.role.as_deref();
        let tool = readable.tool_name.is_some()
            || readable.call_id.is_some()
            || kind.ends_with("_output")
            || kind.contains("tool")
            || kind == "function_call";
        if tool {
            out.push(Row::Tool {
                name: readable
                    .tool_name
                    .clone()
                    .unwrap_or_else(|| "Result".into()),
                body: bounded(text, TOOL_BYTES),
            });
            continue;
        }
        let role = match role {
            Some("user") if text.starts_with('<') => continue,
            Some("user") => MessageRole::User,
            Some("assistant") => MessageRole::Assistant,
            _ => continue,
        };
        out.push(Row::Message {
            role,
            text: bounded(text, MESSAGE_BYTES),
        });
    }
    out
}

/// The transcript node `conversation.rs` builds from rows (`draw`).
pub fn transcript(rows: &[Row]) -> Node<()> {
    let node = |key: String, element| Node {
        key,
        style: Style::default(),
        element,
    };
    let children = rows
        .iter()
        .enumerate()
        .map(|(index, row)| match row {
            Row::Message { role, text } => node(
                format!("r{index}"),
                Element::Message {
                    role: *role,
                    note: None,
                    children: vec![node(
                        format!("r{index}-md"),
                        Element::Markdown {
                            blocks: markdown::parse(text),
                        },
                    )],
                },
            ),
            Row::Tool { name, body } => node(
                format!("r{index}"),
                Element::Tool {
                    name: name.clone(),
                    detail: body
                        .lines()
                        .next()
                        .unwrap_or("")
                        .chars()
                        .take(100)
                        .collect(),
                    state: ToolState::Done,
                    children: vec![node(
                        format!("r{index}-body"),
                        Element::Text {
                            value: body.clone(),
                            role: rust_native::TextRole::Code,
                        },
                    )],
                },
            ),
        })
        .collect();
    node(
        "chat".into(),
        Element::Transcript {
            label: "Messages".into(),
            children,
            earlier: None,
            source: None,
        },
    )
}

/// The chat list's rows as `catalog_view` in `chats.rs` builds them: one
/// button per shown chat in a list.
pub fn list(chats: &[Chat]) -> Node<()> {
    let node = |key: String, element| Node {
        key,
        style: Style::default(),
        element,
    };
    let mut shown: Vec<&Chat> = chats.iter().filter(|chat| shown(chat)).collect();
    shown.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    let rows = shown
        .into_iter()
        .take(200)
        .enumerate()
        .map(|(index, chat)| {
            node(
                format!("chat-{index}"),
                Element::Button {
                    label: format!(
                        "{}\n{:?} · {}",
                        chat.title,
                        chat.harness,
                        chat.updated_at.as_deref().unwrap_or("")
                    ),
                    enabled: true,
                    icon: None,
                    intent: (),
                },
            )
        })
        .collect();
    node(
        "chat-list".into(),
        Element::List {
            label: "Chats from your computers".into(),
            children: rows,
        },
    )
}
