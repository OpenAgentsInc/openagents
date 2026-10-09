use crate::app::{App, CATALOG_WINDOW, Intent};
use base64::Engine;
use coder_history::{RecordChunk, TranscriptPage};
use coder_ui::theme::{Intensity, NEAR_BLACK};
use rust_native::style::{Color, Space, Style};
use rust_native::{Axis, Element, Node, TextRole};

fn color(rgb: u32) -> Color {
    Color::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}
fn text(key: impl Into<String>, value: impl Into<String>, role: TextRole) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(color(Intensity::ThreeQuarters.color())),
            ..Style::default()
        },
        element: Element::Text {
            value: value.into(),
            role,
        },
    }
}
fn button(
    key: impl Into<String>,
    label: impl Into<String>,
    intent: Intent,
    enabled: bool,
) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(color(Intensity::Full.color())),
            ..Style::default()
        },
        element: Element::Button {
            shortcut: None,
            label: label.into(),
            enabled,
            icon: None,
            intent,
        },
    }
}
fn stack(key: impl Into<String>, children: Vec<Node<Intent>>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            gap: Some(Space::Sm),
            ..Style::default()
        },
        element: Element::Stack {
            axis: Axis::Vertical,
            children,
        },
    }
}

fn row(key: &str, children: Vec<Node<Intent>>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            gap: Some(Space::Md),
            ..Style::default()
        },
        element: Element::Stack {
            axis: Axis::Horizontal,
            children,
        },
    }
}

pub(crate) fn root(app: &App) -> Result<Node<Intent>, String> {
    let mut nodes = vec![];
    for (index, notice) in app
        .notices
        .iter()
        .take(if app.details { 8 } else { 1 })
        .enumerate()
    {
        nodes.push(text(format!("notice-{index}"), notice, TextRole::Status));
    }
    if app.details && app.notices.len() > 8 {
        nodes.push(text(
            "more-notices",
            format!(
                "{} more notices; some earlier messages may be missing.",
                app.notices.len() - 8
            ),
            TextRole::Status,
        ));
    }
    if app.selected.is_some() {
        nodes.extend(timeline(app)?);
    } else {
        nodes.extend(catalog(app));
    }
    let mut node = stack("screen", nodes);
    node.style.background = Some(color(NEAR_BLACK));
    node.style.padding_start = Some(Space::Sm);
    node.style.padding_end = Some(Space::Sm);
    Ok(node)
}

fn catalog(app: &App) -> Vec<Node<Intent>> {
    let mut nodes = Vec::new();
    if app.code.is_none() && !app.synthetic {
        nodes.push(text(
            "pair-help",
            "Pair a computer to read chats.",
            TextRole::Body,
        ));
        return nodes;
    }
    nodes.push(text(
        "catalog-count",
        format!("{} chats", app.catalog.len()),
        TextRole::Status,
    ));
    let rows = app
        .catalog
        .iter()
        .skip(app.catalog_start)
        .take(CATALOG_WINDOW)
        .enumerate()
        .map(|(index, chat)| {
            let suffix = if chat.archived {
                " · archived"
            } else if chat.subagent {
                " · subagent"
            } else {
                ""
            };
            let harness = match chat.harness {
                coder_history::Harness::Codex => "Codex",
                coder_history::Harness::Claude => "Claude",
                coder_history::Harness::Coder => "Coder",
                coder_history::Harness::OpenCode => "OpenCode",
                coder_history::Harness::Devin => "Devin",
            };
            let updated = chat.updated_at.as_deref().unwrap_or("");
            button(
                format!("chat-{}", app.catalog_start + index),
                format!(
                    "{}\n{harness}{suffix}{}{}",
                    chat.title,
                    if updated.is_empty() {
                        String::new()
                    } else {
                        format!(" · {updated}")
                    },
                    if chat.source_id.is_none() {
                        " · unavailable"
                    } else {
                        ""
                    }
                ),
                Intent::Open {
                    source: chat.source_id.clone().unwrap_or_default(),
                },
                chat.source_id.is_some(),
            )
        })
        .collect();
    nodes.push(Node {
        key: "chat-list".into(),
        style: Style::default(),
        element: Element::List {
            label: "Saved conversations".into(),
            children: rows,
        },
    });
    if app.catalog.is_empty() {
        nodes.push(text(
            "catalog-empty",
            if app.catalog_state.pages > 0 {
                "No saved chats found on this computer."
            } else if app.error.is_some() {
                "Chats unavailable. Keep openagents pair running and retry."
            } else {
                "Loading chats…"
            },
            TextRole::Body,
        ));
    }
    if app.catalog.len() > CATALOG_WINDOW {
        nodes.push(row(
            "catalog-pages",
            vec![
                button(
                    "previous-chats",
                    "Previous",
                    Intent::PreviousChats,
                    app.catalog_start > 0,
                ),
                button(
                    "next-chats",
                    "Next",
                    Intent::NextChats,
                    app.catalog_start + CATALOG_WINDOW < app.catalog.len(),
                ),
            ],
        ));
    }
    if app.catalog_state.next.is_some() {
        nodes.push(button(
            "load-chats",
            "Load more chats",
            Intent::MoreChats,
            true,
        ));
    }
    if app.catalog.is_empty() && app.error.is_some() {
        nodes.push(button(
            "refresh",
            "Retry",
            Intent::Refresh,
            app.code.is_some(),
        ));
    }

    nodes
}

