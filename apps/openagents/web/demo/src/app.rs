//! The chat window: sidebar of demo chats, the open transcript, and the
//! composer. Everything runs in the page; replies are the desktop demo's
//! scripted turns, streamed word by word through the veil.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    AnyElement, App, ClickEvent, Context, Entity, FocusHandle, Focusable, FollowMode, FontWeight,
    Hsla, Image, ImageFormat, IntoElement, ListAlignment, ListState, ParentElement, Render,
    SharedString, Styled, Window, div, list, prelude::*, px,
};
use openagents_doc::{MessagePart, MessageRole, MessageStatus, SessionMessageEntry};
use openagents_proto::{
    ChangeRequestState, HarnessId, ReasoningLevel, SessionStatus, UserInputQuestion,
};
use web_time::Instant;

use openagents_chat_view::rust_native_gpui;
use openagents_web_chat_view::composer::{Submitted, TextInput};
use openagents_web_chat_view::markdown::{self, Veil};
use openagents_web_chat_view::svg;
use openagents_web_chat_view::theme;
use openagents_web_chat_view::transcript::{
    self, CONTENT_WIDTH, OnClick, SIDEBAR_WIDTH, TurnView, ago, chip, choice_mark, footer_text,
    model_label, reasoning_label, truncate,
};

