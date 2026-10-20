//! Dictation (#12292): the composer's microphone. The page's dictation
//! module records and sends the audio to the chat's own computer, which
//! transcribes it; the text lands in the composer. This side only asks
//! (`oa-dictation-start`, `-stop`) and follows `oa-dictation`.

use futures::StreamExt as _;
use gpui::{ClickEvent, Context, IntoElement, ParentElement, Styled, Task, div, prelude::*, px};
use openagents_web_chat_view::{svg, theme};
use openagents_web_client::store;

use super::ChatApp;

#[derive(Default)]
pub(super) struct Dictation {
    /// Recording, or waiting for the computer's text.
    on: bool,
    _events: Option<Task<()>>,
}

impl ChatApp {
    /// The computer that would transcribe: the open chat's, or a new
    /// session's.
    fn dictation_device(&self) -> Option<String> {
        match self.current.as_deref() {
            Some(id) => self.meta.get(id).map(|m| m.computer.0.clone()),
            None => self.target.computer.clone(),
        }
    }

    fn toggle_dictation(&mut self, cx: &mut Context<Self>) {
        if self.dictation.on {
            store::signal("oa-dictation-stop", "");
            return;
        }
        let Some(device) = self.dictation_device() else {
            return;
        };
        if self.dictation._events.is_none() {
            let mut events = store::listen("oa-dictation");
            self.dictation._events = Some(cx.spawn(async move |this, cx| {
                while let Some(detail) = events.next().await {
                    let value: serde_json::Value =
                        serde_json::from_str(&detail).unwrap_or_default();
                    let done = this.update(cx, |this, cx| {
                        match value["state"].as_str() {
                            Some("done") => {
                                this.dictation.on = false;
                                let text = value["text"].as_str().unwrap_or_default().trim();
                                if !text.is_empty() {
                                    this.composer.update(cx, |composer, cx| {
                                        let before = composer.text(cx);
                                        let joined = if before.trim().is_empty() {
                                            text.to_string()
                                        } else {
                                            format!("{} {text}", before.trim_end())
                                        };
                                        composer.set_text(joined, cx);
                                    });
                                }
                            }
                            Some("failed") => {
                                this.dictation.on = false;
                                this.notice = value["message"].as_str().map(str::to_owned);
                            }
                            _ => {}
                        }
                        cx.notify();
                    });
                    if done.is_err() {
                        break;
                    }
                }
            }));
        }
        self.dictation.on = true;
        self.notice = None;
        store::signal("oa-dictation-start", &device);
        cx.notify();
    }

    /// The microphone, for a chat on a computer.
    pub(super) fn render_mic(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        self.dictation_device()?;
        let on = self.dictation.on;
        Some(
            div()
                .id("dictate")
                .flex_none()
                .size(px(34.))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .when(on, |b| b.bg(theme::danger()))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_dictation(cx)))
                .child(
                    svg()
                        .path("icons/microphone.svg")
                        .size(px(17.))
                        .text_color(if on {
                            theme::on_solid()
                        } else {
                            theme::text_muted()
                        }),
                ),
        )
    }
}