fn timeline(app: &App) -> Result<Vec<Node<Intent>>, String> {
    let chat = app.selected.as_ref().ok_or("no chat selected")?;
    let mut nodes = vec![
        row(
            "transcript-header",
            vec![
                button("back", "Chats", Intent::Back, true),
                button(
                    "details",
                    if app.details {
                        "Hide details"
                    } else {
                        "Details"
                    },
                    Intent::Details,
                    true,
                ),
            ],
        ),
        text("chat-title", &chat.title, TextRole::Heading),
    ];
    let keys = app.page_keys()?;
    let count = keys.len();
    let index = app.window_index(&keys);
    let received = app.transcript.cursor.as_ref().map_or(0, |c| c.offset);
    if app.details {
        nodes.push(text(
            "history-progress",
            format!(
                "{received} / {} bytes received{}",
                app.transcript.snapshot_bytes,
                if app.transcript.has_more {
                    " · loading"
                } else {
                    ""
                }
            ),
            TextRole::Status,
        ));
    }
    if app.transcript.pending_line {
        nodes.push(text(
            "partial",
            "Waiting for the next message…",
            TextRole::Status,
        ));
    }
    nodes.push(row(
        "transcript-pages",
        vec![
            button("earlier", "Earlier", Intent::Earlier, index > 0),
            button("later", "Later", Intent::Later, index + 1 < count),
            button("follow", "Latest", Intent::Follow, true),
        ],
    ));
    let mut rows = Vec::new();
    if let Some(key) = keys.get(index) {
        let page: TranscriptPage = app
            .cache
            .read(key)?
            .ok_or("This page needs to reload from the computer.")?;
        if app.details {
            nodes.push(text(
                "page-position",
                format!(
                    "Page {} of {} · bytes {}–{}",
                    index + 1,
                    count,
                    page.chunks.first().map_or(page.next.offset, |c| c.offset),
                    page.next.offset
                ),
                TextRole::Status,
            ));
        }
        if page.chunks.first().is_some_and(|c| c.offset > 0) && index == 0 {
            nodes.push(text(
                "cache-gap",
                "Earlier messages aren't on this phone. Open Details to reload.",
                TextRole::Status,
            ));
        }
        if app
            .window
            .as_ref()
            .is_some_and(|wanted| !keys.contains(wanted))
        {
            nodes.push(text(
                "evicted-position",
                "This page isn't on this phone anymore. Open Details to reload.",
                TextRole::Status,
            ));
        }
        if index > 0 {
            let previous = app.cache.read::<TranscriptPage>(&keys[index - 1])?;
            if previous.as_ref().is_none_or(|previous| {
                previous.next.offset
                    != page
                        .chunks
                        .first()
                        .map_or(page.next.offset, |chunk| chunk.offset)
            }) {
                nodes.push(text(
                    "cache-internal-gap",
                    "Some earlier messages aren't on this phone anymore. Open Details to reload.",
                    TextRole::Status,
                ));
            }
        }
        let mut groups: Vec<Vec<&RecordChunk>> = Vec::new();
        for chunk in &page.chunks {
            if let Some(group) = groups.last_mut()
                && group[0].record_offset == chunk.record_offset
            {
                group.push(chunk);
            } else {
                groups.push(vec![chunk]);
            }
        }
        for group in &groups {
            rows.push(record(app, group, &keys, index)?);
        }
    } else {
        rows.push(text(
            "empty-transcript",
            if app.error.is_some() {
                "Messages unavailable. Retry."
            } else if app.transcript.cursor.is_some() {
                if app.transcript.snapshot_bytes == 0 {
                    "No messages yet."
                } else {
                    "Messages aren't on this phone anymore. Open Details to reload."
                }
            } else {
                "Loading messages…"
            },
            TextRole::Body,
        ));
    }
    rows.push(text("timeline-end", "", TextRole::Status));
    nodes.push(Node {
        key: "timeline".into(),
        style: Style::default(),
        element: Element::List {
            label: "Conversation transcript".into(),
            children: rows,
        },
    });
    if app.details {
        nodes.push(row(
            "transcript-actions",
            vec![
                button("refresh", "Refresh", Intent::Refresh, app.code.is_some()),
                button(
                    "reload",
                    "Reload history",
                    Intent::Reload,
                    app.code.is_some(),
                ),
            ],
        ));
    }
    Ok(nodes)
}

