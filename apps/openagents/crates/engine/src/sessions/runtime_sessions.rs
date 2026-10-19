//! Each runtime's own provider session in a chat (#12205). A saved agent
//! can move between runtimes (Claude Code, Codex, ...). Every runtime keeps
//! its own last session: switching back resumes it, a report from a runtime
//! that is no longer the chat's never becomes the chat's session, and a
//! session its provider no longer has gives way to the runtime's last good
//! one, else a fresh session that receives the conversation.
use std::collections::BTreeMap;
use std::sync::Arc;

use openagents_doc::SessionDoc;
use openagents_proto::HarnessId;
use serde::{Deserialize, Serialize};

use super::{HarnessSessionRef, Inner, lock, same_dir, saved_agent};
use crate::doc_host::ChatDocHandle;

/// Session-doc meta key: `{harness: {session, cwd}}` as JSON.
const KEY: &str = "openagentsRuntimeSessions";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct Slot {
    session: String,
    cwd: String,
}

fn name(harness: HarnessId) -> String {
    serde_json::to_value(harness)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{harness:?}"))
}

fn slots(doc: &SessionDoc) -> BTreeMap<String, Slot> {
    match doc.doc().get_map("meta").get(KEY) {
        Some(loro::ValueOrContainer::Value(loro::LoroValue::String(s))) => {
            serde_json::from_str(&s).unwrap_or_default()
        }
        _ => BTreeMap::new(),
    }
}

fn save(doc: &SessionDoc, slots: &BTreeMap<String, Slot>) {
    let Ok(json) = serde_json::to_string(slots) else {
        return;
    };
    if doc.doc().get_map("meta").insert(KEY, json).is_ok() {
        doc.doc().commit();
    }
}

/// The last session `harness` reported in this chat.
fn slot(doc: &SessionDoc, harness: HarnessId) -> Option<Slot> {
    slots(doc)
        .remove(&name(harness))
        .filter(|s| !s.session.is_empty())
}

fn set_slot(doc: &SessionDoc, harness: HarnessId, session: &str, cwd: &str) {
    let mut all = slots(doc);
    let slot = Slot {
        session: session.to_owned(),
        cwd: cwd.to_owned(),
    };
    if all.get(&name(harness)) != Some(&slot) {
        all.insert(name(harness), slot);
        save(doc, &all);
    }
}

/// The runtime whose slot holds `session`.
fn slot_owner(doc: &SessionDoc, session: &str) -> Option<HarnessId> {
    slots(doc)
        .into_iter()
        .find(|(_, slot)| slot.session == session)
        .and_then(|(harness, _)| serde_json::from_value(serde_json::Value::String(harness)).ok())
}

fn forget_slot(doc: &SessionDoc, session: &str) {
    let mut all = slots(doc);
    let before = all.len();
    all.retain(|_, slot| slot.session != session);
    if all.len() != before {
        save(doc, &all);
    }
}

/// The provider says the session to resume does not exist (Claude Code's
/// `--resume` of an unknown id).
pub(super) fn missing_session(error: &str) -> bool {
    error.contains("No conversation found with session ID")
}

impl Inner {
    fn chat_doc(&self, chat_id: &str) -> Option<Arc<ChatDocHandle>> {
        self.doc_host()?.open(chat_id).ok()
    }

    /// Whether `harness` is the chat's runtime (true when the chat has no
    /// configuration to say otherwise).
    fn runs_on(&self, chat_id: &str, harness: HarnessId) -> bool {
        self.workspace()
            .and_then(|ws| ws.chat_config(chat_id))
            .is_none_or(|config| config.harness == harness)
    }

