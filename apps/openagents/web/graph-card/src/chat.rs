//! The demo chat at the center of the graph: the person's question, then the
//! scripted reply streamed through the shared chat views (thoughts and tool
//! calls folded into groups, text word by word through the veil, a table),
//! ending with the agent asking before it sends anything. As each tool call
//! runs, the chat hands the graph its cues. Some seconds after it settles
//! the chat starts over.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

use gpui::{
    AnyElement, ClickEvent, Context, FollowMode, IntoElement, ListAlignment, ListState,
    ParentElement, Render, Styled, Window, div, list, px,
};
use openagents_doc::{MessagePart, MessageRole, MessageStatus, SessionMessageEntry};
use openagents_web_chat_view::markdown::{self, Veil};
use openagents_web_chat_view::transcript::{self, OnClick, TurnView, info_card};
use web_time::Instant;

use crate::script::{self, Beat, Cue, PROMPT};

/// From the question appearing to the reply starting.
const THINK: Duration = Duration::from_millis(1100);
/// How long the finished chat stays before it starts over.
const HOLD: Duration = Duration::from_millis(7000);
/// From the first frame to the question appearing. The first frame is the
/// graph and the empty chat, so on a slow computer the transcript's glyphs
/// are rasterized a few at a time, as the `/lander6` card does it.
const OPENING: Duration = Duration::from_millis(500);
const HOST: &str = "web-graph-card";

fn text(id: &str, body: &str) -> MessagePart {
    MessagePart::Text {
        id: id.into(),
        text: body.into(),
    }
}

fn tool(id: &str, call: openagents_proto::ToolCall, output: Option<&str>) -> MessagePart {
    MessagePart::Tool {
        id: id.into(),
        call,
        is_error: false,
        resolved: output.is_some(),
        output: output.map(str::to_owned),
        diff: None,
        output_ref: None,
        output_bytes: output.map(|o| o.len() as u64),
        diff_ref: None,
        diff_stats: None,
        subagent_ref: None,
        subagent_status: None,
        subagent_tail: None,
    }
}

fn entry(id: &str, role: MessageRole, parts: Vec<MessagePart>) -> SessionMessageEntry {
    SessionMessageEntry {
        id: id.into(),
        role,
        parts,
        created_at: crate::now_ms(),
        device_id: HOST.into(),
        status: Some(MessageStatus::Complete),
        continuation_of: None,
        duration_ms: None,
    }
}

enum Phase {
    /// The empty chat; the question appears at the instant.
    Opening(Instant),
    Asking(Instant),
    Streaming(Stream),
    Holding(Instant),
    Still,
}

struct Running {
    part: usize,
    done_at: Instant,
    output: &'static str,
    cues: VecDeque<(Instant, Cue)>,
}

struct Stream {
    beats: VecDeque<Beat>,
    words: VecDeque<String>,
    wait_until: Instant,
    started: Instant,
    tool: Option<Running>,
    veil: Veil,
}

/// What one tick of the chat tells the graph.
#[derive(Default)]
pub struct Tick {
    pub cues: Vec<Cue>,
    /// The chat started over.
    pub reset: bool,
}

pub struct ChatView {
    entries: Vec<SessionMessageEntry>,
    list_state: ListState,
    phase: Phase,
    /// The question the agent is waiting on, once it asks.
    asking: Option<(&'static str, &'static str)>,
    rng: u64,
    /// Demo turns started from the page (`/lander9`); once there is one, the
    /// chat stops starting over.
    #[cfg(feature = "hands")]
    turns: usize,
}

impl ChatView {
    pub fn new(still: bool) -> Self {
        let mut this = Self {
            entries: Vec::new(),
            list_state: ListState::new(0, ListAlignment::Top, px(800.)),
            phase: Phase::Still,
            asking: None,
            rng: 0x2545_f491_4f6c_dd1d,
            #[cfg(feature = "hands")]
            turns: 0,
        };
        if still {
            this.finished();
        } else {
            this.phase = Phase::Opening(Instant::now() + OPENING);
        }
        this
    }

    /// Whether the agent is working.
    pub fn working(&self) -> bool {
        matches!(
            self.phase,
            Phase::Opening(_) | Phase::Asking(_) | Phase::Streaming(_)
        )
    }

