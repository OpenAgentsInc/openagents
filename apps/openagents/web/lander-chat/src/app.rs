//! The page's canvas: the composer at the bottom, the welcome line over it
//! before the first message, and then the conversation in a card that sits
//! on the composer and grows upward as it goes, until it reaches the page's
//! name and scrolls.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

use futures::StreamExt as _;
use gpui::{
    AnyElement, App, ClickEvent, Context, Entity, Focusable, FontWeight, IntoElement,
    ParentElement, Render, ScrollHandle, ScrollWheelEvent, SharedString, Styled, Task, Window, div,
    prelude::*, px, svg,
};
use openagents_doc::{MessagePart, MessageRole, MessageStatus, SessionMessageEntry};
use openagents_web_chat_view::composer::{Submitted, TextInput};
use openagents_web_chat_view::markdown::{Veil, visible_chars};
use openagents_web_chat_view::theme;
use openagents_web_chat_view::transcript::{
    self, OnClick, TurnView, VEIL_FADE_MS, button, truncate, veil_alpha, working_label,
};
use openagents_web_client::web::{Client, Follow};
use openagents_web_client::{Refusal, StreamEvent};
use wasm_bindgen::JsValue;
use web_time::Instant;

use crate::model::{self, Layout, Session};

/// How long between tries to reach the site before the first message.
const RETRY: Duration = Duration::from_secs(3);
/// The tick while an answer streams or the transcript follows its end.
const TICK: Duration = Duration::from_millis(32);

/// The answer being written.
struct Live {
    request: String,
    /// The answer's messages by their position in the chat.
    messages: BTreeMap<usize, (String, String)>,
    /// Chunks of the text now arriving: visible characters, arrived at.
    chunks: Vec<(usize, Instant)>,
    started: Instant,
    stopping: bool,
    _follow: Follow,
    _task: Task<()>,
}

/// A line over the composer, and whether it offers to sign in.
struct Notice {
    text: String,
    sign_in: bool,
}

pub struct LanderChat {
    client: Option<Client>,
    session: Option<Session>,
    still: bool,
    /// The open chat; `None` before the first message.
    current: Option<String>,
    title: String,
    entries: Vec<SessionMessageEntry>,
    live: Option<Live>,
    loading: bool,
    sending: bool,
    notice: Option<Notice>,
    composer: Entity<TextInput>,
    scroll: ScrollHandle,
    /// Whether the transcript keeps its end in view.
    following: bool,
    /// The person scrolled; whether it is still at its end is checked on
    /// the next tick, after the scroll is applied.
    scrolled: bool,
    expanded: HashSet<String>,
    images: HashMap<String, std::sync::Arc<gpui::Image>>,
    /// Bumped on each load, so a slow read for a chat left behind is dropped.
    generation: u64,
    _ticker: Task<()>,
}

