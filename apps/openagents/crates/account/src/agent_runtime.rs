//! A deliberate runtime choice for an existing saved agent, shared by shells.
use crate::Lease;
use openagents::agents::{AgentDefinition, AgentSpec};

/// Change only execution settings. Instructions, skills, access, and autonomy
/// remain the same, so existing assignments still refer to the same agent.
pub fn select(spec: &mut AgentSpec, runtime: &str) -> Result<(), String> {
    spec.select_runtime(runtime).map_err(str::to_owned)
}

/// Save with the latest revision, refusing concurrent edits rather than
/// replacing instructions somebody else changed. The host applies the choice
/// when preparing the agent's next request.
pub async fn switch(lease: &Lease, agent: &str, runtime: &str) -> Result<AgentDefinition, String> {
    let agents = lease
        .client()
        .agents(lease.session())
        .map_err(|_| "Connecting to your account…")?;
    let mut definition = agents
        .get(agent, None, &lease.cancel)
        .await
        .map_err(|_| "Couldn't load this agent. Try again.")?;
    if definition.deleted || !lease.is_current() {
        return Err("This agent is no longer available.".into());
    }
    select(&mut definition.spec, runtime)?;
    let request = agents
        .update_request(definition.revision, definition.spec)
        .map_err(|_| "Couldn't change this agent. Try again.")?;
    let saved = agents
        .update(agent, &request, &lease.cancel)
        .await
        .map_err(|_| "Couldn't change this agent. Refresh it and try again.")?;
    if !lease.is_current() {
        return Err("Your account changed. Open this agent again.".into());
    }
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changing_runtime_keeps_identity_and_powers_in_both_directions() {
        let mut spec: AgentSpec = serde_json::from_value(serde_json::json!({
            "name":"Foreman", "description":"Coordinate work", "instructions":"Skills and instructions",
            "runtime":"claude-code", "model":"opus", "access":"host_default",
            "autonomy":{"check_ins":true,"authority":true}
        })).unwrap();
        let original = spec.clone();
        for runtime in ["codex", "claude-code"] {
            select(&mut spec, runtime).unwrap();
            assert!(spec.same_identity(&original));
            assert_eq!(spec.autonomy, original.autonomy);
            assert_eq!(spec.model, None);
            assert_eq!(spec.runtime, runtime);
        }
        assert!(select(&mut spec, "unknown").is_err());
    }
}