use crate::approval::{self, Approval};
use crate::demo::{fixtures, png, transcripts, transcripts::Step};
use crate::{attachments, now_ms};
const LONG_CHAT: &str = "chat-long";
const WEB_DEVICE: &str = "web-demo";

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum Group {
    Pinned,
    Section(&'static str),
    Project(String),
    Chats,
}

impl Group {
    fn label(&self) -> String {
        match self {
            Group::Pinned => "Pinned".into(),
            Group::Section(name) => (*name).into(),
            Group::Project(name) => name.clone(),
            Group::Chats => "Chats".into(),
        }
    }
}

struct Chat {
    id: String,
    title: SharedString,
    project: Option<String>,
    device: String,
    branch: Option<&'static str>,
    pr: Option<(u64, ChangeRequestState, &'static str)>,
    status: Option<SessionStatus>,
    harness: HarnessId,
    model: &'static str,
    reasoning: ReasoningLevel,
    last_at: i64,
    seen: bool,
    group: Group,
    entries: Vec<SessionMessageEntry>,
    opened: bool,
}

struct Stream {
    chat: usize,
    entry: usize,
    steps: VecDeque<Step>,
    words: VecDeque<String>,
    wait_until: Instant,
    started: Instant,
    /// The running tool: its part index, when it finishes, its output.
    tool: Option<(usize, Instant, Option<String>, bool)>,
    /// The fade on the text part now streaming.
    veil: Veil,
    ends_with_question: bool,
}

pub struct ChatApp {
    chats: Vec<Chat>,
    current: usize,
    list_state: ListState,
    composer: Entity<TextInput>,
    focus_handle: FocusHandle,
    stream: Option<Stream>,
    expanded: HashSet<String>,
    answers: HashMap<String, BTreeSet<usize>>,
    images: HashMap<String, Arc<Image>>,
    rng: u64,
    new_chats: usize,
    /// The fixture approval at the end of the working demo chat.
    approval: Approval,
}

impl ChatApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer = cx.new(|cx| TextInput::new("Ask anything", cx));
        cx.subscribe_in(
            &composer,
            window,
            |this, _, event: &Submitted, window, cx| {
                this.send(event.0.clone(), window, cx);
            },
        )
        .detach();

        let images = transcripts::DEMO_IMAGES
            .iter()
            .map(|(path, width, height, seed)| {
                let bytes = png::gradient(*width, *height, *seed);
                (
                    (*path).to_owned(),
                    Arc::new(Image::from_bytes(ImageFormat::Png, bytes)),
                )
            })
            .collect();

        let list_state = ListState::new(0, ListAlignment::Top, px(1200.));
        let mut this = Self {
            chats: load_chats(),
            current: 0,
            list_state,
            composer,
            focus_handle: cx.focus_handle(),
            stream: None,
            expanded: HashSet::new(),
            answers: HashMap::new(),
            images,
            rng: 0x9e37_79b9_7f4a_7c15,
            new_chats: 0,
            approval: Approval::new(),
        };
        this.open(0, window, cx);

        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        this
    }

    fn random_ms(&mut self, low: u64, high: u64) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        low + self.rng % (high - low).max(1)
    }

    fn open(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.current = ix;
        let chat = &mut self.chats[ix];
        chat.seen = true;
        let first_open = !chat.opened;
        chat.opened = true;
        self.list_state.reset(chat.entries.len());
        self.list_state.set_follow_mode(FollowMode::Tail);
        self.list_state.scroll_to_end();
        // The veil chat's last reply is still streaming when it first opens.
        if first_open
            && chat.id == "chat-veil"
            && self.stream.is_none()
            && let Some(entry) = chat.entries.len().checked_sub(1)
        {
            self.stream = Some(Stream {
                chat: ix,
                entry,
                steps: VecDeque::new(),
                words: split_words(transcripts::VEIL_LIVE_REST),
                wait_until: Instant::now() + Duration::from_millis(600),
                started: Instant::now(),
                tool: None,
                // The reply already on screen stays; only the rest fades in.
                veil: markdown::seeded_veil(),
                ends_with_question: false,
            });
        }
        let placeholder = if chat.entries.is_empty() {
            "What should we work on?"
        } else {
            "Ask anything"
        };
        self.composer
            .update(cx, |composer, _| composer.set_placeholder(placeholder));
        window.focus(&self.composer.focus_handle(cx), cx);
        cx.notify();
    }

    fn new_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.new_chats += 1;
        let chat = Chat {
            id: format!("chat-new-{}", self.new_chats),
            title: "New chat".into(),
            project: None,
            device: fixtures::MAC.into(),
            branch: None,
            pr: None,
            status: None,
            harness: HarnessId::ClaudeCode,
            model: "claude-fable-5",
            reasoning: ReasoningLevel::XHigh,
            last_at: now_ms(),
            seen: true,
            group: Group::Chats,
            entries: Vec::new(),
            opened: false,
        };
        self.chats.insert(0, chat);
        if let Some(stream) = &mut self.stream {
            stream.chat += 1;
        }
        self.open(0, window, cx);
    }

    fn send(&mut self, prompt: String, window: &mut Window, cx: &mut Context<Self>) {
        let prompt = prompt.trim().to_owned();
        if prompt.is_empty() || self.stream.is_some() {
            return;
        }
        let ix = self.current;
        let now = now_ms();
        let chat = &mut self.chats[ix];
        if chat.entries.is_empty() {
            chat.title = truncate(&prompt, 48).into();
        }
        let n = chat.entries.len();
        chat.entries.push(transcripts::entry(
            &format!("web-u{n}"),
            MessageRole::User,
            WEB_DEVICE,
            now,
            vec![transcripts::text("t0", &prompt)],
        ));
        self.sync_list();
        let steps = if prompt.contains("?ask") {
            transcripts::asking()
        } else {
            transcripts::reply(&prompt, false)
        };
        self.start_reply(ix, steps, cx);
        window.focus(&self.composer.focus_handle(cx), cx);
    }

    fn start_reply(&mut self, ix: usize, steps: Vec<Step>, cx: &mut Context<Self>) {
        let now = now_ms();
        let chat = &mut self.chats[ix];
        let n = chat.entries.len();
        let mut entry = transcripts::entry(
            &format!("web-a{n}"),
            MessageRole::Assistant,
            &chat.device,
            now,
            Vec::new(),
        );
        entry.status = Some(MessageStatus::Streaming);
        entry.duration_ms = None;
        chat.entries.push(entry);
        chat.status = Some(SessionStatus::Working);
        chat.last_at = now;
        let entry = chat.entries.len() - 1;
        if ix == self.current {
            self.sync_list();
        }
        let ends_with_question = matches!(steps.last(), Some(Step::Question(_)));
        self.stream = Some(Stream {
            chat: ix,
            entry,
            steps: steps.into(),
            words: VecDeque::new(),
            wait_until: Instant::now() + Duration::from_millis(450),
            started: Instant::now(),
            tool: None,
            veil: markdown::new_veil(),
            ends_with_question,
        });
        cx.notify();
    }

    /// Tells the list about entries appended to the open chat, and keeps it
    /// glued to the bottom so the new turn is in view.
    fn sync_list(&mut self) {
        let count = self.chats[self.current].entries.len();
        let known = self.list_state.item_count();
        if count > known {
            self.list_state.splice(known..known, count - known);
        } else if count < known {
            self.list_state.reset(count);
        }
        self.list_state.set_follow_mode(FollowMode::Tail);
        self.list_state.scroll_to_end();
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        if let Some(stream) = self.stream.take() {
            let chat = &mut self.chats[stream.chat];
            if let Some(entry) = chat.entries.get_mut(stream.entry) {
                entry.status = Some(MessageStatus::Aborted);
            }
            chat.status = None;
            cx.notify();
        }
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        let Some(mut stream) = self.stream.take() else {
            return;
        };
        let mut changed = false;
        if now >= stream.wait_until {
            changed = true;
            let entry = &mut self.chats[stream.chat].entries[stream.entry];
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
                let pause = self.random_ms(28, 110);
                stream.wait_until = now + Duration::from_millis(pause);
            } else if let Some(step) = stream.steps.pop_front() {
                let ix = entry.parts.len();
                match step {
                    Step::Reasoning(body) => {
                        entry.parts.push(MessagePart::Reasoning {
                            id: format!("r{ix}"),
                            text: body,
                        });
                        stream.wait_until = now + Duration::from_millis(700);
                    }
                    Step::Text(body) => {
                        entry.parts.push(transcripts::text(&format!("t{ix}"), ""));
                        stream.words = split_words(&body);
                        stream.veil = markdown::new_veil();
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
                    Step::Question(questions) => {
                        entry.parts.push(MessagePart::Input {
                            id: format!("q{ix}"),
                            request_id: format!("req-web-{}", stream.entry),
                            questions,
                            resolved: false,
                        });
                    }
                }
            } else {
                entry.status = Some(MessageStatus::Complete);
                entry.duration_ms = Some(now.duration_since(stream.started).as_millis() as i64);
                let chat = &mut self.chats[stream.chat];
                chat.status = stream
                    .ends_with_question
                    .then_some(SessionStatus::AwaitingInput);
                if stream.chat == self.current {
                    self.list_state
                        .remeasure_items(stream.entry..stream.entry + 1);
                }
                cx.notify();
                return;
            }
        }
        let fading = markdown::is_fading(&stream.veil);
        if (changed || fading) && stream.chat == self.current {
            self.list_state
                .remeasure_items(stream.entry..stream.entry + 1);
            cx.notify();
        } else if changed {
            cx.notify();
        }
        self.stream = Some(stream);
    }

    fn veil_for(&self, chat: usize, entry: usize, part: usize) -> Option<Veil> {
        let stream = self.stream.as_ref()?;
        if stream.chat != chat || stream.entry != entry {
            return None;
        }
        let parts = &self.chats[chat].entries[entry].parts;
        if part + 1 != parts.len() || !stream.tool.is_none() {
            return None;
        }
        Some(stream.veil.clone())
    }

    fn submit_answers(&mut self, entry_ix: usize, part_ix: usize, cx: &mut Context<Self>) {
        if self.stream.is_some() {
            return;
        }
        let chat_ix = self.current;
        let chat = &mut self.chats[chat_ix];
        let Some(MessagePart::Input {
            questions,
            resolved,
            ..
        }) = chat
            .entries
            .get_mut(entry_ix)
            .and_then(|entry| entry.parts.get_mut(part_ix))
        else {
            return;
        };
        let mut labels = Vec::new();
        for question in questions.iter() {
            let key = answer_key(&chat.id, entry_ix, &question.id);
            if let Some(picked) = self.answers.get(&key) {
                labels.extend(
                    picked
                        .iter()
                        .filter_map(|ix| question.options.get(*ix).cloned()),
                );
            }
        }
        *resolved = true;
        if chat_ix == self.current {
            self.list_state.remeasure_items(entry_ix..entry_ix + 1);
        }
        self.start_reply(chat_ix, transcripts::answered(&labels), cx);
    }

    fn toggle_answer(&mut self, key: String, option: usize, multi: bool, cx: &mut Context<Self>) {
        let picked = self.answers.entry(key).or_default();
        if picked.contains(&option) {
            picked.remove(&option);
        } else {
            if !multi {
                picked.clear();
            }
            picked.insert(option);
        }
        self.list_state.remeasure();
        cx.notify();
    }

    // -----------------------------------------------------------------
    // Sidebar
    // -----------------------------------------------------------------

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut groups: Vec<(Group, Vec<usize>)> = Vec::new();
        let mut order: Vec<usize> = (0..self.chats.len()).collect();
        order.sort_by_key(|ix| std::cmp::Reverse(self.chats[*ix].last_at));
        let rank = |group: &Group| match group {
            Group::Pinned => 0,
            Group::Section(_) => 1,
            Group::Project(_) => 2,
            Group::Chats => 3,
        };
        for ix in order {
            let group = self.chats[ix].group.clone();
            match groups.iter_mut().find(|(g, _)| *g == group) {
                Some((_, members)) => members.push(ix),
                None => groups.push((group, vec![ix])),
            }
        }
        groups.sort_by_key(|(group, members)| {
            (
                rank(group),
                match group {
                    Group::Pinned | Group::Section(_) => 0,
                    _ => -self.chats[members[0]].last_at,
                },
            )
        });

        div()
            .flex()
            .flex_col()
            .w(px(SIDEBAR_WIDTH))
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
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("OpenAgents"),
                    ),
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
                    .hover(|style| style.bg(theme::element_hover()).text_color(theme::text()))
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, window, cx| this.new_chat(window, cx)),
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
                    .id("sidebar-chats")
                    .flex_1()
                    .overflow_y_scroll()
                    .px(px(10.))
                    .pb(px(16.))
                    .children(groups.into_iter().map(|(group, members)| {
                        div()
                            .flex()
                            .flex_col()
                            .pt(px(14.))
                            .child(
                                div()
                                    .px(px(8.))
                                    .pb(px(4.))
                                    .text_size(px(12.5))
                                    .text_color(theme::text_faint())
                                    .child(group.label()),
                            )
                            .children(members.into_iter().map(|ix| self.render_chat_row(ix, cx)))
                    })),
            )
            .child(
                div()
                    .px(px(18.))
                    .py(px(14.))
                    .border_t_1()
                    .border_color(theme::border())
                    .text_size(px(12.))
                    .text_color(theme::text_faint())
                    .child("Demo chats, running in your browser"),
            )
    }

    fn render_chat_row(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let chat = &self.chats[ix];
        let selected = ix == self.current;
        let streaming = self.stream.as_ref().is_some_and(|s| s.chat == ix);
        let dot: Option<Hsla> = match (&chat.status, streaming) {
            (_, true) | (Some(SessionStatus::Working), _) => Some(theme::accent()),
            (Some(SessionStatus::AwaitingInput), _) => Some(theme::warning()),
            (Some(SessionStatus::Errored), _) => Some(theme::danger()),
            _ if !chat.seen => Some(theme::text_muted()),
            _ => None,
        };
        let subtitle = match (&chat.project, chat.branch) {
            (Some(project), Some(branch)) => format!("{project} · {branch}"),
            (Some(project), None) => project.clone(),
            (None, Some(branch)) => branch.to_owned(),
            (None, None) => device_label(&chat.device).to_owned(),
        };
        div()
            .id(SharedString::from(format!("row-{}", chat.id)))
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
            .on_click(
                cx.listener(move |this, _: &ClickEvent, window, cx| this.open(ix, window, cx)),
            )
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
                            .bg(dot.unwrap_or(theme::border())),
                    ),
            )
            .child(
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
                                    .text_color(if selected || !chat.seen {
                                        theme::text()
                                    } else {
                                        theme::text_muted()
                                    })
                                    .child(chat.title.clone()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(px(12.))
                                    .line_height(px(20.))
                                    .text_color(theme::text_faint())
                                    .child(ago(now_ms() - chat.last_at)),
                            ),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(12.))
                            .line_height(px(17.))
                            .text_color(theme::text_faint())
                            .child(subtitle),
                    ),
            )
            .into_any_element()
    }

    // -----------------------------------------------------------------
    // Header and composer
    // -----------------------------------------------------------------

    fn render_header(&self) -> impl IntoElement {
        let chat = &self.chats[self.current];
        let mut chips: Vec<AnyElement> = Vec::new();
        if let Some(project) = &chat.project {
            chips.push(chip(
                "icons/folder.svg",
                project.clone(),
                theme::text_muted(),
            ));
        }
        if let Some(branch) = chat.branch {
            chips.push(chip(
                "icons/git-branch.svg",
                branch.to_owned(),
                theme::text_muted(),
            ));
        }
        if let Some((number, state, _)) = &chat.pr {
            let (label, color) = match state {
                ChangeRequestState::Open => ("Open", theme::success()),
                ChangeRequestState::Merged => ("Merged", theme::accent()),
                ChangeRequestState::Closed => ("Closed", theme::danger()),
            };
            chips.push(chip(
                "icons/pull-request.svg",
                format!("#{number} {label}"),
                color,
            ));
        }
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.))
            .h(px(52.))
            .px(px(20.))
            .border_b_1()
            .border_color(theme::border())
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(px(14.5))
                    .font_weight(FontWeight::MEDIUM)
                    .child(chat.title.clone()),
            )
            .children(chips)
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(12.5))
                    .text_color(theme::text_faint())
                    .child(
                        svg()
                            .path("icons/laptop.svg")
                            .size(px(14.))
                            .text_color(theme::text_faint()),
                    )
                    .child(device_label(&chat.device)),
            )
    }

    fn render_composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let chat = &self.chats[self.current];
        let streaming = self.stream.as_ref().is_some_and(|s| s.chat == self.current);
        let empty = self.composer.read(cx).is_empty();
        let harness_icon = match chat.harness {
            HarnessId::Codex => "icons/openai-mark.svg",
            _ => "icons/claude-mark.svg",
        };
        let harness_color: Hsla = match chat.harness {
            HarnessId::Codex => theme::text(),
            _ => gpui::rgb(0xd97757).into(),
        };
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
            .when(!streaming && empty, |button| button.opacity(0.35))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                if streaming {
                    this.stop(cx);
                } else {
                    let text = this.composer.update(cx, |composer, cx| composer.take(cx));
                    this.send(text, window, cx);
                }
            }))
            .child(
                svg()
                    .path(if streaming {
                        "icons/stop.svg"
                    } else {
                        "icons/arrow-up.svg"
                    })
                    .size(px(17.))
                    .text_color(theme::on_solid()),
            );
        div()
            .flex()
            .justify_center()
            .px(px(24.))
            .pb(px(18.))
            .pt(px(6.))
            .child(
                div()
                    .w_full()
                    .max_w(px(CONTENT_WIDTH + 24.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(14.))
                    .pl(px(20.))
                    .pr(px(8.))
                    .h(px(56.))
                    .rounded(px(28.))
                    .border_1()
                    .border_color(theme::border_strong())
                    .bg(theme::surface_card())
                    .child(self.composer.clone())
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_none()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(13.5))
                            .text_color(theme::text_muted())
                            .child(
                                svg()
                                    .path(harness_icon)
                                    .size(px(15.))
                                    .text_color(harness_color),
                            )
                            .child(model_label(chat.model)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_none()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(13.5))
                            .text_color(theme::text_muted())
                            .child(
                                svg()
                                    .path("icons/tuning.svg")
                                    .size(px(15.))
                                    .text_color(theme::text_muted()),
                            )
                            .child(reasoning_label(chat.reasoning)),
                    )
                    .child(send),
            )
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
        let chat_ix = self.current;
        let Some(entry) = self.chats[chat_ix].entries.get(ix) else {
            return div().into_any_element();
        };
        let body = match entry.role {
            MessageRole::User => self.render_user(entry),
            _ => self.render_assistant(chat_ix, ix, entry, cx),
        };
        let chat = &self.chats[chat_ix];
        let body = if chat.id == approval::CHAT && ix + 1 == chat.entries.len() {
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(body)
                .child(self.render_approval(cx))
                .into_any_element()
        } else {
            body
        };
        transcript::entry_row(ix == 0, body)
    }

    /// The fixture approval, drawn by the shared decision panel through the
    /// desktop's Rust Native adapter.
    fn render_approval(&self, cx: &mut Context<Self>) -> AnyElement {
        let panel = self.approval.panel(theme::desktop());
        let card = rust_native_gpui::mount_root(
            &panel.node,
            "demo-approval",
            1,
            |this: &mut Self, activation, cx| {
                this.approval.press(&activation.node, theme::desktop());
                this.list_state.remeasure();
                cx.notify();
            },
            cx,
        );
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(div().rounded(px(14.)).overflow_hidden().child(card))
            .children(self.approval.answered().map(footer_text))
            .into_any_element()
    }

    fn render_user(&self, entry: &SessionMessageEntry) -> AnyElement {
        let text = entry
            .parts
            .iter()
            .filter_map(|part| match part {
                MessagePart::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        let (body, paths) = attachments::split(&text);
        let row = paths
            .iter()
            .map(|path| match self.images.get(*path) {
                Some(image) => transcript::attachment_image(image.clone()),
                None => {
                    let name = path.rsplit('/').next().unwrap_or(path);
                    chip("icons/paperclip.svg", name.to_owned(), theme::text_muted())
                }
            })
            .collect();
        transcript::user_bubble(&entry.id, body, row)
    }

    fn render_assistant(
        &self,
        chat_ix: usize,
        entry_ix: usize,
        entry: &SessionMessageEntry,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let chat_id = self.chats[chat_ix].id.clone();
        let key = format!("{chat_id}/{}", entry.id);
        let last_text = entry.parts.len().saturating_sub(1);
        let view = TurnView {
            key: &key,
            parts: &entry.parts,
            status: entry.status,
            duration_ms: entry.duration_ms,
            images: &self.images,
            expanded: &self.expanded,
            veil: self.veil_for(chat_ix, entry_ix, last_text),
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
        let mut questions = |part_ix: usize, asked: &[UserInputQuestion], resolved: bool| {
            self.render_questions(&chat_id, entry_ix, part_ix, asked, resolved, cx)
        };
        transcript::assistant_body(view, &toggle, &mut questions)
    }

    fn render_questions(
        &self,
        chat_id: &str,
        entry_ix: usize,
        part_ix: usize,
        questions: &[UserInputQuestion],
        resolved: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut card = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .p(px(16.))
            .rounded(px(14.))
            .border_1()
            .border_color(theme::border_strong())
            .bg(theme::surface_card());
        for question in questions {
            let key = answer_key(chat_id, entry_ix, &question.id);
            let picked = self.answers.get(&key).cloned().unwrap_or_default();
            let multi = question.multi_select;
            card = card.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme::text_faint())
                            .child(question.header.clone()),
                    )
                    .child(div().text_size(px(14.5)).child(question.question.clone()))
                    .child(
                        div().flex().flex_col().gap(px(4.)).children(
                            question
                                .options
                                .iter()
                                .enumerate()
                                .map(|(option_ix, option)| {
                                    let selected = picked.contains(&option_ix);
                                    let key = key.clone();
                                    div()
                                        .id(SharedString::from(format!("{key}/{option_ix}")))
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(10.))
                                        .px(px(10.))
                                        .py(px(7.))
                                        .rounded(px(9.))
                                        .text_size(px(14.))
                                        .border_1()
                                        .border_color(if selected {
                                            theme::accent().opacity(0.6)
                                        } else {
                                            theme::border()
                                        })
                                        .when(selected, |row| row.bg(theme::accent_wash()))
                                        .when(!resolved, |row| {
                                            row.cursor_pointer()
                                                .hover(|style| style.bg(theme::element_hover()))
                                                .on_click(cx.listener(
                                                    move |this, _: &ClickEvent, _, cx| {
                                                        this.toggle_answer(
                                                            key.clone(),
                                                            option_ix,
                                                            multi,
                                                            cx,
                                                        )
                                                    },
                                                ))
                                        })
                                        .child(choice_mark(selected, multi))
                                        .child(option.clone())
                                }),
                        ),
                    ),
            );
        }
        let can_submit = !resolved && self.stream.is_none();
        card = card.child(div().flex().flex_row().justify_end().child(if resolved {
            footer_text("Answered").into_any_element()
        } else {
            div()
                .id(SharedString::from(format!("{chat_id}/{entry_ix}/submit")))
                .px(px(14.))
                .h(px(32.))
                .flex()
                .items_center()
                .rounded(px(16.))
                .bg(theme::solid())
                .text_color(theme::on_solid())
                .text_size(px(13.5))
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .when(!can_submit, |button| button.opacity(0.4))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.submit_answers(entry_ix, part_ix, cx)
                }))
                .child("Submit")
                .into_any_element()
        }));
        card.into_any_element()
    }
}

