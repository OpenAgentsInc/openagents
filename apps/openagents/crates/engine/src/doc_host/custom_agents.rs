//! Public SDK assignments enter the existing host-owned session command ledger.
use super::*;
use crate::{auth::Auth, registry::HarnessRegistry};
use openagents_account::{
    Lease,
    openagents::agents::{Access, assignments::*},
};
use openagents_proto::{AgentEvent, DoneStatus, SandboxLevel};
use std::sync::Arc;

pub(super) struct AgentHost {
    auth: Auth,
    registry: Arc<HarnessRegistry>,
    lab_root: std::path::PathBuf,
}
const ASSIGNMENT: &str = "openagentsAssignment";
const INSTRUCTIONS: &str = "openagentsInstructions";
fn error(message: &str) -> EngineError {
    EngineError::Other(message.into())
}
fn command_id(id: &str) -> String {
    format!(
        "{}-{}-{}-{}-{}",
        &id[..8],
        &id[8..12],
        &id[12..16],
        &id[16..20],
        &id[20..32]
    )
}
/// The largest answer the account service accepts in an assignment report.
const RESULT_BYTES: usize = 16 * 1024;
/// What an older account service accepts; a refused report is retried at this size.
const LEGACY_RESULT_BYTES: usize = 4096;
/// Ends an answer that didn't fit, so a reader knows where the rest is.
const CUT_NOTE: &str = "\n\n… The full answer is in the OpenAgents app.";

/// `text` within `limit` UTF-8 bytes. A longer text is cut on a character
/// boundary and ends with [`CUT_NOTE`].
fn bounded_to(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_owned();
    }
    let room = limit.saturating_sub(CUT_NOTE.len());
    let mut end = room.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{CUT_NOTE}", text[..end].trim_end())
}
fn bounded(text: &str) -> String {
    bounded_to(text, RESULT_BYTES)
}
fn result_too_long_for_legacy(result: &Option<String>) -> bool {
    result
        .as_ref()
        .is_some_and(|t| t.len() > LEGACY_RESULT_BYTES)
}