impl LanderChat {
    pub fn new(still: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer = cx.new(|cx| TextInput::new(model::PLACEHOLDER, cx));
        cx.subscribe_in(
            &composer,
            window,
            |this, _, event: &Submitted, window, cx| {
                this.send(event.0.clone(), window, cx);
            },
        )
        .detach();
        if let Some(draft) = take_draft() {
            composer.update(cx, |composer, cx| composer.set_text(draft, cx));
        }
        window.focus(&composer.focus_handle(cx), cx);
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                    break;
                }
            }
        });
        let mut this = Self {
            client: None,
            session: None,
            still,
            current: route_chat(),
            title: String::new(),
            entries: Vec::new(),
            live: None,
            loading: false,
            sending: false,
            notice: None,
            composer,
            scroll: ScrollHandle::new(),
            following: true,
            scrolled: false,
            expanded: HashSet::new(),
            images: HashMap::new(),
            generation: 0,
            _ticker: ticker,
        };
        this.loading = this.current.is_some();
        this.connect(cx);
        this
    }

    /// Reads the session (the CSRF value and whether the person is signed
    /// in), trying again until the site answers, then opens the chat the
    /// address names.
    fn connect(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                let read = Client::from_page().get::<Session>(model::SESSION).await;
                let done = this
                    .update(cx, |this, cx| match read {
                        Ok(session) => {
                            this.client = Some(Client::with_csrf(session.csrf.clone()));
                            this.session = Some(session);
                            if matches!(this.notice, Some(Notice { sign_in: false, .. })) {
                                this.notice = None;
                            }
                            if let Some(chat) = this.current.clone() {
                                this.load(chat, cx);
                            }
                            cx.notify();
                            true
                        }
                        Err(error) => {
                            log::warn!("lander8 session: {error:#}");
                            if this.current.is_some() {
                                this.notice = Some(Notice {
                                    text: model::CONNECTING.into(),
                                    sign_in: false,
                                });
                                cx.notify();
                            }
                            false
                        }
                    })
                    .unwrap_or(true);
                if done {
                    break;
                }
                cx.background_executor().timer(RETRY).await;
            }
        })
        .detach();
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        if self.scrolled {
            self.scrolled = false;
            let offset = self.scroll.offset().y;
            let max = self.scroll.max_offset().y;
            self.following = f32::from(offset) <= -f32::from(max) + 24.;
        }
        if let Some(live) = &mut self.live {
            let now = Instant::now();
            live.chunks
                .retain(|(_, at)| now.duration_since(*at).as_secs_f32() * 1000. < VEIL_FADE_MS);
            // The veil's fade and the working label's dots.
            cx.notify();
        }
    }

    /// Shows `chat`, reading its messages, and follows its answer when one
    /// is being written.
    fn load(&mut self, chat: String, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        self.generation += 1;
        let generation = self.generation;
        self.loading = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let read = client.thread(&chat).await;
            this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.loading = false;
                match read {
                    Ok(detail) => {
                        let thread = detail.thread;
                        this.title = thread.title.clone();
                        set_title(&thread.title);
                        let pending = thread.pending();
                        let messages = model::without_pending(detail.messages, pending);
                        this.entries = model::entries(&messages);
                        this.following = true;
                        match (pending, thread.request_id) {
                            (true, Some(request)) => this.follow(chat.clone(), request, cx),
                            _ => this.live = None,
                        }
                    }
                    Err(error) => {
                        // A chat that's gone (or another browser's): start
                        // a new one here.
                        let missing = error
                            .downcast_ref::<Refusal>()
                            .is_some_and(|r| r.code == "not_found" || r.code == "http_404");
                        if missing {
                            this.new_chat(cx);
                        } else {
                            this.refused(&error, None, cx);
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn new_chat(&mut self, cx: &mut Context<Self>) {
        if self.live.is_some() || self.sending {
            return;
        }
        self.generation += 1;
        self.current = None;
        self.title.clear();
        self.entries.clear();
        self.expanded.clear();
        self.notice = None;
        self.loading = false;
        push_path(None);
        set_title("OpenAgents");
        cx.notify();
    }

    /// Says what went wrong, or sends the person to sign in when the site
    /// asks for it, keeping `draft` for when they come back.
    fn refused(&mut self, error: &anyhow::Error, draft: Option<&str>, cx: &mut Context<Self>) {
        log::warn!("lander8 request failed: {error:#}");
        match error.downcast_ref::<Refusal>() {
            Some(refusal) if model::asks_sign_in(&refusal.code) => {
                self.notice = Some(Notice {
                    text: refusal.message.clone(),
                    sign_in: true,
                });
                if let Some(draft) = draft {
                    keep_draft(draft);
                }
                sign_in(self.current.as_deref());
            }
            Some(refusal) => {
                self.notice = Some(Notice {
                    text: refusal.message.clone(),
                    sign_in: false,
                })
            }
            None => {
                self.notice = Some(Notice {
                    text: model::WENT_WRONG.into(),
                    sign_in: false,
                })
            }
        }
        cx.notify();
    }

    fn follow(&mut self, chat: String, request: String, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let (follow, mut events) = match client.follow(&chat, &request) {
            Ok(pair) => pair,
            Err(_) => {
                self.notice = Some(Notice {
                    text: model::LOST.into(),
                    sign_in: false,
                });
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
            request,
            messages: BTreeMap::new(),
            chunks: Vec::new(),
            started: Instant::now(),
            stopping: false,
            _follow: follow,
            _task: task,
        });
        cx.notify();
    }

    fn on_event(&mut self, event: StreamEvent, cx: &mut Context<Self>) {
        let still = self.still;
        let Some(live) = &mut self.live else {
            return;
        };
        match event {
            StreamEvent::Text {
                index,
                role,
                text,
                replace,
            } => {
                // The person's own message is already in the transcript.
                if role == "user" {
                    return;
                }
                if !text.trim().is_empty()
                    && live.messages.values().all(|(_, t)| t.trim().is_empty())
                {
                    mark("lander8-first-token");
                }
                let slot = live
                    .messages
                    .entry(index)
                    .or_insert_with(|| (role.clone(), String::new()));
                if replace {
                    slot.1 = text;
                } else {
                    if !still {
                        live.chunks.push((visible_chars(&text), Instant::now()));
                    }
                    slot.1.push_str(&text);
                }
            }
            StreamEvent::Cards(_) => {}
            StreamEvent::Done { outcome } => self.finish(outcome == "stopped", None),
            StreamEvent::Error { message, .. } => self.finish(false, Some(message)),
        }
        cx.notify();
    }

    /// Moves the finished answer into the transcript.
    fn finish(&mut self, stopped: bool, error: Option<String>) {
        let Some(live) = self.live.take() else {
            return;
        };
        let mut entry = model::live_entry(&live.request, &live.messages);
        entry.id = format!("a-{}", live.request);
        if let Some(message) = error {
            entry.parts.push(MessagePart::Error {
                id: "error".into(),
                message,
            });
        }
        entry.status = Some(if stopped {
            MessageStatus::Aborted
        } else {
            MessageStatus::Complete
        });
        entry.duration_ms = Some(live.started.elapsed().as_millis() as i64);
        self.entries.push(entry);
    }

    fn send(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        let text = text.trim().to_owned();
        if text.is_empty() || self.sending || self.live.is_some() {
            if !text.is_empty() {
                self.put_back(&text, cx);
            }
            return;
        }
        if text.chars().count() > model::MAX_CHARS {
            self.put_back(&text, cx);
            self.notice = Some(Notice {
                text: model::TOO_LONG.into(),
                sign_in: false,
            });
            cx.notify();
            return;
        }
        let Some(client) = self.client.clone() else {
            self.put_back(&text, cx);
            self.notice = Some(Notice {
                text: model::CONNECTING.into(),
                sign_in: false,
            });
            cx.notify();
            return;
        };
        let request = uuid::Uuid::new_v4().to_string();
        let index = self.entries.len();
        self.entries
            .push(model::user_entry(format!("u-{request}"), &text));
        self.sending = true;
        self.notice = None;
        self.following = true;
        let current = self.current.clone();
        mark("lander8-send");
        cx.spawn(async move |this, cx| {
            let sent = match &current {
                Some(id) => client.reply(id, &request, &text).await.map(|()| id.clone()),
                None => client.start(&request, &text, None).await,
            };
            this.update(cx, |this, cx| {
                this.sending = false;
                match sent {
                    Ok(id) => {
                        if current.is_none() {
                            push_path(Some(&id));
                            this.title =
                                transcript::truncate(text.lines().next().unwrap_or(""), 60);
                            set_title(&this.title);
                            this.current = Some(id.clone());
                            this.generation += 1;
                        }
                        this.follow(id, request, cx);
                    }
                    Err(error) => {
                        // Put the words back so nothing typed is lost.
                        if this.entries.len() == index + 1 {
                            this.entries.pop();
                        }
                        this.put_back(&text, cx);
                        this.refused(&error, Some(&text), cx);
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

    fn put_back(&mut self, text: &str, cx: &mut Context<Self>) {
        self.composer.update(cx, |composer, cx| {
            if composer.is_empty() {
                composer.set_text(text.to_owned(), cx);
            }
        });
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        let (Some(client), Some(chat)) = (self.client.clone(), self.current.clone()) else {
            return;
        };
        let Some(live) = &mut self.live else {
            return;
        };
        if live.stopping {
            return;
        }
        live.stopping = true;
        let request = live.request.clone();
        cx.spawn(async move |this, cx| {
            if let Err(error) = client.stop(&chat, &request).await {
                this.update(cx, |this, cx| {
                    if let Some(live) = &mut this.live {
                        live.stopping = false;
                    }
                    this.refused(&error, None, cx);
                })
                .ok();
            }
        })
        .detach();
        cx.notify();
    }

    fn veil(&self) -> Option<Veil> {
        let live = self.live.as_ref()?;
        let now = Instant::now();
        let tail = live
            .chunks
            .iter()
            .filter_map(|(chars, at)| {
                let elapsed = now.duration_since(*at).as_secs_f32() * 1000.;
                (elapsed < VEIL_FADE_MS).then(|| (*chars, veil_alpha(elapsed, VEIL_FADE_MS)))
            })
            .collect::<Vec<_>>();
        Some(Veil { tail })
    }

    fn render_assistant(
        &self,
        entry: &SessionMessageEntry,
        veil: Option<Veil>,
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
                    .update(cx, |this: &mut LanderChat, cx| {
                        if !this.expanded.remove(&key) {
                            this.expanded.insert(key);
                        }
                        cx.notify();
                    })
                    .ok();
            })
        };
        // The site's chat asks no questions of its own.
        let mut questions =
            |_: usize, _: &[openagents_proto::UserInputQuestion], _: bool| div().into_any_element();
        transcript::assistant_body(view, &toggle, &mut questions)
    }

    fn render_entry(
        &self,
        entry: &SessionMessageEntry,
        veil: Option<Veil>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match entry.role {
            MessageRole::User => {
                let text = entry
                    .parts
                    .iter()
                    .filter_map(|part| match part {
                        MessagePart::Text { text, .. } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n");
                transcript::user_bubble(&entry.id, &text, Vec::new())
            }
            _ => self.render_assistant(entry, veil, cx),
        }
    }

    fn render_card(&self, layout: &Layout, cx: &mut Context<Self>) -> AnyElement {
        let pad = if layout.narrow { 14. } else { 22. };
        let mut rows: Vec<AnyElement> = Vec::new();
        for entry in &self.entries {
            rows.push(self.render_entry(entry, None, cx));
        }
        if let Some(live) = &self.live {
            let entry = model::live_entry(&live.request, &live.messages);
            rows.push(self.render_entry(&entry, self.veil(), cx));
        }
        if self.sending {
            rows.push(working_label().into_any_element());
        }
        let busy = self.live.is_some() || self.sending;
        let header_right: AnyElement = if busy {
            working_label().into_any_element()
        } else if self.current.is_some() {
            div()
                .id("new-chat")
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.))
                .px(px(8.))
                .h(px(26.))
                .rounded(px(7.))
                .cursor_pointer()
                .text_size(px(12.5))
                .text_color(theme::text_muted())
                .hover(|style| style.bg(theme::element_hover()).text_color(theme::text()))
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.new_chat(cx);
                    window.focus(&this.composer.focus_handle(cx), cx);
                }))
                .child(
                    svg()
                        .path("icons/pen-new-square.svg")
                        .size(px(14.))
                        .text_color(theme::text_muted()),
                )
                .child(model::NEW_CHAT)
                .into_any_element()
        } else {
            div().into_any_element()
        };
        let title: SharedString = truncate(&self.title, 80).into();
        if self.following {
            self.scroll.scroll_to_bottom();
        }
        div()
            .flex()
            .flex_col()
            .flex_shrink(1.)
            .min_h_0()
            .w_full()
            .rounded(px(16.))
            .overflow_hidden()
            .bg(gpui::hsla(0., 0., 0.035, 0.92))
            .border_1()
            .border_color(theme::border_strong())
            .child(
                div()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .pl(px(pad))
                    .pr(px(10.))
                    .h(px(42.))
                    .border_b_1()
                    .border_color(theme::border())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::text_muted())
                            .child(title),
                    )
                    .child(header_right),
            )
            .child(
                div()
                    .id("lander8-transcript")
                    .flex_shrink(1.)
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .on_scroll_wheel(cx.listener(|this, _: &ScrollWheelEvent, _, _| {
                        this.scrolled = true;
                    }))
                    .px(px(pad))
                    .pt(px(18.))
                    .pb(px(20.))
                    .flex()
                    .flex_col()
                    .gap(px(22.))
                    .children(rows),
            )
            .into_any_element()
    }

    fn render_composer(&self, layout: &Layout, cx: &mut Context<Self>) -> AnyElement {
        let live = self.live.is_some();
        let empty = self.composer.read(cx).is_empty();
        let send = div()
            .id("send")
            .flex_none()
            .size(px(36.))
            .rounded_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::solid())
            .cursor_pointer()
            .when((!live && empty) || self.sending, |b| b.opacity(0.35))
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
                    .path(if live {
                        "icons/stop.svg"
                    } else {
                        "icons/arrow-up.svg"
                    })
                    .size(px(17.))
                    .text_color(theme::on_solid()),
            );
        div()
            .flex_none()
            .w_full()
            .h(px(layout.composer.h))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .pl(px(20.))
            .pr(px(8.))
            .rounded(px(layout.composer.h / 2.))
            .border_1()
            .border_color(theme::border_strong())
            .bg(gpui::hsla(0., 0., 0.055, 0.94))
            .child(self.composer.clone())
            .child(send)
            .into_any_element()
    }

    fn render_notice(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let notice = self.notice.as_ref()?;
        let line = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(12.))
            .text_size(px(13.))
            .text_color(theme::text_muted())
            .child(SharedString::from(notice.text.clone()));
        Some(
            if notice.sign_in {
                line.child(
                    button("sign-in", model::SIGN_IN, true).on_click(cx.listener(
                        |this, _: &ClickEvent, _, cx| {
                            let draft = this.composer.read(cx).text();
                            if !draft.trim().is_empty() {
                                keep_draft(&draft);
                            }
                            sign_in(this.current.as_deref());
                        },
                    )),
                )
            } else {
                line
            }
            .into_any_element(),
        )
    }
}

