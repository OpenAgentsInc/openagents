//! The chat window: the account's chats and projects in the sidebar, the
//! open chat's transcript, and the composer. Every read and write goes to
//! the site's JSON API; an answer streams in over its event stream.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt as _;
use gpui::{
    AnyElement, App, ClickEvent, Context, Entity, FocusHandle, Focusable, FollowMode, FontWeight,
    Hsla, Image, IntoElement, ListAlignment, ListState, MouseButton, MouseMoveEvent, MouseUpEvent,
    ParentElement, Render, SharedString, SharedUri, Styled, Task, Window, div, list, prelude::*,
    px,
};
use openagents_doc::{MessagePart, MessageRole, MessageStatus, SessionMessageEntry};
use openagents_gpui_web::img;
use openagents_web_chat_view::composer::{Cancelled, Submitted, TextInput};
use openagents_web_chat_view::markdown::{self, Veil};
use openagents_web_chat_view::svg;
use openagents_web_chat_view::transcript::{
    self, CONTENT_WIDTH, OnClick, SIDEBAR_WIDTH, TurnView, ago, button, chip, footer_text,
    info_card, truncate,
};
use openagents_web_chat_view::{composer::Copy, selection};
use openagents_web_chat_view::{now_ms, theme};
use openagents_web_client::web::{Client, Follow};
use openagents_web_client::{Change, Event, MessageRole as SentBy, Project, Refusal, Thread};
use web_time::Instant;

use crate::model::{self, Group, Msg};
use crate::route;

/// How often the sidebar re-reads the account's chats.
const REFRESH: Duration = Duration::from_secs(5);
/// The composer's longest message, as the site takes it.
const MAX_CHARS: usize = 4_000;

/// The answer being written into the open chat.
struct Live {
    chat: String,
    request: String,
    /// The answer's messages by their position in the chat.
    messages: BTreeMap<usize, (String, String)>,
    cards: Vec<serde_json::Value>,
    /// The fade on the text now arriving.
    veil: Veil,
    started: Instant,
    stopping: bool,
    _follow: Option<Follow>,
    _task: Task<()>,
}

/// A row's open menu, and what it's asking.
#[derive(Clone, PartialEq)]
enum Menu {
    Actions(String),
    Move(String),
    Delete(String),
}

pub struct ChatApp {
    client: Client,
    /// Signed in, or chatting as the browser's visitor (the page's
    /// `openagents-signed-in`).
    signed_in: bool,
    origin: String,
    threads: Vec<Thread>,
    projects: Vec<Project>,
    loaded: bool,
    /// The open chat; `None` is a new chat.
    current: Option<String>,
    /// The open chat's finished entries.
    entries: Vec<SessionMessageEntry>,
    loading_chat: bool,
    live: Option<Live>,
    list_state: ListState,
    composer: Entity<TextInput>,
    search: Entity<TextInput>,
    filter: String,
    rename: Option<(String, Entity<TextInput>)>,
    menu: Option<Menu>,
    /// The project a new chat starts in.
    new_project: Option<String>,
    sending: bool,
    notice: Option<String>,
    sidebar_open: bool,
    focus_handle: FocusHandle,
    expanded: HashSet<String>,
    images: HashMap<String, Arc<Image>>,
    /// The cards each finished answer came with, by entry id.
    entry_cards: HashMap<String, Vec<serde_json::Value>>,
    /// Bumped on each open, so a slow read for a chat left behind is dropped.
    generation: u64,
    _alerts: Option<crate::notifications::browser::Alerts>,
    _refresh: Task<()>,
    _ticker: Task<()>,
    _history: Task<()>,
    /// A transcript selection drag, scrolling at the edges (#12305).
    selection_drag: selection::DragScroll,
}

