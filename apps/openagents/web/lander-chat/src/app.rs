//! The page's canvas: the composer at the bottom, the welcome line over it
//! before the first message, and then the conversation in a card that sits
//! on the composer and grows upward as it goes, until it reaches the page's
//! name and scrolls.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::time::Duration;

use futures::StreamExt as _;
use gpui::{
    AnyElement, ClickEvent, Context, Entity, Focusable, FontWeight, IntoElement, ParentElement,
    Render, ScrollHandle, SharedString, Styled, Task, Window, div, prelude::*, px,
};
use openagents_chat_view::transcript::host::TurnActivity;
use openagents_doc::{MessagePart, MessageStatus, SessionMessageEntry};
use openagents_web_chat_view::composer::{Submitted, TextInput};
use openagents_web_chat_view::desktop_transcript::{self, WebTranscript};
use openagents_web_chat_view::labels::{self, button};
use openagents_web_chat_view::svg;
use openagents_web_chat_view::theme;
use openagents_web_client::web::{Client, Follow};
use openagents_web_client::{Event, MessageRole as SentBy, Refusal};
use wasm_bindgen::{JsCast, JsValue};
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
    /// The fade on the text now arriving.
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
    /// The last frame's layout, and the area published for it.
    layout: Option<Layout>,
    kept: Option<(f32, f32, f32, f32)>,
    client: Option<Client>,
    session: Option<Session>,

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
    /// The open chat as the desktop's transcript reads it.
    shared: Entity<WebTranscript>,
    /// The desktop's transcript, drawing `shared` (#12216).
    transcript: Entity<desktop_transcript::Transcript>,
    /// What `shared` last got: the entries' shape, the answer being
    /// written, and whether a message is on its way.
    published: Option<(Vec<(usize, usize, u8)>, (usize, usize), bool, i64)>,
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
        // A page that asked for no motion draws text as it arrives.
        if still {
            openagents_chat_view::motion::set_reduced_motion(cx, true);
        }
        let shared = cx.new(|_| WebTranscript::default());
        let transcript =
            desktop_transcript::new(&shared, Rc::new(RefCell::new(HashMap::new())), cx);
        transcript.update(cx, |view, cx| view.set_rail_enabled(false, cx));
        let mut this = Self {
            layout: None,
            kept: None,
            client: None,
            session: None,
            current: route_chat(),
            title: String::new(),
            entries: Vec::new(),
            live: None,
            loading: false,
            sending: false,
            notice: None,
            composer,
            scroll: ScrollHandle::new(),
            shared,
            transcript,
            published: None,
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
        self.keep_area();
        if self.live.is_some() || self.sending {
            // The working row's timer.
            cx.notify();
        }
    }

    /// The card and the composer's box on the page, published when it
    /// changes ([`publish_keep`]).
    fn keep_area(&mut self) {
        let Some(layout) = self.layout else {
            return;
        };
        let shows_card = !self.entries.is_empty() || self.live.is_some() || self.sending;
        let transcript = self.scroll.bounds();
        let top = if shows_card && f32::from(transcript.size.height) > 0. {
            let above = if self.current.is_some() { 42. } else { 0. };
            f32::from(transcript.origin.y) - 1. - above
        } else {
            layout.composer.y
        };
        let area = (
            layout.room.x,
            top.max(0.),
            layout.room.w,
            layout.composer.bottom() - top.max(0.),
        );
        let moved = self.kept.is_none_or(|kept| {
            (kept.0 - area.0).abs() > 1.
                || (kept.1 - area.1).abs() > 1.
                || (kept.2 - area.2).abs() > 1.
                || (kept.3 - area.3).abs() > 1.
        });
        if moved {
            self.kept = Some(area);
            publish_keep(area);
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
            started: Instant::now(),
            stopping: false,
            _follow: follow,
            _task: task,
        });
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
                    slot.1.push_str(&text);
                }
            }
            Event::Cards { .. } => {}
            Event::Done { outcome, .. } => self.finish(outcome == "stopped", None),
            Event::Error { error, .. } => self.finish(false, Some(error.message)),
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
                            this.title = labels::truncate(text.lines().next().unwrap_or(""), 60);
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

    /// Hand the chat to the transcript when it changed.
    fn publish(&mut self, cx: &mut Context<Self>) {
        let shape =
            self.entries
                .iter()
                .map(|entry| {
                    let text = entry
                        .parts
                        .iter()
                        .map(|part| match part {
                            MessagePart::Text { text, .. }
                            | MessagePart::Reasoning { text, .. } => text.len(),
                            _ => 1,
                        })
                        .sum();
                    (
                        entry.parts.len(),
                        text,
                        entry.status.map_or(0, |s| s as u8 + 1),
                    )
                })
                .collect();
        let live = self.live.as_ref().map_or((0, 0), |live| {
            (
                live.messages.len() + 1,
                live.messages.values().map(|(_, text)| text.len()).sum(),
            )
        });
        let elapsed = self
            .live
            .as_ref()
            .map_or(-1, |live| live.started.elapsed().as_secs() as i64);
        let fingerprint = (shape, live, self.sending, elapsed);
        if self.published.as_ref() == Some(&fingerprint) {
            return;
        }
        self.published = Some(fingerprint);
        let mut entries = self.entries.clone();
        let activity = if let Some(live) = &self.live {
            entries.push(model::live_entry(&live.request, &live.messages));
            Some(TurnActivity {
                sending: live.messages.is_empty(),
                queued: false,
                elapsed_secs: Some(elapsed),
                turn_ms: 0,
            })
        } else {
            self.sending.then_some(TurnActivity {
                sending: true,
                queued: false,
                elapsed_secs: None,
                turn_ms: 0,
            })
        };
        // Before the first message there is no chat yet; the working row
        // still needs one to belong to.
        let chat = Some(self.current.clone().unwrap_or_else(|| "new".into()));
        self.shared.update(cx, |state, cx| {
            state.open(chat, entries);
            state.set_activity(activity);
            cx.notify();
        });
        self.transcript
            .update(cx, |transcript, cx| transcript.host_changed(None, cx));
    }

    fn render_card(&mut self, layout: &Layout, cx: &mut Context<Self>) -> AnyElement {
        let pad = if layout.narrow { 14. } else { 22. };
        self.publish(cx);
        desktop_transcript::fit(&self.transcript, layout.room.w, true, cx);
        div()
            .flex()
            .flex_col()
            .flex_shrink(1.)
            .min_h_0()
            .w_full()
            .rounded(px(16.))
            .overflow_hidden()
            .bg(gpui::hsla(0., 0., 0.035, model::card_alpha(base())))
            .border_1()
            .border_color(theme::border_strong())
            .child(
                // Measured for the area the page keeps clear (`keep_area`).
                div()
                    .id("lander8-transcript")
                    .track_scroll(&self.scroll)
                    .flex()
                    .flex_col()
                    .flex_shrink(1.)
                    .min_h_0()
                    .px(px(pad - 16.).max(px(0.)))
                    .child(self.transcript.clone()),
            )
            .into_any_element()
    }

    /// "New chat", over the card's right edge: there once a chat is open,
    /// and dimmed while an answer is being written.
    fn render_new_chat(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.current.as_ref()?;
        let busy = self.live.is_some() || self.sending;
        Some(
            div()
                .flex_none()
                .flex()
                .flex_row()
                .justify_end()
                .child(
                    div()
                        .id("new-chat")
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.))
                        .px(px(10.))
                        .h(px(28.))
                        .rounded(px(14.))
                        .text_size(px(12.5))
                        .text_color(theme::text_muted())
                        .bg(gpui::hsla(0., 0., 0.035, 0.7))
                        .when(busy, |b| b.opacity(0.4))
                        .when(!busy, |b| {
                            b.cursor_pointer().hover(|style| {
                                style.bg(theme::element_hover()).text_color(theme::text())
                            })
                        })
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
                        .child(model::NEW_CHAT),
                )
                .into_any_element(),
        )
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
        let layout = Layout::under(w, h, model::card_top(base(), w, h));
        self.layout = Some(layout);
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
            if let Some(new_chat) = self.render_new_chat(cx) {
                column = column.child(new_chat);
            }
            column = column.child(self.render_card(&layout, cx));
        } else if !self.loading && model::welcomes(base()) {
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

/// The page this chat is on (`/lander8` or `/lander10`).
fn base() -> &'static str {
    let path = web_sys::window()
        .and_then(|w| w.location().pathname().ok())
        .unwrap_or_default();
    model::base_of(&path)
}

