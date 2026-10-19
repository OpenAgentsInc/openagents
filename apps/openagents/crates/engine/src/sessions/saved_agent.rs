//! OpenAgents (#11966): a saved agent's runtime can change between turns. The
//! next run resumes the new runtime's own last session in this chat (#12205),
//! or, when it has none, starts a fresh provider session that is given the
//! conversation so far. It never resumes the previous runtime's session.
use super::SessionsEngine;
use crate::EngineError;
use openagents_doc::SessionDoc;
use openagents_proto::HarnessId;

const CHANGED: &str = "openagentsRuntimeChanged";

impl SessionsEngine {
    /// Point the chat at `harness`'s own last session (the tombstone
    /// `resume_for` honours when it has none) and mark its document so a
    /// fresh session receives the conversation as context.
    pub(crate) fn switch_runtime(
        &self,
        chat_id: &str,
        harness: HarnessId,
        doc: &SessionDoc,
    ) -> Result<(), EngineError> {
        let (session, cwd) = self
            .inner
            .runtime_session(chat_id, harness, None)
            .unwrap_or_default();
        self.inner
            .set_chat_session(chat_id, harness, &session, &cwd);
        mark_changed(doc)
    }
}

/// Mark the chat so a fresh provider session receives the conversation.
pub(super) fn mark_changed(doc: &SessionDoc) -> Result<(), EngineError> {
    doc.doc()
        .get_map("meta")
        .insert(CHANGED, true)
        .map_err(|e| EngineError::Other(e.to_string()))?;
    doc.doc().commit();
    Ok(())
}

/// Whether a fresh provider session in this chat must receive the earlier
/// conversation because the agent's runtime changed.
pub(super) fn carries_history(doc: &SessionDoc) -> bool {
    doc.doc().get_map("meta").get(CHANGED).is_some()
}
