//! A new hidden chat and Codex thread for each call.
use super::*;
use openagents_proto::voice::{ORCHESTRATOR_CHAT_PREFIX, ORCHESTRATOR_CHAT_TITLE, VoiceRejection};

impl EngineRpc {
    /// Callers hold the preparation lock and have checked that no call is live.
    pub(super) async fn voice_chat(
        &self,
        mut config: ChatConfig,
    ) -> Result<String, VoiceRejection> {
        config.model_options.clear();
        let chat = format!("{ORCHESTRATOR_CHAT_PREFIX}{}", uuid::Uuid::new_v4());
        self.workspace
            .create_chat(
                &chat,
                None,
                Some(&self.engine_info.device_id),
                Some(config),
                None,
            )
            .map_err(|_| VoiceRejection::Protocol)?;
        self.workspace
            .rename_chat(&chat, ORCHESTRATOR_CHAT_TITLE)
            .map_err(|_| VoiceRejection::Protocol)?;
        Ok(chat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn calls_start_fresh_without_inheriting_the_previous_thread() {
        let temp = tempfile::tempdir().unwrap();
        let core = crate::EngineCore::assemble_with_profile(
            crate::EngineProfile::local(temp.path()).unwrap(),
            std::sync::Arc::new(HarnessRegistry::new()),
            HarnessId::Codex,
            None,
        )
        .unwrap();
        let rpc = core.rpc_service();
        let mut config: ChatConfig = serde_json::from_value(serde_json::json!({
            "harness": "codex", "sandbox": "workspace-write"
        }))
        .unwrap();
        config
            .model_options
            .insert("openagentsInstructions".into(), "user instructions".into());
        let first = rpc.voice_chat(config).await.unwrap();
        core.workspace
            .set_chat_harness_session(&first, "old-thread", "/old");
        let config = core
            .workspace
            .chat(&first)
            .unwrap()
            .unwrap()
            .config
            .unwrap();
        assert!(config.model_options.is_empty());
        let second = rpc.voice_chat(config).await.unwrap();
        assert_ne!(first, second);
        assert_eq!(core.workspace.chat_harness_session(&second), None);
        assert_eq!(
            core.workspace
                .chat(&second)
                .unwrap()
                .unwrap()
                .title
                .as_deref(),
            Some(ORCHESTRATOR_CHAT_TITLE)
        );
        core.sessions.shutdown().await;
    }
}