    /// A run of `harness` reported its provider session. It always becomes
    /// that runtime's own last session; it becomes the chat's session only
    /// while `harness` is still the chat's runtime, so a run that ends after
    /// a runtime switch never hands its id to the next runtime.
    pub(super) fn report_harness_session(
        &self,
        chat_id: &str,
        harness: HarnessId,
        session_id: &str,
        cwd: &str,
    ) {
        if session_id.is_empty() {
            return;
        }
        if let Some(handle) = self.chat_doc(chat_id) {
            let writer = handle.writer();
            set_slot(&writer, harness, session_id, cwd);
        }
        if !self.runs_on(chat_id, harness) {
            tracing::info!(
                chat = %chat_id,
                ?harness,
                "session report from a runtime the chat no longer runs: kept for that runtime only"
            );
            return;
        }
        self.remember_harness_session(chat_id, Some(harness), session_id, cwd);
    }

    /// Make `session_id` (empty = the "do not resume" tombstone) the chat's
    /// session for `harness`: live cache and workspace row.
    pub(super) fn set_chat_session(
        &self,
        chat_id: &str,
        harness: HarnessId,
        session_id: &str,
        cwd: &str,
    ) {
        lock(&self.harness_sessions).insert(
            chat_id.to_owned(),
            HarnessSessionRef {
                session_id: session_id.to_owned(),
                cwd: cwd.to_owned(),
                harness: Some(harness),
            },
        );
        if let Some(ws) = self.workspace() {
            ws.set_chat_harness_session(chat_id, session_id, cwd);
        }
    }

    pub(super) fn is_missing(&self, chat_id: &str, session_id: &str) -> bool {
        lock(&self.missing_sessions)
            .get(chat_id)
            .is_some_and(|ids| ids.contains(session_id))
    }

    /// `harness`'s own newest session in the chat that its provider still
    /// has, usable from `cwd` when given: its slot, else the newest session
    /// it started in the chat's journal (chats from before the slots).
    pub(super) fn runtime_session(
        &self,
        chat_id: &str,
        harness: HarnessId,
        cwd: Option<&str>,
    ) -> Option<(String, String)> {
        let usable = |session: &str, session_cwd: &str| {
            !session.is_empty()
                && !self.is_missing(chat_id, session)
                && cwd.is_none_or(|cwd| same_dir(session_cwd, cwd))
        };
        if let Some(slot) = self
            .chat_doc(chat_id)
            .and_then(|handle| slot(handle.doc(), harness))
            && usable(&slot.session, &slot.cwd)
        {
            return Some((slot.session, slot.cwd));
        }
        self.scan_journal_sessions(chat_id, |session, session_cwd, owner| {
            owner == Some(harness) && usable(session, session_cwd)
        })
        .map(|(session, session_cwd, _)| (session, session_cwd))
    }

    /// The runtime that started `session_id` in this chat, when known.
    pub(super) fn session_owner(&self, chat_id: &str, session_id: &str) -> Option<HarnessId> {
        if let Some(owner) = self
            .chat_doc(chat_id)
            .and_then(|handle| slot_owner(handle.doc(), session_id))
        {
            return Some(owner);
        }
        self.scan_journal_sessions(chat_id, |session, _, owner| {
            owner.is_some() && session == session_id
        })
        .and_then(|(_, _, owner)| owner)
    }

    /// The provider no longer has `missing`: never resume it again, and point
    /// the chat at `harness`'s last good session, else a fresh session that
    /// receives the conversation.
    pub(super) fn recover_missing_session(
        &self,
        chat_id: &str,
        harness: HarnessId,
        missing: &str,
        cwd: &str,
        doc: &SessionDoc,
    ) {
        lock(&self.missing_sessions)
            .entry(chat_id.to_owned())
            .or_default()
            .insert(missing.to_owned());
        forget_slot(doc, missing);
        match self.runtime_session(chat_id, harness, Some(cwd)) {
            Some((session, session_cwd)) => {
                tracing::warn!(chat = %chat_id, missing, session, "provider session missing: resuming the runtime's last good one");
                self.set_chat_session(chat_id, harness, &session, &session_cwd);
            }
            None => {
                tracing::warn!(chat = %chat_id, missing, "provider session missing: starting fresh with the conversation");
                self.set_chat_session(chat_id, harness, "", "");
                let _ = saved_agent::mark_changed(doc);
            }
        }
    }
}