fn record(
    app: &App,
    group: &[&RecordChunk],
    keys: &[String],
    page_index: usize,
) -> Result<Node<Intent>, String> {
    let chunk = group.first().ok_or("cached record group is empty")?;
    let last = group.last().ok_or("cached record group is empty")?;
    let prefix = format!("event-{}", chunk.offset);
    let show_raw = app.raw.contains(&chunk.offset);
    let mut children = Vec::new();
    if chunk.offset > chunk.record_offset {
        children.push(text(
            format!("{prefix}-continued"),
            "Continued from the previous page.",
            TextRole::Status,
        ));
    }
    let complete = assembled(app, chunk, keys, page_index)?;
    let full = complete
        .as_deref()
        .and_then(coder_history::readable_record_full);
    let readable = full
        .as_ref()
        .or_else(|| group.iter().rev().find_map(|item| item.readable.as_ref()));
    if let Some(readable) = readable {
        let role = readable.role.as_deref().unwrap_or(&readable.kind);
        let time = readable.timestamp.as_deref().unwrap_or("");
        let name = readable.tool_name.as_deref().unwrap_or("");
        children.push(text(
            format!("{prefix}-label"),
            if app.details {
                format!("{role} {name} · {time}")
            } else {
                format!("{role} {name}").trim().to_owned()
            },
            TextRole::Status,
        ));
        if !show_raw {
            let role = if matches!(readable.role.as_deref(), Some("user" | "assistant")) {
                TextRole::Markdown
            } else {
                TextRole::Code
            };
            let parts = text_slices(&readable.text);
            let part = app
                .text_parts
                .get(&chunk.record_offset)
                .copied()
                .unwrap_or(0)
                .min(parts.len().saturating_sub(1));
            chunks(
                &mut children,
                &prefix,
                parts.get(part).copied().unwrap_or(""),
                role,
            );
            if parts.len() > 1 {
                children.push(text(
                    format!("{prefix}-part"),
                    format!("Message part {} of {}", part + 1, parts.len()),
                    TextRole::Status,
                ));
                children.push(row(
                    &format!("{prefix}-parts"),
                    vec![
                        button(
                            format!("{prefix}-prev"),
                            "Previous part",
                            Intent::TextPart {
                                record: chunk.record_offset,
                                part: part.saturating_sub(1),
                            },
                            part > 0,
                        ),
                        button(
                            format!("{prefix}-next"),
                            "Next part",
                            Intent::TextPart {
                                record: chunk.record_offset,
                                part: part + 1,
                            },
                            part + 1 < parts.len(),
                        ),
                    ],
                ));
            }
            if readable.text_truncated {
                children.push(text(
                    format!("{prefix}-cut"),
                    "Preview shortened. Open Details for the full message.",
                    TextRole::Status,
                ));
            }
        }
        if app.details || show_raw {
            children.push(button(
                format!("{prefix}-raw"),
                if show_raw {
                    "Show readable text"
                } else {
                    "Show raw text"
                },
                Intent::Raw {
                    record: chunk.offset,
                },
                true,
            ));
        }
    }
    if show_raw || readable.is_none() {
        if readable.is_none() {
            children.push(text(
                format!("{prefix}-label"),
                format!(
                    "Message {} · bytes {}–{}{}",
                    chunk.index,
                    chunk.offset,
                    last.end_offset,
                    if last.complete { "" } else { " · continues" }
                ),
                TextRole::Status,
            ));
        }
        for item in group {
            let raw = base64::engine::general_purpose::STANDARD
                .decode(&item.raw_base64)
                .map_err(|_| "cached record bytes are invalid")?;
            let part_prefix = format!("{prefix}-source-{}", item.offset);
            children.push(text(
                format!("{part_prefix}-range"),
                format!("Bytes {}–{}", item.offset, item.end_offset),
                TextRole::Status,
            ));
            match std::str::from_utf8(&raw) {
                Ok(value) => chunks(&mut children, &part_prefix, value, TextRole::Code),
                Err(_) => {
                    children.push(text(
                        format!("{part_prefix}-encoding"),
                        "Binary fragment · base64",
                        TextRole::Status,
                    ));
                    chunks(
                        &mut children,
                        &format!("{part_prefix}-base64"),
                        &item.raw_base64,
                        TextRole::Code,
                    );
                }
            }
        }
    }
    Ok(stack(prefix, children))
}

