use crate::cache::Cache;
use coder_connect::{Client, ConnectionCode, ErrorCode, Observation, Query, RelayPolicy};
use coder_history::{
    CatalogCursor, CatalogPage, CatalogRequest, Chat, TranscriptCursor, TranscriptPage,
    TranscriptRequest,
};
use rust_native::{Activation, ValidatedView, View};
use secp256k1::{Keypair, Secp256k1, SecretKey};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    str::FromStr,
};

pub(crate) const CATALOG_WINDOW: usize = 24;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub cache_dir: PathBuf,
    pub secret_hex: String,
    #[serde(default)]
    pub synthetic: bool,
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Snapshot,
    Connect {
        code: String,
    },
    Activate {
        instance: String,
        revision: u64,
        node: String,
    },
    Refresh,
    RefreshNow,
    Foreground {
        active: bool,
    },
    Disconnect,
    Follow {
        enabled: bool,
        page: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Intent {
    Open { source: String },
    Back,
    Earlier,
    Later,
    Follow,
    Refresh,
    MoreChats,
    PreviousChats,
    NextChats,
    Reload,
    Raw { record: u64 },
    TextPart { record: u64, part: usize },
    Disconnect,
}

#[derive(Serialize)]
pub struct Packet {
    pub schema: &'static str,
    pub public_key: String,
    pub paired: bool,
    pub reading: bool,
    pub status: String,
    pub error: Option<String>,
    pub follow_target: Option<String>,
    pub follow_page: Option<String>,
    pub view: Option<serde_json::Value>,
}

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct CatalogState {
    pub snapshot: String,
    pub next: Option<CatalogCursor>,
    pub pages: u32,
    pub checked_at: u64,
    #[serde(default)]
    pub refresh_next: Option<CatalogCursor>,
    #[serde(default)]
    pub refresh_page: u32,
}

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct TranscriptState {
    pub cursor: Option<TranscriptCursor>,
    pub snapshot_bytes: u64,
    pub has_more: bool,
    pub pending_line: bool,
    pub checked_at: u64,
}

pub struct App {
    pub(crate) cache: Cache,
    pub(crate) secret: SecretKey,
    pub(crate) public_key: String,
    pub(crate) code: Option<ConnectionCode>,
    client: Option<Client>,
    runtime: tokio::runtime::Runtime,
    pub(crate) catalog: Vec<Chat>,
    pub(crate) catalog_state: CatalogState,
    pub(crate) catalog_start: usize,
    pub(crate) selected: Option<Chat>,
    pub(crate) transcript: TranscriptState,
    /// None follows the latest cached page. Other values select a stable page.
    pub(crate) window: Option<String>,
    pub(crate) raw: BTreeSet<u64>,
    pub(crate) text_parts: BTreeMap<u64, usize>,
    retry_after: u64,
    pub(crate) status: String,
    pub(crate) error: Option<String>,
    pub(crate) notices: Vec<String>,
    pub(crate) instance: String,
    revision: u64,
    current: Option<ValidatedView<Intent>>,
    active: bool,
    pub(crate) synthetic: bool,
}

impl App {
    pub fn new(config: Config) -> Result<Self, String> {
        let secret =
            SecretKey::from_str(&config.secret_hex).map_err(|_| "invalid device identity")?;
        let public_key = Keypair::from_secret_key(&Secp256k1::new(), &secret)
            .x_only_public_key()
            .0
            .to_string();
        let cache = Cache::open(&config.cache_dir, &secret)?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|_| "mobile runtime unavailable")?;
        let mut app = Self {
            cache,
            secret,
            public_key,
            code: None,
            client: None,
            runtime,
            catalog: vec![],
            catalog_state: CatalogState::default(),
            catalog_start: 0,
            selected: None,
            transcript: TranscriptState::default(),
            window: None,
            raw: BTreeSet::new(),
            text_parts: BTreeMap::new(),
            retry_after: 0,
            status: "Not connected · read-only".into(),
            error: None,
            notices: vec![],
            instance: coder_connect::protocol::random_id(),
            revision: 0,
            current: None,
            active: true,
            synthetic: config.synthetic,
        };
        match app.cache.read::<ConnectionCode>("connection") {
            Ok(Some(code)) => match app.make_client(code.clone()) {
                Ok(client) => {
                    app.code = Some(code);
                    app.client = Some(client);
                    if let Err(e) = app.restore_catalog() {
                        app.error = Some(e);
                    }
                    app.status = "Cached · refresh to check the computer".into();
                }
                Err(_) => {
                    app.cache.erase("")?;
                    app.error = Some(
                        "Saved pairing expired or belongs to another device. Pair again.".into(),
                    );
                }
            },
            Ok(None) => {}
            Err(error) => {
                app.error = Some(error);
            }
        }
        if config.synthetic {
            app.seed_synthetic()?;
        }
        app.rebuild()?;
        Ok(app)
    }

    fn policy(&self) -> RelayPolicy {
        if self.synthetic {
            RelayPolicy::LoopbackTest
        } else {
            RelayPolicy::Production
        }
    }
    fn make_client(&self, code: ConnectionCode) -> coder_connect::Result<Client> {
        Client::new_with_policy(code, self.secret, self.policy())
    }

    pub fn call(&mut self, request: Request) -> Packet {
        // Lifecycle callbacks can follow an in-flight pairing call. They must
        // not erase its failure before the user can read it and retry.
        if !matches!(request, Request::Snapshot | Request::Foreground { .. }) {
            self.error = None;
        }
        let result = self.handle(request);
        if let Err(error) = result {
            self.error = Some(error);
        }
        if let Err(error) = self.rebuild() {
            self.error = Some(error);
            self.current = None;
        }
        self.packet()
    }

    fn handle(&mut self, request: Request) -> Result<(), String> {
        if !matches!(request, Request::Connect { .. } | Request::Disconnect)
            && self.code.as_ref().is_some_and(|c| c.expires_at <= now())
        {
            self.disconnect()?;
            return Err("Pairing expired. Pair again on the computer.".into());
        }
        match request {
            Request::Snapshot => Ok(()),
            Request::Follow { enabled, page } => {
                let keys = self.page_keys()?;
                self.window = if enabled {
                    None
                } else {
                    Some(
                        keys.into_iter()
                            .find(|key| Some(key) == page.as_ref())
                            .ok_or("The visible page is no longer cached.")?,
                    )
                };
                Ok(())
            }
            Request::Foreground { active } => {
                self.active = active;
                Ok(())
            }
            Request::Disconnect => self.disconnect(),
            Request::Connect { code } => {
                let text = code.trim();
                let code = if text.starts_with("coder-pair:") {
                    self.runtime
                        .block_on(coder_connect::pairing::redeem(
                            text,
                            &self.secret,
                            self.policy(),
                        ))
                        .map_err(|e| pairing_error(&e))?
                } else {
                    ConnectionCode::parse(text.as_bytes()).map_err(|_| {
                        "This is not a Coder pairing code. Scan the QR code shown by the computer, or paste its complete pairing string.".to_owned()
                    })?
                };
                let client = self.make_client(code.clone()).map_err(|e| e.to_string())?;
                self.disconnect()?;
                self.cache.write("connection", &code)?;
                self.code = Some(code);
                self.client = Some(client);
                self.status = "Paired · refresh to connect".into();
                // Pairing returns immediately; the foreground timer requests data.
                Ok(())
            }
            Request::Refresh => self.refresh(),
            Request::RefreshNow => self.force_refresh(),
            Request::Activate {
                instance,
                revision,
                node,
            } => {
                let intent = self
                    .current
                    .as_ref()
                    .ok_or("view is unavailable")?
                    .activate(&Activation {
                        instance,
                        revision,
                        node,
                    })
                    .map_err(|_| "Screen changed. Try the action again.")?
                    .clone();
                self.intent(intent)
            }
        }
    }

    fn intent(&mut self, intent: Intent) -> Result<(), String> {
        match intent {
            Intent::Open { source } => {
                let chat = self
                    .catalog
                    .iter()
                    .find(|c| c.source_id.as_deref() == Some(&source))
                    .cloned()
                    .ok_or("Chat is no longer in the current list.")?;
                self.transcript = self
                    .cache
                    .read(&format!("chat_{source}"))?
                    .unwrap_or_default();
                self.selected = Some(chat);
                self.window = None;
                self.raw.clear();
                self.text_parts.clear();
                self.retry_after = 0;
                self.status = "Cached transcript · checking for updates".into();
                Ok(())
            }
            Intent::Back => {
                self.selected = None;
                self.raw.clear();
                Ok(())
            }
            Intent::Earlier | Intent::Later => {
                let keys = self.page_keys()?;
                let index = self.window_index(&keys);
                let next = if matches!(intent, Intent::Earlier) {
                    index.saturating_sub(1)
                } else {
                    index.saturating_add(1).min(keys.len().saturating_sub(1))
                };
                self.window = keys.get(next).cloned();
                Ok(())
            }
            Intent::Follow => {
                self.window = None;
                Ok(())
            }
            Intent::Refresh => self.force_refresh(),
            Intent::MoreChats => self.refresh_catalog(false),
            Intent::PreviousChats => {
                self.catalog_start = self.catalog_start.saturating_sub(CATALOG_WINDOW);
                Ok(())
            }
            Intent::NextChats => {
                self.catalog_start = (self.catalog_start + CATALOG_WINDOW)
                    .min(self.catalog.len().saturating_sub(1) / CATALOG_WINDOW * CATALOG_WINDOW);
                Ok(())
            }
            Intent::Reload => {
                self.retry_after = 0;
                if let Some(source) = self.source() {
                    self.cache.erase(&format!("page_{source}_"))?;
                    self.cache.erase(&format!("chat_{source}"))?;
                    self.transcript = TranscriptState::default();
                    self.window = None;
                }
                Ok(())
            }
            Intent::TextPart { record, part } => {
                self.text_parts.insert(record, part);
                Ok(())
            }
            Intent::Raw { record } => {
                if !self.raw.remove(&record) {
                    self.raw.insert(record);
                }
                Ok(())
            }
            Intent::Disconnect => self.disconnect(),
        }
    }

    fn disconnect(&mut self) -> Result<(), String> {
        self.client = None;
        self.code = None;
        self.catalog.clear();
        self.catalog_state = CatalogState::default();
        self.catalog_start = 0;
        self.selected = None;
        self.transcript = TranscriptState::default();
        self.raw.clear();
        self.notices.clear();
        self.retry_after = 0;
        self.text_parts.clear();
        self.status = "Not connected · read-only".into();
        self.cache.erase("")
    }

    fn restore_catalog(&mut self) -> Result<(), String> {
        self.catalog_state = self.cache.read("catalog_state")?.unwrap_or_default();
        if self.catalog_state.pages > 128 {
            return Err("cached catalog exceeds its page bound".into());
        }
        for index in 0..self.catalog_state.pages {
            let key = format!("catalog_{}_{}", self.catalog_state.snapshot, index);
            if let Some(page) = self.cache.read::<CatalogPage>(&key)? {
                self.catalog.extend(page.entries);
            } else {
                self.notices
                    .push("Some cached chats were evicted. Refresh the list.".into());
            }
        }
        Ok(())
    }

    fn observe(&mut self, query: Query) -> Result<Observation, String> {
        let client = self
            .client
            .as_ref()
            .ok_or("Pair this phone with the computer first.")?;
        let outcome = self.runtime.block_on(async {
            tokio::time::timeout(std::time::Duration::from_secs(8), client.observe(query)).await
        });
        let outcome = outcome.unwrap_or_else(|_| {
            Err(coder_connect::Error::new(
                ErrorCode::Transport,
                "Computer refresh timed out. Cached history is still available.",
            ))
        });
        match outcome {
            Ok(result) => {
                self.retry_after = 0;
                self.status = "Connected · read-only".into();
                Ok(result)
            }
            Err(error) => {
                self.retry_after = now().saturating_add(if error.code == ErrorCode::RateLimited {
                    60
                } else {
                    15
                });
                match error.code {
                    ErrorCode::Revoked | ErrorCode::Expired | ErrorCode::Forbidden => {
                        self.disconnect()?;
                        Err("Access ended. Cached conversations were erased; pair again on the computer.".into())
                    }
                    ErrorCode::SourceChanged => {
                        self.status = "Source changed · reload required".into();
                        Err("The computer's history file changed. Reload this transcript to avoid mixing versions.".into())
                    }
                    ErrorCode::Conflict => {
                        self.catalog_state.next = None;
                        self.catalog_state.refresh_next = None;
                        self.status = "Chat list changed · refresh required".into();
                        Err("The chat list changed during paging. Refresh to start a new list snapshot.".into())
                    }
                    _ => {
                        self.status = "Offline or unavailable · showing cached history".into();
                        Err(error.to_string())
                    }
                }
            }
        }
    }

    fn force_refresh(&mut self) -> Result<(), String> {
        self.retry_after = 0;
        self.catalog_state.checked_at = 0;
        self.refresh()
    }

    fn refresh(&mut self) -> Result<(), String> {
        if !self.active || now() < self.retry_after {
            return Ok(());
        }
        if self.synthetic && self.client.is_none() {
            return Ok(());
        }
        if self.selected.is_some() {
            let start = std::time::Instant::now();
            for _ in 0..8 {
                self.refresh_transcript()?;
                if !self.transcript.has_more || start.elapsed() >= std::time::Duration::from_secs(2)
                {
                    break;
                }
            }
            Ok(())
        } else {
            // A catalog refresh starts from its first page after the prior snapshot
            // is complete. Intermediate pages retain their membership cursor.
            if self.catalog_state.next.is_none()
                && self.catalog_state.refresh_next.is_none()
                && now().saturating_sub(self.catalog_state.checked_at) < 30
            {
                return Ok(());
            }
            self.refresh_catalog(
                self.catalog_state.next.is_none() && self.catalog_state.refresh_next.is_none(),
            )
        }
    }

    fn refresh_catalog(&mut self, restart: bool) -> Result<(), String> {
        let request = CatalogRequest {
            cursor: if restart {
                None
            } else {
                self.catalog_state
                    .refresh_next
                    .clone()
                    .or_else(|| self.catalog_state.next.clone())
            },
            limit: 32,
        };
        let Observation::Catalog(page) = self.observe(Query::Catalog(request))? else {
            return Err("computer returned another page type".into());
        };
        self.apply_catalog(page, restart)
    }

    pub(crate) fn apply_catalog(&mut self, page: CatalogPage, restart: bool) -> Result<(), String> {
        if (restart || self.catalog_state.refresh_next.is_some())
            && page.snapshot == self.catalog_state.snapshot
            && self.catalog_state.pages > 0
        {
            // Walk mutable metadata without removing the already visible rows.
            let index = if restart {
                0
            } else {
                self.catalog_state.refresh_page
            };
            self.cache
                .write(&format!("catalog_{}_{}", page.snapshot, index), &page)?;
            for updated in page.entries {
                if let Some(existing) = self
                    .catalog
                    .iter_mut()
                    .find(|chat| chat.id == updated.id && chat.source_id == updated.source_id)
                {
                    *existing = updated;
                }
            }
            self.catalog_state.refresh_page = index + 1;
            self.catalog_state.refresh_next = if index + 1 < self.catalog_state.pages {
                page.next
            } else {
                None
            };
            self.catalog_state.checked_at = now();
            self.notices = page
                .notices
                .iter()
                .map(|n| format!("Source notice: {}", n.code))
                .collect();
            return self.cache.write("catalog_state", &self.catalog_state);
        }
        if restart || page.snapshot != self.catalog_state.snapshot {
            self.catalog.clear();
            self.catalog_state = CatalogState::default();
            self.catalog_state.snapshot = page.snapshot.clone();
        }
        if self.catalog_state.pages >= 128 {
            return Err(
                "Chat list reached 4,096 entries. Narrow the computer's source scope.".into(),
            );
        }
        let key = format!("catalog_{}_{}", page.snapshot, self.catalog_state.pages);
        self.cache.write(&key, &page)?;
        self.notices = page
            .notices
            .iter()
            .map(|n| format!("Source notice: {}", n.code))
            .collect();
        self.catalog_state.pages += 1;
        self.catalog_state.next = page.next.clone();
        self.catalog_state.checked_at = now();
        self.cache.write("catalog_state", &self.catalog_state)?;
        self.catalog.extend(page.entries);
        self.catalog_start = self
            .catalog_start
            .min(self.catalog.len().saturating_sub(1) / CATALOG_WINDOW * CATALOG_WINDOW);
        Ok(())
    }

    fn refresh_transcript(&mut self) -> Result<(), String> {
        let source = self.source().ok_or("select a chat first")?;
        let request = TranscriptRequest {
            source_id: source,
            cursor: self.transcript.cursor.clone(),
            max_bytes: coder_history::MAX_PAGE_BYTES,
        };
        let Observation::Page(page) = self.observe(Query::Page(request))? else {
            return Err("computer returned another page type".into());
        };
        self.apply_page(page)
    }

    pub(crate) fn apply_page(&mut self, page: TranscriptPage) -> Result<(), String> {
        let source = self.source().ok_or("select a chat first")?;
        if page.source_id != source
            || page.next.source_id != source
            || page.next.incarnation != page.incarnation
        {
            return Err("transcript page identity differs".into());
        }
        let start = self.transcript.cursor.as_ref().map_or(0, |c| c.offset);
        if self
            .transcript
            .cursor
            .as_ref()
            .is_some_and(|c| c.incarnation != page.incarnation)
        {
            return Err("transcript incarnation changed; reload required".into());
        }
        let mut offset = start;
        use base64::Engine;
        for chunk in &page.chunks {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&chunk.raw_base64)
                .map_err(|_| "invalid transcript bytes")?;
            if chunk.offset != offset
                || chunk.end_offset <= offset
                || chunk.end_offset - offset != bytes.len() as u64
            {
                return Err("transcript bytes are discontinuous".into());
            }
            offset = chunk.end_offset;
        }
        if page.next.offset != offset
            || offset.saturating_sub(start) > u64::from(coder_history::MAX_PAGE_BYTES)
            || offset > page.snapshot_bytes
        {
            return Err("transcript cursor is inconsistent".into());
        }
        if !page.chunks.is_empty() {
            self.cache
                .write(&format!("page_{source}_{start:020}"), &page)?;
        }
        self.transcript = TranscriptState {
            cursor: Some(page.next),
            snapshot_bytes: page.snapshot_bytes,
            has_more: page.has_more,
            pending_line: page.pending_line,
            checked_at: now(),
        };
        self.cache
            .write(&format!("chat_{source}"), &self.transcript)?;
        self.notices = page
            .notices
            .iter()
            .map(|n| format!("Source notice: {}", n.code))
            .collect();
        Ok(())
    }

    pub(crate) fn source(&self) -> Option<String> {
        self.selected.as_ref().and_then(|c| c.source_id.clone())
    }
    pub(crate) fn page_keys(&self) -> Result<Vec<String>, String> {
        match self.source() {
            Some(source) => self.cache.keys(&format!("page_{source}_")),
            None => Ok(vec![]),
        }
    }
    pub(crate) fn window_index(&self, keys: &[String]) -> usize {
        self.window
            .as_ref()
            .map_or(keys.len().saturating_sub(1), |wanted| {
                keys.iter().position(|k| k == wanted).unwrap_or(0)
            })
    }
    fn rebuild(&mut self) -> Result<(), String> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or("view revision exhausted")?;
        self.current = Some(
            View::new(
                self.instance.clone(),
                self.revision,
                crate::render::root(self)?,
            )
            .validate()
            .map_err(|e| e.to_string())?,
        );
        Ok(())
    }
    fn packet(&self) -> Packet {
        Packet {
            schema: "coder.mobile.v1",
            public_key: self.public_key.clone(),
            paired: self.code.is_some() || (self.synthetic && !self.catalog.is_empty()),
            reading: self.selected.is_some(),
            status: self.status.clone(),
            error: self.error.clone(),
            follow_page: self
                .page_keys()
                .ok()
                .and_then(|keys| keys.get(self.window_index(&keys)).cloned()),
            follow_target: if self.selected.is_some() && self.window.is_none() {
                Some("timeline-end".into())
            } else {
                None
            },
            view: self
                .current
                .as_ref()
                .and_then(|v| serde_json::to_value(v.view()).ok()),
        }
    }

    fn seed_synthetic(&mut self) -> Result<(), String> {
        if self.code.is_some() {
            return Ok(());
        }
        self.status = "Synthetic preview · no computer connected".into();
        self.apply_catalog(
            CatalogPage {
                snapshot: "synthetic".into(),
                entries: vec![Chat {
                    id: "synthetic-chat".into(),
                    harness: coder_history::Harness::Codex,
                    native_id: Some("synthetic".into()),
                    title: "Read-only transcript preview".into(),
                    title_truncated: false,
                    updated_at: None,
                    archived: false,
                    subagent: false,
                    source_id: Some("synthetic".into()),
                    status: coder_history::SourceStatus::Available,
                }],
                next: None,
                notices: vec![],
            },
            true,
        )?;
        use base64::Engine;
        self.selected = self.catalog.first().cloned();
        self.cache.erase("page_synthetic_")?;
        self.transcript = TranscriptState::default();
        let mut offset = 0;
        for page_index in 0..2 {
            let mut chunks = Vec::new();
            for index in 0..16 {
                let number = page_index * 16 + index;
                let body = format!(
                    "# Native timeline\nSynthetic message {}.\n\nUnicode: café 日本語 👩🏽‍💻\n\nNo model or task was started.",
                    number + 1
                );
                let mut bytes=serde_json::to_vec(&serde_json::json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":body}]}})).map_err(|_| "synthetic record encoding failed")?;
                bytes.push(b'\n');
                let end = offset + bytes.len() as u64;
                chunks.push(coder_history::RecordChunk {
                    id: format!("synthetic-{number}"),
                    index: number,
                    record_offset: offset,
                    offset,
                    end_offset: end,
                    raw_base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
                    complete: true,
                    oversized: false,
                    readable: coder_history::readable_record(&bytes),
                });
                offset = end;
            }
            let cursor = TranscriptCursor {
                source_id: "synthetic".into(),
                incarnation: "synthetic-v1".into(),
                offset,
                record_offset: offset,
                record_index: (page_index + 1) * 16,
                prefix_sha256: "synthetic".into(),
            };
            self.apply_page(TranscriptPage {
                source_id: "synthetic".into(),
                incarnation: "synthetic-v1".into(),
                snapshot_bytes: offset,
                chunks,
                next: cursor,
                has_more: false,
                pending_line: false,
                notices: vec![],
            })?;
        }
        self.selected = None;
        Ok(())
    }
}

pub(crate) fn now() -> u64 {
    coder_connect::unix_time().unwrap_or(0)
}

fn pairing_error(error: &coder_connect::Error) -> String {
    match error.code {
        ErrorCode::Transport | ErrorCode::Unavailable => {
            "Could not reach the computer. Keep its pairing command running and check that both devices are online. Try the same code again.".into()
        }
        ErrorCode::Expired => {
            "This pairing code expired. Run the connect command again on the computer to show a new QR code.".into()
        }
        ErrorCode::Conflict | ErrorCode::Revoked | ErrorCode::Forbidden => {
            "The computer refused this pairing code. It may have already been used by another device. Generate a new code on the computer.".into()
        }
        ErrorCode::SourceChanged => {
            "The computer's selected chat folders changed. Run its connect command again.".into()
        }
        ErrorCode::RateLimited => "The computer is busy. Wait briefly, then try again.".into(),
        ErrorCode::Malformed | ErrorCode::Unsupported | ErrorCode::Bounds => {
            "This is not a supported Coder pairing code. Scan the QR code shown by the computer, or paste its complete pairing string.".into()
        }
    }
}
