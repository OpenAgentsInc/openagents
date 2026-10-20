//! The composer's desktop parts on the web (#12267, #12268): per-chat
//! drafts, the send-key choice, the `/`, `$`, and `@` completion lists, and,
//! for a chat on one of the account's computers, the message queue and the
//! question wizard.
//!
//! A chat on a computer is a [`HostChat`]. Its commands, skills, and files
//! come from that computer over host RPC (`ListCommands`, `ListSkills`,
//! `SearchFiles`), and its queue lives on the chat's document there
//! (`WatchQueue` and the queued-message methods), so the desktop sees the
//! same queue. An answer to an agent's question goes back as a
//! `RespondInput` command (`QueueCommand`).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use futures::StreamExt as _;
use gpui::{
    AnyElement, ClickEvent, Context, Entity, Focusable as _, FontWeight, IntoElement,
    ParentElement, SharedString, Styled, Task, Window, div, prelude::*, px,
};
use openagents_chat_view::composer_tokens::MentionToken;
use openagents_chat_view::composer_wizard::{
    AUTO_ADVANCE_MS, Wizard, WizardStep, input_request_resolved, pending_input_request,
};
use openagents_doc::{QueuedMessage, SessionCommandPayload, SessionMessageEntry};
use openagents_proto::SlashCommand;
use openagents_proto::invocation::Skill;
use openagents_web_chat_view::composer::{Cancelled, CompletionKey, Edited, Submitted, TextInput};
use openagents_web_chat_view::theme;
use openagents_web_chat_view::transcript::{button, footer_text, truncate};
use openagents_web_client::computer;
use openagents_web_client::web::Client;
use serde_json::{Value, json};

use crate::complete::{Item, Kind, catalog, draft_key, file_items, filter_catalog, token_at};

/// Where the drafts and the send-key choice are kept in this browser.
const DRAFTS_KEY: &str = "openagents.composer.drafts.v1";
const SEND_KEY: &str = "openagents.composer.send-with-modifier";
/// The most drafts kept, and the longest one.
const MAX_DRAFTS: usize = 50;
const MAX_DRAFT_BYTES: usize = 16 * 1024;
/// The most rows a completion list shows.
const MAX_ROWS: usize = 8;
/// How many of a host chat's last entries are read for a question, and
/// how often.
const QUESTION_WINDOW: usize = 4;
const QUESTION_POLL: Duration = Duration::from_secs(2);
/// A pause after typing before the computer's files are searched.
const SEARCH_PAUSE: Duration = Duration::from_millis(120);

/// A chat that runs on one of the account's computers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostChat {
    /// The computer's device ID (its device room).
    pub device: String,
    /// The chat's ID on that computer.
    pub chat: String,
    /// The harness it runs (`claude`, `codex`, ...), for its commands and
    /// skills.
    pub harness: Option<String>,
}

struct Completion {
    kind: Kind,
    token: MentionToken,
    items: Vec<Item>,
    active: usize,
    loading: bool,
    error: Option<String>,
}

pub struct Compose {
    input: Entity<TextInput>,
    client: Client,
    /// The draft key of the open chat (`new` for a new chat).
    key: String,
    drafts: BTreeMap<String, String>,
    send_with_modifier: bool,
    host: Option<HostChat>,
    completion: Option<Completion>,
    /// A dismissed token and its text: it stays closed until edited.
    dismissed: Option<(Range<usize>, String)>,
    /// Commands and skills by computer, harness, and chat.
    catalogs: HashMap<String, Rc<Vec<(Kind, Item)>>>,
    request: u64,
    _lookup: Option<Task<()>>,
    /// The host chat's queue, as its computer last sent it.
    queue: Vec<QueuedMessage>,
    _queue: Option<Task<()>>,
    /// Follows the host chat's last entries for an agent's question.
    _questions: Option<Task<()>>,
    /// The queued row being retyped, and its field.
    editing: Option<(String, Entity<TextInput>)>,
    /// The agent's open question, and the requests already answered here.
    wizard: Option<Wizard>,
    answer: Entity<TextInput>,
    answered: HashSet<String>,
    _advance: Option<Task<()>>,
    notice: Option<String>,
    file_tx: futures::channel::mpsc::UnboundedSender<web_sys::File>,
    files: Vec<(String, String, Option<String>)>,
    uploading: bool,
    pending_files: std::collections::VecDeque<web_sys::File>,
}

