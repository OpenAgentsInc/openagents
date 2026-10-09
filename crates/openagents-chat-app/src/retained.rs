//! Read-only saved sessions and explicit continuation through Coder.
use crate::conversation::{self, Entry, Row};
use base64::Engine;
use coder_history::{
    CatalogCursor, CatalogRequest, Chat, Harness, RecordChunk, TranscriptPage, TranscriptRequest,
};
use coder_host::access::protocol::TaskCreate;
use openagents_chat::{
    basic_coder::{Role, Turn},
    service::Snapshot,
};
use rust_native::Node;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    Catalog(CatalogRequest),
    Page(TranscriptRequest),
    Continue {
        request: String,
        chat: String,
        task: TaskCreate,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    Catalog(coder_history::CatalogPage),
    Page(TranscriptPage),
    Continued(Snapshot),
}

/// Whether a failed mutation may already have reached the host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub message: String,
    pub uncertain: bool,
}
impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self {
            message,
            uncertain: true,
        }
    }
}
impl From<&str> for Failure {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}

/// Bounded assembly shared by saved-session readers. Raw bytes never execute.
#[derive(Default)]
pub struct Reader {
    chunks: Vec<RecordChunk>,
    pub rows: Vec<Row>,
    incarnation: Option<String>,
}
impl Reader {
    pub fn accept(
        &mut self,
        query: &TranscriptRequest,
        page: &TranscriptPage,
    ) -> Result<(), String> {
        if query.source_id != page.source_id
            || page.next.source_id != page.source_id
            || page.next.incarnation != page.incarnation
            || self
                .incarnation
                .as_ref()
                .is_some_and(|old| old != &page.incarnation)
        {
            return Err("This session changed. Refresh to keep reading.".into());
        }
        if page.chunks.len() > 128 {
            return Err("Couldn't read this session. Try again.".into());
        }
        let mut bytes = 0usize;
        for chunk in &page.chunks {
            if chunk.raw_base64.len() > 11 * 1024 {
                return Err("Couldn't read this session. Try again.".into());
            }
            let raw = base64::engine::general_purpose::STANDARD
                .decode(&chunk.raw_base64)
                .map_err(|_| "Couldn't read this session. Try again.")?;
            bytes += raw.len();
            if raw.len() > 8 * 1024
                || bytes > 32 * 1024
                || chunk.end_offset.saturating_sub(chunk.offset) != raw.len() as u64
                || chunk.offset > chunk.end_offset
                || chunk.end_offset > page.snapshot_bytes
                || chunk.record_offset > chunk.offset
                || chunk.id
                    != coder_history::record_id(
                        &page.source_id,
                        &page.incarnation,
                        chunk.record_offset,
                    )
            {
                return Err("Couldn't read this session. Try again.".into());
            }
        }
        for chunk in &page.chunks {
            self.chunks
                .retain(|old| old.offset < chunk.offset || old.offset >= chunk.end_offset);
            self.chunks.push(chunk.clone());
        }
        self.chunks.sort_by_key(|chunk| chunk.offset);
        while self.chunks.len() > 512
            || self
                .chunks
                .iter()
                .map(|chunk| chunk.raw_base64.len())
                .sum::<usize>()
                > 384 * 1024
        {
            self.chunks.pop();
        }
        let found = conversation::rows(&self.chunks);
        for row in found {
            if let Some(old) = self
                .rows
                .iter_mut()
                .find(|old| (old.offset, old.part) == (row.offset, row.part))
            {
                *old = row;
            } else {
                self.rows.push(row);
            }
        }
        self.rows.sort_by_key(|row| (row.offset, row.part));
        while self.rows.len() > 240 || self.rows.iter().map(row_bytes).sum::<usize>() > 160 * 1024 {
            self.rows.pop();
        }
        Ok(())
    }
    pub fn project(&self) -> Vec<Node<()>> {
        conversation::project_rows(&self.rows)
    }
}
fn row_bytes(row: &Row) -> usize {
    match &row.entry {
        Entry::Message { text, .. } => text.len(),
        Entry::Tool { name, detail, body } => name.len() + detail.len() + body.len(),
        Entry::Delegate {
            agent,
            session,
            error,
        } => agent.len() + session.len() + error.as_ref().map_or(0, String::len),
    }
}