impl ChatApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer = cx.new(|cx| TextInput::multiline("Ask anything", cx));
        cx.subscribe_in(
            &composer,
            window,
            |this, _, event: &Submitted, window, cx| {
                this.send(event.0.clone(), window, cx);
            },
        )
        .detach();
        let search = cx.new(|cx| TextInput::new("Search chats", cx));
        cx.observe(&search, |this, search, cx| {
            let text = search.read(cx).text();
            if text != this.filter {
                this.filter = text;
                cx.notify();
            }
        })
        .detach();
        cx.subscribe(&search, |this, search, _: &Cancelled, cx| {
            search.update(cx, |search, cx| {
                search.take(cx);
            });
            this.filter.clear();
            cx.notify();
        })
        .detach();

        let refresh = cx.spawn(async move |this, cx| {
            loop {
                let Ok(client) = this.read_with(cx, |this, _| this.client.clone()) else {
                    break;
                };
                let threads = client.threads().await;
                if this
                    .update(cx, |this, cx| match threads {
                        Ok(list) => {
                            this.loaded = true;
                            this.threads = list.threads;
                            cx.notify();
                        }
                        Err(error) => this.refused(&error, cx),
                    })
                    .is_err()
                {
                    break;
                }
                cx.background_executor().timer(REFRESH).await;
            }
        });
        // Frames while an answer streams: the veil's fade and the pulse.
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(32))
                    .await;
                let Ok(()) = this.update(cx, |this, cx| {
                    if this.live.is_some() {
                        let at = this.entries.len();
                        this.list_state.remeasure_items(at..at + 1);
                        cx.notify();
                    }
                }) else {
                    break;
                };
            }
        });
        let mut back = route::watch();
        let history = cx.spawn(async move |this, cx| {
            while let Some(chat) = back.next().await {
                if this.update(cx, |this, cx| this.load(chat, cx)).is_err() {
                    break;
                }
            }
        });

        let origin = web_sys::window()
            .and_then(|w| w.location().origin().ok())
            .unwrap_or_default();
        let mut this = Self {
            client: Client::from_page(),
            signed_in: route::signed_in(),
            origin,
            threads: Vec::new(),
            projects: Vec::new(),
            loaded: false,
            current: None,
            entries: Vec::new(),
            loading_chat: false,
            live: None,
            list_state: ListState::new(0, ListAlignment::Top, px(1200.)),
            composer,
            search,
            filter: String::new(),
            rename: None,
            menu: None,
            new_project: None,
            sending: false,
            notice: None,
            sidebar_open: false,
            focus_handle: cx.focus_handle(),
            expanded: HashSet::new(),
            images: HashMap::new(),
            entry_cards: HashMap::new(),
            generation: 0,
            _alerts: None,
            _refresh: refresh,
            _ticker: ticker,
            _history: history,
            selection_drag: selection::DragScroll::default(),
        };
        if this.signed_in {
            let weak = cx.entity().downgrade();
            let mut async_cx = cx.to_async();
            this._alerts = crate::notifications::browser::subscribe(move |text| {
                let _ = weak.update(&mut async_cx, |this, cx| {
                    this.notice = Some(text);
                    cx.notify();
                });
            });
        }
        this.load_projects(cx);
        this.load(route::current(), cx);
        window.focus(&this.composer.focus_handle(cx), cx);
        this
    }

    /// A transcript selection drag moved: follow it, and scroll while it
    /// holds near the top or bottom edge, as the desktop does (#12305).
    fn on_selection_move(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let list = self.list_state.clone();
        self.selection_drag
            .moved(event, &list, |this: &mut Self| &mut this.selection_drag, cx);
    }

    fn on_selection_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.selection_drag.released(cx);
    }

    fn thread(&self, id: &str) -> Option<&Thread> {
        self.threads.iter().find(|t| t.id == id)
    }

    /// Says what went wrong, or signs in again when the sign-in ended.
    fn refused(&mut self, error: &anyhow::Error, cx: &mut Context<Self>) {
        log::warn!("chat request failed: {error:#}");
        match error.downcast_ref::<Refusal>() {
            Some(refusal)
                if self.signed_in
                    && (refusal.code == "signed_out" || refusal.code == "http_401") =>
            {
                route::sign_in();
            }
            Some(refusal) => self.notice = Some(refusal.message.clone()),
            None => self.notice = Some("Something went wrong. Try again.".into()),
        }
        cx.notify();
    }

    fn load_projects(&mut self, cx: &mut Context<Self>) {
        let client = self.client.clone();
        cx.spawn(async move |this, cx| {
            // A site without projects answers with a refusal; the sidebar
            // then just has no project groups.
            if let Ok(projects) = client.projects().await {
                this.update(cx, |this, cx| {
                    this.projects = projects;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn open(&mut self, chat: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        route::push(chat.as_deref());
        self.sidebar_open = false;
        self.load(chat, cx);
        window.focus(&self.composer.focus_handle(cx), cx);
    }

    /// Shows `chat`, reading its messages, and follows its answer when one
    /// is being written.
    fn load(&mut self, chat: Option<String>, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        self.menu = None;
        self.notice = None;
        if self.live.as_ref().map(|l| &l.chat) != chat.as_ref() {
            self.live = None;
        }
        self.current = chat.clone();
        self.entries.clear();
        self.expanded.clear();
        self.list_state.reset(0);
        let placeholder = if chat.is_some() {
            "Ask anything"
        } else {
            "What should we work on?"
        };
        self.composer
            .update(cx, |composer, _| composer.set_placeholder(placeholder));
        let Some(id) = chat else {
            route::set_title("New chat");
            self.loading_chat = false;
            cx.notify();
            return;
        };
        self.loading_chat = true;
        cx.notify();
        let client = self.client.clone();
        cx.spawn(async move |this, cx| {
            let read = async {
                let detail = client.thread(&id).await?;
                let rows = client.messages(&id).await?;
                anyhow::Ok((detail, rows))
            }
            .await;
            this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.loading_chat = false;
                match read {
                    Ok((detail, rows)) => {
                        let thread = detail.thread.clone();
                        route::set_title(&thread.title);
                        match this.threads.iter_mut().find(|t| t.id == thread.id) {
                            Some(row) => *row = thread.clone(),
                            None => this.threads.insert(0, thread.clone()),
                        }
                        let messages = model::messages(&id, &rows, &detail.messages);
                        let pending = thread.pending() && !thread.synced();
                        let (kept, live) = model::split_pending(messages, pending);
                        this.show(&kept);
                        match (pending, thread.request_id.clone()) {
                            (true, Some(request)) => this.follow(id.clone(), request, live, cx),
                            _ => this.live = None,
                        }
                    }
                    Err(error) => this.refused(&error, cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn show(&mut self, messages: &[Msg]) {
        self.entries = model::entries(messages);
        self.sync_list();
    }

    /// Tells the list how many rows there are (the entries, and the answer
    /// being written), and keeps it at the bottom.
    fn sync_list(&mut self) {
        let count = self.entries.len() + usize::from(self.live.is_some());
        let known = self.list_state.item_count();
        if count > known {
            self.list_state.splice(known..known, count - known);
        } else if count < known {
            self.list_state.reset(count);
        }
        self.list_state.set_follow_mode(FollowMode::Tail);
        self.list_state.scroll_to_end();
    }

    fn follow(
        &mut self,
        chat: String,
        request: String,
        seed: BTreeMap<usize, (String, String)>,
        cx: &mut Context<Self>,
    ) {
        let (follow, mut events) = match self.client.follow(&chat, &request) {
            Ok(pair) => pair,
            Err(_) => {
                self.notice = Some("We couldn't follow this answer. Reload to try again.".into());
                return;
            }
        };
        let task = cx.spawn(async move |this, cx| {
            while let Some(event) = events.next().await {
                let ends = event.ends();
                if this
                    .update(cx, |this, cx| this.on_event(event, cx))
                    .is_err()
                    || ends
                {
                    break;
                }
            }
        });
        self.live = Some(Live {
            chat,
            request,
            messages: seed,
            cards: Vec::new(),
            veil: markdown::new_veil(),
            started: Instant::now(),
            stopping: false,
            _follow: Some(follow),
            _task: task,
        });
        self.sync_list();
        cx.notify();
    }

    fn on_event(&mut self, event: Event, cx: &mut Context<Self>) {
        let Some(live) = &mut self.live else {
            return;
        };
        match event {
            Event::Text {
                index,
                role,
                text,
                replace,
                ..
            }
            | Event::Tool {
                index,
                role,
                text,
                replace,
                ..
            } => {
                // The person's own message is already in the transcript.
                if role == SentBy::User {
                    return;
                }
                let role = role.as_str().to_owned();
                let slot = live
                    .messages
                    .entry(index)
                    .or_insert_with(|| (role.clone(), String::new()));
                if replace {
                    slot.1 = text;
                } else {
                    slot.1.push_str(&text);
                }
            }
            Event::Cards { .. } => live.cards = event.card_list(),
            Event::Done { outcome, .. } => self.finish(outcome == "stopped", None, cx),
            Event::Error { error, .. } => self.finish(false, Some(error.message), cx),
        }
        let at = self.entries.len();
        self.list_state.remeasure_items(at..at + 1);
        cx.notify();
    }

    /// Moves the finished answer into the transcript.
    fn finish(&mut self, stopped: bool, error: Option<String>, cx: &mut Context<Self>) {
        let Some(live) = self.live.take() else {
            return;
        };
        let mut parts = model::answer_parts(
            live.messages
                .iter()
                .map(|(index, (role, text))| (*index, role.as_str(), text.as_str())),
        );
        if let Some(message) = error {
            parts.push(MessagePart::Error {
                id: "error".into(),
                message,
            });
        }
        let id = format!("a-{}", live.request);
        if !live.cards.is_empty() {
            self.entry_cards.insert(id.clone(), live.cards.clone());
        }
        self.entries.push(SessionMessageEntry {
            id,
            role: MessageRole::Assistant,
            parts,
            created_at: 0,
            device_id: String::new(),
            status: Some(if stopped {
                MessageStatus::Aborted
            } else {
                MessageStatus::Complete
            }),
            continuation_of: None,
            duration_ms: Some(live.started.elapsed().as_millis() as i64),
        });
        if let Some(thread) = self.threads.iter_mut().find(|t| t.id == live.chat) {
            thread.working = false;
            thread.outcome = Some(if stopped { "stopped" } else { "answered" }.into());
        }
        // The list keeps its row count: the answer's row is now an entry.
        let at = self.entries.len() - 1;
        self.list_state.remeasure_items(at..at + 1);
        self.sync_list();
        cx.notify();
    }

    fn send(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        let text = text.trim().to_owned();
        if text.is_empty() || self.sending || self.live.is_some() {
            return;
        }
        if text.chars().count() > MAX_CHARS {
            self.notice = Some("Messages can be at most 4,000 characters.".into());
            cx.notify();
            return;
        }
        if let Some(thread) = self.current.as_deref().and_then(|id| self.thread(id))
            && !thread.can_reply
        {
            self.notice = Some(reply_refusal(thread).into());
            cx.notify();
            return;
        }
        let request = uuid::Uuid::new_v4().to_string();
        let index = self.entries.len();
        self.entries.push(SessionMessageEntry {
            id: format!("u-{request}"),
            role: MessageRole::User,
            parts: vec![MessagePart::Text {
                id: "t".into(),
                text: text.clone(),
            }],
            created_at: 0,
            device_id: String::new(),
            status: Some(MessageStatus::Complete),
            continuation_of: None,
            duration_ms: None,
        });
        self.sync_list();
        self.sending = true;
        self.notice = None;
        let client = self.client.clone();
        let current = self.current.clone();
        let project = self.new_project.clone();
        cx.spawn(async move |this, cx| {
            let sent = match &current {
                Some(id) => client.reply(id, &request, &text).await.map(|()| id.clone()),
                None => client.start(&request, &text, project.as_deref()).await,
            };
            this.update(cx, |this, cx| {
                this.sending = false;
                match sent {
                    Ok(id) => {
                        if current.is_none() {
                            // The new chat's own address, without reloading it.
                            route::push(Some(&id));
                            route::set_title(&model::title_from(&text));
                            this.current = Some(id.clone());
                            this.generation += 1;
                            this.composer
                                .update(cx, |composer, _| composer.set_placeholder("Ask anything"));
                            this.threads.insert(
                                0,
                                Thread {
                                    id: id.clone(),
                                    title: model::title_from(&text),
                                    surface: "web".into(),
                                    can_reply: true,
                                    updated_unix: (now_ms() / 1000) as u64,
                                    project: project.clone(),
                                    ..Default::default()
                                },
                            );
                            this.new_project = None;
                        }
                        if let Some(thread) = this.threads.iter_mut().find(|t| t.id == id) {
                            thread.working = true;
                            thread.request_id = Some(request.clone());
                            thread.outcome = Some("pending".into());
                        }
                        this.follow(id, request, BTreeMap::new(), cx);
                    }
                    Err(error) => {
                        // Put the words back so nothing typed is lost.
                        if this.entries.len() == index + 1 {
                            this.entries.pop();
                            this.sync_list();
                        }
                        this.composer.update(cx, |composer, cx| {
                            if composer.is_empty() {
                                composer.set_text(text.clone(), cx);
                            }
                        });
                        this.refused(&error, cx);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        window.focus(&self.composer.focus_handle(cx), cx);
        cx.notify();
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        let Some(live) = &mut self.live else {
            return;
        };
        if live.stopping {
            return;
        }
        live.stopping = true;
        let client = self.client.clone();
        let (chat, request) = (live.chat.clone(), live.request.clone());
        cx.spawn(async move |this, cx| {
            if let Err(error) = client.stop(&chat, &request).await {
                this.update(cx, |this, cx| {
                    if let Some(live) = &mut this.live {
                        live.stopping = false;
                    }
                    this.refused(&error, cx);
                })
                .ok();
            }
        })
        .detach();
        cx.notify();
    }

    /// Saves a change to a chat, showing it at once and reading the list
    /// again after.
    fn change(&mut self, id: String, change: Change, cx: &mut Context<Self>) {
        if let Some(thread) = self.threads.iter_mut().find(|t| t.id == id) {
            if let Some(title) = &change.title {
                thread.title = title.clone();
            }
            if let Some(pinned) = change.pinned {
                thread.pinned = pinned;
            }
            if let Some(project) = &change.project {
                thread.project = project.clone();
            }
        }
        if change.archived == Some(true) {
            self.threads.retain(|t| t.id != id);
        }
        self.menu = None;
        let leaving = change.archived == Some(true) && self.current.as_deref() == Some(&id);
        let client = self.client.clone();
        cx.spawn(async move |this, cx| {
            let saved = client.change(&id, &change).await;
            let list = client.threads().await;
            this.update(cx, |this, cx| {
                if let Err(error) = saved {
                    this.refused(&error, cx);
                }
                if let Ok(list) = list {
                    this.threads = list.threads;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        if leaving {
            route::push(None);
            self.load(None, cx);
        }
        cx.notify();
    }

    fn delete(&mut self, id: String, cx: &mut Context<Self>) {
        self.menu = None;
        self.threads.retain(|t| t.id != id);
        if self.current.as_deref() == Some(&id) {
            route::push(None);
            self.load(None, cx);
        }
        let client = self.client.clone();
        cx.spawn(async move |this, cx| {
            let deleted = client.delete(&id).await;
            this.update(cx, |this, cx| {
                if let Err(error) = deleted {
                    this.refused(&error, cx);
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn start_rename(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let title = self
            .thread(&id)
            .map(|t| t.title.clone())
            .unwrap_or_default();
        let field = cx.new(|cx| {
            let mut field = TextInput::new("Chat name", cx);
            field.set_text(title, cx);
            field
        });
        let renamed = id.clone();
        cx.subscribe(&field, move |this, _, event: &Submitted, cx| {
            let title = event.0.trim().to_owned();
            this.rename = None;
            if !title.is_empty() {
                let title = truncate(&title, 120);
                this.change(
                    renamed.clone(),
                    Change {
                        title: Some(title),
                        ..Default::default()
                    },
                    cx,
                );
            }
            cx.notify();
        })
        .detach();
        cx.subscribe(&field, |this, _, _: &Cancelled, cx| {
            this.rename = None;
            cx.notify();
        })
        .detach();
        window.focus(&field.focus_handle(cx), cx);
        self.menu = None;
        self.rename = Some((id, field));
        cx.notify();
    }

    // -----------------------------------------------------------------
    // Sidebar
    // -----------------------------------------------------------------

    fn render_sidebar(&self, full: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let groups = model::groups(&self.threads, &self.projects, &self.filter);
        let empty_line = if !self.loaded {
            Some("Loading your chats…")
        } else if self.threads.is_empty() {
            Some("Your chats show here.")
        } else if groups.is_empty() {
            Some("No chat has that name.")
        } else {
            None
        };
        div()
            .flex()
            .flex_col()
            .when(full, |d| d.w_full())
            .when(!full, |d| d.w(px(SIDEBAR_WIDTH)))
            .flex_none()
            .h_full()
            .bg(theme::surface())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .px(px(18.))
                    .pt(px(18.))
                    .pb(px(10.))
                    .child(
                        svg()
                            .path("icons/openagents-logo.svg")
                            .size(px(18.))
                            .text_color(theme::text()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("OpenAgents"),
                    )
                    .when(full, |row| {
                        row.child(icon_button("close-sidebar", "icons/close.svg").on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.sidebar_open = false;
                                cx.notify();
                            }),
                        ))
                    }),
            )
            .child(
                div()
                    .id("new-chat")
                    .mx(px(10.))
                    .px(px(8.))
                    .h(px(34.))
                    .rounded(px(8.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .text_color(theme::text_muted())
                    .text_size(px(14.))
                    .cursor_pointer()
                    .when(self.current.is_none(), |row| {
                        row.bg(theme::element_active())
                    })
                    .hover(|style| style.bg(theme::element_hover()).text_color(theme::text()))
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, window, cx| this.open(None, window, cx)),
                    )
                    .child(
                        svg()
                            .path("icons/pen-new-square.svg")
                            .size(px(17.))
                            .text_color(theme::text_muted()),
                    )
                    .child("New chat"),
            )
            .child(
                div()
                    .mx(px(10.))
                    .mt(px(6.))
                    .px(px(8.))
                    .h(px(34.))
                    .rounded(px(8.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .border_1()
                    .border_color(theme::border())
                    .bg(theme::input_bg())
                    .child(
                        svg()
                            .path("icons/magnifer.svg")
                            .size(px(15.))
                            .text_color(theme::text_faint()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(13.5))
                            .child(self.search.clone()),
                    ),
            )
            .child(
                div()
                    .id("sidebar-chats")
                    .flex_1()
                    .overflow_y_scroll()
                    .px(px(10.))
                    .pb(px(16.))
                    .children(empty_line.map(|line| {
                        div()
                            .px(px(8.))
                            .pt(px(16.))
                            .text_size(px(13.))
                            .text_color(theme::text_faint())
                            .child(line)
                    }))
                    .children(groups.into_iter().map(|(group, members)| {
                        div()
                            .flex()
                            .flex_col()
                            .pt(px(14.))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(6.))
                                    .px(px(8.))
                                    .pb(px(4.))
                                    .text_size(px(12.5))
                                    .text_color(theme::text_faint())
                                    .when(matches!(group, Group::Project(..)), |row| {
                                        row.child(
                                            svg()
                                                .path("icons/folder.svg")
                                                .size(px(13.))
                                                .text_color(theme::text_faint()),
                                        )
                                    })
                                    .child(group.label()),
                            )
                            .children(members.into_iter().map(|ix| self.render_row(ix, cx)))
                    })),
            )
            .when(!self.signed_in, |side| {
                side.child(
                    div()
                        .id("sign-in")
                        .px(px(18.))
                        .py(px(14.))
                        .border_t_1()
                        .border_color(theme::border())
                        .text_size(px(13.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::text())
                        .cursor_pointer()
                        .hover(|style| style.bg(theme::surface_card()))
                        .on_click(|_: &ClickEvent, _, _| route::sign_in())
                        .child("Sign in to keep your chats"),
                )
            })
            .child(
                div()
                    .id("regular-chat")
                    .px(px(18.))
                    .py(px(14.))
                    .border_t_1()
                    .border_color(theme::border())
                    .text_size(px(12.5))
                    .text_color(theme::text_faint())
                    .cursor_pointer()
                    .hover(|style| style.text_color(theme::text()))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, _| {
                        route::go(&route::regular(this.current.as_deref()));
                    }))
                    .child("Open the regular chat"),
            )
    }

    fn render_row(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let thread = &self.threads[ix];
        let id = thread.id.clone();
        let selected = self.current.as_deref() == Some(id.as_str());
        let live_here = self.live.as_ref().is_some_and(|l| l.chat == id);
        let dot: Option<Hsla> = if thread.working || live_here {
            Some(theme::accent())
        } else if matches!(thread.outcome.as_deref(), Some("failed" | "unknown")) {
            Some(theme::danger())
        } else {
            None
        };
        let menu_open = matches!(&self.menu, Some(Menu::Actions(m) | Menu::Move(m) | Menu::Delete(m)) if *m == id);
        let renaming = self
            .rename
            .as_ref()
            .filter(|(r, _)| *r == id)
            .map(|(_, f)| f.clone());
        let subtitle = if thread.line.is_empty() {
            match thread.surface.as_str() {
                "terminal" => thread.computer.clone().unwrap_or_else(|| "Computer".into()),
                "phone" => "Phone".into(),
                _ => String::new(),
            }
        } else {
            thread.line.clone()
        };
        let open_id = id.clone();
        let menu_id = id.clone();
        let mut row = div()
            .id(SharedString::from(format!("row-{id}")))
            .group("row")
            .relative()
            .flex()
            .flex_row()
            .gap(px(8.))
            .px(px(8.))
            .py(px(7.))
            .rounded(px(9.))
            .cursor_pointer()
            .when(selected, |row| row.bg(theme::element_active()))
            .when(!selected, |row| {
                row.hover(|style| style.bg(theme::element_hover()))
            })
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                if this.current.as_deref() != Some(open_id.as_str()) {
                    this.open(Some(open_id.clone()), window, cx);
                }
            }))
            .child(
                div()
                    .flex_none()
                    .w(px(6.))
                    .h(px(20.))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .size(px(6.))
                            .rounded_full()
                            .bg(dot.unwrap_or(gpui::transparent_black())),
                    ),
            );
        row = match renaming {
            Some(field) => row.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px(px(6.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(theme::accent().opacity(0.6))
                    .text_size(px(14.))
                    .child(field),
            ),
            None => row.child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(14.))
                                    .line_height(px(20.))
                                    .text_color(if selected {
                                        theme::text()
                                    } else {
                                        theme::text_muted()
                                    })
                                    .child(if thread.title.is_empty() {
                                        "New chat".to_owned()
                                    } else {
                                        thread.title.clone()
                                    }),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(px(12.))
                                    .line_height(px(20.))
                                    .text_color(theme::text_faint())
                                    .child(ago(now_ms() - thread.updated_unix as i64 * 1000)),
                            )
                            .child(
                                div()
                                    .id(SharedString::from(format!("more-{id}")))
                                    .flex_none()
                                    .size(px(20.))
                                    .rounded(px(5.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(!menu_open, |b| {
                                        b.opacity(0.).group_hover("row", |s| s.opacity(1.))
                                    })
                                    .hover(|s| s.bg(theme::element_active()))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        cx.stop_propagation();
                                        this.menu = match &this.menu {
                                            Some(
                                                Menu::Actions(m) | Menu::Move(m) | Menu::Delete(m),
                                            ) if *m == menu_id => None,
                                            _ => Some(Menu::Actions(menu_id.clone())),
                                        };
                                        cx.notify();
                                    }))
                                    .child(
                                        svg()
                                            .path("icons/more-horizontal.svg")
                                            .size(px(14.))
                                            .text_color(theme::text_muted()),
                                    ),
                            ),
                    )
                    .when(!subtitle.is_empty(), |col| {
                        col.child(
                            div()
                                .truncate()
                                .text_size(px(12.))
                                .line_height(px(17.))
                                .text_color(theme::text_faint())
                                .child(subtitle),
                        )
                    }),
            ),
        };
        let menu = self.menu.clone().filter(|_| menu_open);
        div()
            .flex()
            .flex_col()
            .child(row)
            .children(menu.map(|menu| self.render_menu(thread, menu, cx)))
            .into_any_element()
    }

    fn render_menu(&self, thread: &Thread, menu: Menu, cx: &mut Context<Self>) -> AnyElement {
        let id = thread.id.clone();
        let mut items: Vec<AnyElement> = Vec::new();
        match menu {
            Menu::Actions(_) => {
                let rename = id.clone();
                items.push(
                    menu_item("menu-rename", "icons/pen.svg", "Rename")
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.start_rename(rename.clone(), window, cx)
                        }))
                        .into_any_element(),
                );
                let pin = id.clone();
                let pinned = thread.pinned;
                items.push(
                    menu_item(
                        "menu-pin",
                        "icons/pin.svg",
                        if pinned { "Unpin" } else { "Pin" },
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.change(
                            pin.clone(),
                            Change {
                                pinned: Some(!pinned),
                                ..Default::default()
                            },
                            cx,
                        )
                    }))
                    .into_any_element(),
                );
                if !self.projects.is_empty() && !thread.synced() {
                    let moving = id.clone();
                    items.push(
                        menu_item("menu-move", "icons/folder.svg", "Move to project")
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.menu = Some(Menu::Move(moving.clone()));
                                cx.notify();
                            }))
                            .into_any_element(),
                    );
                }
                let archive = id.clone();
                items.push(
                    menu_item("menu-archive", "icons/archive-minimalistic.svg", "Archive")
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.change(
                                archive.clone(),
                                Change {
                                    archived: Some(true),
                                    ..Default::default()
                                },
                                cx,
                            )
                        }))
                        .into_any_element(),
                );
                let delete = id.clone();
                items.push(
                    menu_item("menu-delete", "icons/trash-bin-minimalistic.svg", "Delete")
                        .text_color(theme::danger())
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.menu = Some(Menu::Delete(delete.clone()));
                            cx.notify();
                        }))
                        .into_any_element(),
                );
            }
            Menu::Move(_) => {
                let none = id.clone();
                items.push(
                    menu_item("move-none", "icons/close.svg", "No project")
                        .when(thread.project.is_none(), |b| b.bg(theme::element_active()))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.change(
                                none.clone(),
                                Change {
                                    project: Some(None),
                                    ..Default::default()
                                },
                                cx,
                            )
                        }))
                        .into_any_element(),
                );
                for project in self.projects.iter().filter(|p| p.editable()) {
                    let target = id.clone();
                    let project_id = project.id.clone();
                    let here = thread.project.as_deref() == Some(project.id.as_str());
                    items.push(
                        div()
                            .id(SharedString::from(format!("move-{}", project.id)))
                            .child(
                                menu_item_label("icons/folder.svg", project.name.clone())
                                    .when(here, |b| b.bg(theme::element_active())),
                            )
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.change(
                                    target.clone(),
                                    Change {
                                        project: Some(Some(project_id.clone())),
                                        ..Default::default()
                                    },
                                    cx,
                                )
                            }))
                            .into_any_element(),
                    );
                }
            }
            Menu::Delete(_) => {
                let delete = id.clone();
                items.push(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .p(px(8.))
                        .child(
                            div()
                                .text_size(px(13.))
                                .text_color(theme::text_muted())
                                .child("Delete this chat for good?"),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(8.))
                                .child(button("confirm-delete", "Delete", true).on_click(
                                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.delete(delete.clone(), cx)
                                    }),
                                ))
                                .child(button("cancel-delete", "Cancel", false).on_click(
                                    cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.menu = None;
                                        cx.notify();
                                    }),
                                )),
                        )
                        .into_any_element(),
                );
            }
        }
        div()
            .mx(px(4.))
            .my(px(4.))
            .p(px(4.))
            .rounded(px(10.))
            .border_1()
            .border_color(theme::border_strong())
            .bg(theme::surface_card())
            .flex()
            .flex_col()
            .children(items)
            .into_any_element()
    }

    // -----------------------------------------------------------------
    // Header and composer
    // -----------------------------------------------------------------

    fn render_header(&self, wide: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let thread = self.current.as_deref().and_then(|id| self.thread(id));
        let title = match (thread, &self.current) {
            (Some(thread), _) if !thread.title.is_empty() => thread.title.clone(),
            (_, Some(_)) => "Chat".into(),
            (_, None) => "New chat".into(),
        };
        let project = thread
            .and_then(|t| t.project.as_deref())
            .and_then(|p| self.projects.iter().find(|x| x.id == p))
            .map(|p| p.name.clone());
        let computer = thread
            .filter(|t| t.synced())
            .and_then(|t| t.computer.clone());
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.))
            .h(px(52.))
            .px(px(16.))
            .border_b_1()
            .border_color(theme::border())
            .when(!wide, |row| {
                row.child(
                    icon_button("open-sidebar", "icons/chat-round-line.svg").on_click(cx.listener(
                        |this, _: &ClickEvent, _, cx| {
                            this.sidebar_open = true;
                            cx.notify();
                        },
                    )),
                )
            })
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .truncate()
                    .text_size(px(14.5))
                    .font_weight(FontWeight::MEDIUM)
                    .child(title),
            )
            .children(project.map(|p| chip("icons/folder.svg", p, theme::text_muted())))
            .children(computer.map(|c| chip("icons/laptop.svg", c, theme::text_muted())))
            .when(!wide && self.current.is_some(), |row| {
                row.child(
                    icon_button("header-new-chat", "icons/pen-new-square.svg").on_click(
                        cx.listener(|this, _: &ClickEvent, window, cx| this.open(None, window, cx)),
                    ),
                )
            })
    }

    fn render_composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let live = self.live.is_some();
        let busy = live || self.sending;
        let empty = self.composer.read(cx).is_empty();
        let blocked = self
            .current
            .as_deref()
            .and_then(|id| self.thread(id))
            .filter(|t| !t.can_reply && !live)
            .map(reply_refusal);
        let send = div()
            .id("send")
            .flex_none()
            .size(px(34.))
            .rounded_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::solid())
            .cursor_pointer()
            .when((!live && empty) || self.sending || blocked.is_some(), |b| {
                b.opacity(0.35)
            })
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                if this.live.is_some() {
                    this.stop(cx);
                } else {
                    let text = this.composer.update(cx, |composer, cx| composer.take(cx));
                    this.send(text, window, cx);
                }
            }))
            .child(
                svg()
                    .path(if busy && live {
                        "icons/stop.svg"
                    } else {
                        "icons/arrow-up.svg"
                    })
                    .size(px(17.))
                    .text_color(theme::on_solid()),
            );
        div()
            .flex()
            .flex_col()
            .items_center()
            .px(px(16.))
            .pb(px(16.))
            .pt(px(6.))
            .gap(px(8.))
            .children(blocked.map(|line| footer_text(line)))
            .child(
                div()
                    .w_full()
                    .max_w(px(CONTENT_WIDTH + 24.))
                    .flex()
                    .flex_row()
                    .items_end()
                    .gap(px(14.))
                    .pl(px(20.))
                    .pr(px(8.))
                    .py(px(11.))
                    .min_h(px(56.))
                    .rounded(px(28.))
                    .border_1()
                    .border_color(theme::border_strong())
                    .bg(theme::surface_card())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .py(px(5.))
                            .child(self.composer.clone()),
                    )
                    .child(send),
            )
    }

    fn render_new_chat(&self, cx: &mut Context<Self>) -> AnyElement {
        let projects: Vec<&Project> = self.projects.iter().filter(|p| p.editable()).collect();
        let mut choices: Vec<AnyElement> = Vec::new();
        if !projects.is_empty() {
            choices.push(
                project_choice("project-none", "No project", self.new_project.is_none())
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.new_project = None;
                        cx.notify();
                    }))
                    .into_any_element(),
            );
            for project in projects {
                let id = project.id.clone();
                let chosen = self.new_project.as_deref() == Some(project.id.as_str());
                choices.push(
                    div()
                        .id(SharedString::from(format!("project-{}", project.id)))
                        .child(project_choice_label(project.name.clone(), chosen))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.new_project = Some(id.clone());
                            cx.notify();
                        }))
                        .into_any_element(),
                );
            }
        }
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(14.))
            .px(px(24.))
            .child(
                svg()
                    .path("icons/openagents-logo.svg")
                    .size(px(30.))
                    .text_color(theme::text_faint()),
            )
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .child("What should we work on?"),
            )
            .when(!choices.is_empty(), |col| {
                col.child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .justify_center()
                        .gap(px(8.))
                        .max_w(px(CONTENT_WIDTH))
                        .children(choices),
                )
            })
            .into_any_element()
    }

    // -----------------------------------------------------------------
    // Transcript
    // -----------------------------------------------------------------

    fn render_entry(
        &mut self,
        ix: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if ix == self.entries.len() {
            return match &self.live {
                Some(live) => {
                    let entry = live_entry(live);
                    let cards = live.cards.clone();
                    let body = self.render_assistant(&entry, live_veil(live), &cards, cx);
                    transcript::entry_row(ix == 0, body)
                }
                None => div().into_any_element(),
            };
        }
        let Some(entry) = self.entries.get(ix) else {
            return div().into_any_element();
        };
        let body = match entry.role {
            MessageRole::User => self.render_user(entry),
            _ => {
                let cards = self.entry_cards.get(&entry.id).cloned().unwrap_or_default();
                self.render_assistant(entry, None, &cards, cx)
            }
        };
        transcript::entry_row(ix == 0, body)
    }

    fn render_user(&self, entry: &SessionMessageEntry) -> AnyElement {
        let mut text = String::new();
        let mut attachments = Vec::new();
        for part in &entry.parts {
            match part {
                MessagePart::Text { text: t, .. } => text.push_str(t),
                MessagePart::Image { path, name, .. } => {
                    let uri = SharedUri::from(format!("{}{path}", self.origin));
                    attachments.push(
                        img(uri)
                            .h(px(120.))
                            .max_w(px(220.))
                            .rounded(px(12.))
                            .object_fit(gpui::ObjectFit::Cover)
                            .with_fallback({
                                let name = name.clone();
                                move || {
                                    chip("icons/paperclip.svg", name.clone(), theme::text_muted())
                                }
                            })
                            .into_any_element(),
                    );
                }
                _ => {}
            }
        }
        transcript::user_bubble(&entry.id, &text, attachments)
    }

    fn render_assistant(
        &self,
        entry: &SessionMessageEntry,
        veil: Option<Veil>,
        cards: &[serde_json::Value],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let chat = self.current.clone().unwrap_or_default();
        let key = format!("{chat}/{}", entry.id);
        let view = TurnView {
            key: &key,
            parts: &entry.parts,
            status: entry.status,
            duration_ms: entry.duration_ms,
            images: &self.images,
            expanded: &self.expanded,
            veil,
        };
        let entity = cx.entity().downgrade();
        let toggle = move |key: String| -> OnClick {
            let entity = entity.clone();
            Box::new(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                let key = key.clone();
                entity
                    .update(cx, |this: &mut ChatApp, cx| {
                        if !this.expanded.remove(&key) {
                            this.expanded.insert(key);
                        }
                        this.list_state.remeasure();
                        cx.notify();
                    })
                    .ok();
            })
        };
        // Web chats ask no questions of their own.
        let mut questions =
            |_: usize, _: &[openagents_proto::UserInputQuestion], _: bool| div().into_any_element();
        let body = transcript::assistant_body(view, &toggle, &mut questions);
        let copy = entry.status != Some(MessageStatus::Streaming);
        let text = entry
            .parts
            .iter()
            .filter_map(|p| match p {
                MessagePart::Text { id, text } if id.starts_with('m') => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(body)
            .children(cards.iter().enumerate().map(|(ix, card)| {
                // The text view draws the card itself, live (the page's
                // `#text` opens it at the newest card).
                div()
                    .id(SharedString::from(format!("card-{key}-{ix}")))
                    .cursor_pointer()
                    .on_click(|_, _, _| route::go("#text"))
                    .child(info_card(
                        "icons/widget.svg",
                        card_title(card),
                        "Open it in the text view.".into(),
                    ))
            }))
            .when(copy && !text.is_empty(), |col| {
                col.child(
                    div()
                        .id(SharedString::from(format!("copy-{key}")))
                        .w(px(48.))
                        .cursor_pointer()
                        .text_size(px(12.5))
                        .text_color(theme::text_faint())
                        .hover(|s| s.text_color(theme::text()))
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone()))
                        })
                        .child("Copy"),
                )
            })
            .into_any_element()
    }
}