impl Focusable for ChatApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ChatApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let wide = window.viewport_size().width > px(820.);
        let chat = &self.chats[self.current];
        let empty = chat.entries.is_empty();
        let transcript: AnyElement = if empty {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(10.))
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
                .child(
                    div()
                        .text_size(px(13.5))
                        .text_color(theme::text_faint())
                        .child(
                            "Replies here are a scripted demo. Add ?ask to get a question back.",
                        ),
                )
                .into_any_element()
        } else {
            div()
                .flex_1()
                .w_full()
                .flex()
                .flex_col()
                .child(markdown::selection_reset())
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
        let mut root = div()
            .id("openagents-web")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_row()
            .bg(theme::surface())
            .font_family(theme::SANS)
            .text_color(theme::text())
            .text_size(px(theme::BODY));
        root.text_style().font_fallbacks = Some(theme::fallbacks());
        root.when(wide, |root| root.child(self.render_sidebar(cx)))
            .child(
                div()
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
                            .child(self.render_header())
                            .child(transcript)
                            .child(self.render_composer(cx)),
                    ),
            )
    }
}

// ---------------------------------------------------------------------
// Pieces
// ---------------------------------------------------------------------

fn answer_key(chat: &str, entry: usize, question: &str) -> String {
    format!("{chat}/{entry}/{question}")
}

