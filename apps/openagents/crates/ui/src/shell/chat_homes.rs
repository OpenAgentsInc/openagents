//! Account-bound navigation and computer-to-cloud continuation.
use super::*;
use chat_home::{Continued, Home, Links, PartRef};
use openagents_account::{
    Lease,
    history::ExistingChat,
    openagents::{ChatSource, ChatSummary},
};

fn saved(lease: Lease, part: &PartRef, title: String, links: Links) -> Option<ExistingChat> {
    let chat = lease.client().chat(lease.session(), &part.id).ok()?;
    Some(ExistingChat {
        chat: chat.clone(),
        lease,
        summary: ChatSummary {
            home: part.home.clone(),
            links,
            chat,
            title,
            revision: 0,
            updated_unix: 0,
            pinned_unix: None,
            archived_unix: None,
            project: None,
            source: if part.home == Home::Cloud {
                ChatSource::Web
            } else {
                ChatSource::Other("session".into())
            },
            messages: 0,
        },
    })
}

impl Shell {
    pub(super) fn open_chat_part(&mut self, part: PartRef, cx: &mut Context<Self>) {
        if !part.valid() {
            return;
        }
        let lease = crate::openagents_account::AppAccount::global(cx)
            .and_then(|a| a.read(cx).controller.clone())
            .and_then(|a| a.lease());
        let Some(lease) = lease else {
            return;
        };
        match &part.home {
            Home::Computer(device) if !device.starts_with("legacy-") => {
                if !self
                    .state
                    .read(cx)
                    .chats
                    .iter()
                    .any(|c| c.id == part.id && c.device_id == *device)
                {
                    self.show_notice(
                        crate::toast::ToastKind::Error,
                        "Connect the computer that has this part of the chat.",
                        cx,
                    );
                    return;
                }
                self.set_route(Route::Chat, cx);
                self.state
                    .update(cx, |state, cx| state.select_chat(Some(part.id), cx));
            }
            _ => {
                if let Some(target) = saved(lease, &part, "Chat".into(), Links::default()) {
                    self.open_saved_chat(target, cx);
                }
            }
        }
    }

    pub(super) fn move_computer_chat(
        &mut self,
        chat: &str,
        target: PartRef,
        cx: &mut Context<Self>,
    ) {
        let Some(lease) = crate::openagents_account::AppAccount::global(cx)
            .and_then(|a| a.read(cx).controller.clone())
            .and_then(|a| a.lease())
        else {
            return;
        };
        let state = self.state.read(cx);
        let Some(source) = state.chats.iter().find(|c| c.id == chat).cloned() else {
            return;
        };
        let Some(engine) = state.engine().cloned() else {
            return;
        };
        if let Home::Computer(device) = &target.home {
            let part = PartRef {
                id: source.id.clone(),
                home: Home::Computer(source.device_id.clone()),
            };
            let Some(mut earlier) = saved(
                lease,
                &part,
                source.title.clone().unwrap_or_else(|| "Chat".into()),
                source.links.clone(),
            ) else {
                return;
            };
            earlier.summary.revision = source
                .last_message_at
                .unwrap_or(source.created_at)
                .timestamp()
                .max(1) as u64;
            let device = device.clone();
            self.open_new_session(None, cx);
            self.state.update(cx, |state, cx| {
                if state
                    .selected_space_row()
                    .is_some_and(|p| p.device_id != device)
                {
                    state.select_space(None, cx);
                }
                state.select_device(device, cx);
            });
            let composer = self.composer.clone();
            cx.defer(move |cx| {
                composer.update(cx, |composer, cx| {
                    composer.continue_saved_chat_at(earlier, target.id, cx)
                })
            });
            return;
        }
        let account = lease.session().account().id.clone();
        let expected_home = target.home.clone();
        let rpc_source = source.clone();
        let operation = Tokio::spawn(cx, async move {
            let mut params =
                serde_json::json!({"account":account,"chatId":rpc_source.id,"target":target});
            if engine.engine_info().device_id != rpc_source.device_id {
                params["targetDeviceId"] = serde_json::json!(rpc_source.device_id);
            }
            engine
                .client()
                .call_as::<Continued>(methods::CONTINUE_CHAT, params)
                .await
        });
        cx.spawn(async move |this, cx| {
            let result = operation.await;
            this.update(cx, |this, cx| {
                if !lease.is_current() { return; }
                let continued = match result {
                    Ok(Ok(value)) if value.account == lease.session().account().id
                        && value.source.id == source.id && value.source.home == Home::Computer(source.device_id.clone())
                        && value.target.home == expected_home && value.target.valid() && value.seed.valid() => value,
                    _ => {
                        this.show_notice(crate::toast::ToastKind::Error, "Couldn't move this chat. Stop the answer, clear queued messages, and try again.", cx);
                        return;
                    }
                };
                if this.state.read(cx).selected_chat.as_deref() != Some(source.id.as_str()) { return; }
                if continued.target.home == Home::Cloud {
                    if let Some(target) = saved(lease, &continued.target, continued.seed.title, Links {continues: Some(continued.source), continued_by: None}) {
                        this.open_saved_chat(target, cx);
                    }
                } else if let Home::Computer(device) = &continued.target.home {
                    let Some(source) = saved(lease, &continued.source, continued.seed.title, Links::default()) else {return;};
                    let device = device.clone();
                    this.open_new_session(None, cx);
                    this.state.update(cx, |state, cx| {
                        if state.selected_space_row().is_some_and(|p| p.device_id != device) { state.select_space(None, cx); }
                        state.select_device(device, cx);
                    });
                    let composer = this.composer.clone();
                    cx.defer(move |cx| composer.update(cx, |composer, cx| composer.continue_saved_chat_at(source, continued.target.id, cx)));
                }
            }).ok();
        }).detach();
    }
}
