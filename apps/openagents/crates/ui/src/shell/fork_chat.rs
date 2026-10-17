//! Fork chat (#11963): the session card's "Fork chat" makes a new standalone
//! chat with the selected chat's history (`ForkChat` on the chat's own
//! computer), lists it in Sessions and opens it. Its transcript starts below a
//! "Forked from <title>" link to the source.
use super::*;

/// The "Forked from <title>" link in a fork's transcript.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = shell, no_json)]
pub(crate) struct OpenForkSource(pub String);

impl Shell {
    pub(super) fn open_fork_source(&mut self, chat_id: &str, cx: &mut Context<Self>) {
        let exists = self
            .state
            .read(cx)
            .chats
            .iter()
            .any(|chat| chat.id == chat_id);
        if exists {
            self.open_chat(chat_id.to_owned(), cx);
        } else {
            self.show_notice(
                crate::toast::ToastKind::Error,
                "The original chat was deleted",
                cx,
            );
        }
    }

    pub(super) fn fork_selected_chat(&mut self, cx: &mut Context<Self>) {
        if self.fork_creating {
            return;
        }
        let Some(source) = self.state.read(cx).selected_chat_row().cloned() else {
            return;
        };
        let Some(engine) = self.state.read(cx).engine().cloned() else {
            return;
        };
        self.fork_creating = true;
        let params = serde_json::json!({
            "chatId": uuid::Uuid::new_v4().to_string(),
            "sourceChatId": source.id,
            "targetDeviceId": source.device_id,
        });
        cx.spawn(async move |this, cx| {
            let result = engine
                .client()
                .call_as::<openagents_proto::Chat>(methods::FORK_CHAT, params)
                .await;
            let _ = this.update(cx, |this, cx| {
                this.fork_creating = false;
                match result {
                    Ok(chat) => {
                        let id = chat.id.clone();
                        this.state
                            .update(cx, |state, _| state.add_pending_chat(chat));
                        this.open_chat(id, cx);
                    }
                    Err(error) => {
                        let message: SharedString = error.to_string().into();
                        this.composer
                            .update(cx, |composer, cx| composer.show_error(message, cx));
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
