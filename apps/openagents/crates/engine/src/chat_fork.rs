//! Fork chat (#11963): a new standalone chat with the source's history up to
//! its latest completed response, listed in Sessions like any other chat. It
//! keeps the source's project, folder, harness and model, and its transcript
//! starts below a "Forked from <title>" seam that links back. The source is
//! only read. The fork's first run continues from the copied history: the
//! harness forks the source's own session where it can ([`native_fork`]),
//! else the engine seeds the history into the first prompt.
use crate::doc_host::DocHost;
use crate::workspace_host::WorkspaceHost;
use openagents_doc::native_fork::{self, NativeFork};
use openagents_proto::{Chat, HarnessId};
use openagents_rpc::RpcError;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ForkParams {
    pub chat_id: String,
    pub source_chat_id: String,
}

/// Harnesses whose own sessions can be forked natively.
fn forks_natively(harness: HarnessId) -> bool {
    matches!(harness, HarnessId::ClaudeCode | HarnessId::Codex)
}

pub(crate) fn fork(
    workspace: &WorkspaceHost,
    doc_host: &DocHost,
    p: ForkParams,
) -> Result<Chat, RpcError> {
    let failed = |e: crate::EngineError| RpcError::Failed(e.to_string());
    let doc_failed = |e: openagents_doc::DocError| RpcError::Failed(e.to_string());
    let source = workspace
        .chat(&p.source_chat_id)
        .map_err(failed)?
        .ok_or_else(|| RpcError::Failed("This chat no longer exists".into()))?;
    if source.device_id != doc_host.device_id() {
        return Err(RpcError::Failed(
            "Fork this chat on the computer it runs on".into(),
        ));
    }
    let marker_id = format!("fork:{}", p.chat_id);
    if let Some(existing) = workspace.chat(&p.chat_id).map_err(failed)? {
        // A retried request returns the fork it already made; any other
        // chat with this id is never overwritten.
        let entries = doc_host
            .open(&existing.id)
            .map_err(failed)?
            .doc()
            .read_entries()
            .map_err(doc_failed)?;
        return if entries.iter().any(|entry| entry.id == marker_id) {
            Ok(existing)
        } else {
            Err(RpcError::Failed("Chat id already exists".into()))
        };
    }
    let source_doc = doc_host.open(&p.source_chat_id).map_err(failed)?;
    let entries = source_doc.doc().read_entries().map_err(doc_failed)?;
    let boundary = entries
        .iter()
        .rposition(|entry| {
            entry.role == openagents_doc::MessageRole::Assistant
                && entry.status == Some(openagents_doc::MessageStatus::Complete)
        })
        .ok_or_else(|| RpcError::Failed("Wait for a reply before forking this chat".into()))?;
    let source_title = source
        .title
        .clone()
        .or_else(|| source.last_message_preview.clone())
        .unwrap_or_else(|| "New session".into());
    let now = chrono::Utc::now();
    let mut chat = source.clone();
    chat.id = p.chat_id;
    chat.parent_chat_id = None;
    chat.links = Default::default();
    chat.title = Some(format!("Fork of {source_title}"));
    chat.archived = false;
    chat.created_at = now;
    // Listed at the top of Sessions, already seen.
    chat.last_message_at = Some(now);
    chat.last_seen_at = Some(now);
    chat.harness_session_id = None;
    chat.harness_session_cwd = None;
    chat.room_gen = Some(2);
    // Persist history before publishing the registry row so a crash cannot
    // leave a discoverable empty fork.
    let target = doc_host.open(&chat.id).map_err(failed)?;
    let existing = target.doc().read_entries().map_err(doc_failed)?;
    for entry in entries[..=boundary]
        .iter()
        .filter(|entry| !existing.iter().any(|e| e.id == entry.id))
    {
        let mut entry = entry.clone();
        // Historical approvals belong to the source's run; they must never
        // block or send answers from the fork's composer.
        for part in &mut entry.parts {
            if let openagents_doc::MessagePart::Input { resolved, .. } = part {
                *resolved = true;
            }
        }
        target.doc().push_message(&entry).map_err(doc_failed)?;
    }
    // The seam: everything above came from the source. Its own entry
    // (system role, complete) so the copied history and the fork's first
    // turn never share a row.
    if !existing.iter().any(|e| e.id == marker_id) {
        target
            .doc()
            .push_message(&openagents_doc::SessionMessageEntry {
                duration_ms: None,
                id: marker_id.clone(),
                role: openagents_doc::MessageRole::System,
                parts: vec![openagents_doc::MessagePart::Fork {
                    id: marker_id,
                    source_chat_id: source.id.clone(),
                    source_title,
                }],
                created_at: now.timestamp_millis(),
                device_id: doc_host.device_id().to_owned(),
                status: Some(openagents_doc::MessageStatus::Complete),
                continuation_of: None,
            })
            .map_err(doc_failed)?;
    }
    if let (Some(config), Some(session)) = (
        source.config.as_ref(),
        source
            .harness_session_id
            .as_deref()
            .filter(|session| !session.is_empty()),
    ) && forks_natively(config.harness)
    {
        native_fork::set(
            target.doc(),
            &NativeFork {
                harness: config.harness,
                session: session.to_owned(),
                cwd: source
                    .harness_session_cwd
                    .clone()
                    .or_else(|| source.cwd.clone())
                    .unwrap_or_default(),
            },
        )
        .map_err(doc_failed)?;
    }
    doc_host.persist_fork(&target).map_err(failed)?;
    workspace.import_chat_row(&chat).map_err(failed)?;
    Ok(chat)
}

/// The source session the fork's first run should fork natively: only while
/// the fork has no session of its own, for the same harness and folder, and
/// not on a retry after a native fork failed to start.
pub(crate) fn native_source(
    doc: &openagents_doc::SessionDoc,
    harness: HarnessId,
    cwd: &str,
    startup_retry: bool,
) -> Option<String> {
    if startup_retry {
        return None;
    }
    let fork = native_fork::get(doc)?;
    (fork.harness == harness && forks_natively(harness) && fork.cwd == cwd).then_some(fork.session)
}