impl Compose {
    pub fn new(input: Entity<TextInput>, client: Client, cx: &mut Context<Self>) -> Self {
        let send_with_modifier = storage_get(SEND_KEY).as_deref() == Some("1");
        input.update(cx, |input, _| {
            input.set_send_with_modifier(send_with_modifier)
        });
        cx.subscribe(&input, |this, _, _: &Edited, cx| this.edited(cx))
            .detach();
        cx.subscribe(&input, |this, _, key: &CompletionKey, cx| {
            this.key(*key, cx)
        })
        .detach();
        let answer = cx.new(|cx| TextInput::multiline("Type an answer", cx));
        cx.subscribe(&answer, |this, _, event: &Submitted, cx| {
            this.wizard_typed(event.0.clone(), true, cx)
        })
        .detach();
        let drafts = storage_get(DRAFTS_KEY)
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        let (file_tx, mut file_rx) = futures::channel::mpsc::unbounded();
        crate::attachments::intake(file_tx.clone());
        cx.spawn(async move |this, cx| {
            while let Some(file) = file_rx.next().await {
                let _ = this.update(cx, |this, cx| this.attach(file, cx));
            }
        })
        .detach();
        let mut this = Self {
            input,
            client,
            key: draft_key(None),
            drafts,
            send_with_modifier,
            host: None,
            completion: None,
            dismissed: None,
            catalogs: HashMap::new(),
            request: 0,
            _lookup: None,
            queue: Vec::new(),
            _queue: None,
            _questions: None,
            editing: None,
            wizard: None,
            answer,
            answered: HashSet::new(),
            _advance: None,
            notice: None,
            file_tx,
            files: Vec::new(),
            uploading: false,
            pending_files: Default::default(),
        };
        this.restore(cx);
        this
    }

    /// The open chat changed: keep the old one's draft, show the new one's.
    pub fn switch_chat(&mut self, chat: Option<&str>, cx: &mut Context<Self>) {
        let key = draft_key(chat);
        if key == self.key {
            return;
        }
        self.keep_draft(cx);
        self.key = key;
        self.close_completion(cx);
        self.wizard = None;
        self.restore(cx);
    }

    /// A new chat got its ID: its draft moves with it.
    pub fn renamed_chat(&mut self, chat: &str) {
        let key = draft_key(Some(chat));
        if let Some(draft) = self.drafts.remove(&self.key) {
            self.drafts.insert(key.clone(), draft);
        }
        self.key = key;
        self.save_drafts();
    }

    /// The message went out: its draft is done.
    pub fn sent(&mut self, cx: &mut Context<Self>) {
        self.drafts.remove(&self.key);
        self.save_drafts();
        self.close_completion(cx);
    }

    /// The open chat is on a computer (or no longer is).
    pub fn set_host(&mut self, host: Option<HostChat>, cx: &mut Context<Self>) {
        if self.host == host {
            return;
        }
        self.files.clear();
        self.pending_files.clear();
        self.host = host;
        self.queue.clear();
        self.editing = None;
        self._queue = self.host.clone().map(|host| self.watch_queue(host, cx));
        self._questions = self.host.clone().map(|host| self.watch_questions(host, cx));
        if self.host.is_none() {
            self.wizard = None;
        }
        cx.notify();
    }

    pub fn host(&self) -> Option<&HostChat> {
        self.host.as_ref()
    }

    /// The open chat's entries changed: open, keep, or close the wizard.
    pub fn transcript(&mut self, entries: &[SessionMessageEntry], cx: &mut Context<Self>) {
        if self.host.is_none() {
            self.wizard = None;
            return;
        }
        let pending =
            pending_input_request(entries).filter(|(request, _)| !self.answered.contains(request));
        match (&self.wizard, pending) {
            (Some(wizard), Some((request, _))) if wizard.request_id == request => {}
            (_, Some((request, questions))) => {
                let wizard = Wizard::new(request, questions);
                let typed = wizard.typed().to_owned();
                self.answer
                    .update(cx, |answer, cx| answer.set_text(typed, cx));
                self.wizard = Some(wizard);
                cx.notify();
            }
            (Some(_), None) => {
                self.wizard = None;
                cx.notify();
            }
            (None, None) => {}
        }
        self.answered
            .retain(|request| !input_request_resolved(entries, request));
    }