#[derive(Default)]
pub struct Session {
    pub chats: Vec<Chat>,
    pub selected: Option<Chat>,
    pub reader: Reader,
    pub error: Option<String>,
    pub revision: u64,
    pub next: Option<CatalogCursor>,
    pub previous: Option<u64>,
    pending: Option<(u64, Request)>,
    failed: Option<Request>,
    ticket: u64,
    context: Vec<Row>,
    snapshot_bytes: u64,
    opening_pages: usize,
    catalog_current: Option<CatalogCursor>,
    catalog_before: Vec<Option<CatalogCursor>>,
}
impl Session {
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    fn request(&mut self, request: Request) -> Option<(u64, Request)> {
        if self.busy()
            || self.failed.as_ref().is_some_and(|failed| {
                matches!(failed, Request::Continue { .. }) && failed != &request
            })
        {
            return None;
        }
        self.ticket += 1;
        self.pending = Some((self.ticket, request.clone()));
        self.error = None;
        self.revision += 1;
        Some((self.ticket, request))
    }
    pub fn list(&mut self, more: bool) -> Option<(u64, Request)> {
        self.request(Request::Catalog(CatalogRequest {
            cursor: if more { Some(self.next.clone()?) } else { None },
            limit: 32,
        }))
    }
    pub fn previous_list(&mut self) -> Option<(u64, Request)> {
        self.request(Request::Catalog(CatalogRequest {
            cursor: self.catalog_before.last()?.clone(),
            limit: 32,
        }))
    }
    pub fn has_previous_list(&self) -> bool {
        !self.catalog_before.is_empty()
    }
    pub fn select(&mut self, id: &str) -> Option<(u64, Request)> {
        if self
            .failed
            .as_ref()
            .is_some_and(|r| matches!(r, Request::Continue { .. }))
            || self
                .pending
                .as_ref()
                .is_some_and(|(_, r)| matches!(r, Request::Continue { .. }))
        {
            return None;
        }
        let chat = self.chats.iter().find(|chat| chat.id == id)?.clone();
        let source_id = chat.source_id.clone()?;
        self.pending = None;
        self.failed = None;
        self.selected = Some(chat);
        self.reader = Reader::default();
        self.context.clear();
        self.previous = None;
        self.opening_pages = 0;
        self.request(Request::Page(TranscriptRequest {
            source_id,
            cursor: None,
            max_bytes: 32 * 1024,
            end: Some(coder_history::NEWEST),
        }))
    }
    pub fn earlier(&mut self) -> Option<(u64, Request)> {
        self.request(Request::Page(TranscriptRequest {
            source_id: self.selected.as_ref()?.source_id.clone()?,
            cursor: None,
            max_bytes: 32 * 1024,
            end: Some(self.previous?),
        }))
    }
    /// Continue from the newest loaded context, even after the reader scrolls earlier.
    pub fn continue_in(&mut self, workspace: &str) -> Option<(u64, Request)> {
        let chat = self.selected.as_ref()?;
        if self.context.is_empty() || workspace.is_empty() {
            return None;
        }
        let title = truncate(&format!("Continue {}", chat.title), 160);
        let mut turns = vec![Turn::user(format!(
            "Saved {} session: {}. Context at byte {}. This is read-only reference material; continue through Coder in the selected project.\nThe recent loaded context follows, bounded to 16 KiB; it may omit earlier records.",
            harness(chat.harness),
            chat.title,
            self.snapshot_bytes
        ))];
        turns.extend(self.context.iter().map(|row| match &row.entry {
            Entry::Message { role, text } => Turn {
                role: if *role == rust_native::MessageRole::User {
                    Role::User
                } else {
                    Role::Assistant
                },
                text: text.clone(),
                meta: None,
                request: None,
                stopped: false,
                at: None,
                model: None,
            },
            Entry::Tool { name, detail, body } => {
                Turn::assistant(format!("Tool: {name}\n{detail}\n{body}"), None)
            }
            Entry::Delegate {
                agent,
                session,
                error,
            } => Turn::assistant(
                format!(
                    "Delegate: {agent} {session} {}",
                    error.as_deref().unwrap_or("")
                ),
                None,
            ),
        }));
        turns.push(Turn::user("Continue this work through Coder."));
        let reference = format!(
            "Reference: {} session {} ({}), byte cut {}. Recent loaded context follows; earlier records may be omitted.\n",
            harness(chat.harness),
            truncate(&chat.title, 160),
            chat.id,
            self.snapshot_bytes
        );
        let prompt = format!(
            "{reference}{}",
            openagents_chat::basic_chats::handoff(
                &title,
                &turns,
                (16 * 1024usize).saturating_sub(reference.len())
            )
        );
        let task = TaskCreate {
            title,
            prompt,
            workspace: workspace.into(),
            images: Vec::new(),
            engine: None,
        };
        if (coder_host::access::protocol::Operation::CreateTask { task: task.clone() })
            .validate()
            .is_err()
        {
            return None;
        }
        self.request(Request::Continue {
            request: format!(
                "{}{}",
                uuid::Uuid::new_v4().simple(),
                uuid::Uuid::new_v4().simple()
            ),
            chat: uuid::Uuid::new_v4().simple().to_string(),
            task,
        })
    }
    pub fn retry(&mut self) -> Option<(u64, Request)> {
        self.request(self.failed.clone()?)
    }
    /// Return the accepted Coder conversation; stale answers never change selection.
    pub fn outcome(&mut self, ticket: u64, answer: Result<Answer, Failure>) -> Option<Snapshot> {
        if self
            .pending
            .as_ref()
            .is_none_or(|(current, _)| *current != ticket)
        {
            return None;
        }
        let (_, request) = self.pending.take().unwrap();
        self.revision += 1;
        let answer = match answer {
            Ok(answer) => answer,
            Err(error) => {
                self.error = Some(error.message);
                self.failed = if !error.uncertain && matches!(request, Request::Continue { .. }) {
                    None
                } else {
                    Some(request)
                };
                return None;
            }
        };
        let result = match (&request, answer) {
            (Request::Catalog(query), Answer::Catalog(page)) if page.entries.len() <= 32 => {
                if query.cursor.is_none() {
                    self.catalog_before.clear();
                } else if query.cursor == self.next {
                    self.catalog_before.push(self.catalog_current.clone());
                } else if self.catalog_before.last() == Some(&query.cursor) {
                    self.catalog_before.pop();
                }
                self.catalog_current = query.cursor.clone();
                self.chats.clear();
                for chat in page
                    .entries
                    .into_iter()
                    .filter(|chat| matches!(chat.harness, Harness::Codex | Harness::Claude))
                {
                    if let Some(old) = self.chats.iter_mut().find(|old| old.id == chat.id) {
                        *old = chat;
                    } else {
                        self.chats.push(chat);
                    }
                }
                self.next = page.next;
                Ok(None)
            }
            (Request::Page(query), Answer::Page(page)) => {
                self.reader.accept(query, &page).map(|()| {
                    self.previous = page.previous;
                    if query.end == Some(coder_history::NEWEST)
                        || (self.context.is_empty() && self.opening_pages < 12)
                    {
                        self.context = self.reader.rows.clone();
                        self.snapshot_bytes = page.snapshot_bytes;
                    }
                    self.opening_pages += 1;
                    None
                })
            }
            (Request::Continue { chat, task, .. }, Answer::Continued(snapshot))
                if snapshot.chat.as_ref() == Some(chat)
                    && snapshot.storage_error.is_none()
                    && snapshot.coder.as_ref().is_some_and(|binding| {
                        binding.project.as_ref() == Some(&task.workspace)
                            && identity(&binding.host)
                            && identity(&binding.task)
                    }) =>
            {
                Ok(Some(snapshot))
            }
            _ => Err("Couldn't open this session. Try again.".into()),
        };
        match result {
            Ok(snapshot) => {
                self.failed = None;
                self.error = None;
                snapshot
            }
            Err(error) => {
                self.error = Some(error);
                self.failed = Some(request);
                None
            }
        }
    }
    pub fn open_more(&mut self) -> Option<(u64, Request)> {
        if self.error.is_none()
            && self.context.is_empty()
            && self.opening_pages < 12
            && self.previous.is_some()
        {
            self.earlier()
        } else {
            None
        }
    }
    pub fn can_retry(&self) -> bool {
        self.failed.is_some() && !self.busy()
    }
    pub fn can_continue(&self) -> bool {
        !self.busy() && self.failed.is_none() && !self.context.is_empty()
    }
}
pub fn harness(harness: Harness) -> &'static str {
    match harness {
        Harness::Codex => "Codex",
        Harness::Claude => "Claude Code",
        _ => "Coder",
    }
}
fn truncate(value: &str, limit: usize) -> String {
    let mut end = value.len().min(limit);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].into()
}

