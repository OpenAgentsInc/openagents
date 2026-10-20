//! Move a computer chat to a cloud computer and bring it back (#12282), as
//! the desktop's move sheet does. The chat's own computer plans the move,
//! saves the project and the agent's conversation, uploads them, and
//! continues the chat on a new cloud computer; the browser sends it the
//! `PlanMove` family through the device room and follows `MoveStatus`.

use std::time::Duration;

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, ParentElement, Styled, Task, div,
    prelude::*, px,
};
use openagents_proto::DeviceOrigin;
use openagents_web_chat_view::theme;
use openagents_web_chat_view::transcript::button;
use openagents_web_client::computer;
use openagents_web_client::moves::{self, MoveStatus, PlanView};
use openagents_web_client::web::Client;
use serde_json::Value;

use super::ChatApp;

/// How often an open move card reads the plan's progress.
const POLL: Duration = Duration::from_secs(3);
/// Where this browser remembers which computer a chat moved from.
const SOURCE_KEY: &str = "openagents.move.source.";

pub(super) struct MoveSheet {
    chat: String,
    /// The computer that planned the move and does its work.
    source: String,
    plan: Option<PlanView>,
    busy: bool,
    error: Option<String>,
    _poll: Option<Task<()>>,
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

async fn call(client: &Client, device: &str, method: &str, params: Value) -> anyhow::Result<Value> {
    let computer = computer::connect(client, device, || {}).await?;
    computer.call(method, params).await
}

impl ChatApp {
    /// Whether computer chat `id` runs on a cloud computer now.
    pub(super) fn on_cloud(&self, id: &str) -> bool {
        let Some(device) = self.meta.get(id).map(|m| m.computer.0.clone()) else {
            return false;
        };
        self.registry.as_ref().is_some_and(|r| {
            r.read(|r| {
                r.devices().into_iter().any(|d| {
                    d.id == device && matches!(d.origin, DeviceOrigin::ManagedCloud { .. })
                })
            })
        })
    }

    /// Plans moving chat `id` to a new cloud computer, and shows the plan.
    pub(super) fn open_move(&mut self, id: String, cx: &mut Context<Self>) {
        self.menu = None;
        let Some(source) = self.meta.get(&id).map(|m| m.computer.0.clone()) else {
            return;
        };
        if let Some(storage) = storage() {
            let _ = storage.set_item(&format!("{SOURCE_KEY}{id}"), &source);
        }
        self.moving = Some(MoveSheet {
            chat: id.clone(),
            source,
            plan: None,
            busy: false,
            error: None,
            _poll: None,
        });
        self.move_call(moves::PLAN_MOVE, moves::plan_params(&id), cx);
    }

    /// Shows the move of chat `id`, now on a cloud computer, so it can come
    /// back.
    pub(super) fn open_bring_back(&mut self, id: String, cx: &mut Context<Self>) {
        self.menu = None;
        let source = storage()
            .and_then(|s| s.get_item(&format!("{SOURCE_KEY}{id}")).ok().flatten())
            .or_else(|| self.meta.get(&id).map(|m| m.computer.0.clone()));
        let Some(source) = source else {
            return;
        };
        self.moving = Some(MoveSheet {
            chat: id,
            source,
            plan: None,
            busy: true,
            error: None,
            _poll: None,
        });
        self.follow_move(cx);
        cx.notify();
    }

    fn close_move(&mut self, cx: &mut Context<Self>) {
        self.moving = None;
        cx.notify();
    }