fn text_slices(value: &str) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut rest = value;
    while !rest.is_empty() {
        let mut end = rest.len().min(8192);
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        pieces.push(&rest[..end]);
        rest = &rest[end..];
    }
    pieces
}

/// Reassemble at most one bounded record from neighboring authenticated cache
/// pages. A hole, incomplete write, or larger record falls back to exact chunks.
fn assembled(
    app: &App,
    chunk: &RecordChunk,
    keys: &[String],
    index: usize,
) -> Result<Option<Vec<u8>>, String> {
    if chunk.complete && chunk.offset == chunk.record_offset {
        return base64::engine::general_purpose::STANDARD
            .decode(&chunk.raw_base64)
            .map(Some)
            .map_err(|_| "invalid cached bytes".into());
    }
    let mut bytes = Vec::new();
    let mut next = chunk.record_offset;
    let start = keys
        .partition_point(|key| {
            key.rsplit('_')
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .is_some_and(|offset| offset <= chunk.record_offset)
        })
        .saturating_sub(1);
    if index.saturating_sub(start) > 32 {
        return Ok(None);
    }
    for key in keys.iter().skip(start).take(34) {
        let Some(page) = app.cache.read::<TranscriptPage>(key)? else {
            return Ok(None);
        };
        for item in page
            .chunks
            .iter()
            .filter(|c| c.record_offset == chunk.record_offset)
        {
            if item.offset != next {
                return Ok(None);
            }
            let raw = base64::engine::general_purpose::STANDARD
                .decode(&item.raw_base64)
                .map_err(|_| "invalid cached bytes")?;
            if bytes.len() + raw.len() > coder_history::MAX_READABLE_RECORD_BYTES {
                return Ok(None);
            }
            bytes.extend(raw);
            next = item.end_offset;
            if item.complete {
                return Ok(Some(bytes));
            }
        }
        if page.next.offset > chunk.record_offset && page.next.record_offset > chunk.record_offset {
            return Ok(None);
        }
    }
    Ok(None)
}

