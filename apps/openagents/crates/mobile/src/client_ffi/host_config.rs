//! Host-aware model choice for the new-session picker.
use super::{ChatConfig, CoreClient, CoreResult, on_runtime};

#[uniffi::export]
impl CoreClient {
    /// Keep a supported draft, or choose from the selected host's successful live catalog.
    pub async fn available_chat_config(
        &self,
        device_id: String,
        config: ChatConfig,
    ) -> CoreResult<Option<ChatConfig>> {
        let config = config.try_into()?;
        let client = self.client.clone();
        on_runtime(async move {
            Ok(client
                .available_chat_config(&device_id, config)
                .await
                .map(|config| (&config).into()))
        })
        .await
    }
}