    /// Put `text` on the host chat's queue. The computer sends it when the
    /// agent is free, or steers it into the running turn.
    pub fn queue(&mut self, text: String, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            return;
        };
        if self.uploading {
            self.notice = Some("Wait for the upload to finish.".into());
            self.input.update(cx, |input, cx| input.set_text(text, cx));
            return;
        }
        let paths = self
            .files
            .iter()
            .map(|(_, path, _)| path.clone())
            .collect::<Vec<_>>();
        let text = openagents_chat_view::attachments::with_attachments(&text, &paths);
        self.files.clear();
        self.sent(cx);
        self.call(
            &host,
            openagents_rpc_methods::QUEUE_MESSAGE,
            json!({ "chatId": host.chat, "text": text }),
            Some(text),
            cx,
        );
    }

    fn attach(&mut self, file: web_sys::File, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            return;
        };
        if self.uploading {
            self.pending_files.push_back(file);
            cx.notify();
            return;
        }
        if file.size() > crate::attachment_chunks::MAX_BYTES as f64 {
            self.notice = Some("This file is too large (24 MB max).".into());
            cx.notify();
            return;
        }
        self.uploading = true;
        let preview_file = file.clone();
        let client = self.client.clone();
        let name = file.name().replace(['\r', '\n'], "_");
        self.notice = Some(format!("Uploading {name}… 0%"));
        cx.spawn(async move |this, cx| {
            let result = async {
                let buffer = wasm_bindgen_futures::JsFuture::from(file.array_buffer())
                    .await
                    .map_err(|_| anyhow::anyhow!("This file could not be read."))?;
                let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
                let computer = computer::connect(&client, &host.device, || {}).await?;
                let id = uuid::Uuid::new_v4().to_string();
                let chunks = crate::attachment_chunks::chunks(&bytes);
                let count = chunks.len();
                for (seq, data) in chunks.into_iter().enumerate() {
                    computer
                        .call(
                            "UploadChunk",
                            json!({"uploadId": id, "seq": seq, "data": data}),
                        )
                        .await?;
                    let _ = this.update(cx, |this, cx| {
                        this.notice =
                            Some(format!("Uploading {name}… {}%", (seq + 1) * 100 / count));
                        cx.notify();
                    });
                }
                let reply = computer
                    .call("UploadCommit", json!({"uploadId": id, "fileName": name}))
                    .await?;
                reply["path"]
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| anyhow::anyhow!("The upload returned no file."))
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                this.uploading = false;
                if this.host.as_ref() == Some(&host) {
                    match result {
                        Ok(path) => {
                            this.files.push((
                                name,
                                path,
                                Some(crate::attachments::preview_url(&preview_file)),
                            ));
                            this.notice = None;
                        }
                        Err(error) => this.notice = Some(error.to_string()),
                    }
                }
                if let Some(file) = this.pending_files.pop_front() {
                    this.attach(file, cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn render_attachments(&self, cx: &mut Context<Self>) -> AnyElement {
        let action = |id: String, label: String| {
            div()
                .id(SharedString::from(id))
                .cursor_pointer()
                .child(label)
        };
        let mut row = div().flex().gap(px(8.));
        if self.host.is_some() {
            row = row.child(action("attach-file".into(), "Attach file".into()).on_click(cx.listener(|this, _, _, _| {
                crate::attachments::pick(this.file_tx.clone());
            }))).child(action("attach-screen".into(), "Screenshot".into()).on_click(cx.listener(|this, _, _, cx| {
                let tx = this.file_tx.clone();
                cx.spawn(async move |this, cx| {
                    match crate::attachments::screenshot().await {
                        Ok(file) => { let _ = tx.unbounded_send(file); }
                        Err(_) => { let _ = this.update(cx, |this, cx| {
                            this.notice = Some("No screenshot was attached. Choose a tab or screen to try again.".into());
                            cx.notify();
                        }); }
                    }
                }).detach();
            })));
            for (index, (name, _, preview)) in self.files.iter().enumerate() {
                if let Some(url) = preview.clone() {
                    row = row.child(
                        action(format!("preview-{index}"), name.clone())
                            .on_click(move |_, _, _| crate::attachments::preview(&url)),
                    );
                }
                row = row.child(
                    action(format!("attached-{index}"), format!("{name} ×")).on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.files.remove(index);
                            cx.notify();
                        },
                    )),
                );
            }
        }
        row.into_any_element()
    }

    pub fn has_files(&self) -> bool {
        !self.files.is_empty()
    }

    fn keep_draft(&mut self, cx: &mut Context<Self>) {
        let text = self.input.read(cx).text();
        if text.trim().is_empty() || text.len() > MAX_DRAFT_BYTES {
            self.drafts.remove(&self.key);
        } else {
            self.drafts.insert(self.key.clone(), text);
            while self.drafts.len() > MAX_DRAFTS {
                let Some(first) = self.drafts.keys().next().cloned() else {
                    break;
                };
                self.drafts.remove(&first);
            }
        }
        self.save_drafts();
    }

    fn restore(&mut self, cx: &mut Context<Self>) {
        let draft = self.drafts.get(&self.key).cloned().unwrap_or_default();
        self.input.update(cx, |input, cx| {
            input.take(cx);
            if !draft.is_empty() {
                input.set_text(draft, cx);
            }
        });
    }

    fn save_drafts(&self) {
        if let Ok(json) = serde_json::to_string(&self.drafts) {
            storage_set(DRAFTS_KEY, &json);
        }
    }

    fn toggle_send_key(&mut self, cx: &mut Context<Self>) {
        let on = !self.send_with_modifier;
        self.set_send_with_modifier(on, cx);
    }

    /// Send with Cmd/Ctrl+Enter (Settings → General's "Send messages
    /// with", #12288).
    pub fn set_send_with_modifier(&mut self, on: bool, cx: &mut Context<Self>) {
        self.send_with_modifier = on;
        storage_set(SEND_KEY, if on { "1" } else { "0" });
        self.input
            .update(cx, |input, _| input.set_send_with_modifier(on));
        cx.notify();
    }

    pub fn send_with_modifier(&self) -> bool {
        self.send_with_modifier
    }

    // -----------------------------------------------------------------
    // Completion
    // -----------------------------------------------------------------

    fn edited(&mut self, cx: &mut Context<Self>) {
        let (text, cursor, composing) = {
            let input = self.input.read(cx);
            (input.text(), input.cursor(), input.composing())
        };
        // Drafts follow the text as it's typed.
        if text.trim().is_empty() {
            if self.drafts.remove(&self.key).is_some() {
                self.save_drafts();
            }
        } else if text.len() <= MAX_DRAFT_BYTES {
            self.drafts.insert(self.key.clone(), text.clone());
            self.save_drafts();
        }
        if composing {
            return;
        }
        let Some(host) = self.host.clone() else {
            return self.close_completion(cx);
        };
        let Some((kind, token)) = token_at(&text, cursor) else {
            self.dismissed = None;
            return self.close_completion(cx);
        };
        let whole = text[token.range.clone()].to_owned();
        if self
            .dismissed
            .as_ref()
            .is_some_and(|(range, text)| *range == token.range && *text == whole)
        {
            return;
        }
        self.dismissed = None;
        let same = self
            .completion
            .as_ref()
            .is_some_and(|c| c.kind == kind && c.token == token);
        if same {
            return;
        }
        let previous = self.completion.take();
        self.completion = Some(Completion {
            kind,
            token: token.clone(),
            items: previous
                .filter(|c| c.kind == kind)
                .map(|c| c.items)
                .unwrap_or_default(),
            active: 0,
            loading: true,
            error: None,
        });
        self.input.update(cx, |input, _| input.set_completing(true));
        self.lookup(host, kind, token.query, cx);
        cx.notify();
    }

    fn lookup(&mut self, host: HostChat, kind: Kind, query: String, cx: &mut Context<Self>) {
        self.request += 1;
        let request = self.request;
        let client = self.client.clone();
        if kind == Kind::File {
            self._lookup = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(SEARCH_PAUSE).await;
                let found = async {
                    let computer = computer::connect(&client, &host.device, || {}).await?;
                    let value = computer
                        .call(
                            openagents_rpc_methods::SEARCH_FILES,
                            json!({ "query": query, "chatId": host.chat }),
                        )
                        .await?;
                    anyhow::Ok(file_items(serde_json::from_value(value)?))
                }
                .await;
                this.update(cx, |this, cx| this.found(request, found, cx))
                    .ok();
            }));
            return;
        }
        let catalog_key = format!("{}|{:?}|{}", host.device, host.harness, host.chat);
        if let Some(catalog) = self.catalogs.get(&catalog_key).cloned() {
            let items = filter_catalog(&catalog, kind, &query);
            return self.found(request, Ok(items), cx);
        }
        self._lookup = Some(cx.spawn(async move |this, cx| {
            let loaded = async {
                let Some(harness) = host.harness.clone() else {
                    return anyhow::Ok(Vec::new());
                };
                let computer = computer::connect(&client, &host.device, || {}).await?;
                let params = json!({ "harness": harness, "chatId": host.chat });
                let commands = computer
                    .call(openagents_rpc_methods::LIST_COMMANDS, params.clone())
                    .await
                    .ok()
                    .and_then(|v| serde_json::from_value::<Vec<SlashCommand>>(v).ok())
                    .unwrap_or_default();
                let skills = computer
                    .call(openagents_rpc_methods::LIST_SKILLS, params)
                    .await
                    .ok()
                    .and_then(|v| serde_json::from_value::<Option<Vec<Skill>>>(v).ok())
                    .flatten()
                    .unwrap_or_default();
                anyhow::Ok(catalog(commands, skills))
            }
            .await;
            this.update(cx, |this, cx| match loaded {
                Ok(catalog) => {
                    let catalog = Rc::new(catalog);
                    this.catalogs.insert(catalog_key, catalog.clone());
                    let items = filter_catalog(&catalog, kind, &query);
                    this.found(request, Ok(items), cx);
                }
                Err(error) => this.found(request, Err(error), cx),
            })
            .ok();
        }));
    }

    fn found(&mut self, request: u64, found: anyhow::Result<Vec<Item>>, cx: &mut Context<Self>) {
        if request != self.request {
            return;
        }
        let Some(completion) = &mut self.completion else {
            return;
        };
        completion.loading = false;
        match found {
            Ok(items) => {
                completion.items = items;
                completion.error = None;
            }
            Err(error) => {
                completion.items.clear();
                completion.error = Some(error.to_string());
            }
        }
        completion.active = completion
            .active
            .min(completion.items.len().saturating_sub(1));
        cx.notify();
    }

    fn key(&mut self, key: CompletionKey, cx: &mut Context<Self>) {
        let Some(completion) = &mut self.completion else {
            self.input
                .update(cx, |input, _| input.set_completing(false));
            return;
        };
        let count = completion.items.len().min(MAX_ROWS);
        match key {
            CompletionKey::Up if count > 0 => {
                completion.active = (completion.active + count - 1) % count;
            }
            CompletionKey::Down if count > 0 => {
                completion.active = (completion.active + 1) % count;
            }
            CompletionKey::Up | CompletionKey::Down => {}
            CompletionKey::Accept => {
                let active = completion.active;
                return self.accept(active, cx);
            }
            CompletionKey::Dismiss => {
                let range = completion.token.range.clone();
                let text = self.input.read(cx).text();
                self.dismissed = text.get(range.clone()).map(|t| (range, t.to_owned()));
                return self.close_completion(cx);
            }
        }
        cx.notify();
    }

    fn accept(&mut self, row: usize, cx: &mut Context<Self>) {
        let Some(completion) = self.completion.take() else {
            return;
        };
        self.input
            .update(cx, |input, _| input.set_completing(false));
        let Some(item) = completion.items.get(row) else {
            // Nothing to choose: Enter goes on as it would.
            cx.notify();
            return;
        };
        let range = completion.token.range.clone();
        let insert = item.insert.clone();
        self.input.update(cx, |input, cx| {
            let next = input.text()[range.end..].chars().next();
            let space = if next.is_some_and(char::is_whitespace) {
                ""
            } else {
                " "
            };
            input.replace_range(range, &format!("{insert}{space}"), cx);
        });
        cx.notify();
    }

    fn close_completion(&mut self, cx: &mut Context<Self>) {
        self._lookup = None;
        self.request += 1;
        if self.completion.take().is_some() {
            cx.notify();
        }
        self.input
            .update(cx, |input, _| input.set_completing(false));
    }

    // -----------------------------------------------------------------
    // Queue
    // -----------------------------------------------------------------

    fn watch_queue(&mut self, host: HostChat, cx: &mut Context<Self>) -> Task<()> {
        let client = self.client.clone();
        cx.spawn(async move |this, cx| {
            let Ok(computer) = computer::connect(&client, &host.device, || {}).await else {
                return;
            };
            let Ok(mut items) = computer.watch(
                openagents_rpc_methods::WATCH_QUEUE,
                json!({ "chatId": host.chat }),
            ) else {
                return;
            };
            while let Some(Ok(value)) = items.next().await {
                let queue: Vec<QueuedMessage> =
                    serde_json::from_value(value["items"].clone()).unwrap_or_default();
                if this
                    .update(cx, |this, cx| {
                        this.queue = queue;
                        if this
                            .editing
                            .as_ref()
                            .is_some_and(|(id, _)| !this.queue.iter().any(|row| &row.id == id))
                        {
                            this.editing = None;
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
    }

    /// Read the host chat's last entries every few seconds, so an agent's
    /// question opens the wizard here as it does on the desktop.
    fn watch_questions(&mut self, host: HostChat, cx: &mut Context<Self>) -> Task<()> {
        let client = self.client.clone();
        cx.spawn(async move |this, cx| {
            loop {
                let read = async {
                    let computer = computer::connect(&client, &host.device, || {}).await?;
                    let window = computer
                        .call(
                            openagents_rpc_methods::READ_CHAT_WINDOW,
                            json!({ "chatId": host.chat, "limit": QUESTION_WINDOW }),
                        )
                        .await?;
                    anyhow::Ok(serde_json::from_value::<Vec<SessionMessageEntry>>(
                        window["entries"].clone(),
                    )?)
                }
                .await;
                if let Ok(entries) = read {
                    let alive = this
                        .update(cx, |this, cx| this.transcript(&entries, cx))
                        .is_ok();
                    if !alive {
                        break;
                    }
                }
                cx.background_executor().timer(QUESTION_POLL).await;
            }
        })
    }

    /// One host call; a failure shows as a notice, and `restore` puts typed
    /// text back in the composer.
    fn call(
        &mut self,
        host: &HostChat,
        method: &'static str,
        params: Value,
        restore: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let client = self.client.clone();
        let device = host.device.clone();
        cx.spawn(async move |this, cx| {
            let result = async {
                let computer = computer::connect(&client, &device, || {}).await?;
                computer.call(method, params).await
            }
            .await;
            if let Err(error) = result {
                this.update(cx, |this, cx| {
                    this.notice = Some(error.to_string());
                    if let Some(text) = restore {
                        this.input.update(cx, |input, cx| {
                            if input.is_empty() {
                                input.set_text(text, cx);
                            }
                        });
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn queue_action(
        &mut self,
        method: &'static str,
        id: &str,
        extra: Value,
        cx: &mut Context<Self>,
    ) {
        let Some(host) = self.host.clone() else {
            return;
        };
        let mut params = json!({ "chatId": host.chat, "id": id });
        if let (Value::Object(params), Value::Object(extra)) = (&mut params, extra) {
            params.extend(extra);
        }
        self.call(&host, method, params, None, cx);
    }

    fn move_row(&mut self, id: &str, by: isize, cx: &mut Context<Self>) {
        let Some(at) = self.queue.iter().position(|row| row.id == id) else {
            return;
        };
        let to = at as isize + by;
        if to < 0 || to as usize >= self.queue.len() {
            return;
        }
        // Show the move now; the computer's next queue confirms it.
        let row = self.queue.remove(at);
        self.queue.insert(to as usize, row);
        cx.notify();
        self.queue_action(
            openagents_rpc_methods::MOVE_QUEUED_MESSAGE,
            id,
            json!({ "toIndex": to }),
            cx,
        );
    }

    fn edit_row(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.queue.iter().find(|row| row.id == id) else {
            return;
        };
        let text = row.text.clone();
        let field = cx.new(|cx| {
            let mut field = TextInput::multiline("Edit the message", cx);
            field.set_text(text, cx);
            field
        });
        let row_id = id.to_owned();
        cx.subscribe(&field, move |this, _, event: &Submitted, cx| {
            let text = event.0.clone();
            this.editing = None;
            this.queue_action(
                openagents_rpc_methods::UPDATE_QUEUED_MESSAGE,
                &row_id,
                json!({ "text": text }),
                cx,
            );
            cx.notify();
        })
        .detach();
        cx.subscribe(&field, |this, _, _: &Cancelled, cx| {
            this.editing = None;
            cx.notify();
        })
        .detach();
        window.focus(&field.focus_handle(cx), cx);
        self.editing = Some((id.to_owned(), field));
        cx.notify();
    }

    // -----------------------------------------------------------------
    // Question wizard
    // -----------------------------------------------------------------

    fn wizard_pick(&mut self, option: usize, cx: &mut Context<Self>) {
        let Some(wizard) = &mut self.wizard else {
            return;
        };
        match wizard.select(option) {
            WizardStep::AutoAdvance => {
                self._advance = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(AUTO_ADVANCE_MS))
                        .await;
                    this.update(cx, |this, cx| this.wizard_next(cx)).ok();
                }));
            }
            WizardStep::Done(answers) => self.wizard_finish(answers, cx),
            WizardStep::Stay => {}
        }
        cx.notify();
    }

    fn wizard_typed(&mut self, text: String, advance: bool, cx: &mut Context<Self>) {
        if let Some(wizard) = &mut self.wizard {
            wizard.set_typed(text);
        }
        if advance {
            self.wizard_next(cx);
        }
    }

    fn wizard_next(&mut self, cx: &mut Context<Self>) {
        self._advance = None;
        let typed = self.answer.read(cx).text();
        let Some(wizard) = &mut self.wizard else {
            return;
        };
        wizard.set_typed(typed);
        match wizard.advance() {
            WizardStep::Done(answers) => self.wizard_finish(answers, cx),
            _ => {
                let typed = wizard.typed().to_owned();
                self.answer.update(cx, |answer, cx| {
                    answer.take(cx);
                    if !typed.is_empty() {
                        answer.set_text(typed, cx);
                    }
                });
            }
        }
        cx.notify();
    }

    fn wizard_back(&mut self, cx: &mut Context<Self>) {
        let typed = self.answer.read(cx).text();
        let Some(wizard) = &mut self.wizard else {
            return;
        };
        wizard.set_typed(typed);
        if wizard.back() {
            let typed = wizard.typed().to_owned();
            self.answer.update(cx, |answer, cx| {
                answer.take(cx);
                if !typed.is_empty() {
                    answer.set_text(typed, cx);
                }
            });
        }
        cx.notify();
    }

    fn wizard_finish(
        &mut self,
        answers: Vec<openagents_proto::UserInputAnswer>,
        cx: &mut Context<Self>,
    ) {
        let (Some(wizard), Some(host)) = (self.wizard.take(), self.host.clone()) else {
            return;
        };
        self.answered.insert(wizard.request_id.clone());
        let command = SessionCommandPayload::RespondInput {
            request_id: wizard.request_id,
            answers,
        };
        let Ok(command) = serde_json::to_value(&command) else {
            return;
        };
        self.answer.update(cx, |answer, cx| {
            answer.take(cx);
        });
        self.call(
            &host,
            openagents_rpc_methods::QUEUE_COMMAND,
            json!({ "chatId": host.chat, "command": command }),
            None,
            cx,
        );
        cx.notify();
    }

    // -----------------------------------------------------------------
    // Drawing
    // -----------------------------------------------------------------

    /// The list over the composer, while a token is under the caret.
    pub fn render_completion(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let completion = self.completion.as_ref()?;
        let mut card = card("completion");
        if completion.items.is_empty() {
            let line = if completion.loading {
                "Looking…".to_owned()
            } else if let Some(error) = &completion.error {
                error.clone()
            } else {
                match completion.kind {
                    Kind::File => "No matching files".to_owned(),
                    Kind::Command | Kind::Skill if self.host_harness().is_none() => {
                        "This chat's agent has no commands here".to_owned()
                    }
                    Kind::Command => "No matching commands".to_owned(),
                    Kind::Skill => "No matching skills".to_owned(),
                }
            };
            return Some(
                card.child(div().px(px(10.)).py(px(6.)).child(footer_text(line)))
                    .into_any_element(),
            );
        }
        for (ix, item) in completion.items.iter().take(MAX_ROWS).enumerate() {
            let active = ix == completion.active;
            card = card.child(
                div()
                    .id(("completion-row", ix))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(8.))
                    .cursor_pointer()
                    .when(active, |row| row.bg(theme::element_active()))
                    .hover(|row| row.bg(theme::element_hover()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.accept(ix, cx)))
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(13.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::text())
                            .child(truncate(&item.label, 48)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_size(px(12.5))
                            .text_color(theme::text_faint())
                            .child(truncate(&item.detail, 80)),
                    ),
            );
        }
        Some(card.into_any_element())
    }

    fn host_harness(&self) -> Option<&str> {
        self.host.as_ref()?.harness.as_deref()
    }

    /// The host chat's queue, over the composer.
    pub fn render_queue(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.queue.is_empty() {
            return None;
        }
        let mut card = card("queue").child(
            div()
                .px(px(10.))
                .pt(px(4.))
                .pb(px(2.))
                .child(footer_text(format!("Queued ({})", self.queue.len()))),
        );
        let last = self.queue.len() - 1;
        for (ix, row) in self.queue.iter().enumerate() {
            let id = row.id.clone();
            let editing = self
                .editing
                .as_ref()
                .filter(|(editing, _)| *editing == row.id)
                .map(|(_, field)| field.clone());
            let body: AnyElement = match editing {
                Some(field) => div()
                    .flex_1()
                    .min_w_0()
                    .px(px(8.))
                    .py(px(4.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(theme::border_strong())
                    .child(field)
                    .into_any_element(),
                None => div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_size(px(13.5))
                    .text_color(theme::text())
                    .child(truncate(&row.text.replace('\n', " "), 120))
                    .into_any_element(),
            };
            let action = |name: &'static str, label: &'static str| {
                button(
                    SharedString::from(format!("queue-{name}-{ix}")),
                    label,
                    false,
                )
            };
            let actions = div()
                .flex()
                .flex_row()
                .flex_none()
                .gap(px(4.))
                .when(ix > 0, |row| {
                    let id = id.clone();
                    row.child(action("up", "Up").on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| this.move_row(&id, -1, cx)),
                    ))
                })
                .when(ix < last, |row| {
                    let id = id.clone();
                    row.child(action("down", "Down").on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| this.move_row(&id, 1, cx)),
                    ))
                })
                .child({
                    let id = id.clone();
                    action("edit", "Edit").on_click(cx.listener(
                        move |this, _: &ClickEvent, window, cx| this.edit_row(&id, window, cx),
                    ))
                })
                .child({
                    let id = id.clone();
                    action("steer", "Steer now").on_click(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.queue_action(
                                openagents_rpc_methods::STEER_QUEUED_MESSAGE_NOW,
                                &id,
                                json!({}),
                                cx,
                            )
                        },
                    ))
                })
                .child({
                    let id = id.clone();
                    action("send", "Send now").on_click(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.queue_action(
                                openagents_rpc_methods::SEND_QUEUED_MESSAGE_NOW,
                                &id,
                                json!({}),
                                cx,
                            )
                        },
                    ))
                })
                .child({
                    let id = id.clone();
                    action("remove", "Remove").on_click(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.queue.retain(|row| row.id != id);
                            this.queue_action(
                                openagents_rpc_methods::REMOVE_QUEUED_MESSAGE,
                                &id,
                                json!({}),
                                cx,
                            );
                            cx.notify();
                        },
                    ))
                });
            card = card.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .px(px(10.))
                    .py(px(5.))
                    .child(body)
                    .child(actions),
            );
        }
        Some(card.into_any_element())
    }

    /// The agent's question, in place of the composer.
    pub fn render_wizard(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let wizard = self.wizard.as_ref()?;
        let question = wizard.current()?;
        let header = if question.header.is_empty() {
            wizard.counter()
        } else {
            format!("{}  {}", wizard.counter(), question.header)
        };
        let mut options = div().flex().flex_col().gap(px(4.));
        for (ix, option) in question.options.iter().enumerate() {
            let picked = wizard.is_picked(ix);
            options = options.child(
                div()
                    .id(("wizard-option", ix))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(if picked {
                        theme::accent()
                    } else {
                        theme::border()
                    })
                    .when(picked, |row| row.bg(theme::accent_wash()))
                    .cursor_pointer()
                    .hover(|row| row.bg(theme::element_hover()))
                    .on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| this.wizard_pick(ix, cx)),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(12.5))
                            .text_color(theme::text_faint())
                            .child(format!("{}", ix + 1)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(14.))
                            .child(option.clone()),
                    ),
            );
        }
        let last = wizard.page + 1 >= wizard.questions.len();
        let first = wizard.page == 0;
        Some(
            div()
                .w_full()
                .max_w(px(
                    openagents_web_chat_view::transcript::content_width() + 24.
                ))
                .flex()
                .flex_col()
                .gap(px(10.))
                .p(px(16.))
                .rounded(px(20.))
                .border_1()
                .border_color(theme::border_strong())
                .bg(theme::surface_card())
                .child(footer_text(header))
                .child(
                    div()
                        .text_size(px(15.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(question.question.clone()),
                )
                .child(options)
                .child(
                    div()
                        .px(px(12.))
                        .py(px(8.))
                        .rounded(px(10.))
                        .border_1()
                        .border_color(theme::border())
                        .child(self.answer.clone()),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap(px(8.))
                        .when(!first, |row| {
                            row.child(button("wizard-back", "Back", false).on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| this.wizard_back(cx)),
                            ))
                        })
                        .child(
                            button("wizard-next", if last { "Send" } else { "Next" }, true)
                                .on_click(
                                    cx.listener(|this, _: &ClickEvent, _, cx| this.wizard_next(cx)),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }

    /// The send key, under the composer: click to switch.
    pub fn render_send_key(&self, cx: &mut Context<Self>) -> AnyElement {
        let modifier = if is_mac() { "⌘" } else { "Ctrl+" };
        let line = if self.send_with_modifier {
            format!("{modifier}Enter to send, Enter for a new line")
        } else {
            "Enter to send, Shift+Enter for a new line".to_owned()
        };
        div()
            .id("send-key")
            .cursor_pointer()
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_send_key(cx)))
            .child(footer_text(line))
            .into_any_element()
    }

    /// A call to the computer that failed, once.
    pub fn take_notice(&mut self) -> Option<String> {
        self.notice.take()
    }
}