impl Focusable for ChatApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ChatApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Once the chats and the open chat have arrived, this frame is the
        // one a page snapshot shows until the app draws (`gpui-ready`), and
        // the one it keeps for the next visit (#12244).
        if self.loaded && !self.loading_chat {
            openagents_gpui_web::announce_ready();
        }
        let wide = window.viewport_size().width > px(820.);
        let rows = self.entries.len() + usize::from(self.live.is_some());
        let transcript: AnyElement = if self.current.is_none() && rows == 0 {
            self.render_new_chat(cx)
        } else if self.loading_chat && rows == 0 {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(footer_text("Loading the chat…"))
                .into_any_element()
        } else {
            div()
                .flex_1()
                .w_full()
                .flex()
                .flex_col()
                .child(markdown::selection_reset())
                .on_mouse_move(cx.listener(Self::on_selection_move))
                .on_mouse_up(MouseButton::Left, cx.listener(Self::on_selection_up))
                .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_selection_up))
                .child(
                    list(
                        self.list_state.clone(),
                        cx.processor(|this, ix: usize, window, cx| {
                            this.render_entry(ix, window, cx)
                        }),
                    )
                    .flex_1()
                    .w_full(),
                )
                .into_any_element()
        };
        let notice = self.notice.clone().map(|line| {
            div()
                .id("notice")
                .mx(px(16.))
                .mt(px(10.))
                .px(px(12.))
                .py(px(8.))
                .rounded(px(10.))
                .bg(theme::danger().opacity(0.08))
                .border_1()
                .border_color(theme::danger().opacity(0.25))
                .text_size(px(13.))
                .text_color(theme::danger())
                .cursor_pointer()
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.notice = None;
                    cx.notify();
                }))
                .child(line)
        });
        let main = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .py(px(8.))
            .pr(px(8.))
            .when(!wide, |main| main.pl(px(8.)))
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .rounded(px(14.))
                    .border_1()
                    .border_color(theme::border())
                    .bg(theme::bg())
                    .overflow_hidden()
                    .child(self.render_header(wide, cx))
                    .children(notice)
                    .child(transcript)
                    .child(self.render_composer(cx)),
            );
        let mut root = div()
            .id("openagents-web-chat")
            .track_focus(&self.focus_handle)
            // Cmd/Ctrl+C with the focus outside the composer: the
            // transcript's selection (#12305).
            .on_action(cx.listener(|_, _: &Copy, _, cx| {
                if !selection::copy(cx) {
                    cx.propagate();
                }
            }))
            .size_full()
            .flex()
            .flex_row()
            .bg(theme::surface())
            .font_family(theme::SANS)
            .text_color(theme::text())
            .text_size(px(theme::BODY));
        root.text_style().font_fallbacks = Some(theme::fallbacks());
        if wide {
            root.child(self.render_sidebar(false, cx)).child(main)
        } else if self.sidebar_open {
            root.child(self.render_sidebar(true, cx))
        } else {
            root.child(main)
        }
    }
}

