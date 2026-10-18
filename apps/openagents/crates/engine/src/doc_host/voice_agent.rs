//! Resolve the account voice choice without using display names.
use super::*;
impl DocHost {
    /// The account's voice choice; the caller must re-read it for each spoken request.
    pub async fn voice_agent(
        &self,
    ) -> Result<openagents_account::openagents::agents::voice::VoiceAgent, EngineError> {
        let lease = self
            .openagents_lease()
            .ok_or_else(|| EngineError::Other("Connecting to your account…".into()))?;
        let choice = lease
            .client()
            .agents(lease.session())
            .map_err(|e| EngineError::Other(e.to_string()))?
            .voice_agent(&lease.cancel)
            .await
            .map_err(|e| EngineError::Other(e.to_string()))?;
        if !lease.is_current() {
            return Err(EngineError::Other(
                "Your account changed. Start a new call.".into(),
            ));
        }
        Ok(choice)
    }
}
