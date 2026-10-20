//! The demo chat at the center of the graph: the person's question, then the
//! scripted reply drawn by the desktop's transcript (thoughts and tool
//! calls folded into groups, text word by word, a table),
//! ending with the agent asking before it sends anything. As each tool call
//! runs, the chat hands the graph its cues. Some seconds after it settles
//! the chat starts over.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AppContext as _, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div,
};
use openagents_doc::{MessagePart, MessageRole, MessageStatus, SessionMessageEntry};
use openagents_web_chat_view::desktop_transcript::{self, WebTranscript};
use openagents_web_chat_view::labels::info_card;
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
    /// The chat as the desktop's transcript reads it.
    shared: Entity<WebTranscript>,
    /// The desktop's transcript, drawing `shared` (#12216).
    transcript: Entity<desktop_transcript::Transcript>,
    published: Option<(Vec<(usize, usize, u8)>, bool)>,
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
    pub fn new(still: bool, cx: &mut Context<Self>) -> Self {
        let shared = cx.new(|_| WebTranscript::default());
        let transcript =
            desktop_transcript::new(&shared, Rc::new(RefCell::new(HashMap::new())), cx);
        {
            let chat = cx.entity().downgrade();
            transcript.update(cx, |view, cx| {
                view.set_rail_enabled(false, cx);
                // The agent's question, under its reply once it asks.
                view.media.after_entry = Some(Rc::new(move |entry, _, cx| {
                    let chat = chat.upgrade()?;
                    let chat = chat.read(cx);
                    let (title, line) = chat.asking?;
                    (chat.entries.last().map(|last| last.id.as_str()) == Some(entry)).then(|| {
                        div()
                            .pt(gpui::px(14.))
                            .child(info_card("icons/check.svg", title.into(), line.into()))
                            .into_any_element()
                    })
                }));
            });
        }
        let mut this = Self {
            entries: Vec::new(),
            shared,
            transcript,
            published: None,
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

    fn ask(&mut self) {
        self.entries = vec![entry(
            "graph-u",
            MessageRole::User,
            vec![text("t0", PROMPT)],
        )];
        self.asking = None;
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
        self.phase = Phase::Still;
    }

    pub fn tick(&mut self, cx: &mut Context<Self>) -> Tick {
        let now = Instant::now();
        let mut out = Tick::default();
        match std::mem::replace(&mut self.phase, Phase::Still) {
            Phase::Asking(at) if now >= at => {
                self.entries
                    .push(Self::reply_entry(MessageStatus::Streaming));
                self.phase = Phase::Streaming(Stream {
                    beats: script::reply().into(),
                    words: VecDeque::new(),
                    wait_until: now + Duration::from_millis(400),
                    started: now,
                    tool: None,
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
                self.phase = Phase::Holding(now + HOLD);
                cx.notify();
                return;
            }
        }
        if changed {
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
        let now = Instant::now();
        self.phase = Phase::Streaming(Stream {
            beats: beats.into(),
            words: VecDeque::new(),
            wait_until: now + THINK / 2,
            started: now,
            tool: None,
        });
        cut
    }
}

impl Render for ChatView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let published = (
            desktop_transcript::shape(&self.entries),
            self.asking.is_some(),
        );
        if self.published.as_ref() != Some(&published) {
            self.published = Some(published);
            desktop_transcript::show(
                &self.shared,
                &self.transcript,
                Some("graph".into()),
                self.entries.clone(),
                None,
                cx,
            );
        }
        desktop_transcript::fit(
            &self.transcript,
            f32::from(window.viewport_size().width),
            false,
            cx,
        );
        div().size_full().child(self.transcript.clone())
    }
}