impl Render for LanderChat {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let (w, h) = (f32::from(viewport.width), f32::from(viewport.height));
        let layout = Layout::new(w, h);
        let shows_card = !self.entries.is_empty() || self.live.is_some() || self.sending;
        let mut column = div()
            .absolute()
            .left(px(layout.room.x))
            .w(px(layout.room.w))
            .top(px(layout.room.y))
            .h(px(layout.composer.bottom() - layout.room.y))
            .flex()
            .flex_col()
            .justify_end()
            .gap(px(if layout.narrow { 10. } else { 14. }));
        if shows_card {
            column = column.child(self.render_card(&layout, cx));
        } else if !self.loading {
            column = column.child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .justify_end()
                    .items_center()
                    .pb(px(if layout.narrow { 18. } else { 26. }))
                    .text_size(px(if layout.narrow { 24. } else { 30. }))
                    .line_height(px(if layout.narrow { 30. } else { 38. }))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::text())
                    .child(model::WELCOME),
            );
        }
        if let Some(notice) = self.render_notice(cx) {
            column = column.child(notice);
        }
        column = column.child(self.render_composer(&layout, cx));
        let mut root = div()
            .id("lander8")
            .size_full()
            .relative()
            .font_family(theme::SANS)
            .text_color(theme::text())
            .text_size(px(theme::BODY))
            .child(column);
        root.text_style().font_fallbacks = Some(theme::fallbacks());
        root
    }
}