/// Tells the page where the chat is drawn, when the page asks (an element
/// `#lander-chat-keep`, which `/lander10`'s pylons stay out of): the box
/// is moved to cover the card and the composer, and `pylons-keep` fires on
/// the document.
fn publish_keep(area: (f32, f32, f32, f32)) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Some(element) = document.get_element_by_id("lander-chat-keep") else {
        return;
    };
    let Ok(element) = element.dyn_into::<web_sys::HtmlElement>() else {
        return;
    };
    let style = element.style();
    let (x, y, w, h) = area;
    let _ = style.set_property("left", &format!("{x:.0}px"));
    let _ = style.set_property("top", &format!("{y:.0}px"));
    let _ = style.set_property("width", &format!("{w:.0}px"));
    let _ = style.set_property("height", &format!("{h:.0}px"));
    if let Ok(event) = web_sys::Event::new("pylons-keep") {
        let _ = document.dispatch_event(&event);
    }
}

fn route_chat() -> Option<String> {
    let path = web_sys::window()?.location().pathname().ok()?;
    model::chat_of(&path)
}

/// Shows `chat`'s address without loading the page again.
fn push_path(chat: Option<&str>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let path = model::path_for(base(), chat);
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
    let back = String::from(js_sys::encode_uri_component(&model::path_for(base(), chat)));
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