// ---------------------------------------------------------------------
// Pieces
// ---------------------------------------------------------------------

fn live_entry(live: &Live) -> SessionMessageEntry {
    let parts = model::answer_parts(
        live.messages
            .iter()
            .map(|(index, (role, text))| (*index, role.as_str(), text.as_str())),
    );
    SessionMessageEntry {
        id: format!("live-{}", live.request),
        role: MessageRole::Assistant,
        parts,
        created_at: 0,
        device_id: String::new(),
        status: Some(MessageStatus::Streaming),
        continuation_of: None,
        duration_ms: None,
    }
}

fn live_veil(live: &Live) -> Option<Veil> {
    Some(live.veil.clone())
}

/// A card's title: its own, or the name of the Lab that made it. Each
/// card's own view lives in the regular chat for now.
fn card_title(card: &serde_json::Value) -> String {
    if let Some(title) = card["title"].as_str().filter(|t| !t.is_empty()) {
        return title.to_owned();
    }
    let kind = card["card"]
        .as_str()
        .or(card["schema"].as_str())
        .unwrap_or("");
    card_name(kind).to_owned()
}

fn card_name(kind: &str) -> &'static str {
    match kind {
        "video_lab" => "Video Lab",
        "game_lab" => "Game Lab",
        "cad_lab" => "CAD Lab",
        "robot_lab" => "Robot Lab",
        "ship_lab" => "Ship Lab",
        _ => "Card",
    }
}

