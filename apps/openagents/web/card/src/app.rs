//! The card: a header and one chat. A person's question appears, then the
//! desktop demo's scripted reply streams in (a thought, text with code, a
//! file read, a test run, a table), word by word through the veil, as the
//! demo at /demo/gpui plays it. Some seconds after it settles the chat
//! starts over.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

use gpui::{
    AnyElement, ClickEvent, Context, FollowMode, FontWeight, IntoElement, ListAlignment, ListState,
    ParentElement, Render, Styled, Window, div, list, prelude::*, px, svg,
};
use openagents_doc::{MessagePart, MessageRole, MessageStatus, SessionMessageEntry};
use web_time::Instant;

use openagents_web_chat_view::markdown::{Veil, visible_chars};
use openagents_web_chat_view::theme;
use openagents_web_chat_view::transcript::{
    self, OnClick, TurnView, VEIL_FADE_MS, footer_text, veil_alpha, working_label,
};

use crate::demo::transcripts::{self, Step};
use crate::{now_ms, split_words};

/// The question the scripted reply answers.
const PROMPT: &str = "How does a streamed reply render on this device?";
const HOST: &str = "web-card";
/// The box the page gives the card (`openagents-gpui-web`'s mount id).
const MOUNT_ID: &str = "gpui-mount";
/// From the question appearing to the reply starting.
const THINK: Duration = Duration::from_millis(900);
/// How long the finished chat stays before it starts over.
const HOLD: Duration = Duration::from_millis(6000);
/// From the card's first frame to the question appearing. The first frame
/// is the header alone: on a slow computer every new glyph is rasterized on
/// the main thread the first time it is drawn, so the transcript's arrive a
/// few at a time instead of all in the frame that starts the card.
const OPENING: Duration = Duration::from_millis(500);
/// The card's update rate while it plays: 30 times a second, which the
/// word-by-word stream (a word every 28 to 110 ms) never outpaces, and half
/// the main-thread layout of 60.
const TICK: Duration = Duration::from_millis(33);

/// Tells the page something about the card: an event on `#gpui-mount`.
pub(crate) fn signal(name: &str) {
    let mount = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(MOUNT_ID));
    if let (Some(mount), Ok(event)) = (mount, web_sys::Event::new(name)) {
        let _ = mount.dispatch_event(&event);
    }
}

enum Phase {
    /// The empty card; the question appears at the instant.
    Opening(Instant),
    /// The question is showing; the reply starts at the instant.
    Asking(Instant),
    Streaming(Stream),
    /// The reply has settled; the chat starts over at the instant.
    Holding(Instant),
    /// Reduced motion: the finished chat, still.
    Still,
}

struct Stream {
    steps: VecDeque<Step>,
    words: VecDeque<String>,
    wait_until: Instant,
    started: Instant,
    /// The running tool: its part index, when it finishes, its output.
    tool: Option<(usize, Instant, Option<String>, bool)>,
    /// Chunks of the text part now streaming: visible characters, appended at.
    chunks: Vec<(usize, Instant)>,
}

pub struct CardApp {
    entries: Vec<SessionMessageEntry>,
    list_state: ListState,
    phase: Phase,
    rng: u64,
}