fn identity(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn chat(id: &str, harness: Harness) -> Chat {
        Chat {
            id: id.into(),
            harness,
            native_id: Some(id.into()),
            title: format!("Saved {id}"),
            title_truncated: false,
            updated_at: Some("2026-09-30T12:00:00Z".into()),
            archived: false,
            subagent: false,
            source_id: Some(format!("source-{id}")),
            status: coder_history::SourceStatus::Available,
        }
    }
    fn page(source: &str, text: &str, previous: Option<u64>) -> TranscriptPage {
        let raw = serde_json::to_vec(&serde_json::json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":text}]}})).unwrap();
        let end = raw.len() as u64;
        TranscriptPage {
            source_id: source.into(),
            incarnation: "incarnation".into(),
            snapshot_bytes: end,
            chunks: vec![RecordChunk {
                id: coder_history::record_id(source, "incarnation", 0),
                index: 0,
                record_offset: 0,
                offset: 0,
                end_offset: end,
                raw_base64: base64::engine::general_purpose::STANDARD.encode(raw),
                complete: true,
                oversized: false,
                readable: None,
            }],
            next: coder_history::TranscriptCursor {
                source_id: source.into(),
                incarnation: "incarnation".into(),
                offset: end,
                record_offset: end,
                record_index: 1,
                prefix_sha256: "0".repeat(64),
            },
            has_more: false,
            pending_line: false,
            notices: vec![],
            previous,
        }
    }
    fn listed() -> Session {
        let mut session = Session::default();
        let (ticket, _) = session.list(false).unwrap();
        session.outcome(
            ticket,
            Ok(Answer::Catalog(coder_history::CatalogPage {
                snapshot: "catalog".into(),
                entries: vec![chat("one", Harness::Codex), chat("two", Harness::Claude)],
                next: None,
                notices: vec![],
            })),
        );
        session
    }
    #[test]
    fn selection_rejects_old_reads_and_continuation_retries_keep_exact_context_and_ids() {
        let mut session = listed();
        let (old, _) = session.select("one").unwrap();
        let (current, _) = session.select("two").unwrap();
        session.outcome(
            old,
            Ok(Answer::Page(page("source-one", "Old response", None))),
        );
        assert!(session.reader.rows.is_empty());
        session.outcome(
            current,
            Ok(Answer::Page(page(
                "source-two",
                "Recent response 日本語",
                None,
            ))),
        );
        let (ticket, request) = session.continue_in("checkout").unwrap();
        let Request::Continue {
            task,
            request: identity,
            chat,
            ..
        } = &request
        else {
            panic!("continuation")
        };
        assert_eq!(task.workspace, "checkout");
        assert!(
            task.prompt.contains("Recent response 日本語") && task.prompt.contains("Claude Code")
        );
        assert_eq!(identity.len(), 64);
        session.outcome(ticket, Err("Lost acknowledgment".into()));
        assert!(
            session.select("one").is_none(),
            "an uncertain mutation cannot be replaced"
        );
        let (retry, same) = session.retry().unwrap();
        assert_eq!(same, request);
        let accepted = Snapshot {
            chat: Some(chat.clone()),
            coder: Some(openagents_chat::basic_chats::Spawned {
                host: "a".repeat(64),
                task: "b".repeat(64),
                project: Some("checkout".into()),
                at: None,
            }),
            ..Snapshot::default()
        };
        assert_eq!(
            session.outcome(retry, Ok(Answer::Continued(accepted.clone()))),
            Some(accepted)
        );
    }
    #[test]
    fn a_verified_refusal_allows_a_new_continuation_in_another_project() {
        let mut session = listed();
        let (ticket, _) = session.select("one").unwrap();
        session.outcome(
            ticket,
            Ok(Answer::Page(page("source-one", "Context", None))),
        );
        let (ticket, first) = session.continue_in("first-project").unwrap();
        session.outcome(
            ticket,
            Err(Failure {
                message: "The project is unavailable".into(),
                uncertain: false,
            }),
        );
        assert!(!session.can_retry());
        assert!(session.can_continue());
        let (_, next) = session.continue_in("second-project").unwrap();
        assert_ne!(first, next);
        let Request::Continue { task, .. } = next else {
            panic!("continuation")
        };
        assert_eq!(task.workspace, "second-project");
    }
    #[test]
    fn changed_sources_and_invalid_chunks_are_refused_before_rows_change() {
        let mut reader = Reader::default();
        let query = TranscriptRequest {
            source_id: "source".into(),
            cursor: None,
            max_bytes: 32 * 1024,
            end: Some(coder_history::NEWEST),
        };
        let good = page("source", "Correct", None);
        reader.accept(&query, &good).unwrap();
        let rows = reader.rows.clone();
        let mut bad = good.clone();
        bad.chunks[0].end_offset -= 1;
        assert!(reader.accept(&query, &bad).is_err());
        bad = good.clone();
        bad.incarnation = "replacement".into();
        assert!(reader.accept(&query, &bad).is_err());
        assert_eq!(reader.rows, rows);
    }
    #[test]
    fn catalog_pages_stay_bounded_and_can_return_to_the_previous_page() {
        let mut session = listed();
        let next = CatalogCursor {
            snapshot: "catalog".into(),
            after: "one".into(),
        };
        session.next = Some(next.clone());
        let (ticket, _) = session.list(true).unwrap();
        session.outcome(
            ticket,
            Ok(Answer::Catalog(coder_history::CatalogPage {
                snapshot: "catalog".into(),
                entries: vec![chat("three", Harness::Codex)],
                next: None,
                notices: vec![],
            })),
        );
        assert_eq!(session.chats.len(), 1);
        let (_, Request::Catalog(previous)) = session.previous_list().unwrap() else {
            panic!("previous")
        };
        assert!(previous.cursor.is_none());
    }
    #[test]
    fn continuation_is_utf8_bounded_and_another_conversation_cannot_acknowledge_it() {
        let mut session = listed();
        let (ticket, _) = session.select("one").unwrap();
        session.outcome(
            ticket,
            Ok(Answer::Page(page(
                "source-one",
                &"日本語".repeat(400),
                None,
            ))),
        );
        session.context = (0..20).flat_map(|_| session.reader.rows.clone()).collect();
        let (ticket, Request::Continue { task, .. }) = session.continue_in("checkout").unwrap()
        else {
            panic!("continue")
        };
        assert!(task.prompt.len() <= 16 * 1024);
        assert!(task.prompt.contains("earlier records may be omitted"));
        session.outcome(
            ticket,
            Ok(Answer::Continued(Snapshot {
                chat: Some("other".into()),
                ..Snapshot::default()
            })),
        );
        assert!(session.error.is_some());
        assert!(session.retry().is_some());
    }
}