/// The computer's method names (`openagents_rpc::methods`), which this
/// crate doesn't link.
mod openagents_rpc_methods {
    pub const LIST_COMMANDS: &str = "ListCommands";
    pub const LIST_SKILLS: &str = "ListSkills";
    pub const SEARCH_FILES: &str = "SearchFiles";
    pub const QUEUE_COMMAND: &str = "QueueCommand";
    pub const WATCH_QUEUE: &str = "WatchQueue";
    pub const READ_CHAT_WINDOW: &str = "ReadChatWindow";
    pub const QUEUE_MESSAGE: &str = "QueueMessage";
    pub const UPDATE_QUEUED_MESSAGE: &str = "UpdateQueuedMessage";
    pub const MOVE_QUEUED_MESSAGE: &str = "MoveQueuedMessage";
    pub const REMOVE_QUEUED_MESSAGE: &str = "RemoveQueuedMessage";
    pub const SEND_QUEUED_MESSAGE_NOW: &str = "SendQueuedMessageNow";
    pub const STEER_QUEUED_MESSAGE_NOW: &str = "SteerQueuedMessageNow";
}

fn card(id: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .w_full()
        .max_w(px(
            openagents_web_chat_view::transcript::content_width() + 24.
        ))
        .flex()
        .flex_col()
        .gap(px(2.))
        .p(px(6.))
        .rounded(px(14.))
        .border_1()
        .border_color(theme::border_strong())
        .bg(theme::surface_card())
}

/// Tell the page's script something: `oa-host` (the open chat's computer
/// and chat, `DEVICE\nCHAT`, or empty) and `oa-files-toggle` (the file
/// panel's button).
pub fn page_event(name: &str, detail: &str) {
    let init = web_sys::CustomEventInit::new();
    init.set_detail(&wasm_bindgen::JsValue::from_str(detail));
    if let Some(window) = web_sys::window()
        && let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(name, &init)
    {
        let _ = window.dispatch_event(&event);
    }
}

fn is_mac() -> bool {
    web_sys::window()
        .and_then(|w| w.navigator().platform().ok())
        .is_some_and(|platform| platform.starts_with("Mac"))
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn storage_get(key: &str) -> Option<String> {
    storage()?.get_item(key).ok().flatten()
}

fn storage_set(key: &str, value: &str) {
    if let Some(storage) = storage() {
        let _ = storage.set_item(key, value);
    }
}
