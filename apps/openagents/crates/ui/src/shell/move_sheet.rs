//! Move running chats to a cloud computer and bring them back (#11933,
//! `docs/desktop/move-running-agents.md`): the sheet, its progress, and the
//! approval for moves and Bring backs an agent asked for.
//!
//! The engine keeps the plan; this sheet plans, approves, retries, and
//! reads it. Approving is the person's step: the agent tools can't.

use std::collections::HashSet;

use super::*;
use openagents_rpc::moves::{
    APPROVE_MOVE, AfterBringBack, BRING_BACK, ChatProgress, DISCARD_MOVE, MOVE_STATUS, MoveStatus,
    PLAN_MOVE, PlanView, Requester, START_MOVE, estimate_label,
};

/// How often an open sheet reads a running plan's progress.
const PROGRESS_EVERY: Duration = Duration::from_secs(2);
/// How often the app looks for moves an agent asked for.
const REQUESTS_EVERY: Duration = Duration::from_secs(15);

#[derive(Default)]
pub(super) struct Moves {
    sheet: Option<MoveSheet>,
    /// Agent requests the person already closed, by plan ID.
    seen: HashSet<String>,
    watch: Option<Task<()>>,
}

struct MoveSheet {
    /// None while the plan is being made.
    plan: Option<PlanView>,
    busy: bool,
    error: Option<String>,
    after: AfterBringBack,
    poll: Option<Task<()>>,
}

impl MoveSheet {
    fn new(plan: Option<PlanView>) -> Self {
        Self {
            busy: plan.is_none(),
            plan,
            error: None,
            after: AfterBringBack::Stop,
            poll: None,
        }
    }
}

/// The chats a plan would move now.
fn moving(plan: &PlanView) -> usize {
    plan.chats
        .iter()
        .filter(|c| c.state == openagents_rpc::moves::MoveState::Planned)
        .count()
}

fn move_label(count: usize) -> String {
    match count {
        1 => "Move 1 chat".into(),
        n => format!("Move {n} chats"),
    }
}

/// A chat stopped partway and can be tried again.
fn retryable(chat: &ChatProgress) -> bool {
    chat.reason.is_some() && !chat.state.finished() && !chat.state.moved()
        || chat
            .bring_back
            .as_ref()
            .is_some_and(|b| b.approved && !b.done && chat.reason.is_some())
}

/// Plain words for a chat's row.
fn status_line(chat: &ChatProgress) -> String {
    match &chat.bring_back {
        Some(back) if back.done => match &back.branch {
            Some(branch) => format!("Back on this computer. Its work is on the branch {branch}."),
            None => chat.state.label().into(),
        },
        Some(back) if back.approved => "Coming back to this computer".into(),
        Some(_) => "Waiting for you to approve bringing it back".into(),
        None => chat.state.label().into(),
    }
}

impl Shell {
    fn move_engine(&self, cx: &App) -> Option<crate::state::EngineHandle> {
        self.state.read(cx).engine().cloned()
    }

    /// Plan moving these chats, or every agent running here when empty.
    pub(crate) fn open_move_sheet(&mut self, chats: Vec<String>, cx: &mut Context<Self>) {
        self.moves.sheet = Some(MoveSheet::new(None));
        cx.notify();
        let params = serde_json::json!({ "chats": chats, "requestedBy": Requester::Person });
        self.move_call(PLAN_MOVE, params, cx);
    }

    /// Show a plan the engine already has.
    fn show_move_plan(&mut self, plan: PlanView, cx: &mut Context<Self>) {
        self.moves.sheet = Some(MoveSheet::new(Some(plan)));
        self.follow_move(cx);
        cx.notify();
    }

    fn close_move_sheet(&mut self, cx: &mut Context<Self>) {
        if let Some(plan) = self.moves.sheet.take().and_then(|s| s.plan) {
            self.moves.seen.insert(plan.id);
        }
        cx.notify();
    }

