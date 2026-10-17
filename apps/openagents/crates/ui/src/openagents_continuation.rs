//! A saved-chat reference follows the normal new-session folder and harness pickers.
use crate::{composer::Composer, theme::Theme};
use gpui::{prelude::*, *};
use openagents_account::{history::ExistingChat, openagents::continuations::Source};
use std::{collections::HashMap, sync::Arc};

struct Attempt {
    target: ExistingChat,
    chat: String,
    message: String,
}
#[derive(Default)]
pub struct Drafts(HashMap<String, Arc<Attempt>>);
impl Drafts {
    pub fn install(&mut self, target: ExistingChat) {
        self.0.insert(
            String::new(),
            Arc::new(Attempt {
                target,
                chat: uuid::Uuid::new_v4().to_string(),
                message: uuid::Uuid::new_v4().to_string(),
            }),
        );
    }
    pub fn validate(&self, key: &str, state: &crate::state::AppState) -> Result<(), &'static str> {
        let Some(source) = self.0.get(key) else {
            return Ok(());
        };
        if !source.target.is_current() {
            return Err("Sign in again before continuing.");
        }
        let engine = state
            .engine()
            .ok_or("Connect this computer before continuing.")?;
        if !engine
            .engine_info()
            .supports(openagents_proto::capabilities::OPENAGENTS_CONTINUATIONS_V1)
        {
            return Err("Update OpenAgents on this computer before continuing.");
        }
        if !key.is_empty() {
            return Ok(());
        }
        let Some(project) = state.selected_space_row() else {
            return Err("Choose a project folder on this computer.");
        };
        if state.local_device_id.as_deref() != Some(project.device_id.as_str()) {
            return Err("Choose a project folder on this computer.");
        }
        Ok(())
    }
    pub fn preferred_chat(&self, key: &str) -> Option<String> {
        self.0.get(key).map(|a| a.chat.clone())
    }
    pub fn message(&self, key: &str) -> Option<String> {
        self.0.get(key).map(|a| a.message.clone())
    }
    pub fn accepted(&mut self, chat: &str) {
        if self.0.get("").is_some_and(|a| a.chat == chat) {
            self.0.remove("");
        }
        self.0.remove(chat);
    }
    pub fn observe_active(&mut self, key: &str, state: &crate::state::AppState) {
        if !key.is_empty()
            && state
                .selected_chat_row()
                .and_then(|r| r.config.as_ref())
                .is_some_and(|c| {
                    !c.model_options
                        .contains_key(openagents_account::openagents::continuations::REQUEST_OPTION)
                })
        {
            self.accepted(key);
        }
    }
    pub fn bind(&mut self, key: &str, chat: &str) -> Option<Source> {
        if key != chat {
            if let Some(a) = self.0.get(key).cloned() {
                self.0.insert(chat.into(), a);
            }
        }
        self.0.get(chat).map(|a| Source {
            account: a.target.chat.account().into(),
            chat: a.target.chat.id().into(),
            revision: a.target.summary.revision,
        })
    }
    pub fn banner(&self, key: &str, cx: &mut Context<Composer>) -> Option<AnyElement> {
        let attempt = self.0.get(key).filter(|a| a.target.is_current())?;
        let target = &attempt.target;
        let hover = Theme::of(cx).glass_hover();
        Some(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px(px(12.))
                .py(px(4.))
                .text_size(px(12.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(SharedString::from(format!("From {}", target.summary.title))),
                )
                .when(key.is_empty(), |line| {
                    line.child(
                        div()
                            .id("cancel-saved-continuation")
                            .cursor_pointer()
                            .px(px(6.))
                            .rounded(px(4.))
                            .hover(move |s| s.bg(hover))
                            .child("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.continuations.0.remove(&this.current_key);
                                cx.notify();
                            })),
                    )
                })
                .into_any_element(),
        )
    }
}
impl Composer {
    pub fn continue_saved_chat(&mut self, target: ExistingChat, cx: &mut Context<Self>) {
        self.continuations.install(target);
        self.input
            .update(cx, |input, cx| input.set_text("Continue from here.", cx));
        cx.notify();
    }
}