impl DocHost {
    pub(super) fn openagents_lease(&self) -> Option<Lease> {
        self.inner.custom_agents.get()?.auth.openagents_lease()
    }
    pub fn start_custom_agents(
        &self,
        auth: Auth,
        registry: Arc<HarnessRegistry>,
        lab_root: std::path::PathBuf,
    ) {
        if auth.account().is_none()
            || self
                .inner
                .custom_agents
                .set(AgentHost {
                    auth,
                    registry,
                    lab_root,
                })
                .is_err()
        {
            return;
        }
        let weak = Arc::downgrade(&self.inner);
        self.spawn_worker(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                let Some(inner) = weak.upgrade() else { break };
                let host = DocHost { inner };
                let Some(binding) = host.inner.custom_agents.get() else {
                    break;
                };
                let Some(lease) = binding.auth.openagents_lease() else {
                    continue;
                };
                if host.sessions().is_none() || host.workspace().is_none() {
                    continue;
                }
                let _ = host.recover_continuations(&lease).await;
                // Summary rows for the account's chat list (#11897).
                let _ = host
                    .publish_session_index(&lease, &binding.auth.state().to_proto())
                    .await;
                let _ = host.process_web_moves(&lease).await;
                // Failures leave the original requests queued; an outage never starts another run.
                let _ = host
                    .agent_lab_tick(&lease, &binding.lab_root, &binding.registry)
                    .await;
                let _ = host.custom_agent_tick(&lease).await;
            }
        });
    }
    pub(crate) async fn prepare_custom_agent(
        &self,
        chat_id: &str,
        harness: HarnessId,
        mut request: openagents_proto::RunRequest,
    ) -> Result<openagents_proto::RunRequest, EngineError> {
        let Some(id) = request
            .model_options
            .get(ASSIGNMENT)
            .and_then(|v| v.as_str())
            .map(str::to_owned)
        else {
            return Ok(request);
        };
        if !openagents_account::openagents::agents::valid_id(&id) {
            return Err(error("Choose the agent again."));
        }
        let host = self
            .inner
            .custom_agents
            .get()
            .ok_or_else(|| error("Sign in to OpenAgents on this computer."))?;
        let lease = host
            .auth
            .openagents_lease()
            .ok_or_else(|| error("Sign in to OpenAgents on this computer."))?;
        let agents = lease
            .client()
            .agents(lease.session())
            .map_err(|_| error("Sign in to OpenAgents again."))?;
        let a = agents
            .assignment(&id, &lease.cancel)
            .await
            .map_err(EngineError::AgentService)?;
        let latest = agents
            .get(&a.spec.agent, None, &lease.cancel)
            .await
            .map_err(EngineError::AgentService)?;
        if !lease.is_current() || latest.deleted || a.archived {
            return Err(error("This agent is no longer available."));
        }
        let workspace = self
            .workspace()
            .ok_or_else(|| error("This computer is not ready."))?;
        let chat = workspace
            .chat(chat_id)?
            .ok_or_else(|| error("This conversation could not be found."))?;
        validate_binding(
            &a,
            &self.inner.config.device_id,
            chat_id,
            chat.space_id.as_deref(),
            harness,
            &request,
        )?;
        admit_runtime(&host.registry, harness, &a.definition.spec).await?;
        if let Some((cwd, deadline)) = super::agent_lab::trial_cwd(&host.lab_root, &a)? {
            let remaining = deadline.saturating_sub(now_ms().max(0) as u64);
            if remaining == 0 {
                return Err(error("This experiment reached its time limit."));
            }
            request.cwd = cwd;
            let weak = Arc::downgrade(&self.inner);
            let chat = chat_id.to_owned();
            self.spawn_worker(async move {
                tokio::time::sleep(std::time::Duration::from_millis(remaining)).await;
                if let Some(inner) = weak.upgrade() {
                    let host = DocHost { inner };
                    if let Some(sessions) = host.sessions() {
                        let _ = sessions.interrupt(&chat).await;
                    }
                }
            });
        }
        if request.worktree.is_some() {
            return Err(error(
                "Choose the project's existing folder for this agent.",
            ));
        }
        request.cwd = if let Some(project) = &a.spec.project {
            let space = workspace
                .space(project)?
                .ok_or_else(|| error("This project is no longer available."))?;
            if space.device_id != self.inner.config.device_id {
                return Err(error("Choose a project on this computer."));
            }
            space.path
        } else {
            chat.cwd
                .ok_or_else(|| error("Choose a folder for this conversation."))?
        };
        request.model_options.clear();
        request.model_options.insert(ASSIGNMENT.into(), id.into());
        if harness == HarnessId::Sdk {
            request.model_options.insert(
                "openagentsAgentSpec".into(),
                serde_json::to_value(&a.definition.spec)
                    .map_err(|_| error("The agent could not be read."))?,
            );
            request
                .model_options
                .insert("openagentsConversation".into(), chat_id.into());
        }
        request.model_options.insert(
            INSTRUCTIONS.into(),
            a.definition.spec.instructions.clone().into(),
        );
        request.sandbox = match a.definition.spec.access {
            Access::HostDefault => SandboxLevel::DangerFullAccess,
            Access::ReadOnly => SandboxLevel::ReadOnly,
            Access::WorkspaceWrite => SandboxLevel::WorkspaceWrite,
        };
        Ok(request)
    }
    async fn custom_agent_tick(&self, lease: &Lease) -> Result<(), EngineError> {
        let agents = lease
            .client()
            .agents(lease.session())
            .map_err(|_| error("Sign in to OpenAgents again."))?;
        let mut after = None;
        loop {
            let page = agents
                .assignments(
                    Some(&self.inner.config.device_id),
                    after.as_deref(),
                    &lease.cancel,
                )
                .await
                .map_err(|_| error("Assignments could not be loaded."))?;
            for a in page.data {
                if !lease.is_current() {
                    return Ok(());
                }
                if let Err(e) = self.receive_custom_assignment(lease, &a).await {
                    if matches!(&e, EngineError::AgentService(_) | EngineError::Harness(_)) {
                        continue;
                    }
                    let _ = self
                        .report_custom_assignment(
                            lease,
                            &a,
                            if self.sessions().is_some_and(|s| s.turn_in_flight(&a.chat)) {
                                AssignmentState::Running
                            } else if a.commands.iter().any(|c| {
                                openagents_account::openagents::agents::valid_id(&c.request_id)
                                    && self
                                        .inner
                                        .store
                                        .is_processed(&command_id(&c.request_id))
                                        .unwrap_or(true)
                            }) {
                                AssignmentState::Unknown
                            } else {
                                AssignmentState::Failed
                            },
                            Some(bounded(&e.to_string())),
                        )
                        .await;
                }
            }
            after = page.next;
            if after.is_none() {
                break;
            }
        }
        Ok(())
    }
    async fn report_custom_assignment(
        &self,
        lease: &Lease,
        a: &Assignment,
        state: AssignmentState,
        result: Option<String>,
    ) -> Result<(), EngineError> {
        if !lease.is_current() || a.state == state && a.result == result {
            return Ok(());
        }
        let agents = lease
            .client()
            .agents(lease.session())
            .map_err(|_| error("Sign in again."))?;
        let result_copy = result.clone();
        let r = agents
            .report_request(
                self.inner.config.device_id.clone(),
                a.revision,
                state,
                result,
                a.active_command
                    .clone()
                    .or_else(|| a.commands.first().map(|c| c.request_id.clone())),
                a.after_seq,
            )
            .map_err(EngineError::AgentService)?;
        match agents.report(&a.id, &r, &lease.cancel).await {
            Ok(_) => Ok(()),
            // An account service from before the 16 KiB limit refuses a longer
            // answer as invalid. Retry once with the answer it accepts.
            Err(openagents_account::openagents::Error::Refused { status: 400, .. })
                if result_too_long_for_legacy(&result_copy) =>
            {
                let short = result_copy
                    .as_deref()
                    .map(|t| bounded_to(t, LEGACY_RESULT_BYTES));
                let r = agents
                    .report_request(
                        self.inner.config.device_id.clone(),
                        a.revision,
                        state,
                        short,
                        a.active_command
                            .clone()
                            .or_else(|| a.commands.first().map(|c| c.request_id.clone())),
                        a.after_seq,
                    )
                    .map_err(EngineError::AgentService)?;
                agents
                    .report(&a.id, &r, &lease.cancel)
                    .await
                    .map(|_| ())
                    .map_err(EngineError::AgentService)
            }
            Err(e) => Err(EngineError::AgentService(e)),
        }
    }
    async fn receive_custom_assignment(
        &self,
        lease: &Lease,
        a: &Assignment,
    ) -> Result<(), EngineError> {
        a.spec.validate().map_err(error)?;
        if a.commands
            .iter()
            .any(|c| !openagents_account::openagents::agents::valid_id(&c.request_id))
        {
            return Err(error("The assignment could not be read."));
        }
        if a.archived
            || a.spec.host != self.inner.config.device_id
            || matches!(
                a.state,
                AssignmentState::Failed | AssignmentState::Stopped | AssignmentState::Unknown
            )
        {
            return Ok(());
        }
        let workspace = self
            .workspace()
            .ok_or_else(|| error("This computer is not ready."))?;
        let sessions = self
            .sessions()
            .ok_or_else(|| error("This computer is not ready."))?;
        let pending: Vec<_> = a
            .commands
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                !self
                    .inner
                    .store
                    .is_processed(&command_id(&c.request_id))
                    .unwrap_or(true)
            })
            .collect();
        if pending.is_empty() {
            if matches!(
                a.state,
                AssignmentState::Completed
                    | AssignmentState::Failed
                    | AssignmentState::Stopped
                    | AssignmentState::Unknown
            ) {
                return Ok(());
            }
            if sessions.turn_in_flight(&a.chat) {
                return self
                    .report_custom_assignment(lease, a, AssignmentState::Running, None)
                    .await;
            }
            let (state, result) = terminal_result(&sessions, &a.chat, a.after_seq);
            return self.report_custom_assignment(lease, a, state, result).await;
        }
        // Stop also cancels earlier work that has not yet entered the session ledger.
        if let Some((index, stop)) = pending
            .iter()
            .rev()
            .find(|(_, c)| matches!(c.action, AssignmentAction::Stop))
        {
            for c in a.commands.iter().take(*index) {
                self.inner
                    .store
                    .mark_processed(&command_id(&c.request_id))?;
            }
            if workspace.chat(&a.chat)?.is_some() {
                let entry = entry(
                    stop,
                    SessionCommandPayload::Interrupt {},
                    &self.inner.config.device_id,
                );
                self.ingest_relayed_command(&a.chat, entry).await?;
                if sessions.turn_in_flight(&a.chat) {
                    return Ok(());
                }
            } else {
                self.inner
                    .store
                    .mark_processed(&command_id(&stop.request_id))?;
            }
            return self
                .report_custom_assignment(lease, a, AssignmentState::Stopped, None)
                .await;
        }
        if sessions.turn_in_flight(&a.chat) {
            return Ok(());
        }
        let agents = lease
            .client()
            .agents(lease.session())
            .map_err(|_| error("Sign in again."))?;
        let latest = agents
            .get(&a.spec.agent, None, &lease.cancel)
            .await
            .map_err(EngineError::AgentService)?;
        if latest.deleted {
            return Err(error("This agent was deleted before the work started."));
        }
        if !lease.is_current() {
            return Ok(());
        }
        let harness: HarnessId =
            serde_json::from_value(a.definition.spec.runtime.clone().into())
                .map_err(|_| error("This runtime is not supported on this computer."))?;
        let config = openagents_proto::ChatConfig {
            harness,
            model: a.definition.spec.model.clone(),
            reasoning: None,
            model_options: serde_json::Map::from_iter([(ASSIGNMENT.into(), a.id.clone().into())]),
            sandbox: SandboxLevel::DangerFullAccess,
        };
        if let Some(project) = &a.spec.project {
            let space = workspace
                .space(project)?
                .ok_or_else(|| error("This project is no longer available."))?;
            if space.device_id != self.inner.config.device_id {
                return Err(error("Choose a project on this computer."));
            }
        }
        if let Some(chat) = workspace.chat(&a.chat)? {
            if chat.device_id != a.spec.host
                || chat.space_id != a.spec.project
                || chat
                    .config
                    .as_ref()
                    .and_then(|c| c.model_options.get(ASSIGNMENT))
                    .and_then(|v| v.as_str())
                    != Some(a.id.as_str())
            {
                return Err(error("This conversation belongs to another assignment."));
            }
        } else {
            let binding = self
                .inner
                .custom_agents
                .get()
                .ok_or_else(|| error("Sign in on this computer."))?;
            admit_runtime(&binding.registry, harness, &a.definition.spec).await?;
            if !lease.is_current() {
                return Ok(());
            }
            workspace.create_chat(
                &a.chat,
                a.spec.project.as_deref(),
                Some(&a.spec.host),
                Some(config),
                a.spec.project.is_none().then(|| "~".into()),
            )?;
            workspace.rename_chat(&a.chat, &a.definition.spec.name)?;
        }
        let Some((_, command)) = pending.first() else {
            return Ok(());
        };
        if a.active_command.as_deref() != Some(command.request_id.as_str()) {
            let seq = sessions
                .subscribe(&a.chat, 0)
                .map(|(events, _)| events.last().map_or(0, |e| e.seq))
                .unwrap_or(0);
            let request = agents
                .report_request(
                    self.inner.config.device_id.clone(),
                    a.revision,
                    AssignmentState::Running,
                    None,
                    Some(command.request_id.clone()),
                    seq,
                )
                .map_err(EngineError::AgentService)?;
            agents
                .report(&a.id, &request, &lease.cancel)
                .await
                .map_err(EngineError::AgentService)?;
            return Ok(());
        }
        if let AssignmentAction::FollowUp { text } = &command.action {
            let request = self
                .request_from_chat_row(&a.chat, text)
                .ok_or_else(|| error("This conversation is not ready."))?;
            let request = self.prepare_custom_agent(&a.chat, harness, request).await?;
            // The existing ledger marks the fixed command ID before dispatch. A crash is observed, never replayed as fresh work.
            self.ingest_relayed_command(
                &a.chat,
                entry(
                    command,
                    SessionCommandPayload::Run {
                        request,
                        message_id: command_id(&command.request_id),
                    },
                    &self.inner.config.device_id,
                ),
            )
            .await?;
            self.report_custom_assignment(lease, a, AssignmentState::Running, None)
                .await?;
        }
        Ok(())
    }
}
fn validate_runtime_access(harness: HarnessId, access: Access) -> Result<(), EngineError> {
    if !matches!(
        harness,
        HarnessId::ClaudeCode
            | HarnessId::Codex
            | HarnessId::Opencode
            | HarnessId::Sdk
            | HarnessId::Mock
    ) {
        return Err(error("Choose a supported runtime for this agent."));
    }
    // These adapters use the host's permission settings.
    if matches!(
        harness,
        HarnessId::ClaudeCode | HarnessId::Opencode | HarnessId::Sdk
    ) && access != Access::HostDefault
    {
        return Err(error(
            "This runtime uses this computer's access settings. Choose that access option.",
        ));
    }
    Ok(())
}