fn reply_refusal(thread: &Thread) -> &'static str {
    match thread.surface.as_str() {
        "phone" => "This chat is on your phone. Reply there.",
        "terminal" if !thread.online => "This chat's computer is offline. Reply when it's back.",
        "terminal" => "Continue this chat on its computer.",
        _ if thread.pending() => "We're still answering the last message.",
        _ => "This chat can't take replies here.",
    }
}

fn icon_button(id: &'static str, icon: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_none()
        .size(px(32.))
        .rounded(px(8.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(theme::element_hover()))
        .child(
            svg()
                .path(icon)
                .size(px(17.))
                .text_color(theme::text_muted()),
        )
}

fn menu_item_label(icon: &'static str, label: String) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(9.))
        .px(px(8.))
        .h(px(30.))
        .rounded(px(7.))
        .text_size(px(13.5))
        .cursor_pointer()
        .hover(|s| s.bg(theme::element_hover()))
        .child(
            svg()
                .path(icon)
                .size(px(14.))
                .text_color(theme::text_muted()),
        )
        .child(div().truncate().child(label))
}

fn menu_item(
    id: &'static str,
    icon: &'static str,
    label: &'static str,
) -> gpui::Stateful<gpui::Div> {
    div().id(id).child(menu_item_label(icon, label.to_owned()))
}

fn project_choice_label(label: String, chosen: bool) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.))
        .px(px(10.))
        .h(px(30.))
        .rounded(px(15.))
        .border_1()
        .border_color(if chosen {
            theme::accent().opacity(0.6)
        } else {
            theme::border()
        })
        .when(chosen, |b| b.bg(theme::accent_wash()))
        .text_size(px(13.))
        .text_color(if chosen {
            theme::text()
        } else {
            theme::text_muted()
        })
        .cursor_pointer()
        .hover(|s| s.bg(theme::element_hover()))
        .child(
            svg()
                .path("icons/folder.svg")
                .size(px(13.))
                .text_color(theme::text_muted()),
        )
        .child(label)
}

fn project_choice(
    id: &'static str,
    label: &'static str,
    chosen: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .child(project_choice_label(label.to_owned(), chosen))
}
