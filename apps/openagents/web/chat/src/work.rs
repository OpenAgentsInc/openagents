//! A Coder chat's running work in the canvas (#12316): the question Coder
//! waits on, drawn by the shared decision panel (`decision-panel`) through
//! the desktop's Rust Native adapter (`openagents_chat_view::rust_native_gpui`),
//! and its background agents with Stop. Both come from `GET /v1/agents`, and
//! the answers go back through `POST /v1/agents/actions`, as the phone's do.

use std::time::Duration;

use decision_panel::{ALLOWED, Flow, Paints, Step};
use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, Hsla, IntoElement, ParentElement, Render, Styled,
    Task, Window, div, prelude::*, px,
};
use openagents_chat_view::rust_native_gpui;
use openagents_web_chat_view::theme;
use openagents_web_client::web::Client;
use openagents_web_client::{Boards, ItemAction};
use rust_native::style::Color;

/// How often the open Coder chat's work is read again.
const EVERY: Duration = Duration::from_secs(4);

/// The open question, with the panel's own state.
struct Asking {
    computer: String,
    item: String,
    question: String,
    flow: Flow,
}

pub struct Work {
    client: Client,
    /// The open chat's Coder session; `None` shows nothing.
    session: Option<String>,
    boards: Boards,
    asking: Option<Asking>,
    /// An answer or a stop on its way.
    sending: bool,
    note: Option<String>,
    revision: u64,
    _poll: Task<()>,
}

