//! Saved agents start each runtime change with a bounded handoff (#12346).
use super::SessionsEngine;
use crate::EngineError;
use openagents_doc::SessionDoc;
use openagents_proto::HarnessId;

const CHANGED: &str = "openagentsRuntimeChanged";

impl SessionsEngine {
    /// Keep earlier provider sessions as history and start a fresh one.
    /// The workspace tombstone survives a restart; old slots remain readable.
    pub(crate) fn switch_runtime(
        &self,
        chat_id: &str,
        harness: HarnessId,
        doc: &SessionDoc,
    ) -> Result<(), EngineError> {
        self.inner.set_chat_session(chat_id, harness, "", "");
        mark_changed(doc)
    }

    /// Start `harness`'s next run in this chat in a fresh provider session
    /// that receives the conversation, the same path a runtime change takes
    /// when the runtime has no session of its own (#12207). Returns how many
    /// sessions it retired.
    pub(crate) fn start_fresh_runtime(
        &self,
        chat_id: &str,
        harness: HarnessId,
        doc: &SessionDoc,
    ) -> usize {
        self.inner.retire_runtime_sessions(chat_id, harness, doc)
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