fn split_words(body: &str) -> VecDeque<String> {
    let mut out = VecDeque::new();
    let mut current = String::new();
    let mut in_space = false;
    for ch in body.chars() {
        let space = ch.is_whitespace();
        if !space && in_space {
            out.push_back(std::mem::take(&mut current));
        }
        in_space = space;
        current.push(ch);
    }
    if !current.is_empty() {
        out.push_back(current);
    }
    out
}

fn device_label(device: &str) -> &'static str {
    match device {
        fixtures::MAC => "MacBook Pro",
        fixtures::VPS => "Devbox VPS",
        fixtures::STUDIO => "Mac Studio",
        _ => "This browser",
    }
}

/// The desktop demo's chats, grouped the way its seeded sidebar groups them.
fn load_chats() -> Vec<Chat> {
    let now = now_ms();
    let spaces = fixtures::spaces(now);
    let pinned = ["chat-veil", "chat-picker"];
    let sections: [(&'static str, &[&str]); 2] = [
        ("P0", &["chat-tabs", "chat-errored"]),
        ("Mobile", &["chat-ios-scroll", "chat-ios-keyboard"]),
    ];
    let mut chats: Vec<Chat> = fixtures::chats()
        .into_iter()
        .filter(|demo| !demo.archived)
        .map(|demo| {
            let space = demo.space.and_then(|id| spaces.iter().find(|s| s.id == id));
            let project = space.map(|space| {
                space.name.clone().unwrap_or_else(|| {
                    space
                        .path
                        .rsplit('/')
                        .next()
                        .unwrap_or(&space.path)
                        .to_owned()
                })
            });
            let group = if pinned.contains(&demo.id) {
                Group::Pinned
            } else if let Some((name, _)) = sections.iter().find(|(_, ids)| ids.contains(&demo.id))
            {
                Group::Section(name)
            } else if let Some(project) = &project {
                Group::Project(project.clone())
            } else {
                Group::Chats
            };
            let last_at = now - demo.last_ago_ms;
            Chat {
                id: demo.id.to_owned(),
                title: demo.title.into(),
                project,
                device: demo.device.to_owned(),
                branch: demo.branch,
                pr: demo.pr,
                status: demo.status,
                harness: demo.harness,
                model: demo.model,
                reasoning: demo.reasoning,
                last_at,
                seen: demo.seen,
                group,
                entries: transcripts::fixture(demo.id, demo.device, last_at),
                opened: false,
            }
        })
        .collect();
    // The long synthetic transcript the desktop benchmarks scrolling with.
    let turns = 150u32;
    chats.push(Chat {
        id: LONG_CHAT.into(),
        title: "Ref index latency (150 turns)".into(),
        project: Some("openagents".into()),
        device: fixtures::MAC.into(),
        branch: Some("ref-index-cache"),
        pr: None,
        status: None,
        harness: HarnessId::Codex,
        model: "gpt-5.6-terra",
        reasoning: ReasoningLevel::High,
        last_at: now - 3 * 3_600_000,
        seen: true,
        group: Group::Project("openagents".into()),
        entries: transcripts::synthetic(
            turns,
            fixtures::MAC,
            now - 3 * 3_600_000 - turns as i64 * 60_000,
        ),
        opened: false,
    });
    // Open on the chat whose reply is still streaming.
    if let Some(ix) = chats.iter().position(|chat| chat.id == "chat-veil") {
        let chat = chats.remove(ix);
        chats.insert(0, chat);
    }
    chats
}