impl Work {
    pub fn new(client: Client, cx: &mut Context<Self>) -> Self {
        let poll = cx.spawn(async move |this, cx| {
            loop {
                let Ok((client, session)) =
                    this.read_with(cx, |this, _| (this.client.clone(), this.session.clone()))
                else {
                    break;
                };
                if session.is_some() {
                    let boards = client.agents().await;
                    if this
                        .update(cx, |this, cx| {
                            if let Ok(boards) = boards {
                                this.boards = boards;
                                this.sync();
                                cx.notify();
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                cx.background_executor().timer(EVERY).await;
            }
        });
        Self {
            client,
            session: None,
            boards: Boards::default(),
            asking: None,
            sending: false,
            note: None,
            revision: 0,
            _poll: poll,
        }
    }

    /// Follow the chat with Coder session `session`, or nothing.
    pub fn show(&mut self, session: Option<String>, cx: &mut Context<Self>) {
        if self.session == session {
            return;
        }
        self.session = session;
        self.boards = Boards::default();
        self.asking = None;
        self.note = None;
        cx.notify();
    }

    /// Keeps the open question's panel while it is the same question.
    fn sync(&mut self) {
        let Some(session) = self.session.as_deref() else {
            self.asking = None;
            return;
        };
        match self.boards.asking(session) {
            Some((computer, item, question)) => {
                if self.asking.as_ref().map(|a| &a.question) != Some(&question.id) {
                    self.asking = Some(Asking {
                        computer: computer.to_owned(),
                        item: item.id.clone(),
                        question: question.id.clone(),
                        flow: Flow::approval(&question.text),
                    });
                    self.revision += 1;
                }
            }
            None => self.asking = None,
        }
    }

    fn act(
        &mut self,
        computer: String,
        item: String,
        action: &'static str,
        question: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.sending = true;
        self.note = None;
        let client = self.client.clone();
        cx.spawn(async move |this, cx| {
            let request = uuid::Uuid::new_v4().to_string();
            let sent = client
                .act(&ItemAction {
                    request_id: &request,
                    computer: &computer,
                    item: &item,
                    action,
                    question: question.as_deref(),
                    text: None,
                })
                .await;
            this.update(cx, |this, cx| {
                this.sending = false;
                this.note = Some(match (sent, action) {
                    (Ok(()), "stop") => format!("Asked Coder on {computer} to stop it."),
                    (Ok(()), _) => format!("Sent to Coder on {computer}."),
                    (Err(_), _) => "That didn't go through. Try again.".to_owned(),
                });
                if action != "stop" {
                    this.asking = None;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn press(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(asking) = self.asking.as_mut() else {
            return;
        };
        let panel = asking.flow.view("chat", true, &paints());
        let Some(control) = panel
            .controls
            .iter()
            .find_map(|(k, control)| (k == key).then_some(*control))
        else {
            return;
        };
        self.revision += 1;
        if let Step::Done(answer) = asking.flow.control(control) {
            let action = if answer == ALLOWED { "approve" } else { "deny" };
            let (computer, item, question) = (
                asking.computer.clone(),
                asking.item.clone(),
                asking.question.clone(),
            );
            self.act(computer, item, action, Some(question), cx);
        }
        cx.notify();
    }
}

impl Render for Work {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(session) = self.session.clone() else {
            return div().into_any_element();
        };
        let agents: Vec<(String, String, String, String, bool)> = self
            .boards
            .agents(&session)
            .into_iter()
            .map(|(computer, item)| {
                (
                    computer.to_owned(),
                    item.id.clone(),
                    item.title.clone(),
                    item.line.clone().unwrap_or_default(),
                    item.status == "working" || item.status == "asking",
                )
            })
            .collect();
        let asking = self.asking.as_ref().map(|asking| {
            let panel = asking.flow.view("chat", !self.sending, &paints());
            let card = rust_native_gpui::mount_root(
                &panel.node,
                "chat-decision",
                self.revision,
                |this: &mut Self, activation, cx| this.press(&activation.node, cx),
                cx,
            );
            div().rounded(px(14.)).overflow_hidden().child(card)
        });
        if asking.is_none() && agents.is_empty() && self.note.is_none() {
            return div().into_any_element();
        }
        div()
            .w_full()
            .max_w(px(760.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .px(px(16.))
            .children(asking)
            .when(!agents.is_empty(), |col| {
                col.child(
                    div()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::text_muted())
                        .child("Agents"),
                )
            })
            .children(agents.into_iter().enumerate().map(
                |(ix, (computer, item, title, line, live))| {
                    agent_row(
                        ix,
                        title,
                        line,
                        live,
                        self.sending,
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.act(computer.clone(), item.clone(), "stop", None, cx)
                        }),
                    )
                },
            ))
            .children(self.note.clone().map(|note| {
                div()
                    .text_size(px(12.5))
                    .text_color(theme::text_faint())
                    .child(note)
            }))
            .into_any_element()
    }
}

fn agent_row(
    ix: usize,
    title: String,
    line: String,
    live: bool,
    sending: bool,
    stop: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> AnyElement {
    div()
        .id(("chat-agent", ix))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.))
        .px(px(12.))
        .py(px(8.))
        .rounded(px(10.))
        .bg(theme::surface_card())
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(div().truncate().text_size(px(13.5)).child(title))
                .when(!line.is_empty(), |col| {
                    col.child(
                        div()
                            .truncate()
                            .text_size(px(12.))
                            .text_color(theme::text_faint())
                            .child(line),
                    )
                }),
        )
        .when(live, |row| {
            row.child(
                div()
                    .id(("chat-agent-stop", ix))
                    .px(px(10.))
                    .py(px(4.))
                    .rounded(px(12.))
                    .border_1()
                    .border_color(theme::border_strong())
                    .text_size(px(12.5))
                    .cursor_pointer()
                    .when(sending, |b| b.opacity(0.4))
                    .when(!sending, |b| b.on_click(stop))
                    .child("Stop"),
            )
        })
        .into_any_element()
}

/// The panel's colors from the desktop theme.
fn paints() -> Paints {
    let theme = theme::desktop();
    Paints {
        card: native(theme.surface_card),
        risk_low: native(theme.success.opacity(0.22)),
        risk_medium: native(theme.warning.opacity(0.22)),
        risk_high: native(theme.danger.opacity(0.22)),
    }
}

fn native(color: Hsla) -> Color {
    let rgba = gpui::Rgba::from(color);
    let channel = |value: f32| (value.clamp(0., 1.) * 255.).round() as u8;
    Color {
        red: channel(rgba.r),
        green: channel(rgba.g),
        blue: channel(rgba.b),
        alpha: channel(rgba.a),
    }
}
