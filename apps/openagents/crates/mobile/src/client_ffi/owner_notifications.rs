//! The shell displays owner notes and opens the checked chat ID supplied by Rust.
use super::{CoreClient, CoreError, CoreResult, on_runtime};
use openagents_account::openagents::owner_notifications::OwnerNotification;

/// Check a chat link before the shell opens it, including a cold launch.
#[uniffi::export]
pub fn owner_notification_chat_from_link(link: String) -> Option<String> {
    openagents_account::openagents::owner_notifications::chat_from_deep_link(&link)
        .map(str::to_owned)
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct OwnerNoteView {
    pub request_id: String,
    pub chat_id: String,
    pub title: String,
    pub body: String,
    pub deep_link: String,
}
fn checked(value: serde_json::Value) -> CoreResult<Option<OwnerNoteView>> {
    if value["notification"].is_null() {
        return Ok(None);
    }
    let n: OwnerNotification =
        serde_json::from_value(value["notification"].clone()).map_err(|_| {
            CoreError::HostError {
                message: "Couldn't read this notification.".into(),
            }
        })?;
    n.note.validate().map_err(|_| CoreError::HostError {
        message: "Couldn't read this notification.".into(),
    })?;
    if n.deep_link != n.note.deep_link()
        || !openagents_account::openagents::agents::valid_id(&n.request_id)
    {
        return Err(CoreError::HostError {
            message: "Couldn't open this chat.".into(),
        });
    }
    Ok(Some(OwnerNoteView {
        request_id: n.request_id,
        chat_id: n.note.chat,
        title: n.note.title,
        body: n.note.body,
        deep_link: n.deep_link,
    }))
}

#[uniffi::export]
impl CoreClient {
    pub async fn owner_notification(&self) -> CoreResult<Option<OwnerNoteView>> {
        let client = self.client.clone();
        let value = on_runtime(async move { client.owner_notification().await }).await?;
        value.map(checked).transpose().map(Option::flatten)
    }

    pub async fn read_owner_notification(&self, request_id: String) -> CoreResult<()> {
        let client = self.client.clone();
        on_runtime(async move { client.read_owner_notification(&request_id).await }).await
    }
    pub async fn push_available(&self) -> CoreResult<bool> {
        let client = self.client.clone();
        on_runtime(async move { client.push_config().await.map(|c| c.is_some()) }).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn chat_links_open_only_the_checked_chat_destination() {
        assert_eq!(
            owner_notification_chat_from_link("openagents://chat/helper-chat".into()),
            Some("helper-chat".into())
        );
        for bad in [
            "https://other.example/chat/x",
            "openagents://chat/../x",
            "openagents://chat/",
            "openagents://chat/x?other=y",
            "openagents://voice",
        ] {
            assert!(owner_notification_chat_from_link(bad.into()).is_none());
        }
    }
    #[test]
    fn attention_opens_the_sending_agents_chat() {
        let n = json!({"notification":{"request_id":"a".repeat(64),"note":{"agent":"b".repeat(64),"chat":"helper-chat","title":"I need your help","body":"The computer stopped answering."},"deep_link":"openagents://chat/helper-chat"}});
        let view = checked(n.clone()).unwrap().unwrap();
        assert_eq!(view.chat_id, "helper-chat");
        assert_eq!(view.title, "I need your help");
        assert_eq!(view.body, "The computer stopped answering.");
        let mut invalid = n;
        invalid["notification"]["deep_link"] = json!("https://other.example");
        assert!(checked(invalid).is_err());
        assert_eq!(checked(json!({"notification":null})).unwrap(), None);
    }
}