    /// Call a move method and show the plan it returns.
    fn move_call(
        &mut self,
        method: &'static str,
        params: serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        let Some(engine) = self.move_engine(cx) else {
            if let Some(sheet) = &mut self.moves.sheet {
                sheet.busy = false;
                sheet.error = Some("OpenAgents isn't connected yet. Try again in a moment.".into());
            }
            cx.notify();
            return;
        };
        if let Some(sheet) = &mut self.moves.sheet {
            sheet.busy = true;
            sheet.error = None;
        }
        let call = Tokio::spawn(cx, async move {
            engine.client().call_as::<PlanView>(method, params).await
        });
        cx.spawn(async move |this, cx| {
            let result = call.await;
            this.update(cx, |this, cx| {
                let Some(sheet) = &mut this.moves.sheet else {
                    return;
                };
                sheet.busy = false;
                match result {
                    Ok(Ok(plan)) => sheet.plan = Some(plan),
                    Ok(Err(error)) => sheet.error = Some(error.to_string()),
                    Err(_) => sheet.error = Some("We couldn't reach OpenAgents. Try again.".into()),
                }
                this.follow_move(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Read an approved plan's progress while the sheet is open.
    fn follow_move(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = &mut self.moves.sheet else {
            return;
        };
        let Some(plan) = &sheet.plan else {
            return;
        };
        if sheet.poll.is_some() || !plan.approved && !plan.needs_approval() {
            return;
        }
        let id = plan.id.clone();
        sheet.poll = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(PROGRESS_EVERY).await;
                let Ok(Some(engine)) = this.update(cx, |this, cx| this.move_engine(cx)) else {
                    return;
                };
                let params = serde_json::json!({ "planId": id });
                let status = Tokio::spawn(cx, async move {
                    engine
                        .client()
                        .call_as::<MoveStatus>(MOVE_STATUS, params)
                        .await
                });
                let plan = status
                    .await
                    .ok()
                    .and_then(Result::ok)
                    .and_then(|s| s.plans.into_iter().next());
                let open = this
                    .update(cx, |this, cx| {
                        let Some(sheet) = &mut this.moves.sheet else {
                            return false;
                        };
                        if sheet.plan.as_ref().map(|p| &p.id) != Some(&id) {
                            return false;
                        }
                        if let Some(plan) = plan
                            && !sheet.busy
                        {
                            sheet.plan = Some(plan);
                            cx.notify();
                        }
                        true
                    })
                    .unwrap_or(false);
                if !open {
                    return;
                }
            }
        }));
    }

    /// Look now and then for a move or Bring back an agent asked for, and
    /// show it for the person to approve.
    fn watch_move_requests(&mut self, cx: &mut Context<Self>) {
        if self.moves.watch.is_some() {
            return;
        }
        self.moves.watch = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(REQUESTS_EVERY).await;
                let Ok(engine) = this.update(cx, |this, cx| this.move_engine(cx)) else {
                    return;
                };
                let Some(engine) = engine else { continue };
                let status = Tokio::spawn(cx, async move {
                    engine
                        .client()
                        .call_as::<MoveStatus>(MOVE_STATUS, serde_json::json!({}))
                        .await
                });
                let Some(Ok(status)) = status.await.ok() else {
                    continue;
                };
                let alive = this.update(cx, |this, cx| {
                    if this.moves.sheet.is_some() {
                        return;
                    }
                    let asked = status.plans.into_iter().find(|p| {
                        !this.moves.seen.contains(&p.id)
                            && p.needs_approval()
                            && (p.requested_by == Requester::Agent
                                || p.chats.iter().any(|c| {
                                    c.bring_back
                                        .as_ref()
                                        .is_some_and(|b| b.requested_by == Requester::Agent)
                                }))
                    });
                    if let Some(plan) = asked {
                        this.show_move_plan(plan, cx);
                    }
                });
                if alive.is_err() {
                    return;
                }
            }
        }));
    }

    fn approve_move(&mut self, bring_back: Option<String>, cx: &mut Context<Self>) {
        let Some(id) = self.move_plan_id() else {
            return;
        };
        let params = serde_json::json!({ "planId": id, "bringBack": bring_back });
        self.move_call(APPROVE_MOVE, params, cx);
    }

    fn retry_move(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.move_plan_id() else {
            return;
        };
        self.move_call(START_MOVE, serde_json::json!({ "planId": id }), cx);
    }

    fn discard_move(&mut self, cx: &mut Context<Self>) {
        let (Some(id), Some(engine)) = (self.move_plan_id(), self.move_engine(cx)) else {
            self.close_move_sheet(cx);
            return;
        };
        let call = Tokio::spawn(cx, async move {
            engine
                .client()
                .call(DISCARD_MOVE, serde_json::json!({ "planId": id }))
                .await
        });
        cx.spawn(async move |_, _| {
            let _ = call.await;
        })
        .detach();
        self.close_move_sheet(cx);
    }

    /// The person brings a chat back: ask, then approve it at once.
    fn bring_back_here(&mut self, chat: String, cx: &mut Context<Self>) {
        let (Some(id), Some(engine)) = (self.move_plan_id(), self.move_engine(cx)) else {
            return;
        };
        let after = self
            .moves
            .sheet
            .as_ref()
            .map(|s| s.after)
            .unwrap_or_default();
        if let Some(sheet) = &mut self.moves.sheet {
            sheet.busy = true;
            sheet.error = None;
        }
        cx.notify();
        let call = Tokio::spawn(cx, async move {
            let client = engine.client();
            let params = serde_json::json!({ "chat": chat, "after": after, "requestedBy": Requester::Person });
            client.call_as::<PlanView>(BRING_BACK, params).await?;
            let params = serde_json::json!({ "planId": id, "bringBack": chat });
            client.call_as::<PlanView>(APPROVE_MOVE, params).await
        });
        cx.spawn(async move |this, cx| {
            let result = call.await;
            this.update(cx, |this, cx| {
                let Some(sheet) = &mut this.moves.sheet else {
                    return;
                };
                sheet.busy = false;
                match result {
                    Ok(Ok(plan)) => sheet.plan = Some(plan),
                    Ok(Err(error)) => sheet.error = Some(error.to_string()),
                    Err(_) => sheet.error = Some("We couldn't reach OpenAgents. Try again.".into()),
                }
                this.follow_move(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn move_plan_id(&self) -> Option<String> {
        self.moves
            .sheet
            .as_ref()?
            .plan
            .as_ref()
            .map(|p| p.id.clone())
    }

    pub(super) fn move_sheet_open(&self) -> bool {
        self.moves.sheet.is_some()
    }

    pub(super) fn render_move_overlays(
        &mut self,
        viewport: gpui::Size<Pixels>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        if self.moves.watch.is_none() && self.move_engine(cx).is_some() {
            self.watch_move_requests(cx);
        }
        let Some(sheet) = &self.moves.sheet else {
            return Vec::new();
        };
        let theme = Theme::of(cx).for_popup();
        let plan = sheet.plan.clone();
        let busy = sheet.busy;
        let after = sheet.after;
        let approved = plan.as_ref().is_some_and(|p| p.approved);
        let title = if approved {
            "Moving to a cloud computer"
        } else {
            "Move to a cloud computer"
        };
        let mut card = popover::dialog_card(&theme)
            .w(px(480.0))
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    this.close_move_sheet(cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(popover::dialog_title(&theme, title))
                    .child(
                        div()
                            .id("move-sheet-close")
                            .size(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.close_move_sheet(cx)))
                            .child(
                                icon(icons::CLOSE)
                                    .size(px(16.0))
                                    .text_color(theme.text_muted),
                            ),
                    ),
            );
        let muted = |text: String| div().mt(px(8.0)).text_color(theme.text_muted).child(text);
        match &plan {
            None => {
                card = card.child(muted(if busy {
                    "Getting the plan ready…".into()
                } else {
                    String::new()
                }));
            }
            Some(plan) => {
                if plan.requested_by == Requester::Agent && !plan.approved {
                    card = card.child(muted("An agent asked to move these chats.".into()));
                }
                card = card.child(muted(format!(
                    "Destination: {}. The chats keep running there while this computer restarts or updates.",
                    plan.destination
                )));
                if moving(plan) > 0 || plan.approved {
                    card = card.child(muted(format!(
                        "{}. Usage appears with your other cloud computer time.",
                        estimate_label(plan.estimate_minutes)
                    )));
                }
                let mut rows = div().mt(px(16.0)).flex().flex_col().gap(px(10.0));
                for chat in &plan.chats {
                    let mut row = div().flex().flex_col().child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .flex_1()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(format!("{} · {}", chat.title, chat.agent)),
                            )
                            .child(div().text_color(theme.text_muted).child(status_line(chat))),
                    );
                    if let Some(reason) = &chat.reason {
                        row = row.child(div().text_color(theme.text_muted).child(reason.clone()));
                    }
                    let back = chat.bring_back.as_ref();
                    if chat.state.moved() && back.is_none_or(|b| !b.approved) {
                        let target = chat.chat.clone();
                        let asked = back.is_some();
                        let label = if asked {
                            "Approve bringing it back"
                        } else {
                            "Bring back"
                        };
                        row = row.child(
                            div().mt(px(6.0)).flex().child(
                                popover::btn_ghost(
                                    &theme,
                                    label,
                                    format!("move-back-{}", chat.chat),
                                )
                                .id(SharedString::from(format!("move-back-{}", chat.chat)))
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        if asked {
                                            this.approve_move(Some(target.clone()), cx);
                                        } else {
                                            this.bring_back_here(target.clone(), cx);
                                        }
                                    },
                                )),
                            ),
                        );
                    }
                    rows = rows.child(row);
                }
                card = card.child(rows);
                if plan.chats.iter().any(|c| c.state.moved()) {
                    let mut choices = div().mt(px(16.0)).flex().items_center().gap(px(8.0)).child(
                        div()
                            .text_color(theme.text_muted)
                            .child("After a chat comes back:"),
                    );
                    for (choice, label) in [
                        (AfterBringBack::Stop, "Stop the computer"),
                        (AfterBringBack::Delete, "Delete it"),
                        (AfterBringBack::Keep, "Keep it"),
                    ] {
                        let key = format!("move-after-{label}");
                        let button = if after == choice {
                            popover::btn_primary(&theme, label)
                        } else {
                            popover::btn_ghost(&theme, label, key.clone())
                        };
                        choices = choices.child(button.id(SharedString::from(key)).on_click(
                            cx.listener(move |this, _, _, cx| {
                                if let Some(sheet) = &mut this.moves.sheet {
                                    sheet.after = choice;
                                }
                                cx.notify();
                            }),
                        ));
                    }
                    card = card.child(choices);
                }
            }
        }
        if let Some(error) = &sheet.error {
            card = card.child(
                div()
                    .mt(px(12.0))
                    .text_color(theme.danger)
                    .child(error.clone()),
            );
        }
        let mut buttons = div().mt(px(20.0)).flex().justify_end().gap(px(8.0));
        match &plan {
            Some(plan) if !plan.approved && moving(plan) > 0 => {
                let count = moving(plan);
                buttons = buttons
                    .child(
                        popover::btn_ghost(&theme, "Cancel", "move-cancel")
                            .id("move-cancel")
                            .on_click(cx.listener(|this, _, _, cx| this.discard_move(cx))),
                    )
                    .child(
                        popover::btn_primary(&theme, &move_label(count))
                            .id("move-approve")
                            .opacity(if busy { 0.5 } else { 1.0 })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if !this.moves.sheet.as_ref().is_some_and(|s| s.busy) {
                                    this.approve_move(None, cx);
                                }
                            })),
                    );
            }
            Some(plan) => {
                if plan.approved && plan.chats.iter().any(retryable) {
                    buttons = buttons.child(
                        popover::btn_ghost(&theme, "Try again", "move-retry")
                            .id("move-retry")
                            .on_click(cx.listener(|this, _, _, cx| this.retry_move(cx))),
                    );
                }
                buttons = buttons.child(
                    popover::btn_primary(&theme, "Done")
                        .id("move-done")
                        .on_click(cx.listener(|this, _, _, cx| this.close_move_sheet(cx))),
                );
            }
            None => {
                buttons = buttons.child(
                    popover::btn_ghost(&theme, "Cancel", "move-cancel")
                        .id("move-cancel")
                        .on_click(cx.listener(|this, _, _, cx| this.close_move_sheet(cx))),
                );
            }
        }
        card = card.child(buttons);
        vec![popover::modal(
            "move-sheet",
            viewport,
            card.into_any_element(),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_rpc::moves::{BringBackView, MoveState};

    fn chat(state: MoveState, reason: Option<&str>) -> ChatProgress {
        ChatProgress {
            chat: "c".into(),
            title: "Fix the build".into(),
            agent: "Codex".into(),
            state,
            reason: reason.map(Into::into),
            bring_back: None,
        }
    }

    #[test]
    fn rows_say_where_each_chat_is() {
        assert_eq!(move_label(1), "Move 1 chat");
        assert_eq!(move_label(3), "Move 3 chats");
        let mut back = chat(MoveState::BroughtBack, None);
        back.bring_back = Some(BringBackView {
            after: AfterBringBack::Stop,
            requested_by: Requester::Person,
            approved: true,
            branch: Some("openagents/cloud-0123abcd".into()),
            done: true,
        });
        assert_eq!(
            status_line(&back),
            "Back on this computer. Its work is on the branch openagents/cloud-0123abcd."
        );
        assert_eq!(
            status_line(&chat(MoveState::Released, None)),
            "Running on the cloud computer"
        );
    }

    #[test]
    fn only_a_stopped_move_offers_try_again() {
        assert!(retryable(&chat(
            MoveState::Captured,
            Some("We couldn't reach the cloud computer.")
        )));
        assert!(!retryable(&chat(MoveState::Captured, None)));
        assert!(!retryable(&chat(
            MoveState::StaysHere,
            Some("This project isn't a Git repository.")
        )));
        assert!(!retryable(&chat(MoveState::Released, None)));
    }
}