#[cfg(test)]
pub(super) fn test_slot(doc: &SessionDoc, harness: HarnessId) -> Option<(String, String)> {
    slot(doc, harness).map(|s| (s.session, s.cwd))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::SessionsEngine;
    use openagents_proto::{AgentEvent, ChatConfig, DoneStatus, RunRequest, SandboxLevel};

    struct Fixture {
        _temp: tempfile::TempDir,
        core: crate::EngineCore,
    }

    fn fixture(registry: crate::HarnessRegistry) -> Fixture {
        let temp = tempfile::tempdir().unwrap();
        let core = crate::EngineCore::assemble_with_profile(
            crate::EngineProfile::local(temp.path()).unwrap(),
            Arc::new(registry),
            HarnessId::ClaudeCode,
            None,
        )
        .unwrap();
        core.workspace
            .create_chat("a", None, Some(&core.device_id), None, None)
            .unwrap();
        Fixture { _temp: temp, core }
    }

    impl Fixture {
        fn sessions(&self) -> &SessionsEngine {
            &self.core.sessions
        }
        fn runs_on(&self, harness: HarnessId) {
            let config = ChatConfig {
                harness,
                model: None,
                reasoning: None,
                model_options: Default::default(),
                sandbox: SandboxLevel::WorkspaceWrite,
            };
            self.core.workspace.set_chat_config("a", &config).unwrap();
        }
        /// What `apply_runtime` does on a change of runtime.
        fn switch(&self, harness: HarnessId) {
            self.runs_on(harness);
            let handle = self.core.doc_host.open("a").unwrap();
            self.sessions()
                .switch_runtime("a", harness, &handle.writer())
                .unwrap();
        }
        fn resume(&self, harness: HarnessId) -> Option<String> {
            self.sessions().inner.resume_for("a", "/tmp", harness)
        }
        fn started(&self, harness: HarnessId, session: &str) {
            let event = AgentEvent::SessionStarted {
                harness,
                model: String::new(),
                tools: vec![],
                cwd: "/tmp".into(),
                session_id: session.into(),
                assistant_message_id: String::new(),
            };
            self.sessions().inner.journal.append("a", &event).unwrap();
        }
    }

    #[tokio::test]
    async fn claude_codex_claude_keeps_each_runtime_session() {
        let f = fixture(crate::HarnessRegistry::new());
        f.runs_on(HarnessId::ClaudeCode);
        let inner = &f.sessions().inner;
        inner.report_harness_session("a", HarnessId::ClaudeCode, "claude-1", "/tmp");
        assert_eq!(f.resume(HarnessId::ClaudeCode).as_deref(), Some("claude-1"));

        // First time on Codex: no Codex session yet, so a fresh one that
        // carries the conversation.
        f.switch(HarnessId::Codex);
        assert_eq!(f.resume(HarnessId::Codex), None);
        let handle = f.core.doc_host.open("a").unwrap();
        assert!(saved_agent::carries_history(handle.doc()));
        inner.report_harness_session("a", HarnessId::Codex, "codex-1", "/tmp");
        assert_eq!(f.resume(HarnessId::Codex).as_deref(), Some("codex-1"));

        // Back to Claude Code: its own session, never the Codex thread.
        f.switch(HarnessId::ClaudeCode);
        assert_eq!(f.resume(HarnessId::ClaudeCode).as_deref(), Some("claude-1"));
        assert_eq!(
            f.core.workspace.chat_harness_session("a").unwrap().0,
            "claude-1"
        );
        f.switch(HarnessId::Codex);
        assert_eq!(f.resume(HarnessId::Codex).as_deref(), Some("codex-1"));
    }

    #[tokio::test]
    async fn a_report_from_the_previous_runtime_is_kept_for_it_only() {
        let f = fixture(crate::HarnessRegistry::new());
        f.runs_on(HarnessId::ClaudeCode);
        let inner = &f.sessions().inner;
        inner.report_harness_session("a", HarnessId::ClaudeCode, "claude-1", "/tmp");
        f.switch(HarnessId::Codex);
        inner.report_harness_session("a", HarnessId::Codex, "codex-1", "/tmp");
        f.switch(HarnessId::ClaudeCode);
        // The Codex run's Done lands after the switch back.
        inner.report_harness_session("a", HarnessId::Codex, "codex-2", "/tmp");
        assert_eq!(f.resume(HarnessId::ClaudeCode).as_deref(), Some("claude-1"));
        assert_eq!(
            f.core.workspace.chat_harness_session("a").unwrap().0,
            "claude-1"
        );
        let handle = f.core.doc_host.open("a").unwrap();
        assert_eq!(
            test_slot(handle.doc(), HarnessId::Codex).unwrap().0,
            "codex-2"
        );
    }

    /// Foreman's chat (#12205): a Claude Code chat whose row holds the Codex
    /// thread from before the fix resumes its own Claude session, found in
    /// the journal, after a restart.
    #[tokio::test]
    async fn a_chat_holding_another_runtimes_id_resumes_its_own_from_the_journal() {
        let f = fixture(crate::HarnessRegistry::new());
        f.started(HarnessId::ClaudeCode, "claude-good");
        f.started(HarnessId::Codex, "codex-thread");
        let inner = &f.sessions().inner;
        inner
            .journal
            .append(
                "a",
                &AgentEvent::Done {
                    status: DoneStatus::Errored,
                    result: None,
                    error: Some("No conversation found with session ID: codex-thread".into()),
                    session_id: Some("codex-thread".into()),
                },
            )
            .unwrap();
        f.runs_on(HarnessId::ClaudeCode);
        f.core
            .workspace
            .set_chat_harness_session("a", "codex-thread", "/tmp");
        assert_eq!(
            f.resume(HarnessId::ClaudeCode).as_deref(),
            Some("claude-good")
        );
        assert_eq!(
            f.core.workspace.chat_harness_session("a").unwrap().0,
            "claude-good"
        );
        // Codex, in turn, finds its own thread.
        assert_eq!(f.resume(HarnessId::Codex).as_deref(), Some("codex-thread"));
    }

    #[tokio::test]
    async fn a_missing_session_falls_back_to_the_last_good_one_then_fresh() {
        let f = fixture(crate::HarnessRegistry::new());
        f.runs_on(HarnessId::ClaudeCode);
        f.started(HarnessId::ClaudeCode, "claude-old");
        f.started(HarnessId::ClaudeCode, "claude-new");
        let inner = &f.sessions().inner;
        assert_eq!(
            f.resume(HarnessId::ClaudeCode).as_deref(),
            Some("claude-new")
        );
        let handle = f.core.doc_host.open("a").unwrap();
        let writer = handle.writer();
        inner.recover_missing_session("a", HarnessId::ClaudeCode, "claude-new", "/tmp", &writer);
        assert_eq!(
            f.resume(HarnessId::ClaudeCode).as_deref(),
            Some("claude-old")
        );
        assert!(!saved_agent::carries_history(handle.doc()));
        inner.recover_missing_session("a", HarnessId::ClaudeCode, "claude-old", "/tmp", &writer);
        assert_eq!(f.resume(HarnessId::ClaudeCode), None);
        assert!(saved_agent::carries_history(handle.doc()));
    }

    /// A Claude Code fixture that, like the CLI, refuses to resume `gone`.
    struct Forgetful {
        resumes: Arc<std::sync::Mutex<Vec<Option<String>>>>,
    }

    #[async_trait::async_trait]
    impl openagents_harness::Harness for Forgetful {
        fn id(&self) -> HarnessId {
            HarnessId::ClaudeCode
        }
        fn display_name(&self) -> &str {
            "Forgetful fixture"
        }
        fn supports_steering(&self) -> bool {
            false
        }
        fn steering_mode(&self) -> openagents_proto::SteeringMode {
            openagents_proto::SteeringMode::TurnBoundary
        }
        fn reasoning_levels(&self) -> &[openagents_proto::ReasoningLevel] {
            &[]
        }
        async fn models(
            &self,
        ) -> Result<Vec<openagents_proto::Model>, openagents_harness::HarnessError> {
            Ok(vec![])
        }
        async fn run(
            &self,
            request: RunRequest,
            _controls: openagents_harness::RunControls,
        ) -> Result<
            futures::stream::BoxStream<
                'static,
                Result<AgentEvent, openagents_harness::HarnessError>,
            >,
            openagents_harness::HarnessError,
        > {
            use futures::StreamExt;
            lock(&self.resumes).push(request.resume.clone());
            let events = if request.resume.as_deref() == Some("gone") {
                vec![AgentEvent::Done {
                    status: DoneStatus::Errored,
                    result: None,
                    error: Some("No conversation found with session ID: gone".into()),
                    session_id: Some("gone".into()),
                }]
            } else {
                let session = request.resume.unwrap_or_else(|| "fresh".into());
                vec![
                    AgentEvent::SessionStarted {
                        harness: HarnessId::ClaudeCode,
                        model: String::new(),
                        tools: vec![],
                        cwd: request.cwd,
                        session_id: session.clone(),
                        assistant_message_id: String::new(),
                    },
                    AgentEvent::Done {
                        status: DoneStatus::Completed,
                        result: None,
                        error: None,
                        session_id: Some(session),
                    },
                ]
            };
            Ok(futures::stream::iter(events.into_iter().map(Ok)).boxed())
        }
    }

    async fn run_until_idle(
        f: &Fixture,
        expected_runs: usize,
        resumes: &std::sync::Mutex<Vec<Option<String>>>,
    ) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let idle = f
                    .sessions()
                    .session_status("a")
                    .is_some_and(|s| s.status != openagents_proto::SessionStatus::Working);
                if idle && lock(resumes).len() >= expected_runs {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }

    fn request() -> RunRequest {
        RunRequest {
            mcp: None,
            prompt: "hello".into(),
            harness: None,
            model: None,
            reasoning: None,
            model_options: serde_json::Map::new(),
            cwd: "/tmp".into(),
            sandbox: SandboxLevel::WorkspaceWrite,
            auto_approve: true,
            resume: None,
            attachments: Vec::new(),
            worktree: None,
        }
    }

    #[tokio::test]
    async fn a_run_whose_session_is_gone_retries_on_the_last_good_one() {
        let resumes = Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry = crate::HarnessRegistry::new();
        registry.register(Arc::new(Forgetful {
            resumes: resumes.clone(),
        }));
        let f = fixture(registry);
        f.runs_on(HarnessId::ClaudeCode);
        f.started(HarnessId::ClaudeCode, "good");
        f.sessions()
            .inner
            .remember_harness_session("a", None, "gone", "/tmp");
        f.sessions()
            .dispatch("a", HarnessId::ClaudeCode, request(), None)
            .await
            .unwrap();
        run_until_idle(&f, 2, &resumes).await;
        assert_eq!(
            *lock(&resumes),
            vec![Some("gone".to_owned()), Some("good".to_owned())]
        );
        assert_eq!(
            f.sessions().session_status("a").unwrap().status,
            openagents_proto::SessionStatus::Idle
        );
    }

    #[tokio::test]
    async fn a_run_with_no_good_session_left_starts_fresh_with_the_conversation() {
        let resumes = Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry = crate::HarnessRegistry::new();
        registry.register(Arc::new(Forgetful {
            resumes: resumes.clone(),
        }));
        let f = fixture(registry);
        f.runs_on(HarnessId::ClaudeCode);
        f.sessions()
            .inner
            .remember_harness_session("a", None, "gone", "/tmp");
        f.sessions()
            .dispatch("a", HarnessId::ClaudeCode, request(), None)
            .await
            .unwrap();
        run_until_idle(&f, 2, &resumes).await;
        assert_eq!(*lock(&resumes), vec![Some("gone".to_owned()), None]);
        assert_eq!(f.resume(HarnessId::ClaudeCode).as_deref(), Some("fresh"));
    }
}
