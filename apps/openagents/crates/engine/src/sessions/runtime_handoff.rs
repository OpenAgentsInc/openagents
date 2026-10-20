//! Bounded context for a saved agent moving between provider sessions.
//! Complete messages and state stay in a local archive; excerpts never replace them.
use crate::EngineError;
use openagents_account::openagents::agents::AgentSpec;
use openagents_doc::{MessagePart, SessionDoc, SessionMessageEntry};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path};

const KEY: &str = "openagentsRuntimeHandoff";
/// UTF-8 bytes also bound the number of byte-fallback tokens. Leave room for
/// the current request and provider instructions in a 200k-token context.
pub(crate) const MAX_CONTEXT_BYTES: usize = 128 * 1024;
const SNAPSHOT_BYTES: usize = 96 * 1024;
const RECENT_BYTES: usize = 20 * 1024;
const RECENT_TURNS: usize = 12;

#[derive(Serialize, Deserialize)]
struct Handoff {
    id: String,
    text: String,
    through: Option<String>,
}

fn prefix(s: &str, limit: usize) -> &str {
    let mut end = s.len().min(limit);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn excerpt(s: &str, limit: usize) -> String {
    if s.len() <= limit {
        return s.into();
    }
    let head = prefix(s, limit.saturating_sub(64) / 2);
    let mut start = s.len().saturating_sub(limit.saturating_sub(64) / 2);
    while !s.is_char_boundary(start) {
        start += 1;
    }
    format!(
        "{head}\n[excerpt; read the archive for the full text]\n{}",
        &s[start..]
    )
}

fn message(entry: &SessionMessageEntry, limit: usize) -> String {
    let mut text = String::new();
    for part in &entry.parts {
        if text.len() >= limit {
            break;
        }
        let room = limit - text.len();
        match part {
            MessagePart::Text { text: part, .. } => text.push_str(&excerpt(part, room)),
            MessagePart::Tool { call, output, .. } => {
                text.push_str(&excerpt(
                    &format!("Tool: {call:?}\n{}", output.as_deref().unwrap_or_default()),
                    room,
                ));
            }
            _ => {}
        }
        text.push('\n');
    }
    format!(
        "Message {} ({:?}, {}):\n{}\n",
        entry.id, entry.role, entry.created_at, text
    )
}

/// The newest whole messages that fit, in chronological order. A large
/// individual message is represented by an explicitly labeled excerpt.
fn recent(entries: &[SessionMessageEntry], budget: usize) -> String {
    let mut rows = Vec::new();
    let mut remaining = budget;
    for entry in entries.iter().rev().take(RECENT_TURNS) {
        let row = message(entry, remaining.min(RECENT_BYTES / 2).saturating_sub(256));
        if row.len() > remaining || remaining < 512 {
            break;
        }
        remaining -= row.len();
        rows.push(row);
    }
    rows.reverse();
    rows.concat()
}

/// Capture before changing chat configuration. Failure leaves its runtime
/// alone. Skills are already included in the definition's instructions.
pub(crate) fn capture(
    doc: &SessionDoc,
    root: &Path,
    chat: &str,
    spec: &AgentSpec,
    state: &serde_json::Value,
    decision_log: Option<&Path>,
) -> Result<(), EngineError> {
    let entries = doc.read_entries()?;
    let id = crate::new_id();
    let dir = root.join("runtime-handoffs").join(&id);
    fs::create_dir_all(&dir)?;
    let dir = fs::canonicalize(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    }
    let transcript = dir.join("conversation.jsonl");
    let mut file = std::io::BufWriter::new(fs::File::create(&transcript)?);
    for entry in &entries {
        serde_json::to_writer(&mut file, entry).map_err(|e| EngineError::Other(e.to_string()))?;
        file.write_all(b"\n")?;
    }
    file.flush()?;
    file.get_ref().sync_all()?;
    let state_path = dir.join("state.json");
    fs::write(
        &state_path,
        serde_json::to_vec_pretty(&serde_json::json!({"agent":spec, "state":state}))
            .map_err(|e| EngineError::Other(e.to_string()))?,
    )?;
    let log = dir.join("decisions.jsonl");
    if let Some(source) = decision_log.filter(|p| p.exists()) {
        fs::copy(source, &log)?;
    }
    let mut text = format!(
        "Continue as {}, the same saved agent in chat {chat}. The runtime changed; earlier provider sessions are history, not current context.\n\
         Instructions and skills:\n{}\n\
         Full conversation (JSONL, message IDs and tool results): {}\n\
         Full agent definition, saved work, grants and check-in state: {}\n\
         Decision log snapshot: {}\n\
         Read these files when excerpts do not answer a question. Do not invent missing details.\n\
         Saved work and check-in history (excerpts if large):\n{}\n",
        spec.name,
        spec.instructions,
        transcript.display(),
        state_path.display(),
        if log.exists() {
            log.display().to_string()
        } else {
            "No decision log has been saved.".into()
        },
        excerpt(
            &serde_json::to_string_pretty(state).unwrap_or_default(),
            12 * 1024
        ),
    );
    let recent = recent(
        &entries,
        RECENT_BYTES.min(SNAPSHOT_BYTES.saturating_sub(text.len() + 512)),
    );
    let older = entries.len().saturating_sub(RECENT_TURNS);
    text.push_str(
        "Earlier conversation summary (extractive; use the archive for full messages):\n",
    );
    // Sample the full history, including its beginning, rather than dropping
    // everything before a recent tail of a multi-megabyte chat.
    let room = SNAPSHOT_BYTES.saturating_sub(text.len() + recent.len() + 256);
    let count = older.min((room / 512).min(64));
    for i in 0..count {
        let index = if count > 1 {
            i * (older - 1) / (count - 1)
        } else {
            0
        };
        let row = message(&entries[index], 256);
        if text.len() + row.len() + recent.len() + 128 <= SNAPSHOT_BYTES {
            text.push_str(&row);
        }
    }
    text.push_str("\nRecent conversation (large messages are marked as excerpts):\n");
    text.push_str(&recent);
    if text.len() > SNAPSHOT_BYTES {
        return Err(EngineError::Other("This agent's instructions are too long to carry to another runtime. Shorten them and try again.".into()));
    }
    fs::write(dir.join("handoff.txt"), &text)?;
    let saved = serde_json::to_string(&Handoff {
        id,
        text,
        through: entries.last().map(|e| e.id.clone()),
    })
    .map_err(|e| EngineError::Other(e.to_string()))?;
    doc.doc()
        .get_map("meta")
        .insert("forkHistorySession", "")
        .map_err(|e| EngineError::Other(e.to_string()))?;
    doc.doc()
        .get_map("meta")
        .insert(KEY, saved)
        .map_err(|e| EngineError::Other(e.to_string()))?;
    doc.doc().commit();
    Ok(())
}

fn load(doc: &SessionDoc) -> Option<Handoff> {
    match doc.doc().get_map("meta").get(KEY)? {
        loro::ValueOrContainer::Value(loro::LoroValue::String(s)) => serde_json::from_str(&s).ok(),
        _ => None,
    }
}

pub(super) fn generation(doc: &SessionDoc) -> Option<String> {
    load(doc).map(|h| h.id)
}

pub(super) fn has_handoff(doc: &SessionDoc) -> bool {
    doc.doc().get_map("meta").get(KEY).is_some()
}

pub(super) fn prompt(doc: &SessionDoc, current: Option<&str>, request: &str) -> Option<String> {
    let saved = load(doc)?;
    // Read scalar IDs before materializing the tail. An old, large tool
    // result already covered by the handoff must not be loaded again.
    use loro::{Container, ValueOrContainer};
    let messages = doc.doc().get_list("messages");
    let mut parts = 0;
    for index in (0..messages.len()).rev() {
        let Some(ValueOrContainer::Container(Container::Map(entry))) = messages.get(index) else {
            continue;
        };
        if let Some(ValueOrContainer::Value(loro::LoroValue::String(id))) = entry.get("id")
            && saved.through.as_deref() == Some(id.as_str())
        {
            break;
        }
        parts += match entry.get("parts") {
            Some(ValueOrContainer::Container(Container::List(list))) => list.len().max(1),
            _ => 1,
        };
        if parts >= 128 {
            break;
        }
    }
    let entries = if parts == 0 {
        Vec::new()
    } else {
        doc.read_opening_tail(parts.min(128)).ok()?
    };
    let tail: Vec<_> = entries
        .into_iter()
        .filter(|e| Some(e.id.as_str()) != current)
        .collect();
    let tail = recent(
        &tail,
        MAX_CONTEXT_BYTES.saturating_sub(saved.text.len() + 256),
    );
    Some(format!(
        "<runtime_handoff>\n{}\nSince the handoff:\n{}\n</runtime_handoff>\n\n{}",
        saved.text, tail, request
    ))
}

/// Only the session that received this handoff may resume. The marker lives
/// in the chat document, so a restart or a late old-session report cannot
/// resurrect a provider session from before the switch.
pub(super) fn can_resume(doc: &SessionDoc, session: &str) -> bool {
    !has_handoff(doc) || doc.fork_history_session().as_deref() == Some(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_doc::{MessageRole, MessageStatus};
    use openagents_proto::{HarnessId, SandboxLevel};

    fn spec() -> AgentSpec {
        serde_json::from_value(serde_json::json!({
            "name":"Foreman", "description":"Coordinate work", "instructions":"Keep the watch list. Skill: read issue claims first.",
            "runtime":"claude-code", "model":null, "access":"host_default"
        })).unwrap()
    }
    fn push(doc: &SessionDoc, id: &str, text: &str) {
        doc.push_message(&SessionMessageEntry {
            id: id.into(),
            role: MessageRole::User,
            parts: vec![MessagePart::Text {
                id: format!("{id}-text"),
                text: text.into(),
            }],
            created_at: 1,
            device_id: "test-device".into(),
            status: Some(MessageStatus::Complete),
            continuation_of: None,
            duration_ms: None,
        })
        .unwrap();
    }
    fn state() -> serde_json::Value {
        serde_json::json!({
            "working": {"items":[{"issue":12341,"chat":"worker-one","status":"watching","note":"Keep the native route behind parity checks."}]},
            "recent_decisions":[{"kind":"decision","text":"Do not deploy until the test account check passes."}],
            "check_ins":{"every_minutes":10,"grants":["chat.message"]}
        })
    }

    #[test]
    fn huge_unicode_chat_is_bounded_and_complete_records_remain_readable() {
        let temp = tempfile::tempdir().unwrap();
        let doc = SessionDoc::init("huge").unwrap();
        // 83 MiB of UTF-8 text, including a single message much larger than
        // a provider context. Neither the archive nor its tool data is cut.
        let text = "界".repeat(29 * 1024 * 1024);
        push(&doc, "huge-message", &text);
        push(
            &doc,
            "latest",
            "Watch #12341 in worktree /work/parity; worker-one owns it.",
        );
        capture(&doc, temp.path(), "huge", &spec(), &state(), None).unwrap();
        let saved = load(&doc).unwrap();
        assert!(saved.text.len() <= SNAPSHOT_BYTES);
        assert!(saved.text.contains("#12341"));
        assert!(saved.text.contains("read issue claims first"));
        assert!(saved.text.contains("excerpts"));
        let dir = fs::read_dir(temp.path().join("runtime-handoffs"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let archive = fs::read_to_string(dir.join("conversation.jsonl")).unwrap();
        let first: SessionMessageEntry =
            serde_json::from_str(archive.lines().next().unwrap()).unwrap();
        assert!(
            matches!(&first.parts[0], MessagePart::Text {text: archived, ..} if archived == &text)
        );
        let request = prompt(&doc, None, "What is worker-one doing?").unwrap();
        assert!(request.len() <= MAX_CONTEXT_BYTES + "What is worker-one doing?".len());
    }

    #[test]
    fn maximum_instructions_and_large_state_still_fit_without_cutting_instructions() {
        let temp = tempfile::tempdir().unwrap();
        let doc = SessionDoc::init("maximum").unwrap();
        let mut spec = spec();
        spec.instructions = "🦀".repeat(16 * 1024);
        for i in 0..20 {
            push(&doc, &i.to_string(), &"recent text ".repeat(4000));
        }
        let state = serde_json::json!({"working": "watch #12341 ".repeat(10000)});
        capture(&doc, temp.path(), "maximum", &spec, &state, None).unwrap();
        let saved = load(&doc).unwrap();
        assert!(saved.text.contains(&spec.instructions));
        assert!(saved.text.len() <= SNAPSHOT_BYTES);
    }

    #[tokio::test]
    async fn both_directions_carry_decisions_and_never_resume_stale_sessions_even_after_restart() {
        let temp = tempfile::tempdir().unwrap();
        let core = crate::EngineCore::assemble_with_profile(
            crate::EngineProfile::local(temp.path()).unwrap(),
            std::sync::Arc::new(crate::HarnessRegistry::new()),
            HarnessId::ClaudeCode,
            None,
        )
        .unwrap();
        core.workspace
            .create_chat("agent", None, Some(&core.device_id), None, None)
            .unwrap();
        let handle = core.doc_host.open("agent").unwrap();
        let doc = handle.writer();
        let log = temp.path().join("original-decisions.jsonl");
        fs::write(
            &log,
            "{\"kind\":\"decision\",\"text\":\"Keep issue 12341 with worker-one\"}\n",
        )
        .unwrap();
        let mut spec = spec();
        let inner = &core.sessions.inner;
        inner.report_harness_session("agent", HarnessId::ClaudeCode, "old-claude", "/work");
        inner.report_harness_session("agent", HarnessId::Codex, "old-codex", "/work");
        for (target, word, session) in [
            (HarnessId::Codex, "codex", "new-codex"),
            (HarnessId::ClaudeCode, "claude-code", "new-claude"),
        ] {
            push(
                &doc,
                word,
                "Worker-one owns #12341; keep watching its tokenizer parity work.",
            );
            spec.runtime = word.into();
            capture(&doc, temp.path(), "agent", &spec, &state(), Some(&log)).unwrap();
            core.workspace
                .set_chat_config(
                    "agent",
                    &openagents_proto::ChatConfig {
                        harness: target,
                        model: None,
                        reasoning: None,
                        model_options: Default::default(),
                        sandbox: SandboxLevel::WorkspaceWrite,
                    },
                )
                .unwrap();
            core.sessions.switch_runtime("agent", target, &doc).unwrap();
            assert_eq!(inner.resume_for("agent", "/work", target), None);
            // A fresh process loses the in-memory tombstone but keeps the
            // workspace row and the handoff's document marker.
            super::super::lock(&inner.harness_sessions).clear();
            assert_eq!(inner.resume_for("agent", "/work", target), None);
            let delivered = inner
                .fork_history_prompt(
                    &doc,
                    target,
                    "What work am I watching?",
                    None,
                    super::super::ProviderSession::Fresh,
                )
                .unwrap();
            let warm = inner
                .fork_history_prompt(
                    &doc,
                    target,
                    "What work am I watching?",
                    None,
                    super::super::ProviderSession::Continued(Some("fresh-prewarmed-session")),
                )
                .unwrap();
            assert!(warm.contains("worker-one"));
            assert!(warm.contains("read issue claims first"));
            assert!(delivered.contains("worker-one"));
            assert!(delivered.contains("Do not deploy until the test account check passes."));
            assert!(delivered.contains("read issue claims first"));
            assert!(!can_resume(&doc, "old-codex"));
            assert!(!can_resume(&doc, "old-claude"));
            inner.report_harness_session("agent", target, session, "/work");
            super::super::note_fork_history_session(
                &doc,
                true,
                session,
                generation(&doc).as_deref(),
            );
            assert_eq!(
                inner.resume_for("agent", "/work", target).as_deref(),
                Some(session)
            );
            assert!(
                inner
                    .fork_history_prompt(
                        &doc,
                        target,
                        "next",
                        None,
                        super::super::ProviderSession::Continued(Some(session))
                    )
                    .is_none()
            );
            push(
                &doc,
                session,
                "Parity is still blocked. Continue watching worker-one.",
            );
            // Metadata and the resume guard survive serialized doc recovery.
            let snapshot = doc.doc().export(loro::ExportMode::Snapshot).unwrap();
            let recovered = loro::LoroDoc::new();
            recovered.import(&snapshot).unwrap();
            let recovered = SessionDoc::from_doc(recovered);
            assert!(can_resume(&recovered, session));
            assert!(
                prompt(&recovered, None, "Continue")
                    .unwrap()
                    .contains("Parity is still blocked")
            );
        }
        let dirs: Vec<_> = fs::read_dir(temp.path().join("runtime-handoffs"))
            .unwrap()
            .collect();
        assert_eq!(dirs.len(), 2);
        for dir in dirs {
            let path = dir.unwrap().path();
            assert_eq!(
                fs::read(path.join("decisions.jsonl")).unwrap(),
                fs::read(&log).unwrap()
            );
            let saved: serde_json::Value =
                serde_json::from_slice(&fs::read(path.join("state.json")).unwrap()).unwrap();
            assert_eq!(saved["state"], state());
        }
    }

    #[test]
    fn failed_capture_leaves_the_previous_handoff_and_session_marker_alone() {
        let temp = tempfile::tempdir().unwrap();
        let doc = SessionDoc::init("a").unwrap();
        capture(&doc, temp.path(), "a", &spec(), &state(), None).unwrap();
        doc.set_fork_history_session("current-session").unwrap();
        let old_generation = generation(&doc);
        capture(&doc, temp.path(), "a", &spec(), &state(), None).unwrap();
        super::super::note_fork_history_session(
            &doc,
            true,
            "late-old-session",
            old_generation.as_deref(),
        );
        assert!(!can_resume(&doc, "late-old-session"));
        doc.set_fork_history_session("current-session").unwrap();
        let previous = load(&doc).unwrap().text;
        let file = temp.path().join("not-a-directory");
        fs::write(&file, "occupied").unwrap();
        assert!(capture(&doc, &file, "a", &spec(), &state(), None).is_err());
        assert_eq!(load(&doc).unwrap().text, previous);
        assert!(can_resume(&doc, "current-session"));
    }
}
