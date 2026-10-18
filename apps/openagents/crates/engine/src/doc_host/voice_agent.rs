//! Resolve the account voice choice without using display names.
use super::*;
use openagents_account::openagents::agents::voice::VoiceTarget;
impl DocHost {
    /// The account's voice choice; the caller must re-read it for each spoken request.
    pub async fn voice_agent(
        &self,
    ) -> Result<openagents_account::openagents::agents::voice::VoiceAgent, EngineError> {
        let lease = self
            .openagents_lease()
            .ok_or_else(|| EngineError::Other("Connecting to your account…".into()))?;
        let agents = lease
            .client()
            .agents(lease.session())
            .map_err(|e| EngineError::Other(e.to_string()))?;
        let mut choice = agents
            .voice_agent(&lease.cancel)
            .await
            .map_err(|e| EngineError::Other(e.to_string()))?;
        // The default is a configured chat, never a display-name lookup.
        if choice.target.is_none() {
            if let Some(chat_id) = self.foreman_status().settings.chat_id {
                let assignment = self
                    .workspace()
                    .and_then(|w| w.chat(&chat_id).ok().flatten())
                    .and_then(|c| c.config)
                    .and_then(|c| {
                        c.model_options
                            .get(super::custom_agents::ASSIGNMENT)
                            .and_then(|v| v.as_str())
                            .map(str::to_owned)
                    });
                if let Some(id) = assignment {
                    let a = agents
                        .assignment(&id, &lease.cancel)
                        .await
                        .map_err(|e| EngineError::Other(e.to_string()))?;
                    if !a.archived && a.chat == chat_id {
                        choice.target = Some(VoiceTarget {
                            agent_id: a.spec.agent,
                            chat_id: a.chat,
                            host_id: a.spec.host,
                        });
                    }
                }
            }
        }
        if let Some(target) = choice.target.as_ref() {
            let mut after = None;
            let mut found = false;
            let mut count = 0;
            loop {
                let page = agents
                    .assignments(Some(&target.host_id), after.as_deref(), &lease.cancel)
                    .await
                    .map_err(|e| EngineError::Other(e.to_string()))?;
                count += page.data.len();
                found |= page.data.iter().any(|a| {
                    !a.archived
                        && a.spec.agent == target.agent_id
                        && a.chat == target.chat_id
                        && a.spec.host == target.host_id
                });
                after = page.next;
                if found || after.is_none() || count >= 256 {
                    break;
                }
            }
            let definition = agents
                .get(&target.agent_id, None, &lease.cancel)
                .await
                .map_err(|e| EngineError::Other(e.to_string()))?;
            if !target.validate() || !found || definition.deleted {
                choice.target = None;
            }
        }
        if !lease.is_current() {
            return Err(EngineError::Other(
                "Your account changed. Start a new call.".into(),
            ));
        }
        Ok(choice)
    }
}