    /// Sends a move command to the chat's computer and shows the plan it
    /// answers with.
    fn move_call(&mut self, method: &'static str, params: Value, cx: &mut Context<Self>) {
        let Some(sheet) = &mut self.moving else {
            return;
        };
        sheet.busy = true;
        sheet.error = None;
        let (client, source) = (self.client.clone(), sheet.source.clone());
        cx.spawn(async move |this, cx| {
            let answer = call(&client, &source, method, params).await;
            this.update(cx, |this, cx| {
                let Some(sheet) = &mut this.moving else {
                    return;
                };
                sheet.busy = false;
                match answer.and_then(|v| Ok(serde_json::from_value::<PlanView>(v)?)) {
                    Ok(plan) => sheet.plan = Some(plan),
                    Err(error) => sheet.error = Some(error.to_string()),
                }
                this.follow_move(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Reads the plan's progress every few seconds while the card is open.
    fn follow_move(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = &mut self.moving else {
            return;
        };
        let (client, source, chat) = (
            self.client.clone(),
            sheet.source.clone(),
            sheet.chat.clone(),
        );
        let plan = sheet.plan.as_ref().map(|p| p.id.clone());
        sheet._poll = Some(cx.spawn(async move |this, cx| {
            loop {
                let params = match &plan {
                    Some(id) => moves::plan_id_params(id),
                    None => serde_json::json!({}),
                };
                let status = call(&client, &source, moves::MOVE_STATUS, params)
                    .await
                    .and_then(|v| Ok(serde_json::from_value::<MoveStatus>(v)?));
                let done = this
                    .update(cx, |this, cx| {
                        let Some(sheet) = &mut this.moving else {
                            return true;
                        };
                        sheet.busy = false;
                        match status {
                            Ok(status) => match status.plan_for(&chat) {
                                Some(found) => sheet.plan = Some(found.clone()),
                                None if sheet.plan.is_none() => {
                                    sheet.error = Some(
                                        "We couldn't find this chat's move on its computer.".into(),
                                    )
                                }
                                None => {}
                            },
                            Err(error) => sheet.error = Some(error.to_string()),
                        }
                        cx.notify();
                        sheet.plan.as_ref().is_some_and(|p| p.finished())
                    })
                    .unwrap_or(true);
                if done {
                    break;
                }
                cx.background_executor().timer(POLL).await;
            }
        }));
    }

    fn move_plan(&self) -> Option<String> {
        self.moving.as_ref()?.plan.as_ref().map(|p| p.id.clone())
    }

    /// The person brings the chat back: ask, then approve it at once.
    fn bring_back(&mut self, cx: &mut Context<Self>) {
        let (Some(plan), Some(sheet)) = (self.move_plan(), self.moving.as_mut()) else {
            return;
        };
        sheet.busy = true;
        sheet.error = None;
        let (client, source, chat) = (
            self.client.clone(),
            sheet.source.clone(),
            sheet.chat.clone(),
        );
        cx.spawn(async move |this, cx| {
            let answer = async {
                call(
                    &client,
                    &source,
                    moves::BRING_BACK,
                    moves::bring_back_params(&chat),
                )
                .await?;
                let approved = call(
                    &client,
                    &source,
                    moves::APPROVE_MOVE,
                    moves::approve_params(&plan, Some(&chat)),
                )
                .await?;
                Ok::<_, anyhow::Error>(serde_json::from_value::<PlanView>(approved)?)
            }
            .await;
            this.update(cx, |this, cx| {
                let Some(sheet) = &mut this.moving else {
                    return;
                };
                sheet.busy = false;
                match answer {
                    Ok(plan) => sheet.plan = Some(plan),
                    Err(error) => sheet.error = Some(error.to_string()),
                }
                this.follow_move(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn discard_move(&mut self, cx: &mut Context<Self>) {
        if let (Some(plan), Some(sheet)) = (self.move_plan(), self.moving.as_ref()) {
            let (client, source) = (self.client.clone(), sheet.source.clone());
            cx.spawn(async move |_, _| {
                let _ = call(
                    &client,
                    &source,
                    moves::DISCARD_MOVE,
                    moves::plan_id_params(&plan),
                )
                .await;
            })
            .detach();
        }
        self.close_move(cx);
    }

    /// The move card, over the open chat.
    pub(super) fn render_move_sheet(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let sheet = self.moving.as_ref()?;
        let plan = sheet.plan.as_ref();
        let chat = plan.and_then(|p| p.chats.iter().find(|c| c.chat == sheet.chat));
        let mut actions: Vec<AnyElement> = Vec::new();
        if let Some(plan) = plan {
            if plan.needs_approval() {
                let id = plan.id.clone();
                actions.push(
                    button("move-approve", "Move", true)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.move_call(
                                moves::APPROVE_MOVE,
                                moves::approve_params(&id, None),
                                cx,
                            )
                        }))
                        .into_any_element(),
                );
                actions.push(
                    button("move-discard", "Don't move", false)
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.discard_move(cx)))
                        .into_any_element(),
                );
            } else if plan.retryable() {
                let id = plan.id.clone();
                actions.push(
                    button("move-retry", "Try again", true)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.move_call(moves::START_MOVE, moves::plan_id_params(&id), cx)
                        }))
                        .into_any_element(),
                );
            }
            if chat.is_some_and(|c| c.state.moved() && c.bring_back.is_none()) {
                actions.push(
                    button("move-bring-back", "Bring back", plan.retryable())
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.bring_back(cx)))
                        .into_any_element(),
                );
            }
        }
        actions.push(
            button("move-close", "Close", false)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_move(cx)))
                .into_any_element(),
        );
        let line = match (chat, plan) {
            (Some(chat), _) => moves::status_line(chat),
            (None, _) if sheet.busy => "Asking this chat's computer…".into(),
            _ => String::new(),
        };
        let estimate = plan.filter(|p| p.needs_approval()).map(|p| {
            format!(
                "{}. {}",
                p.destination,
                moves::estimate_label(p.estimate_minutes)
            )
        });
        Some(
            div()
                .mx(px(16.))
                .mt(px(10.))
                .p(px(12.))
                .rounded(px(12.))
                .border_1()
                .border_color(theme::border_strong())
                .bg(theme::surface_card())
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .child("Move to a cloud computer"),
                )
                .children(estimate.map(|line| {
                    div()
                        .text_size(px(13.))
                        .text_color(theme::text_muted())
                        .child(line)
                }))
                .when(!line.is_empty(), |card| {
                    card.child(div().text_size(px(13.)).child(line))
                })
                .children(sheet.error.clone().map(|line| {
                    div()
                        .text_size(px(13.))
                        .text_color(theme::danger())
                        .child(line)
                }))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap(px(8.))
                        .when(sheet.busy, |row| row.opacity(0.5))
                        .children(actions),
                )
                .into_any_element(),
        )
    }
}