impl CardApp {
    pub fn new(still: bool, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            entries: Vec::new(),
            list_state: ListState::new(0, ListAlignment::Top, px(800.)),
            phase: Phase::Still,
            rng: 0x9e37_79b9_7f4a_7c15,
        };
        if still {
            this.finished();
        } else {
            this.phase = Phase::Opening(Instant::now() + OPENING);
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(TICK).await;
                    if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                        break;
                    }
                }
            })
            .detach();
        }
        this
    }

    fn random_ms(&mut self, low: u64, high: u64) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        low + self.rng % (high - low).max(1)
    }

    fn question() -> SessionMessageEntry {
        transcripts::entry(
            "card-u",
            MessageRole::User,
            HOST,
            now_ms(),
            vec![transcripts::text("t0", PROMPT)],
        )
    }

    fn reply_entry(status: MessageStatus) -> SessionMessageEntry {
        let mut entry =
            transcripts::entry("card-a", MessageRole::Assistant, HOST, now_ms(), Vec::new());
        entry.status = Some(status);
        entry.duration_ms = None;
        entry
    }

    fn sync_list(&mut self) {
        self.list_state.reset(self.entries.len());
        self.list_state.set_follow_mode(FollowMode::Tail);
        self.list_state.scroll_to_end();
    }

    /// The question alone, with the reply to follow.
    fn ask(&mut self) {
        self.entries = vec![Self::question()];
        self.sync_list();
        self.phase = Phase::Asking(Instant::now() + THINK);
    }

    /// The whole exchange, finished, with nothing moving.
    fn finished(&mut self) {
        let mut reply = Self::reply_entry(MessageStatus::Complete);
        for step in transcripts::reply(PROMPT, false) {
            let ix = reply.parts.len();
            match step {
                Step::Reasoning(text) => reply.parts.push(MessagePart::Reasoning {
                    id: format!("r{ix}"),
                    text,
                }),
                Step::Text(body) => reply
                    .parts
                    .push(transcripts::text(&format!("t{ix}"), &body)),
                Step::Tool {
                    call,
                    output,
                    is_error,
                    ..
                } => reply.parts.push(transcripts::tool(
                    &format!("k{ix}"),
                    call,
                    is_error,
                    output.as_deref(),
                )),
                Step::Question(_) => {}
            }
        }
        reply.duration_ms = Some(9_000);
        self.entries = vec![Self::question(), reply];
        self.sync_list();
        self.phase = Phase::Still;
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        match std::mem::replace(&mut self.phase, Phase::Still) {
            Phase::Asking(at) if now >= at => {
                self.entries
                    .push(Self::reply_entry(MessageStatus::Streaming));
                self.sync_list();
                self.phase = Phase::Streaming(Stream {
                    steps: transcripts::reply(PROMPT, false).into(),
                    words: VecDeque::new(),
                    wait_until: now + Duration::from_millis(450),
                    started: now,
                    tool: None,
                    chunks: Vec::new(),
                });
                cx.notify();
            }
            Phase::Opening(at) if now >= at => {
                self.ask();
                cx.notify();
            }
            Phase::Holding(at) if now >= at => {
                self.ask();
                cx.notify();
            }
            Phase::Streaming(stream) => self.advance(stream, now, cx),
            phase => self.phase = phase,
        }
    }

    /// One step of the stream, as the demo's `tick` takes it.
    fn advance(&mut self, mut stream: Stream, now: Instant, cx: &mut Context<Self>) {
        let pause = self.random_ms(28, 110);
        let mut changed = false;
        let last = self.entries.len() - 1;
        if now >= stream.wait_until {
            changed = true;
            let entry = &mut self.entries[last];
            if let Some((part, done_at, output, is_error)) = stream.tool.take() {
                if now >= done_at {
                    if let Some(MessagePart::Tool {
                        resolved,
                        output: out,
                        is_error: err,
                        output_bytes,
                        ..
                    }) = entry.parts.get_mut(part)
                    {
                        *resolved = true;
                        *output_bytes = output.as_ref().map(|o| o.len() as u64);
                        *out = output;
                        *err = is_error;
                    }
                    stream.wait_until = now + Duration::from_millis(250);
                } else {
                    stream.tool = Some((part, done_at, output, is_error));
                    changed = false;
                }
            } else if let Some(word) = stream.words.pop_front() {
                match entry.parts.last_mut() {
                    Some(MessagePart::Text { text, .. }) => text.push_str(&word),
                    _ => entry.parts.push(transcripts::text("t-live", &word)),
                }
                stream.chunks.push((visible_chars(&word), now));
                stream.wait_until = now + Duration::from_millis(pause);
            } else if let Some(step) = stream.steps.pop_front() {
                let ix = entry.parts.len();
                match step {
                    Step::Reasoning(text) => {
                        entry.parts.push(MessagePart::Reasoning {
                            id: format!("r{ix}"),
                            text,
                        });
                        stream.wait_until = now + Duration::from_millis(700);
                    }
                    Step::Text(body) => {
                        entry.parts.push(transcripts::text(&format!("t{ix}"), ""));
                        stream.words = split_words(&body);
                        stream.chunks.clear();
                    }
                    Step::Tool {
                        call,
                        output,
                        is_error,
                        run_ms,
                    } => {
                        let mut part = transcripts::tool(&format!("k{ix}"), call, false, None);
                        if let MessagePart::Tool { resolved, .. } = &mut part {
                            *resolved = false;
                        }
                        entry.parts.push(part);
                        stream.tool = Some((
                            ix,
                            now + Duration::from_millis(run_ms.max(300)),
                            output,
                            is_error,
                        ));
                    }
                    Step::Question(_) => {}
                }
            } else {
                entry.status = Some(MessageStatus::Complete);
                entry.duration_ms = Some(now.duration_since(stream.started).as_millis() as i64);
                self.list_state.remeasure_items(last..last + 1);
                self.phase = Phase::Holding(now + HOLD);
                cx.notify();
                return;
            }
        }
        let fading = stream
            .chunks
            .iter()
            .any(|(_, at)| now.duration_since(*at).as_secs_f32() * 1000. < VEIL_FADE_MS);
        if changed || fading {
            self.list_state.remeasure_items(last..last + 1);
            cx.notify();
        }
        self.phase = Phase::Streaming(stream);
    }

    fn veil(&self, part: usize) -> Option<Veil> {
        let Phase::Streaming(stream) = &self.phase else {
            return None;
        };
        let parts = &self.entries.last()?.parts;
        if part + 1 != parts.len() || stream.tool.is_some() {
            return None;
        }
        let now = Instant::now();
        let tail = stream
            .chunks
            .iter()
            .filter_map(|(chars, at)| {
                let elapsed = now.duration_since(*at).as_secs_f32() * 1000.;
                (elapsed < VEIL_FADE_MS).then(|| (*chars, veil_alpha(elapsed, VEIL_FADE_MS)))
            })
            .collect::<Vec<_>>();
        Some(Veil { tail })
    }

    fn render_entry(&mut self, ix: usize) -> AnyElement {
        let Some(entry) = self.entries.get(ix) else {
            return div().into_any_element();
        };
        let body = match entry.role {
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
            _ => {
                let key = format!("card/{}", entry.id);
                // Every tool group shows its steps; the card has no clicks.
                let expanded: HashSet<String> = (0..entry.parts.len())
                    .map(|start| format!("{key}/{start}"))
                    .collect();
                let images = HashMap::new();
                let view = TurnView {
                    key: &key,
                    parts: &entry.parts,
                    status: entry.status,
                    duration_ms: entry.duration_ms,
                    images: &images,
                    expanded: &expanded,
                    veil: self.veil(entry.parts.len().saturating_sub(1)),
                };
                let toggle = |_: String| -> OnClick { Box::new(|_: &ClickEvent, _, _| {}) };
                let mut questions = |_: usize, _: &[_], _: bool| div().into_any_element();
                transcript::assistant_body(view, &toggle, &mut questions)
            }
        };
        transcript::entry_row(ix == 0, body)
    }

    fn render_header(&self) -> impl IntoElement {
        let working = matches!(
            self.phase,
            Phase::Opening(_) | Phase::Asking(_) | Phase::Streaming(_)
        );
        div()
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .px(px(18.))
            .h(px(44.))
            .border_b_1()
            .border_color(theme::border())
            .child(
                svg()
                    .path("icons/claude-mark.svg")
                    .size(px(15.))
                    .text_color(gpui::rgb(0xd97757)),
            )
            .child(
                div()
                    .text_size(px(13.5))
                    .font_weight(FontWeight::MEDIUM)
                    .child("Claude Code"),
            )
            .child(div().flex_1())
            .child(if working {
                working_label().into_any_element()
            } else {
                footer_text("Done").into_any_element()
            })
    }
}

impl Render for CardApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut root = div()
            .id("openagents-card")
            .size_full()
            .flex()
            .flex_col()
            // A faint tint over the page's glass (WebGPU draws the card on a
            // transparent canvas); near-black where the canvas is opaque.
            .bg(gpui::hsla(0., 0., 0.05, 0.35))
            .font_family(theme::SANS)
            .text_color(theme::text())
            .text_size(px(theme::BODY));
        root.text_style().font_fallbacks = Some(theme::fallbacks());
        root.child(self.render_header()).child(
            list(
                self.list_state.clone(),
                cx.processor(|this, ix: usize, _window, _cx| this.render_entry(ix)),
            )
            .flex_1()
            .w_full(),
        )
    }
}