    fn random_ms(&mut self, low: u64, high: u64) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        low + self.rng % (high - low).max(1)
    }

    fn reply_entry(status: MessageStatus) -> SessionMessageEntry {
        let mut entry = entry("graph-a", MessageRole::Assistant, Vec::new());
        entry.status = Some(status);
        entry
    }

    fn sync_list(&mut self) {
        self.list_state.reset(self.entries.len());
        self.list_state.set_follow_mode(FollowMode::Tail);
        self.list_state.scroll_to_end();
    }

    fn ask(&mut self) {
        self.entries = vec![entry(
            "graph-u",
            MessageRole::User,
            vec![text("t0", PROMPT)],
        )];
        self.asking = None;
        self.sync_list();
        self.phase = Phase::Asking(Instant::now() + THINK);
    }

    /// The whole exchange, finished and still.
    fn finished(&mut self) {
        let mut reply = Self::reply_entry(MessageStatus::Complete);
        for beat in script::reply() {
            let ix = reply.parts.len();
            match beat {
                Beat::Think(t) => reply.parts.push(MessagePart::Reasoning {
                    id: format!("r{ix}"),
                    text: t.into(),
                }),
                Beat::Text(t) => reply.parts.push(text(&format!("t{ix}"), t)),
                Beat::Tool { call, output, .. } => {
                    reply
                        .parts
                        .push(tool(&format!("k{ix}"), call, Some(output)))
                }
                Beat::Ask { title, line } => self.asking = Some((title, line)),
            }
        }
        reply.duration_ms = Some(14_000);
        self.entries = vec![
            entry("graph-u", MessageRole::User, vec![text("t0", PROMPT)]),
            reply,
        ];
        self.sync_list();
        self.phase = Phase::Still;
    }

    pub fn tick(&mut self, cx: &mut Context<Self>) -> Tick {
        let now = Instant::now();
        let mut out = Tick::default();
        match std::mem::replace(&mut self.phase, Phase::Still) {
            Phase::Asking(at) if now >= at => {
                self.entries
                    .push(Self::reply_entry(MessageStatus::Streaming));
                self.sync_list();
                self.phase = Phase::Streaming(Stream {
                    beats: script::reply().into(),
                    words: VecDeque::new(),
                    wait_until: now + Duration::from_millis(400),
                    started: now,
                    tool: None,
                    veil: markdown::new_veil(),
                });
                cx.notify();
            }
            Phase::Opening(at) if now >= at => {
                self.ask();
                cx.notify();
            }
            #[cfg(feature = "hands")]
            Phase::Holding(_) if self.turns > 0 => {}
            Phase::Holding(at) if now >= at => {
                self.ask();
                out.reset = true;
                cx.notify();
            }
            Phase::Streaming(stream) => self.advance(stream, now, &mut out, cx),
            phase => self.phase = phase,
        }
        out
    }

    fn advance(
        &mut self,
        mut stream: Stream,
        now: Instant,
        out: &mut Tick,
        cx: &mut Context<Self>,
    ) {
        let pause = self.random_ms(24, 90);
        let last = self.entries.len() - 1;
        let mut changed = false;
        if let Some(running) = stream.tool.as_mut() {
            while running.cues.front().is_some_and(|(at, _)| *at <= now) {
                if let Some((_, cue)) = running.cues.pop_front() {
                    out.cues.push(cue);
                }
            }
        }
        if now >= stream.wait_until {
            let entry = &mut self.entries[last];
            if let Some(running) = stream.tool.take() {
                if now >= running.done_at {
                    out.cues
                        .extend(running.cues.into_iter().map(|(_, cue)| cue));
                    if let Some(MessagePart::Tool {
                        resolved,
                        output,
                        output_bytes,
                        ..
                    }) = entry.parts.get_mut(running.part)
                    {
                        *resolved = true;
                        *output_bytes = Some(running.output.len() as u64);
                        *output = Some(running.output.into());
                    }
                    stream.wait_until = now + Duration::from_millis(300);
                    changed = true;
                } else {
                    stream.tool = Some(running);
                }
            } else if let Some(word) = stream.words.pop_front() {
                match entry.parts.last_mut() {
                    Some(MessagePart::Text { text, .. }) => text.push_str(&word),
                    _ => entry.parts.push(text("t-live", &word)),
                }
                stream.wait_until = now + Duration::from_millis(pause);
                changed = true;
            } else if let Some(beat) = stream.beats.pop_front() {
                let ix = entry.parts.len();
                changed = true;
                match beat {
                    Beat::Think(t) => {
                        entry.parts.push(MessagePart::Reasoning {
                            id: format!("r{ix}"),
                            text: t.into(),
                        });
                        stream.wait_until = now + Duration::from_millis(800);
                    }
                    Beat::Text(body) => {
                        entry.parts.push(text(&format!("t{ix}"), ""));
                        stream.words = crate::split_words(body);
                        stream.veil = markdown::new_veil();
                    }
                    Beat::Tool {
                        call,
                        run_ms,
                        output,
                        cues,
                    } => {
                        entry.parts.push(tool(&format!("k{ix}"), call, None));
                        stream.tool = Some(Running {
                            part: ix,
                            done_at: now + Duration::from_millis(run_ms),
                            output,
                            cues: cues
                                .into_iter()
                                .map(|(ms, cue)| (now + Duration::from_millis(ms), cue))
                                .collect(),
                        });
                    }
                    Beat::Ask { title, line } => {
                        self.asking = Some((title, line));
                        stream.wait_until = now + Duration::from_millis(200);
                    }
                }
            } else {
                entry.status = Some(MessageStatus::Complete);
                entry.duration_ms = Some(now.duration_since(stream.started).as_millis() as i64);
                self.list_state.remeasure_items(last..last + 1);
                self.list_state.scroll_to_end();
                self.phase = Phase::Holding(now + HOLD);
                cx.notify();
                return;
            }
        }
        let fading = markdown::is_fading(&stream.veil);
        // A running tool group pulses its summary line.
        let running = stream.tool.is_some();
        if changed || fading || running {
            self.list_state.remeasure_items(last..last + 1);
            cx.notify();
        }
        self.phase = Phase::Streaming(stream);
    }

    /// Starts a demo turn: the person's `prompt`, then `beats` streamed as
    /// the reply. A reply already playing finishes at once (the scripted
    /// one jumps to its end). Returns whether the scripted reply was cut
    /// short, so the graph can settle too.
    #[cfg(feature = "hands")]
    pub fn start_turn(&mut self, prompt: &'static str, beats: Vec<Beat>) -> bool {
        let mut cut = false;
        match std::mem::replace(&mut self.phase, Phase::Still) {
            Phase::Opening(_) | Phase::Asking(_) | Phase::Streaming(_) if self.turns == 0 => {
                self.finished();
                cut = true;
            }
            Phase::Streaming(_) => {
                if let Some(last) = self.entries.last_mut() {
                    last.status = Some(MessageStatus::Complete);
                    for part in &mut last.parts {
                        if let MessagePart::Tool {
                            resolved, output, ..
                        } = part
                            && !*resolved
                        {
                            *resolved = true;
                            *output = Some(String::new());
                        }
                    }
                }
            }
            _ => {}
        }
        self.turns += 1;
        self.asking = None;
        let n = self.turns;
        self.entries.push(entry(
            &format!("graph-u{n}"),
            MessageRole::User,
            vec![text("t0", prompt)],
        ));
        let mut reply = Self::reply_entry(MessageStatus::Streaming);
        reply.id = format!("graph-a{n}");
        self.entries.push(reply);
        self.sync_list();
        let now = Instant::now();
        self.phase = Phase::Streaming(Stream {
            beats: beats.into(),
            words: VecDeque::new(),
            wait_until: now + THINK / 2,
            started: now,
            tool: None,
            veil: markdown::new_veil(),
        });
        cut
    }

    fn veil(&self, part: usize) -> Option<Veil> {
        let Phase::Streaming(stream) = &self.phase else {
            return None;
        };
        let parts = &self.entries.last()?.parts;
        if part + 1 != parts.len() || stream.tool.is_some() {
            return None;
        }
        Some(stream.veil.clone())
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
                let key = format!("graph/{}", entry.id);
                // Every tool group shows its steps; the chat takes no clicks.
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
                let body = transcript::assistant_body(view, &toggle, &mut questions);
                match self.asking {
                    Some((title, line)) if ix + 1 == self.entries.len() => div()
                        .flex()
                        .flex_col()
                        .gap(px(14.))
                        .child(body)
                        .child(info_card("icons/check.svg", title.into(), line.into()))
                        .into_any_element(),
                    _ => body,
                }
            }
        };
        transcript::entry_row(ix == 0, body)
    }
}

impl Render for ChatView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(markdown::selection_reset())
            .child(
                list(
                    self.list_state.clone(),
                    cx.processor(|this, ix: usize, _window, _cx| this.render_entry(ix)),
                )
                .size_full(),
            )
    }
}