// ---------------------------------------------------------------------
// The page around the canvas
// ---------------------------------------------------------------------

fn route_chat() -> Option<String> {
    let path = web_sys::window()?.location().pathname().ok()?;
    model::chat_of(&path)
}

/// Shows `chat`'s address without loading the page again.
fn push_path(chat: Option<&str>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let path = model::path_for(chat);
    if window.location().pathname().ok().as_deref() == Some(path.as_str()) {
        return;
    }
    if let Ok(history) = window.history() {
        let _ = history.push_state_with_url(&JsValue::NULL, "", Some(&path));
    }
}

fn set_title(title: &str) {
    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        if title.is_empty() || title == "OpenAgents" {
            document.set_title("OpenAgents");
        } else {
            document.set_title(&format!("{title} \u{b7} OpenAgents"));
        }
    }
}

/// Signs in, then comes back to `chat` here.
fn sign_in(chat: Option<&str>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let back = String::from(js_sys::encode_uri_component(&model::path_for(chat)));
    let _ = window
        .location()
        .assign(&format!("/login?return_to={back}"));
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.session_storage().ok().flatten()
}

/// Keeps a message for after sign-in, in this tab only.
fn keep_draft(text: &str) {
    if let Some(storage) = storage() {
        let _ = storage.set_item(model::DRAFT_KEY, text);
    }
}

/// The message kept before sign-in, once.
fn take_draft() -> Option<String> {
    let storage = storage()?;
    let draft = storage.get_item(model::DRAFT_KEY).ok().flatten()?;
    let _ = storage.remove_item(model::DRAFT_KEY);
    (!draft.trim().is_empty()).then_some(draft)
}

/// A performance mark the page's timing checks read.
fn mark(name: &str) {
    if let Some(performance) = web_sys::window().and_then(|w| w.performance()) {
        let _ = performance.mark(name);
    }
}