pub(super) async fn admit_runtime(
    registry: &HarnessRegistry,
    harness: HarnessId,
    spec: &openagents_account::openagents::agents::AgentSpec,
) -> Result<(), EngineError> {
    spec.validate().map_err(error)?;
    validate_runtime_access(harness, spec.access)?;
    let descriptor = registry
        .descriptors()
        .into_iter()
        .find(|d| d.id == harness)
        .ok_or_else(|| error("Install this runtime on the selected computer."))?;
    if !descriptor.installed || !crate::registry::descriptor_enabled(&descriptor) {
        return Err(error("Enable this runtime on the selected computer."));
    }
    if let Some(model) = &spec.model {
        let models = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            registry.discover_models(harness),
        )
        .await
        .map_err(|_| error("The model list could not be loaded."))??;
        if !models.iter().any(|m| &m.id == model) {
            return Err(error(
                "This model is not available on the selected computer.",
            ));
        }
    }
    Ok(())
}
fn entry(
    command: &AssignmentCommand,
    payload: SessionCommandPayload,
    device: &str,
) -> SessionCommandEntry {
    SessionCommandEntry {
        id: command_id(&command.request_id),
        payload,
        issued_by: device.into(),
        issued_at: now_ms(),
        based_on: None,
        expires_at: Some(now_ms() + 10 * 60 * 1000),
        status: SessionCommandStatus::Pending,
        resolution: None,
    }
}
fn validate_binding(
    a: &Assignment,
    host: &str,
    chat: &str,
    project: Option<&str>,
    harness: HarnessId,
    request: &openagents_proto::RunRequest,
) -> Result<(), EngineError> {
    a.definition.spec.validate().map_err(error)?;
    if a.spec.host != host
        || a.chat != chat
        || a.spec.project.as_deref() != project
        || a.spec.agent != a.definition.id
        || a.spec.agent_revision != a.definition.revision
    {
        return Err(error(
            "This assignment does not match the selected computer and project.",
        ));
    }
    let runtime = serde_json::to_value(harness)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned));
    if runtime.as_deref() != Some(a.definition.spec.runtime.as_str())
        || request.model != a.definition.spec.model
    {
        return Err(error("This agent uses a different runtime or model."));
    }
    validate_runtime_access(harness, a.definition.spec.access)
}
fn terminal_result(
    sessions: &crate::sessions::SessionsEngine,
    chat: &str,
    after_seq: u64,
) -> (AssignmentState, Option<String>) {
    let Ok((events, _)) = sessions.subscribe(chat, after_seq) else {
        return (
            AssignmentState::Unknown,
            Some("The result could not be confirmed. Open the conversation to check it.".into()),
        );
    };
    terminal_events(events.iter().map(|e| &e.event))
}
fn terminal_events<'a>(
    events: impl Iterator<Item = &'a AgentEvent>,
) -> (AssignmentState, Option<String>) {
    let mut text = String::new();
    // Whether part of the current answer didn't fit in `text`.
    let mut cut = false;
    let mut terminal = None;
    for event in events {
        match event {
            AgentEvent::TextDelta { text: fragment } => {
                let remaining = RESULT_BYTES.saturating_sub(text.len());
                let before = text.len();
                text.extend(fragment.chars().scan(0usize, |bytes, ch| {
                    *bytes += ch.len_utf8();
                    (*bytes <= remaining).then_some(ch)
                }));
                cut |= text.len() - before < fragment.len();
            }
            AgentEvent::Done {
                status,
                result,
                error,
                ..
            } => {
                let state = match status {
                    DoneStatus::Completed => AssignmentState::Completed,
                    DoneStatus::Interrupted => AssignmentState::Stopped,
                    DoneStatus::Errored => AssignmentState::Failed,
                };
                let summary = result
                    .as_deref()
                    .or(error.as_deref())
                    .map(bounded)
                    .or_else(|| {
                        (!text.is_empty()).then(|| {
                            if cut {
                                // Some of the answer was dropped: make the cut show.
                                bounded_to(&format!("{text}…"), text.len())
                            } else {
                                text.clone()
                            }
                        })
                    });
                terminal = Some((state, summary));
                text.clear();
                cut = false;
            }
            _ => {}
        }
    }
    terminal.unwrap_or((
        AssignmentState::Unknown,
        Some("The result could not be confirmed. Open the conversation to check it.".into()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_account::openagents::agents::{AgentDefinition, AgentSpec};
    fn fixture() -> (Assignment, openagents_proto::RunRequest) {
        let request:openagents_proto::RunRequest=serde_json::from_value(serde_json::json!({"prompt":"Test","cwd":"/workspace","sandbox":"read-only","model":null,"reasoning":null,"resume":null})).unwrap();
        let a = Assignment {
            id: "a".repeat(64),
            revision: 1,
            spec: AssignmentSpec {
                agent: "b".repeat(64),
                agent_revision: 1,
                host: "host".into(),
                project: Some("project".into()),
                task: "Test".into(),
            },
            definition: AgentDefinition {
                id: "b".repeat(64),
                revision: 1,
                deleted: false,
                spec: AgentSpec {
                    name: "Reviewer".into(),
                    description: String::new(),
                    instructions: "Find defects".into(),
                    runtime: "codex".into(),
                    model: None,
                    access: Access::ReadOnly,
                },
            },
            chat: "chat".into(),
            state: AssignmentState::Queued,
            commands: vec![],
            result: None,
            archived: false,
            active_command: None,
            after_seq: 0,
        };
        (a, request)
    }
    #[test]
    fn exact_assignment_binding_and_permission_support() {
        let (mut a, request) = fixture();
        assert!(
            validate_binding(
                &a,
                "host",
                "chat",
                Some("project"),
                HarnessId::Codex,
                &request
            )
            .is_ok()
        );
        for (host, chat, project) in [
            ("other", "chat", Some("project")),
            ("host", "other", Some("project")),
            ("host", "chat", None),
        ] {
            assert!(validate_binding(&a, host, chat, project, HarnessId::Codex, &request).is_err());
        }
        a.definition.revision = 2;
        assert!(
            validate_binding(
                &a,
                "host",
                "chat",
                Some("project"),
                HarnessId::Codex,
                &request
            )
            .is_err()
        );
        a.definition.revision = 1;
        a.definition.spec.runtime = "claude-code".into();
        assert!(
            validate_binding(
                &a,
                "host",
                "chat",
                Some("project"),
                HarnessId::ClaudeCode,
                &request
            )
            .is_err()
        );
        a.definition.spec.access = Access::HostDefault;
        assert!(
            validate_binding(
                &a,
                "host",
                "chat",
                Some("project"),
                HarnessId::ClaudeCode,
                &request
            )
            .is_ok()
        );
    }
    #[test]
    fn opencode_assignment_preserves_its_model_and_host_access() {
        let (mut assignment, mut request) = fixture();
        assignment.definition.spec.runtime = "opencode".into();
        assignment.definition.spec.model = Some("openagents/openagents/fast".into());
        assignment.definition.spec.access = Access::HostDefault;
        request.model = assignment.definition.spec.model.clone();
        assert!(
            validate_binding(
                &assignment,
                "host",
                "chat",
                Some("project"),
                HarnessId::Opencode,
                &request
            )
            .is_ok()
        );
        for access in [Access::ReadOnly, Access::WorkspaceWrite] {
            assignment.definition.spec.access = access;
            assert!(
                validate_binding(
                    &assignment,
                    "host",
                    "chat",
                    Some("project"),
                    HarnessId::Opencode,
                    &request
                )
                .is_err()
            );
        }
        assignment.definition.spec.access = Access::HostDefault;
        request.model = Some("foreign-model".into());
        assert!(
            validate_binding(
                &assignment,
                "host",
                "chat",
                Some("project"),
                HarnessId::Opencode,
                &request
            )
            .is_err()
        );
    }

    struct RuntimeFixture {
        id: HarnessId,
        probes: Arc<std::sync::atomic::AtomicUsize>,
    }
    #[async_trait::async_trait]
    impl openagents_harness::Harness for RuntimeFixture {
        fn id(&self) -> HarnessId {
            self.id
        }
        fn display_name(&self) -> &str {
            "Runtime fixture"
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
            self.probes
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(vec![openagents_proto::Model {
                id: "openagents/openagents/fast".into(),
                label: "Fixture".into(),
                description: None,
                reasoning_levels: vec![],
                options: vec![],
            }])
        }
        async fn run(
            &self,
            _request: openagents_proto::RunRequest,
            _controls: openagents_harness::RunControls,
        ) -> Result<
            futures::stream::BoxStream<
                'static,
                Result<AgentEvent, openagents_harness::HarnessError>,
            >,
            openagents_harness::HarnessError,
        > {
            panic!("Admission must not launch a runtime.");
        }
    }

    #[tokio::test]
    async fn opencode_admission_checks_access_enablement_and_the_advertised_model() {
        let registry = HarnessRegistry::new();
        let probes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        registry.register(Arc::new(RuntimeFixture {
            id: HarnessId::Opencode,
            probes: probes.clone(),
        }));
        let (assignment, _) = fixture();
        let mut spec = assignment.definition.spec;
        spec.runtime = "opencode".into();
        spec.access = Access::HostDefault;
        spec.model = Some("openagents/openagents/fast".into());
        admit_runtime(&registry, HarnessId::Opencode, &spec)
            .await
            .unwrap();
        assert_eq!(probes.load(std::sync::atomic::Ordering::SeqCst), 1);
        for access in [Access::ReadOnly, Access::WorkspaceWrite] {
            spec.access = access;
            assert!(
                admit_runtime(&registry, HarnessId::Opencode, &spec)
                    .await
                    .is_err()
            );
        }
        assert_eq!(probes.load(std::sync::atomic::Ordering::SeqCst), 1);
        spec.access = Access::HostDefault;
        spec.model = Some("foreign-model".into());
        assert!(
            admit_runtime(&registry, HarnessId::Opencode, &spec)
                .await
                .is_err()
        );
        spec.model = Some("openagents/openagents/fast".into());
        assert!(
            admit_runtime(&HarnessRegistry::new(), HarnessId::Opencode, &spec)
                .await
                .is_err()
        );
        registry.register(Arc::new(RuntimeFixture {
            id: HarnessId::Codex,
            probes: probes.clone(),
        }));
        registry.set_enabled(HarnessId::Opencode, false).unwrap();
        let before = probes.load(std::sync::atomic::Ordering::SeqCst);
        assert!(
            admit_runtime(&registry, HarnessId::Opencode, &spec)
                .await
                .is_err()
        );
        assert_eq!(probes.load(std::sync::atomic::Ordering::SeqCst), before);
    }

    #[test]
    fn native_streamed_answers_supply_the_summary_without_reusing_an_earlier_turn() {
        let done = || AgentEvent::Done {
            status: DoneStatus::Completed,
            result: None,
            error: None,
            session_id: None,
        };
        let events = [
            AgentEvent::TextDelta {
                text: "Earlier answer".into(),
            },
            done(),
            AgentEvent::TextDelta {
                text: "CUSTOM_".into(),
            },
            AgentEvent::TextDelta {
                text: "AGENT_OK".into(),
            },
            done(),
        ];
        assert_eq!(
            terminal_events(events.iter()),
            (AssignmentState::Completed, Some("CUSTOM_AGENT_OK".into()))
        );
        assert_eq!(
            terminal_events([done()].iter()),
            (AssignmentState::Completed, None)
        );
        // A 7.6 KB answer (what Foreman wrote) arrives whole.
        let events = [
            AgentEvent::TextDelta {
                text: "a".repeat(7642),
            },
            done(),
        ];
        assert_eq!(terminal_events(events.iter()).1.unwrap().len(), 7642);
        // A longer one is cut within the limit and says where the rest is.
        let events = [
            AgentEvent::TextDelta {
                text: "é".repeat(16 * 1024),
            },
            done(),
        ];
        let summary = terminal_events(events.iter()).1.unwrap();
        assert!(summary.len() <= RESULT_BYTES && summary.ends_with(CUT_NOTE));
    }
    #[test]
    fn summaries_preserve_utf8_and_command_ids_are_stable() {
        assert!(bounded(&"é".repeat(16 * 1024)).len() <= RESULT_BYTES);
        assert!(bounded(&"é".repeat(16 * 1024)).ends_with(CUT_NOTE));
        assert_eq!(bounded("short answer"), "short answer");
        let legacy = bounded_to(&"word ".repeat(2000), LEGACY_RESULT_BYTES);
        assert!(legacy.len() <= LEGACY_RESULT_BYTES && legacy.ends_with(CUT_NOTE));
        let id = "ab".repeat(32);
        assert_eq!(command_id(&id), command_id(&id));
        assert_eq!(command_id(&id).len(), 36);
    }
}
