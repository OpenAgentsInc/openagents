//! Any saved agent can use the account's separately granted owner notifications.
use super::*;
use openagents_account::openagents::{
    agents::assignments::Assignment, owner_notifications::OwnerNote,
};

fn error(message: &str) -> EngineError {
    EngineError::Other(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_agent_can_only_notify_from_its_own_assignment() {
        let mut a:Assignment=serde_json::from_value(serde_json::json!({
            "id":"a".repeat(64),"revision":1,
            "spec":{"agent":"b".repeat(64),"agent_revision":1,"host":"host","project":null,"task":"Check the work"},
            "definition":{"id":"b".repeat(64),"revision":1,"deleted":false,"spec":{"name":"Any helper","description":"","instructions":"Check the work","runtime":"codex","model":null,"access":"read_only","fallbacks":[]}},
            "chat":"chat","state":"queued","commands":[],"result":null,"archived":false,"active_command":null,"after_seq":0
        })).unwrap();
        let agent = "b".repeat(64);
        assert!(check_binding(&a, &agent, "chat", "host").is_ok());
        assert!(check_binding(&a, &"c".repeat(64), "chat", "host").is_err());
        assert!(check_binding(&a, &agent, "other-chat", "host").is_err());
        assert!(check_binding(&a, &agent, "chat", "other-host").is_err());
        a.archived = true;
        assert!(check_binding(&a, &agent, "chat", "host").is_err());
        a.archived = false;
        a.definition.deleted = true;
        assert!(check_binding(&a, &agent, "chat", "host").is_err());
    }
}

fn check_binding(a: &Assignment, agent: &str, chat: &str, device: &str) -> Result<(), EngineError> {
    if a.spec.agent != agent
        || a.definition.id != agent
        || a.chat != chat
        || a.spec.host != device
        || a.archived
        || a.definition.deleted
    {
        return Err(error("Choose this agent's own chat."));
    }
    Ok(())
}

impl DocHost {
    /// Ask the owner to open this agent's chat. The account must separately grant
    /// this agent permission. Use one stable 64-character request ID per alarm,
    /// question, or final stall escalation, and keep it across retries.
    pub async fn notify_owner(
        &self,
        agent_id: &str,
        request_id: &str,
        chat_id: &str,
        title: &str,
        body: &str,
    ) -> Result<(), EngineError> {
        let note = OwnerNote {
            agent: agent_id.into(),
            chat: chat_id.into(),
            title: title.into(),
            body: body.into(),
        };
        note.validate().map_err(EngineError::AgentService)?;
        let lease = self
            .openagents_lease()
            .ok_or_else(|| error("Connecting to your account…"))?;
        let a = self.owner_notification_assignment(chat_id, &lease).await?;
        check_binding(&a, agent_id, chat_id, &self.inner.config.device_id)?;
        lease
            .client()
            .notify_owner(lease.session(), request_id, note, &lease.cancel)
            .await
            .map_err(EngineError::AgentService)
    }

    /// RPC/tool entry point: derive the agent ID from the calling chat's assignment.
    pub async fn notify_owner_from_chat(
        &self,
        chat_id: &str,
        request_id: &str,
        title: &str,
        body: &str,
    ) -> Result<(), EngineError> {
        let lease = self
            .openagents_lease()
            .ok_or_else(|| error("Connecting to your account…"))?;
        let a = self.owner_notification_assignment(chat_id, &lease).await?;
        check_binding(&a, &a.spec.agent, chat_id, &self.inner.config.device_id)?;
        let note = OwnerNote {
            agent: a.spec.agent,
            chat: chat_id.into(),
            title: title.into(),
            body: body.into(),
        };
        lease
            .client()
            .notify_owner(lease.session(), request_id, note, &lease.cancel)
            .await
            .map_err(EngineError::AgentService)
    }
    async fn owner_notification_assignment(
        &self,
        chat_id: &str,
        lease: &openagents_account::Lease,
    ) -> Result<Assignment, EngineError> {
        let chat = self
            .workspace()
            .ok_or_else(|| error("This computer is not ready."))?
            .chat(chat_id)?
            .ok_or_else(|| error("This chat could not be found."))?;
        let assignment = chat
            .config
            .as_ref()
            .and_then(|c| {
                c.model_options
                    .get(openagents_proto::saved_agent::ASSIGNMENT)
            })
            .and_then(|v| v.as_str())
            .ok_or_else(|| error("Choose a saved agent's chat."))?;
        let agents = lease
            .client()
            .agents(lease.session())
            .map_err(EngineError::AgentService)?;
        let assignment = agents
            .assignment(assignment, &lease.cancel)
            .await
            .map_err(EngineError::AgentService)?;
        let latest = agents
            .get(&assignment.spec.agent, None, &lease.cancel)
            .await
            .map_err(EngineError::AgentService)?;
        if latest.deleted || !lease.is_current() {
            return Err(error("This agent is no longer available."));
        }
        Ok(assignment)
    }
}
