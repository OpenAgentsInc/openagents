//! Requests to an app on a person's computer, carried by that computer's signed-in session.
use super::{AgentDefinition, AgentRequest, Agents, request_id, valid_id};
use crate::{CancellationToken, Error};
use reqwest::Method;
use serde::{Deserialize, Serialize};

/// What to run: an agent revision, the computer and optional project, and the task text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssignmentSpec {
    pub agent: String,
    pub agent_revision: u64,
    pub host: String,
    pub project: Option<String>,
    pub task: String,
}
impl AssignmentSpec {
    /// Checks the fields and returns a message for the user when one is not valid.
    pub fn validate(&self) -> Result<(), &'static str> {
        if !valid_id(&self.agent) || self.agent_revision == 0 {
            return Err("Choose an agent again.");
        }
        if self.host.is_empty()
            || self.host.len() > 128
            || !self
                .host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        {
            return Err("Choose a computer.");
        }
        if self.project.as_ref().is_some_and(|s| {
            s.is_empty()
                || s.len() > 128
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        }) {
            return Err("Choose a project.");
        }
        if self.task.trim().is_empty() || self.task.len() > 64 * 1024 || self.task.contains('\0') {
            return Err("Enter a task or shorten the text.");
        }
        Ok(())
    }
}
/// The request body that assigns a task to an agent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateAssignment {
    pub request_id: String,
    pub spec: AssignmentSpec,
}
/// Where an assignment is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssignmentState {
    Queued,
    Running,
    Completed,
    Failed,
    Stopped,
    #[serde(other)]
    Unknown,
}
/// A user action on an assignment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum AssignmentAction {
    FollowUp { text: String },
    Stop,
    Archive,
}
/// A user action on an assignment with its request ID.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssignmentCommand {
    pub request_id: String,
    pub action: AssignmentAction,
}
/// A task assigned to an agent on a computer, with its chat, state, and result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assignment {
    pub id: String,
    pub revision: u64,
    pub spec: AssignmentSpec,
    pub definition: AgentDefinition,
    pub chat: String,
    pub state: AssignmentState,
    pub commands: Vec<AssignmentCommand>,
    pub result: Option<String>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub active_command: Option<String>,
    #[serde(default)]
    pub after_seq: u64,
}
/// One page of assignments and the cursor for the next page.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssignmentPage {
    pub data: Vec<Assignment>,
    pub next: Option<String>,
}
/// The request body a computer sends to report an assignment's state and result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssignmentReport {
    pub active_command: Option<String>,
    pub after_seq: u64,
    pub request_id: String,
    pub host: String,
    pub expected_revision: u64,
    pub state: AssignmentState,
    pub result: Option<String>,
}
impl Agents<'_> {
    /// Checks `spec` and prepares a request that creates an assignment.
    pub fn assignment_request(
        &self,
        spec: AssignmentSpec,
    ) -> Result<AgentRequest<CreateAssignment>, Error> {
        spec.validate().map_err(|_| Error::InvalidReference)?;
        Ok(self.bind(CreateAssignment {
            request_id: request_id()?,
            spec,
        }))
    }
    /// Checks `action` and prepares a request that sends it to an assignment.
    pub fn command_request(
        &self,
        action: AssignmentAction,
    ) -> Result<AgentRequest<AssignmentCommand>, Error> {
        if let AssignmentAction::FollowUp { text } = &action {
            if text.trim().is_empty() || text.len() > 64 * 1024 || text.contains('\0') {
                return Err(Error::InvalidReference);
            }
        }
        Ok(self.bind(AssignmentCommand {
            request_id: request_id()?,
            action,
        }))
    }
    /// Prepares a request a computer uses to report an assignment's state.
    pub fn report_request(
        &self,
        host: String,
        expected_revision: u64,
        state: AssignmentState,
        result: Option<String>,
        active_command: Option<String>,
        after_seq: u64,
    ) -> Result<AgentRequest<AssignmentReport>, Error> {
        Ok(self.bind(AssignmentReport {
            active_command,
            after_seq,
            request_id: request_id()?,
            host,
            expected_revision,
            state,
            result,
        }))
    }
    /// Creates an assignment from a prepared request.
    pub async fn assign(
        &self,
        request: &AgentRequest<CreateAssignment>,
        cancel: &CancellationToken,
    ) -> Result<Assignment, Error> {
        self.send(
            Method::POST,
            &["agent-assignments"],
            &[],
            Some(self.body(request)?),
            cancel,
        )
        .await
    }
    /// Fetches the assignment `id`.
    pub async fn assignment(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> Result<Assignment, Error> {
        if !valid_id(id) {
            return Err(Error::InvalidReference);
        }
        self.send(Method::GET, &["agent-assignments", id], &[], None, cancel)
            .await
    }
    /// Lists assignments, optionally for one computer, starting after the cursor `after`.
    pub async fn assignments(
        &self,
        host: Option<&str>,
        after: Option<&str>,
        cancel: &CancellationToken,
    ) -> Result<AssignmentPage, Error> {
        let mut q = vec![];
        if let Some(host) = host {
            q.push(("host", host.into()));
        }
        if let Some(after) = after {
            q.push(("after", after.into()));
        }
        self.send(Method::GET, &["agent-assignments"], &q, None, cancel)
            .await
    }
    /// Sends a prepared user action to the assignment `id`.
    pub async fn command(
        &self,
        id: &str,
        request: &AgentRequest<AssignmentCommand>,
        cancel: &CancellationToken,
    ) -> Result<Assignment, Error> {
        if !valid_id(id) {
            return Err(Error::InvalidReference);
        }
        self.send(
            Method::POST,
            &["agent-assignments", id, "commands"],
            &[],
            Some(self.body(request)?),
            cancel,
        )
        .await
    }
    /// Sends a prepared state report for the assignment `id`.
    pub async fn report(
        &self,
        id: &str,
        request: &AgentRequest<AssignmentReport>,
        cancel: &CancellationToken,
    ) -> Result<Assignment, Error> {
        if !valid_id(id) {
            return Err(Error::InvalidReference);
        }
        self.send(
            Method::POST,
            &["agent-assignments", id, "report"],
            &[],
            Some(self.body(request)?),
            cancel,
        )
        .await
    }
}