fn chunks(nodes: &mut Vec<Node<Intent>>, prefix: &str, value: &str, role: TextRole) {
    let mut rest = value;
    let mut index = 0;
    loop {
        let mut end = rest.len().min(8192);
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        nodes.push(text(format!("{prefix}-text-{index}"), &rest[..end], role));
        if end == rest.len() {
            break;
        }
        rest = &rest[end..];
        index += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;
    use coder_history::{CatalogRequest, Config as HistoryConfig, History, TranscriptRequest};

    fn fixture(root: &std::path::Path, text_bytes: usize) -> (App, Vec<u8>) {
        let source_root = root.join("history");
        std::fs::create_dir_all(source_root.join("sessions")).unwrap();
        let mut raw = serde_json::to_vec(&serde_json::json!({
            "type": "response_item",
            "payload": {"type": "message", "role": "assistant", "content": [
                {"type": "output_text", "text": "x".repeat(text_bytes)}
            ]}
        }))
        .unwrap();
        raw.push(b'\n');
        std::fs::write(source_root.join("sessions/record.jsonl"), &raw).unwrap();
        let history = History::open(HistoryConfig {
            codex: Some(source_root),
            claude: None,
            coder: None,
            opencode: None,
            devin: None,
        })
        .unwrap();
        let mut app = App::new(Config {
            cache_dir: root.join("cache"),
            secret_hex: "01".repeat(32),
            synthetic: false,
            loopback_test: false,
            push: None,
        })
        .unwrap();
        app.selected = history
            .catalog(CatalogRequest::default())
            .unwrap()
            .entries
            .into_iter()
            .next();
        loop {
            let page = history
                .transcript(TranscriptRequest {
                    source_id: app.source().unwrap(),
                    cursor: app.transcript.cursor.clone(),
                    max_bytes: coder_history::MAX_PAGE_BYTES,
                    end: None,
                })
                .unwrap();
            let more = page.has_more;
            app.apply_page(page).unwrap();
            if !more {
                break;
            }
        }
        (app, raw)
    }

    fn texts<'a>(node: &'a Node<Intent>, out: &mut Vec<(&'a str, &'a str)>) {
        match &node.element {
            Element::Stack { children, .. } | Element::List { children, .. } => {
                for node in children {
                    texts(node, out);
                }
            }
            Element::Text { value, .. } => out.push((&node.key, value)),
            Element::Button { label, .. } => out.push((&node.key, label)),
            Element::Surface { label, .. } => out.push((&node.key, label)),
            // The reader does not draw conversation elements.
            _ => {}
        }
    }

    #[test]
    fn one_readable_row_per_record_keeps_every_raw_fragment_available() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, raw) = fixture(dir.path(), 25_000);
        app.details = true;
        let view = root(&app).unwrap();
        let mut values = vec![];
        texts(&view, &mut values);
        assert_eq!(
            values
                .iter()
                .filter(|(_, value)| *value == "Show raw text")
                .count(),
            1
        );
        assert!(
            !values
                .iter()
                .any(|(key, _)| key.starts_with("event-0-source-"))
        );
        app.raw.insert(0);
        let raw_view = root(&app).unwrap();
        let mut values = vec![];
        texts(&raw_view, &mut values);
        let reconstructed: String = values
            .into_iter()
            .filter(|(key, _)| key.starts_with("event-0-source-") && key.contains("-text-"))
            .map(|(_, value)| value)
            .collect();
        assert_eq!(reconstructed.as_bytes(), raw);
    }

    #[test]
    fn an_evicted_middle_page_reports_the_internal_history_gap() {
        let dir = tempfile::tempdir().unwrap();
        let (app, _) = fixture(dir.path(), 90_000);
        let keys = app.page_keys().unwrap();
        assert_eq!(keys.len(), 3);
        app.cache.erase(&keys[1]).unwrap();
        let view = root(&app).unwrap();
        let mut values = vec![];
        texts(&view, &mut values);
        assert!(
            values
                .iter()
                .any(|(key, value)| *key == "cache-internal-gap"
                    && value.contains("on this phone anymore"))
        );
    }

    #[test]
    fn an_empty_transcript_reports_its_completed_read_instead_of_loading_forever() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("history");
        std::fs::create_dir_all(source.join("sessions")).unwrap();
        std::fs::write(source.join("sessions/empty.jsonl"), b"").unwrap();
        let history = History::open(HistoryConfig {
            codex: Some(source),
            claude: None,
            coder: None,
            opencode: None,
            devin: None,
        })
        .unwrap();
        let mut app = App::new(Config {
            cache_dir: dir.path().join("cache"),
            secret_hex: "01".repeat(32),
            synthetic: false,
            loopback_test: false,
            push: None,
        })
        .unwrap();
        app.selected = history
            .catalog(CatalogRequest::default())
            .unwrap()
            .entries
            .into_iter()
            .next();
        let empty_text = |app: &App| {
            let view = root(app).unwrap();
            let mut values = vec![];
            texts(&view, &mut values);
            values
                .into_iter()
                .find(|(key, _)| *key == "empty-transcript")
                .unwrap()
                .1
                .to_owned()
        };
        assert_eq!(empty_text(&app), "Loading messages…");
        app.apply_page(
            history
                .transcript(TranscriptRequest {
                    source_id: app.source().unwrap(),
                    cursor: None,
                    max_bytes: coder_history::MAX_PAGE_BYTES,
                    end: None,
                })
                .unwrap(),
        )
        .unwrap();
        assert!(app.page_keys().unwrap().is_empty());
        assert_eq!(empty_text(&app), "No messages yet.");
        app.error = Some("Computer unavailable.".into());
        assert_eq!(empty_text(&app), "Messages unavailable. Retry.");
    }

    #[test]
    fn an_evicted_transcript_reports_its_missing_cache_instead_of_loading() {
        let dir = tempfile::tempdir().unwrap();
        let (app, _) = fixture(dir.path(), 100);
        for key in app.page_keys().unwrap() {
            app.cache.erase(&key).unwrap();
        }
        let view = root(&app).unwrap();
        let mut values = vec![];
        texts(&view, &mut values);
        assert!(values.iter().any(|(key, value)| {
            *key == "empty-transcript"
                && *value == "Messages aren't on this phone anymore. Open Details to reload."
        }));
    }
}

