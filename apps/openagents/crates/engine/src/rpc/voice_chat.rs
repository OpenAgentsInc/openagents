//! A new hidden chat and Codex thread for each call.
use super::*;
use openagents_proto::voice::{
    EARLIER_ORCHESTRATOR_TITLES, ORCHESTRATOR_CHAT_PREFIX, ORCHESTRATOR_CHAT_TITLE, VoiceRejection,
    is_orchestrator_chat,
};

impl EngineRpc {
    /// Callers hold the preparation lock and have checked that no call is live.
    pub(super) async fn voice_chat(
        &self,
        mut config: ChatConfig,
    ) -> Result<String, VoiceRejection> {
        config.model_options.clear();
        // Earlier builds' voice chats on this device take today's title.
        let device = &self.engine_info.device_id;
        for old in self.workspace.read_chats().unwrap_or_default() {
            if is_orchestrator_chat(&old.id)
                && &old.device_id == device
                && old
                    .title
                    .as_deref()
                    .is_some_and(|t| EARLIER_ORCHESTRATOR_TITLES.contains(&t))
            {
                let _ = self.workspace.rename_chat(&old.id, ORCHESTRATOR_CHAT_TITLE);
            }
        }
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
        // An earlier build's title is renamed by the next call.
        core.workspace.rename_chat(&first, "Voice session").unwrap();
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
                .chat(&first)
                .unwrap()
                .unwrap()
                .title
                .as_deref(),
            Some("Voice transcript")
        );
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
