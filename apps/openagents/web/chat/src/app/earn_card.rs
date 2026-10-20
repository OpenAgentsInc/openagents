//! Earn on a paired computer (#12294), as the desktop's Earn page shows it:
//! whether the computer shares, what it earned, and its pauses. The
//! computer runs its own `openagents pylon` command for each read and
//! change; the browser sends the `Pylon*` calls through its device room.
//! Where payouts go is changed only at the computer.

use std::time::Duration;

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, ParentElement, Styled, Task, div,
    prelude::*, px,
};
use openagents_chat_view::earn::{Earnings, pauses, status_line};
use openagents_web_chat_view::theme;
use openagents_web_chat_view::transcript::button;
use openagents_web_client::computer;
use openagents_web_client::earn::{self, Pause, Reply};
use openagents_web_client::web::Client;
use serde_json::Value;

use super::ChatApp;

/// How often an open Earn card reads the computer.
const REFRESH: Duration = Duration::from_secs(10);

pub(super) struct EarnCard {
    pub(super) device: String,
    status: Option<Reply>,
    earnings: Option<Reply>,
    settings: Option<Reply>,
    /// `up` still sets up on the computer.
    setting_up: bool,
    busy: bool,
    error: Option<String>,
    _poll: Option<Task<()>>,
}

async fn call(client: &Client, device: &str, method: &str, params: Value) -> Reply {
    let answer = async {
        let computer = computer::connect(client, device, || {}).await?;
        computer.call(method, params).await
    }
    .await;
    match answer {
        Ok(answer) => Reply::read(&answer),
        Err(error) => Reply::Failed(error.to_string()),
    }
}

impl ChatApp {
    /// Opens computer `device`'s Earn card, or closes it when it's open.
    pub(super) fn toggle_earn(&mut self, device: String, cx: &mut Context<Self>) {
        if self.earning.as_ref().is_some_and(|c| c.device == device) {
            self.earning = None;
            cx.notify();
            return;
        }
        self.earning = Some(EarnCard {
            device,
            status: None,
            earnings: None,
            settings: None,
            setting_up: false,
            busy: false,
            error: None,
            _poll: None,
        });
        self.follow_earn(cx);
        cx.notify();
    }

    /// Reads status, earnings, and settings now and every [`REFRESH`].
    fn follow_earn(&mut self, cx: &mut Context<Self>) {
        let Some(card) = &mut self.earning else {
            return;
        };
        let (client, device) = (self.client.clone(), card.device.clone());
        card._poll = Some(cx.spawn(async move |this, cx| {
            loop {
                let status = call(&client, &device, earn::PYLON_STATUS, Value::Null).await;
                let earnings = call(&client, &device, earn::PYLON_EARNINGS, Value::Null).await;
                let settings = call(&client, &device, earn::PYLON_SETTINGS, Value::Null).await;
                let open = this
                    .update(cx, |this, cx| {
                        let Some(card) = this.earning.as_mut().filter(|c| c.device == device)
                        else {
                            return false;
                        };
                        if status
                            .data()
                            .is_some_and(|d| d["state"] == "serving" || d["state"] == "paused")
                        {
                            card.setting_up = false;
                        }
                        card.status = Some(status);
                        card.earnings = Some(earnings);
                        card.settings = Some(settings);
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !open {
                    break;
                }
                cx.background_executor().timer(REFRESH).await;
            }
        }));
    }

    /// Sends one change, then reads the computer again.
    fn earn_change(&mut self, method: &'static str, params: Value, cx: &mut Context<Self>) {
        let Some(card) = &mut self.earning else {
            return;
        };
        if card.busy {
            return;
        }
        card.busy = true;
        card.error = None;
        let (client, device) = (self.client.clone(), card.device.clone());
        cx.spawn(async move |this, cx| {
            let answer = call(&client, &device, method, params).await;
            this.update(cx, |this, cx| {
                let Some(card) = this.earning.as_mut().filter(|c| c.device == device) else {
                    return;
                };
                card.busy = false;
                match answer {
                    Reply::Failed(message) => card.error = Some(message),
                    Reply::Unavailable => card.status = Some(Reply::Unavailable),
                    Reply::Running => card.setting_up = true,
                    Reply::Ok(_) => card.setting_up = method == earn::PYLON_UP,
                }
                this.follow_earn(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// The Earn card under computer `device`'s row, when it's open.
    pub(super) fn render_earn_card(
        &self,
        device: &str,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let card = self.earning.as_ref().filter(|c| c.device == device)?;
        let line = |text: String| div().text_size(px(13.)).child(text);
        let quiet = |text: String| {
            div()
                .text_size(px(13.))
                .text_color(theme::text_muted())
                .child(text)
        };
        let mut body = div()
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
                    .child("Earn"),
            );
        if matches!(card.status, Some(Reply::Unavailable)) {
            body = body.child(quiet(
                "This computer's OpenAgents command line doesn't have Earn yet. Update it on that computer.".into(),
            ));
            return Some(body.into_any_element());
        }
        let status = card.status.as_ref().and_then(Reply::data);
        let on = status.is_some_and(earn::sharing) || card.setting_up;
        body = body.child(line(match (status, &card.status) {
            (Some(data), _) => status_line(data, card.setting_up),
            (None, Some(Reply::Failed(message))) => message.clone(),
            _ => "Asking this computer…".into(),
        }));
        if let Some(data) = card.earnings.as_ref().and_then(Reply::data) {
            let earned = Earnings::from(data);
            let (title, under) = earned.lines();
            body = body.child(line(title)).child(quiet(under));
            body = body.child(quiet(match &earned.elsewhere {
                Some(address) => format!("Payouts go to {address}."),
                None => "Payouts go to this computer's wallet.".into(),
            }));
        }
        let mut actions: Vec<AnyElement> = vec![
            button("earn-switch", if on { "Turn off" } else { "Turn on" }, !on)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let method = if on { earn::PYLON_DOWN } else { earn::PYLON_UP };
                    this.earn_change(method, Value::Null, cx)
                }))
                .into_any_element(),
        ];
        if let Some(data) = card.settings.as_ref().and_then(Reply::data) {
            let (battery, hot) = pauses(data);
            for (pause, set, id) in [
                (Pause::Battery, battery, "earn-battery"),
                (Pause::Hot, hot, "earn-hot"),
            ] {
                let text = match (pause, set) {
                    (Pause::Battery, true) => "Pause on battery: On",
                    (Pause::Battery, false) => "Pause on battery: Off",
                    (Pause::Hot, true) => "Pause when hot: On",
                    (Pause::Hot, false) => "Pause when hot: Off",
                };
                actions.push(
                    button(id, text, false)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.earn_change(
                                earn::PYLON_SET_PAUSE,
                                earn::set_pause_params(pause, !set),
                                cx,
                            )
                        }))
                        .into_any_element(),
                );
            }
        }
        body = body
            .children(card.error.clone().map(|text| {
                div()
                    .text_size(px(13.))
                    .text_color(theme::danger())
                    .child(text)
            }))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(8.))
                    .when(card.busy, |row| row.opacity(0.5))
                    .children(actions),
            );
        Some(body.into_any_element())
    }
}
