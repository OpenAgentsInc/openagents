//! OpenAgents (#11966): a saved agent's runtime can change between turns. The
//! next run then starts a fresh provider session that is given the
//! conversation so far, instead of resuming the previous runtime's session.
use super::{HarnessSessionRef, SessionsEngine, lock};
use crate::EngineError;
use openagents_doc::SessionDoc;

const CHANGED: &str = "openagentsRuntimeChanged";

impl SessionsEngine {
    /// Forget the chat's provider session (the tombstone `resume_for`
    /// honours) and mark its document so the fresh session receives the
    /// conversation as context.
    pub(crate) fn start_fresh_runtime(
        &self,
        chat_id: &str,
        doc: &SessionDoc,
    ) -> Result<(), EngineError> {
        lock(&self.inner.harness_sessions).insert(
            chat_id.to_owned(),
            HarnessSessionRef {
                session_id: String::new(),
                cwd: String::new(),
            },
        );
        if let Some(ws) = self.inner.workspace() {
            ws.set_chat_harness_session(chat_id, "", "");
        }
        doc.doc()
            .get_map("meta")
            .insert(CHANGED, true)
            .map_err(|e| EngineError::Other(e.to_string()))?;
        doc.doc().commit();
        Ok(())
    }
}

/// Whether a fresh provider session in this chat must receive the earlier
/// conversation because the agent's runtime changed.
pub(super) fn carries_history(doc: &SessionDoc) -> bool {
    doc.doc().get_map("meta").get(CHANGED).is_some()
}