#[cfg(test)]
mod presentation_tests {
    use super::*;
    use crate::app::{Config, Request};

    #[test]
    fn transcript_prioritizes_messages_and_keeps_source_details_available() {
        let directory = tempfile::tempdir().unwrap();
        let mut app = App::new(Config {
            cache_dir: directory.path().into(),
            secret_hex: "01".repeat(32),
            synthetic: true,
            loopback_test: false,
            push: None,
        })
        .unwrap();
        let catalog = app.call(Request::Snapshot).view.unwrap();
        let opened = app
            .call(Request::Activate {
                instance: catalog["instance"].as_str().unwrap().into(),
                revision: catalog["revision"].as_u64().unwrap(),
                node: "chat-0".into(),
            })
            .view
            .unwrap();
        let readable = opened.to_string();
        assert!(readable.contains("Native timeline"));
        assert!(!readable.contains("history-progress"));
        assert!(!readable.contains("page-position"));
        assert!(!readable.contains("Show raw text"));
        let detailed = app
            .call(Request::Activate {
                instance: opened["instance"].as_str().unwrap().into(),
                revision: opened["revision"].as_u64().unwrap(),
                node: "details".into(),
            })
            .view
            .unwrap()
            .to_string();
        assert!(detailed.contains("Native timeline"));
        assert!(detailed.contains("history-progress"));
        assert!(detailed.contains("page-position"));
        assert!(detailed.contains("Show raw text"));
    }
}
