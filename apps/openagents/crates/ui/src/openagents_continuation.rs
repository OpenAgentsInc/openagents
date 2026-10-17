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
pub struct Drafts(HashMap<String, Arc<Attempt>>, HomeControl);
#[derive(Default)]
struct HomeControl {
    revision: u64,
    fingerprint: String,
    choices: Option<rust_native::ValidatedView<chat_home::Home>>,
    parts: Option<rust_native::ValidatedView<chat_home::PartRef>>,
    attempts: HashMap<String, chat_home::PartRef>,
}
impl Drafts {
    pub fn lease(&self, key: &str) -> Option<openagents_account::Lease> {
        self.0.get(key).map(|a| a.target.lease.clone())
    }
    pub fn native_source(&self, key: &str) -> Option<(String, ExistingChat)> {
        let target = &self.0.get(key)?.target;
        let chat_home::Home::Computer(device) = &target.summary.home else {
            return None;
        };
        matches!(&target.summary.source, openagents_account::openagents::ChatSource::Other(kind) if kind == "session")
            .then(|| (device.clone(), target.clone()))
    }
    pub fn install(&mut self, target: ExistingChat) {
        self.install_at(target, uuid::Uuid::new_v4().to_string());
    }
    pub fn install_at(&mut self, target: ExistingChat, chat: String) {
        self.0.insert(
            String::new(),
            Arc::new(Attempt {
                target,
                chat,
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
        if let Some((source, _)) = self.native_source(key) {
            if project.device_id == source {
                return Err("Choose another computer to continue this chat.");
            }
            if !state.devices.iter().any(|d| {
                d.id == project.device_id
                    && d.supports(openagents_proto::capabilities::CHAT_HOME_TRANSFER_V1)
            }) {
                return Err("Update OpenAgents on the selected computer before continuing.");
            }
        }
        if !state.devices.iter().any(|device| {
            device.id == project.device_id
                && device.supports(openagents_proto::capabilities::OPENAGENTS_CONTINUATIONS_V1)
        }) && state.local_device_id.as_deref() != Some(project.device_id.as_str())
        {
            return Err("Update OpenAgents on the selected computer before continuing.");
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
    pub fn continue_saved_chat_at(
        &mut self,
        target: ExistingChat,
        chat: String,
        cx: &mut Context<Self>,
    ) {
        self.continuations.install_at(target, chat);
        self.input
            .update(cx, |input, cx| input.set_text("Continue from here.", cx));
        cx.notify();
    }

    pub(crate) fn chat_home_controls(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        use chat_home::{Home, PartRef, view::Destination};
        let state = self.state.read(cx);
        let chat = state.selected_chat_row()?.clone();
        if !chat.is_top_level()
            || chat
                .config
                .as_ref()
                .is_some_and(|config| config.model_options.contains_key(chat_home::REQUEST_OPTION))
        {
            return None;
        }
        let lease = crate::openagents_account::AppAccount::global(cx)
            .and_then(|a| a.read(cx).controller.clone())
            .and_then(|a| a.lease())?;
        let local = state.local_device_id.as_deref();
        let current = Destination {
            home: Home::Computer(chat.device_id.clone()),
            available: true,
            label: if local == Some(chat.device_id.as_str()) {
                "This Mac".into()
            } else {
                state
                    .devices
                    .iter()
                    .find(|d| d.id == chat.device_id)
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| "Computer".into())
            },
        };
        let mut destinations = vec![Destination {
            home: Home::Cloud,
            label: "OpenAgents".into(),
            available: true,
        }];
        destinations.extend(
            state
                .devices
                .iter()
                .filter(|d| !matches!(d.platform.as_str(), "ios" | "android" | "ipados"))
                .filter(|d| d.supports(openagents_proto::capabilities::CHAT_HOME_TRANSFER_V1))
                .filter_map(|d| {
                    Some(Destination {
                        home: Home::computer(&d.id)?,
                        label: if local == Some(d.id.as_str()) {
                            "This Mac".into()
                        } else {
                            d.name.clone()
                        },
                        available: true,
                    })
                }),
        );
        let busy = state
            .sessions
            .iter()
            .find(|s| s.chat_id == chat.id)
            .is_some_and(|s| {
                matches!(
                    s.status,
                    openagents_proto::SessionStatus::Working
                        | openagents_proto::SessionStatus::AwaitingInput
                )
            })
            || !state.queue.is_empty();
        let can_move = state.devices.iter().any(|d| {
            d.id == chat.device_id
                && d.supports(openagents_proto::capabilities::CHAT_HOME_TRANSFER_V1)
        }) || (local == Some(chat.device_id.as_str())
            && state.engine().is_some_and(|e| {
                e.engine_info()
                    .supports(openagents_proto::capabilities::CHAT_HOME_TRANSFER_V1)
            }));
        if !can_move {
            destinations.clear();
        }
        let blocked = if chat.links.continued_by.is_some() {
            Some("This chat continues elsewhere.")
        } else if busy {
            Some("Stop the current answer and clear queued messages before continuing elsewhere.")
        } else if !self.input.read(cx).text().trim().is_empty()
            || self
                .attachments
                .get(&self.current_key)
                .is_some_and(|a| !a.is_empty())
        {
            Some("Send or clear your draft before moving this chat.")
        } else {
            None
        };
        let ui = &mut self.continuations.1;
        let fingerprint = serde_json::to_string(&(
            &chat.id,
            &chat.links,
            lease.generation,
            &current.home,
            &current.label,
            blocked,
            destinations
                .iter()
                .map(|d| (&d.home, &d.label, d.available))
                .collect::<Vec<_>>(),
        ))
        .ok()?;
        if ui.fingerprint != fingerprint {
            ui.revision = ui.revision.wrapping_add(1);
            ui.choices = chat_home::view::choices(
                &format!("home-{}", chat.id),
                ui.revision,
                &current,
                &destinations,
                blocked,
            )
            .ok();
            ui.parts =
                chat_home::view::parts(&format!("parts-{}", chat.id), ui.revision, &chat.links)
                    .ok();
            ui.fingerprint = fingerprint;
        }
        let mut body = div().flex().flex_col().gap(px(6.)).px(px(12.)).py(px(6.));
        if let Some(view) = &ui.choices {
            let source = chat.id.clone();
            let lease = lease.clone();
            body = body.child(crate::native_actions::mount(
                view,
                move |this: &mut Self, event, cx| {
                    if !lease.is_current()
                        || this.state.read(cx).selected_chat.as_deref() != Some(source.as_str())
                        || !this.input.read(cx).text().trim().is_empty()
                    {
                        return;
                    }
                    let Some(home) = this
                        .continuations
                        .1
                        .choices
                        .as_ref()
                        .and_then(|v| v.activate(&event).ok())
                        .cloned()
                    else {
                        return;
                    };
                    let target = this
                        .continuations
                        .1
                        .attempts
                        .entry(format!(
                            "{}:{}",
                            source,
                            serde_json::to_string(&home).unwrap()
                        ))
                        .or_insert_with(|| PartRef {
                            id: uuid::Uuid::new_v4().to_string(),
                            home,
                        })
                        .clone();
                    cx.emit(crate::composer::ComposerEvent::ContinueOn {
                        chat: source.clone(),
                        target,
                    });
                },
                cx,
            ));
        }
        if let Some(view) = &self.continuations.1.parts {
            let source = chat.id.clone();
            body = body.child(crate::native_actions::mount(
                view,
                move |this: &mut Self, event, cx| {
                    if !lease.is_current()
                        || this.state.read(cx).selected_chat.as_deref() != Some(source.as_str())
                    {
                        return;
                    }
                    if let Some(part) = this
                        .continuations
                        .1
                        .parts
                        .as_ref()
                        .and_then(|v| v.activate(&event).ok())
                        .cloned()
                    {
                        cx.emit(crate::composer::ComposerEvent::OpenPart(part));
                    }
                },
                cx,
            ));
        }
        Some(body.into_any_element())
    }
    pub fn continue_saved_chat(&mut self, target: ExistingChat, cx: &mut Context<Self>) {
        self.continuations.install(target);
        self.input
            .update(cx, |input, cx| input.set_text("Continue from here.", cx));
        cx.notify();
    }
}
