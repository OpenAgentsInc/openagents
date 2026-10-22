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

mod grid;

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
    /// When the first words arrived, ms after the message was sent: for
    /// the Grid homepage's visit (#12439).
    first_ms: Option<u128>,
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
    game_mode: bool,
    game_chat: Option<String>,
    grid_chat: Option<String>,
    /// The Grid's help desk (#12443): while it is open, the chat asks the
    /// site's free hosted agent through [`Self::plain`], with the visitor's
    /// usual limits, in a chat of its own.
    desk_mode: bool,
    desk_chat: Option<String>,
    plain: Option<Client>,
    /// The Grid Foreman (#12446): while its chat is open, the chat asks the
    /// Foreman through [`Self::foreman`], with the visitor's usual limits,
    /// in a chat of its own.
    foreman_mode: bool,
    foreman_chat: Option<String>,
    foreman: Option<Client>,
    title: String,
    entries: Vec<SessionMessageEntry>,
    live: Option<Live>,
    loading: bool,
    sending: bool,
    notice: Option<Notice>,
    composer: Entity<TextInput>,
    scroll: ScrollHandle,
    notice_bounds: ScrollHandle,
    /// The open chat as the desktop's transcript reads it.
    shared: Entity<WebTranscript>,
    /// The desktop's transcript, drawing `shared` (#12216).
    transcript: Entity<desktop_transcript::Transcript>,
    /// What `shared` last got: the entries' shape, the answer being
    /// written, and whether a message is on its way.
    published: Option<(Vec<(usize, usize, u8)>, (usize, usize), bool, i64)>,
    /// Bumped on each load, so a slow read for a chat left behind is dropped.
    generation: u64,
    /// The Grid terminal (#12421): whether a ready DeepSeek V4.1 group is
    /// online, and whether this chat asks it rather than any Pylon.
    group_ready: bool,
    ask_group: bool,
    flint_ready: bool,
    /// The visitor is a site admin, so the Grid terminal offers Kiln.
    admin: bool,
    ask_flint: bool,
    /// When the last message was sent, and how many this page has sent,
    /// for the visit's events.
    sent_at: Option<Instant>,
    sent_count: u32,
    /// The Grid homepage's overlay around the chat (#12459).
    grid: grid::Grid,
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
            game_mode: is_game_lab(),
            game_chat: None,
            grid_chat: None,
            desk_mode: false,
            desk_chat: None,
            foreman_mode: false,
            foreman_chat: None,
            foreman: None,
            plain: None,
            title: String::new(),
            entries: Vec::new(),
            live: None,
            loading: false,
            sending: false,
            notice: None,
            composer,
            scroll: ScrollHandle::new(),
            notice_bounds: ScrollHandle::new(),
            shared,
            transcript,
            published: None,
            generation: 0,
            group_ready: false,
            ask_group: true,
            flint_ready: false,
            admin: false,
            ask_flint: false,
            sent_at: None,
            sent_count: 0,
            grid: grid::Grid::default(),
            _ticker: ticker,
        };
        let kept = kept_pick(this.current.as_deref());
        this.ask_group = model::asks_group(kept.as_deref());
        this.ask_flint = model::asks_flint(kept.as_deref());
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
                            let client = Client::with_csrf(session.csrf.clone());
                            this.plain = Some(client.clone());
                            this.foreman = Some(client.clone().with_chat_surface("foreman"));
                            this.client = Some(if base() == model::HOME_BASE {
                                client.with_chat_surface("home")
                            } else {
                                client
                            });
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
        let game = is_game_lab();
        if game != self.game_mode && self.live.is_none() && !self.sending {
            if self.game_mode {
                self.game_chat = self.current.clone();
            } else {
                self.grid_chat = self.current.clone();
            }
            self.new_chat(cx);
            self.game_mode = game;
            let chat = if game {
                self.game_chat.clone()
            } else {
                self.grid_chat.clone()
            };
            if let Some(chat) = chat {
                self.current = Some(chat.clone());
                push_path(Some(&chat));
                self.load(chat, cx);
            }
        }
        let (desk, foreman) = (is_desk(), !is_desk() && is_foreman());
        if (desk, foreman) != (self.desk_mode, self.foreman_mode)
            && self.live.is_none()
            && !self.sending
        {
            if self.desk_mode {
                self.desk_chat = self.current.clone();
            } else if self.foreman_mode {
                self.foreman_chat = self.current.clone();
            } else {
                self.grid_chat = self.current.clone();
            }
            self.new_chat(cx);
            self.desk_mode = desk;
            self.foreman_mode = foreman;
            self.sync_pick();
            let chat = if desk {
                self.desk_chat.clone()
            } else if foreman {
                self.foreman_chat.clone()
            } else {
                self.grid_chat.clone()
            };
            if let Some(chat) = chat {
                self.current = Some(chat.clone());
                push_path(Some(&chat));
                self.load(chat, cx);
            }
        }
        if base() == model::HOME_BASE {
            let ready = js_sys::Reflect::get(&js_sys::global(), &model::GROUP_READY_GLOBAL.into())
                .ok()
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let flint = js_sys::Reflect::get(&js_sys::global(), &model::FLINT_READY_GLOBAL.into())
                .ok()
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let admin = js_sys::Reflect::get(&js_sys::global(), &model::ADMIN_GLOBAL.into())
                .ok()
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if admin != self.admin {
                self.admin = admin;
                cx.notify();
            }
            if flint != self.flint_ready {
                self.flint_ready = flint;
                self.sync_pick();
                cx.notify();
            }
            if ready != self.group_ready {
                self.group_ready = ready;
                self.sync_pick();
                cx.notify();
            }
        }
        if base() == model::HOME_BASE
            && let Some(panel) = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id("pylon-composer"))
        {
            let _ = panel.set_attribute(
                "data-draft-chars",
                &self.composer.read(cx).text().chars().count().to_string(),
            );
            let _ = panel.set_attribute("data-message-count", &self.entries.len().to_string());
            let _ = panel.set_attribute(
                "data-pylon-pick",
                if self.desk_mode || self.foreman_mode {
                    "none"
                } else if self.ask_flint {
                    "flint"
                } else {
                    match (self.group_ready, self.ask_group) {
                        (false, _) => {
                            if self.flint_ready {
                                "any"
                            } else {
                                "none"
                            }
                        }
                        (true, true) => "group",
                        (true, false) => "any",
                    }
                },
            );
            let _ = panel.set_attribute(
                "data-chat-state",
                if self.live.is_some() {
                    "streaming"
                } else if self.sending {
                    "sending"
                } else if self.notice.is_some() {
                    "notice"
                } else if self.client.is_some() {
                    "ready"
                } else {
                    "connecting"
                },
            );
        }
        self.keep_area();
        if base() == model::HOME_BASE {
            self.grid_tick(cx);
        }
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
        let mut top = if shows_card && f32::from(transcript.size.height) > 0. {
            f32::from(transcript.origin.y) - 1.
        } else {
            layout.composer.y
        };
        if self.notice.is_some() {
            let notice = self.notice_bounds.bounds();
            if f32::from(notice.size.height) > 0. {
                top = top.min(f32::from(notice.origin.y));
            }
        }
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
        let Some(client) = self.active_client() else {
            return;
        };
        self.generation += 1;
        let generation = self.generation;
        self.loading = true;
        let kept = kept_pick(Some(&chat));
        self.ask_group = model::asks_group(kept.as_deref());
        self.ask_flint = model::asks_flint(kept.as_deref());
        self.sync_pick();
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
        // A new chat asks DeepSeek V4.1 again while a group is online.
        self.ask_group = true;
        self.ask_flint = false;
        self.sync_pick();
        push_path(None);
        set_title("OpenAgents");
        cx.notify();
    }

    /// Points the chat's client at this chat's choice: it names
    /// [`model::GROUP_MODEL`] on each turn while DeepSeek V4.1 is chosen and
    /// a ready group is online.
    fn sync_pick(&self) {
        let value = if is_game_lab() || self.desk_mode || self.foreman_mode {
            JsValue::UNDEFINED
        } else if self.ask_flint {
            let pin = js_sys::Reflect::get(&js_sys::global(), &model::FLINT_PIN_GLOBAL.into())
                .ok()
                .and_then(|v| v.as_string());
            JsValue::from_str(model::flint_choice(pin.as_deref()))
        } else if self.group_ready && self.ask_group {
            JsValue::from_str(model::GROUP_MODEL)
        } else {
            JsValue::UNDEFINED
        };
        let _ = js_sys::Reflect::set(&js_sys::global(), &model::MODEL_GLOBAL.into(), &value);
    }

    /// Chooses DeepSeek V4.1 or any Pylon for this chat, and keeps it.
    fn pick(&mut self, pick: &str, cx: &mut Context<Self>) {
        self.ask_group = pick == "group";
        self.ask_flint = pick == "flint";
        track(
            "model_chosen",
            &format!(
                "{{\"model\":\"{}\"}}",
                model::pick_word(self.ask_group, self.ask_flint)
            ),
        );
        if let Some(chat) = &self.current {
            keep_pick(chat, self.ask_group, self.ask_flint);
        }
        self.sync_pick();
        cx.notify();
    }

    /// Shows failures inline and keeps the draft; signing in is an explicit action.
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
        let Some(client) = self.active_client() else {
            return;
        };
        let (follow, mut events) = match client.follow(&chat, &request) {
            Ok(pair) => pair,
            Err(_) => {
                self.notice = Some(Notice {
                    text: model::LOST.into(),
                    sign_in: false,
                });
                track("answer_failed", "{\"why\":\"lost\"}");
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
            first_ms: None,
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
                    live.first_ms = self.sent_at.map(|at| at.elapsed().as_millis());
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
            Event::Cards { cards, .. } if base() == model::HOME_BASE => {
                self.game_cards(&cards, cx);
            }
            Event::Cards { cards, .. } => {
                if let Ok(callback) =
                    js_sys::Reflect::get(&js_sys::global(), &"homeGameCards".into())
                {
                    if let Some(callback) = callback.dyn_ref::<js_sys::Function>() {
                        let _ = callback
                            .call1(&JsValue::UNDEFINED, &JsValue::from_str(&cards.to_string()));
                    }
                }
            }
            Event::Done {
                outcome,
                model_family,
                ..
            } => {
                if outcome == "stopped" {
                    track("answer_failed", "{\"why\":\"stopped\"}");
                } else {
                    let text = live
                        .messages
                        .values()
                        .find(|(_, text)| !text.trim().is_empty())
                        .map_or("", |(_, text)| text.as_str());
                    let (source, family) = model::answered(text, model_family.as_deref());
                    let ms = live
                        .first_ms
                        .or_else(|| self.sent_at.map(|at| at.elapsed().as_millis()))
                        .unwrap_or_default();
                    track(
                        "answered",
                        &format!(
                            "{{\"source\":\"{source}\",\"family\":\"{family}\",\"time\":{ms}}}"
                        ),
                    );
                }
                self.finish(outcome == "stopped", None)
            }
            Event::Error { error, .. } => {
                track(
                    "answer_failed",
                    &format!("{{\"why\":\"{}\"}}", stream_why(&error.code)),
                );
                self.finish(false, Some(error.message))
            }
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
        let Some(client) = self.active_client() else {
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
        self.sent_at = Some(Instant::now());
        self.sent_count += 1;
        let nth = if self.sent_count == 1 {
            "first"
        } else {
            "later"
        };
        if self.desk_mode {
            track("help_desk_message", &format!("{{\"nth\":\"{nth}\"}}"));
        } else if self.foreman_mode {
            track("foreman_message", &format!("{{\"nth\":\"{nth}\"}}"));
        } else {
            track(
                "message_sent",
                &format!(
                    "{{\"model\":\"{}\",\"nth\":\"{nth}\"}}",
                    model::pick_word(self.group_ready && self.ask_group, self.ask_flint),
                ),
            );
        }
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
                            keep_pick(&id, this.ask_group, this.ask_flint);
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
                        let code = error.downcast_ref::<Refusal>().map(|r| r.code.as_str());
                        track(
                            "answer_failed",
                            &format!("{{\"why\":\"{}\"}}", model::failure_word(code)),
                        );
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

    /// The client for the open chat: the help desk's asks the site's
    /// hosted agent, the terminal's asks the pylons first.
    fn active_client(&self) -> Option<Client> {
        if self.desk_mode {
            self.plain.clone()
        } else if self.foreman_mode {
            self.foreman.clone()
        } else {
            self.client.clone()
        }
    }

    fn put_back(&mut self, text: &str, cx: &mut Context<Self>) {
        self.composer.update(cx, |composer, cx| {
            if composer.is_empty() {
                composer.set_text(text.to_owned(), cx);
            }
        });
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        let (Some(client), Some(chat)) = (self.active_client(), self.current.clone()) else {
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
        // On the Grid homepage the messages float over the world, each
        // reply on a plate of its own, with no card behind them (#12459).
        let floats = base() == model::HOME_BASE;
        if floats {
            desktop_transcript::fit_plated(&self.transcript, layout.room.w, cx);
        } else {
            desktop_transcript::fit(&self.transcript, layout.room.w, true, cx);
        }
        div()
            .flex()
            .flex_col()
            .flex_shrink(1.)
            .min_h_0()
            .w_full()
            .rounded(px(16.))
            .overflow_hidden()
            .when(!floats, |card| {
                card.bg(gpui::hsla(0., 0., 0.035, model::card_alpha(base())))
                    .border_1()
                    .border_color(theme::border_strong())
            })
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

    /// "New chat", in the composer's row beside the model choice (#12459):
    /// there once a chat is open, and dimmed while an answer is being
    /// written. On a phone it is its icon alone.
    fn render_new_chat(&self, layout: &Layout, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.current.as_ref()?;
        let busy = self.live.is_some() || self.sending;
        Some(
            div()
                .id("new-chat")
                .flex_none()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.))
                .h(px(32.))
                .px(px(if layout.narrow { 8. } else { 12. }))
                .rounded(px(16.))
                .border_1()
                .border_color(theme::border_strong())
                .text_size(px(12.5))
                .text_color(theme::text_muted())
                .when(busy, |b| b.opacity(0.4))
                .when(!busy, |b| {
                    b.cursor_pointer()
                        .hover(|style| style.bg(theme::element_hover()).text_color(theme::text()))
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
                .when(!layout.narrow, |b| b.child(model::NEW_CHAT))
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
            .when_some(self.render_new_chat(layout, cx), |row, new_chat| {
                row.child(new_chat)
            })
            .when_some(self.render_kiln(layout), |row, kiln| row.child(kiln))
            .when_some(self.render_pick(layout, cx), |row, pick| row.child(pick))
            .child(send)
            .into_any_element()
    }

    /// Kiln, beside the model choice, for a site admin on the Grid (#12440):
    /// it opens Kiln's panel over the Grid.
    fn render_kiln(&self, layout: &Layout) -> Option<AnyElement> {
        if self.desk_mode
            || self.foreman_mode
            || !model::offers_kiln(base(), is_game_lab(), self.admin)
        {
            return None;
        }
        Some(
            div()
                .id("open-kiln")
                .flex_none()
                .flex()
                .items_center()
                .h(px(32.))
                .px(px(if layout.narrow { 10. } else { 12. }))
                .rounded(px(16.))
                .border_1()
                .border_color(theme::border_strong())
                .text_size(px(12.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::text())
                .cursor_pointer()
                .hover(|s| s.bg(gpui::hsla(0., 0., 1., 0.08)))
                .on_click(|_: &ClickEvent, _, _| {
                    if let Ok(open) =
                        js_sys::Reflect::get(&js_sys::global(), &model::OPEN_KILN.into())
                        && let Some(open) = open.dyn_ref::<js_sys::Function>()
                    {
                        let _ = open.call1(&JsValue::UNDEFINED, &JsValue::TRUE);
                    }
                })
                .child(model::KILN)
                .into_any_element(),
        )
    }

    /// DeepSeek V4.1 or any Pylon, beside the send button: on the Grid
    /// terminal while a ready V4.1 group is online (#12421).
    fn render_pick(&self, layout: &Layout, cx: &mut Context<Self>) -> Option<AnyElement> {
        if is_game_lab() || self.desk_mode || self.foreman_mode {
            return None;
        }
        if base() != model::HOME_BASE || !(self.group_ready || self.flint_ready || self.ask_flint) {
            return None;
        }
        let busy = self.live.is_some() || self.sending;
        let segment = |id: &'static str, label: &'static str, on: bool, pick: &'static str| {
            div()
                .id(id)
                .flex()
                .items_center()
                .h_full()
                .px(px(if layout.narrow { 10. } else { 12. }))
                .rounded(px(14.))
                .text_size(px(12.5))
                .font_weight(if on {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .when(on, |b| b.bg(theme::solid()).text_color(theme::on_solid()))
                .when(!on, |b| {
                    b.text_color(theme::text_muted())
                        .when(!busy, |b| b.hover(|s| s.text_color(theme::text())))
                })
                .when(!busy && !on, |b| b.cursor_pointer())
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    if this.live.is_none() && !this.sending {
                        this.pick(pick, cx);
                    }
                    window.focus(&this.composer.focus_handle(cx), cx);
                }))
                .child(label)
        };
        let (group_label, any_label) = if layout.narrow {
            (model::PICK_GROUP_SHORT, model::PICK_ANY_SHORT)
        } else {
            (model::PICK_GROUP, model::PICK_ANY)
        };
        Some(
            div()
                .id("pylon-pick")
                .flex_none()
                .flex()
                .flex_row()
                .items_center()
                .h(px(32.))
                .p(px(2.))
                .gap(px(2.))
                .rounded(px(16.))
                .border_1()
                .border_color(theme::border_strong())
                .when(busy, |b| b.opacity(0.5))
                .when(self.group_ready, |row| {
                    row.child(segment(
                        "pick-group",
                        group_label,
                        self.ask_group && !self.ask_flint,
                        "group",
                    ))
                })
                .when(self.flint_ready || self.ask_flint, |row| {
                    row.child(segment(
                        "pick-flint",
                        model::PICK_FLINT,
                        self.ask_flint,
                        "flint",
                    ))
                })
                .child(segment(
                    "pick-any",
                    any_label,
                    !(self.group_ready && self.ask_group) && !self.ask_flint,
                    "any",
                ))
                .into_any_element(),
        )
    }

    fn render_notice(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let notice = self.notice.as_ref()?;
        let line = div()
            .id("lander8-notice")
            .track_scroll(&self.notice_bounds)
            .flex_none()
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
        let mut layout = Layout::under(w, h, model::card_top(base(), w, h));
        // Beside Game Lab's panel on a wide page, the chat moves left of it.
        if base() == model::HOME_BASE && self.grid.lab.state != grid::LabState::Closed {
            layout = model::beside_panel(layout, w, h);
        }
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
            .text_size(px(theme::BODY));
        root = if base() != model::HOME_BASE {
            root.child(column)
        } else if self.grid.lab.playing.is_some() {
            // A game full screen: only the way back.
            root.child(self.render_back(cx))
        } else {
            root.child(self.render_bar(layout.narrow, cx))
                .when(self.grid.open, |root| root.child(column))
                .when(self.grid.lab.state != grid::LabState::Closed, |root| {
                    root.child(self.render_lab(w, h, cx))
                })
                .when(self.grid.kiln, |root| {
                    root.child(self.render_kiln_panel(w, h, cx))
                })
        };
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
    let location = web_sys::window()?.location();
    let path = location.pathname().ok()?;
    if model::base_of(&path) == model::HOME_BASE {
        model::home_chat(&location.search().ok()?)
    } else {
        model::chat_of(&path)
    }
}

/// Shows `chat`'s address without loading the page again. A chat at the
/// help desk keeps the page's address: it belongs to the desk.
fn push_path(chat: Option<&str>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    if chat.is_some() && (is_desk() || is_foreman()) {
        return;
    }
    let mut path = model::path_for(base(), chat);
    if is_game_lab() {
        path.push_str(if path.contains('?') {
            "&lab=game"
        } else {
            "?lab=game"
        });
    }
    if base() == model::HOME_BASE
        && window
            .location()
            .search()
            .is_ok_and(|q| q.trim_start_matches('?').split('&').any(|p| p == "gl"))
    {
        path.push_str(if path.contains('?') { "&gl" } else { "?gl" });
    }
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

/// The choice kept for `chat` in this browser, if any.
fn kept_pick(chat: Option<&str>) -> Option<String> {
    let chat = chat?;
    web_sys::window()?
        .local_storage()
        .ok()
        .flatten()?
        .get_item(&format!("{}{chat}", model::PICK_KEY))
        .ok()
        .flatten()
}

/// Keeps `chat`'s choice in this browser, so it stays when the chat is
/// opened again.
fn keep_pick(chat: &str, group: bool, flint: bool) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item(
            &format!("{}{chat}", model::PICK_KEY),
            model::kept_pick(group, flint),
        );
    }
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

/// What a turn that ended in a refusal is recorded as (#12439).
fn stream_why(code: &openagents_web_client::RefusalCode) -> &'static str {
    use openagents_web_client::RefusalCode as Code;
    match code {
        Code::Busy | Code::RateLimited => "busy",
        Code::Unavailable => "unavailable",
        Code::AccessLost => "sign_in",
        Code::QuotaExhausted => "refused",
        Code::Timeout | Code::StreamTimeout => "error",
        Code::Other(code) if code == "disconnected" => "lost",
        Code::Other(code) => model::failure_word(Some(code)),
    }
}

/// Adds one event to the Grid homepage's visit (#12439) through the
/// page's `globalThis.oaTrack`, which only that page defines, and only
/// when the browser didn't ask not to be tracked. `detail` is JSON of
/// fixed words and durations, never what anyone typed.
fn track(name: &str, detail: &str) {
    if let Ok(callback) = js_sys::Reflect::get(&js_sys::global(), &"oaTrack".into())
        && let Some(callback) = callback.dyn_ref::<js_sys::Function>()
    {
        let _ = callback.call2(&JsValue::UNDEFINED, &name.into(), &detail.into());
    }
}

/// A performance mark the page's timing checks read.
fn mark(name: &str) {
    if let Some(performance) = web_sys::window().and_then(|w| w.performance()) {
        let _ = performance.mark(name);
    }
}

/// Whether the chat is open with the Grid Foreman (#12446).
fn is_foreman() -> bool {
    js_sys::Reflect::get(&js_sys::global(), &model::FOREMAN_GLOBAL.into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Whether the chat is open at the Grid's help desk (#12443).
fn is_desk() -> bool {
    js_sys::Reflect::get(&js_sys::global(), &model::DESK_GLOBAL.into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn is_game_lab() -> bool {
    js_sys::Reflect::get(&js_sys::global(), &"openagentsLab".into())
        .ok()
        .and_then(|v| v.as_string())
        .as_deref()
        == Some("game")
}
