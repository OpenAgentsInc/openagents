//! Private, revisioned agent definitions. Saving a definition never starts work.
pub mod assignments;
pub mod place;
pub mod runner;
pub mod voice;
use crate::{CancellationToken, Client, Deployment, Error, Session};
use reqwest::Method;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub use crate::agent_lab::{ForkAgent, Forked, PublishDefinition, PublishedAgent, Upstream};
pub use agent_contract::{
    ANY_RUNTIME, Access, AgentDefinition, AgentGrant, AgentSpec, Autonomy, ForkedFrom, GrantOver,
    GrantScope, MAX_FALLBACKS, MAX_INSTRUCTIONS_BYTES, NEVER_GRANTED_REFUSAL, Profile,
    RuntimeChoice, WakeEvent, valid_id,
};

/// The request body that grants an agent scopes (#12044). Scopes travel as
/// words so the server can refuse never-list requests by name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantAgent {
    pub request_id: String,
    pub scopes: Vec<String>,
    pub over: GrantOver,
}

/// An agent's grants, oldest first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrantList {
    pub data: Vec<AgentGrant>,
}

/// What an agent declares to wake on: a definition setting, so a fork copies
/// it (#12044).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Wakes {
    /// The events it wakes on; empty is every wake.
    pub events: Vec<WakeEvent>,
    /// Minutes between timed check-ins, when hosts turn them on.
    pub every_minutes: Option<u32>,
    /// Seconds events wait for quiet before one check-in.
    pub coalesce_secs: Option<u32>,
}
/// The request body that saves a new agent definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateAgent {
    pub request_id: String,
    pub spec: AgentSpec,
}
/// The request body that replaces an agent definition at an expected revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateAgent {
    pub request_id: String,
    pub expected_revision: u64,
    pub spec: AgentSpec,
}
/// The request body that deletes an agent definition at an expected revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteAgent {
    pub request_id: String,
    pub expected_revision: u64,
}
/// What to call a fork (#12049).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForkOptions {
    pub name: String,
}
/// One page of agent definitions and the cursor for the next page.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentPage {
    pub data: Vec<AgentDefinition>,
    pub next: Option<String>,
}

/// A saved mutation belongs to exactly one account and deployment. Keep it across retries.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentRequest<T> {
    account: String,
    deployment: Deployment,
    pub body: T,
}

/// Returns a new random request ID as 64 hexadecimal characters.
pub fn request_id() -> Result<String, Error> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| Error::InvalidReference)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Calls the agent definition and assignment routes for one signed-in session.
pub struct Agents<'a> {
    client: &'a Client,
    session: &'a Session,
}
impl Client {
    /// Returns the agents client for `session`, or an error if the session belongs to another deployment.
    pub fn agents<'a>(&'a self, session: &'a Session) -> Result<Agents<'a>, Error> {
        self.check_session(session)?;
        Ok(Agents {
            client: self,
            session,
        })
    }
}
impl Agents<'_> {
    /// Checks `spec` and prepares a request that saves it as a new agent.
    pub fn create_request(&self, spec: AgentSpec) -> Result<AgentRequest<CreateAgent>, Error> {
        spec.validate().map_err(|_| Error::InvalidReference)?;
        Ok(self.bind(CreateAgent {
            request_id: request_id()?,
            spec,
        }))
    }
    /// Checks `spec` and prepares a request that replaces the agent at `expected_revision`.
    pub fn update_request(
        &self,
        expected_revision: u64,
        spec: AgentSpec,
    ) -> Result<AgentRequest<UpdateAgent>, Error> {
        spec.validate().map_err(|_| Error::InvalidReference)?;
        Ok(self.bind(UpdateAgent {
            request_id: request_id()?,
            expected_revision,
            spec,
        }))
    }
    /// Prepares a request that deletes the agent at `expected_revision`.
    pub fn delete_request(
        &self,
        expected_revision: u64,
    ) -> Result<AgentRequest<DeleteAgent>, Error> {
        Ok(self.bind(DeleteAgent {
            request_id: request_id()?,
            expected_revision,
        }))
    }
    /// Prepares a request that forks a published agent under `options.name`.
    /// The fork's ID is the request's ID.
    pub fn fork_request(&self, options: ForkOptions) -> Result<AgentRequest<ForkAgent>, Error> {
        let name = options.name;
        if name.trim().is_empty() || name.len() > 80 || name.chars().any(char::is_control) {
            return Err(Error::InvalidReference);
        }
        Ok(self.bind(ForkAgent {
            request_id: request_id()?,
            name,
        }))
    }
    /// Prepares a request that publishes one of your agents. Pass the Agent
    /// Lab experiment it was made from; a fork may pass `None`.
    pub fn publish_request(
        &self,
        template: Option<crate::agent_lab::Experiment>,
    ) -> Result<AgentRequest<PublishDefinition>, Error> {
        Ok(self.bind(PublishDefinition {
            request_id: request_id()?,
            template,
        }))
    }
    fn bind<T>(&self, body: T) -> AgentRequest<T> {
        AgentRequest {
            account: self.session.account().id.clone(),
            deployment: self.client.deployment().clone(),
            body,
        }
    }
    async fn send<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &[&str],
        query: &[(&str, String)],
        body: Option<serde_json::Value>,
        cancel: &CancellationToken,
    ) -> Result<T, Error> {
        self.client.check_session(self.session)?;
        tokio::select! { biased;
            _=cancel.cancelled()=>Err(Error::Cancelled),
            result=self.client.request(method,path,query,body,Some(self.session.token()))=>result,
        }
    }
    fn body<T: Serialize>(&self, request: &AgentRequest<T>) -> Result<serde_json::Value, Error> {
        if request.account != self.session.account().id {
            return Err(Error::WrongAccount);
        }
        if request.deployment != *self.client.deployment() {
            return Err(Error::WrongDeployment);
        }
        serde_json::to_value(&request.body).map_err(|_| Error::InvalidReference)
    }
    /// The grants the account owner gave the agent `id` (#12044).
    pub async fn grants(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> Result<Vec<AgentGrant>, Error> {
        if !valid_id(id) {
            return Err(Error::InvalidReference);
        }
        let list: GrantList = self
            .send(
                Method::GET,
                &["agent-definitions", id, "grants"],
                &[],
                None,
                cancel,
            )
            .await?;
        Ok(list.data)
    }
    /// Grants the agent `id` the scopes `scopes` over `over`. Only the
    /// account owner can; money, keys, secrets, deploys and closing other
    /// agents' issues are refused for every agent.
    pub async fn grant(
        &self,
        id: &str,
        scopes: Vec<String>,
        over: GrantOver,
        cancel: &CancellationToken,
    ) -> Result<AgentGrant, Error> {
        if !valid_id(id) || over.validate().is_err() {
            return Err(Error::InvalidReference);
        }
        for word in &scopes {
            GrantScope::grantable(word).map_err(|_| Error::InvalidReference)?;
        }
        let body = GrantAgent {
            request_id: request_id()?,
            scopes,
            over,
        };
        let body = serde_json::to_value(body).map_err(|_| Error::InvalidReference)?;
        self.send(
            Method::POST,
            &["agent-definitions", id, "grants"],
            &[],
            Some(body),
            cancel,
        )
        .await
    }
    /// Revokes the grant `grant` of the agent `id`.
    pub async fn revoke_grant(
        &self,
        id: &str,
        grant: &str,
        cancel: &CancellationToken,
    ) -> Result<(), Error> {
        if !valid_id(id) || !valid_id(grant) {
            return Err(Error::InvalidReference);
        }
        let _: serde_json::Value = self
            .send(
                Method::DELETE,
                &["agent-definitions", id, "grants", grant],
                &[],
                None,
                cancel,
            )
            .await?;
        Ok(())
    }
    /// Sets what the agent `id` wakes on, as a new revision of its
    /// definition's `autonomy`. Wakes are definition data, so a fork copies
    /// them; grants never copy.
    pub async fn set_wakes(
        &self,
        id: &str,
        wakes: Wakes,
        cancel: &CancellationToken,
    ) -> Result<AgentDefinition, Error> {
        let current = self.get(id, None, cancel).await?;
        let mut spec = current.spec.clone();
        let mut autonomy = spec.autonomy.take().unwrap_or_default();
        autonomy.wakes = wakes.events;
        autonomy.every_minutes = wakes.every_minutes;
        autonomy.coalesce_secs = wakes.coalesce_secs;
        spec.autonomy = Some(autonomy);
        let request = self.update_request(current.revision, spec)?;
        self.update(id, &request, cancel).await
    }
    /// Lists up to `limit` agent definitions (1 to 100), starting after the cursor `after`.
    pub async fn list(
        &self,
        after: Option<&str>,
        limit: usize,
        cancel: &CancellationToken,
    ) -> Result<AgentPage, Error> {
        if !(1..=100).contains(&limit) || after.is_some_and(|a| !valid_id(a)) {
            return Err(Error::InvalidCursor);
        }
        let mut q = vec![("limit", limit.to_string())];
        if let Some(after) = after {
            q.push(("after", after.into()));
        }
        self.send(Method::GET, &["agent-definitions"], &q, None, cancel)
            .await
    }
    /// Fetches one agent definition, at `revision` when given or the latest otherwise.
    pub async fn get(
        &self,
        id: &str,
        revision: Option<u64>,
        cancel: &CancellationToken,
    ) -> Result<AgentDefinition, Error> {
        if !valid_id(id) {
            return Err(Error::InvalidReference);
        }
        let q = revision
            .map(|r| vec![("revision", r.to_string())])
            .unwrap_or_default();
        self.send(Method::GET, &["agent-definitions", id], &q, None, cancel)
            .await
    }
    /// Saves a new agent definition from a prepared request.
    pub async fn create(
        &self,
        request: &AgentRequest<CreateAgent>,
        cancel: &CancellationToken,
    ) -> Result<AgentDefinition, Error> {
        self.send(
            Method::POST,
            &["agent-definitions"],
            &[],
            Some(self.body(request)?),
            cancel,
        )
        .await
    }
    /// Replaces the agent definition `id` from a prepared request.
    pub async fn update(
        &self,
        id: &str,
        request: &AgentRequest<UpdateAgent>,
        cancel: &CancellationToken,
    ) -> Result<AgentDefinition, Error> {
        if !valid_id(id) {
            return Err(Error::InvalidReference);
        }
        self.send(
            Method::PUT,
            &["agent-definitions", id],
            &[],
            Some(self.body(request)?),
            cancel,
        )
        .await
    }
    /// Forks the published agent `published` into your agents. A fork copies
    /// the agent's settings, skills and tests; it has no grants until you
    /// give it some.
    pub async fn fork(
        &self,
        published: &str,
        request: &AgentRequest<ForkAgent>,
        cancel: &CancellationToken,
    ) -> Result<Forked, Error> {
        if !valid_id(published) {
            return Err(Error::InvalidReference);
        }
        self.send(
            Method::POST,
            &["published-agents", published, "fork"],
            &[],
            Some(self.body(request)?),
            cancel,
        )
        .await
    }
    /// Publishes the latest revision of your agent `id` for anyone to fork:
    /// a new published agent the first time, its next revision after.
    pub async fn publish(
        &self,
        id: &str,
        request: &AgentRequest<PublishDefinition>,
        cancel: &CancellationToken,
    ) -> Result<PublishedAgent, Error> {
        if !valid_id(id) {
            return Err(Error::InvalidReference);
        }
        self.send(
            Method::POST,
            &["agent-definitions", id, "publish"],
            &[],
            Some(self.body(request)?),
            cancel,
        )
        .await
    }
    /// Where your fork `id` came from, and the original's newer revision
    /// when it has published one since.
    pub async fn upstream(&self, id: &str, cancel: &CancellationToken) -> Result<Upstream, Error> {
        if !valid_id(id) {
            return Err(Error::InvalidReference);
        }
        self.send(
            Method::GET,
            &["agent-definitions", id, "upstream"],
            &[],
            None,
            cancel,
        )
        .await
    }
    /// Deletes the agent definition `id` from a prepared request.
    pub async fn delete(
        &self,
        id: &str,
        request: &AgentRequest<DeleteAgent>,
        cancel: &CancellationToken,
    ) -> Result<AgentDefinition, Error> {
        if !valid_id(id) {
            return Err(Error::InvalidReference);
        }
        self.send(
            Method::DELETE,
            &["agent-definitions", id],
            &[],
            Some(self.body(request)?),
            cancel,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn definitions_reject_commands_paths_controls_and_oversized_inputs() {
        let mut spec = AgentSpec {
            name: "Review helper".into(),
            description: String::new(),
            instructions: "Review the project".into(),
            runtime: "claude-code".into(),
            model: None,
            access: Access::ReadOnly,
            fallbacks: vec![],
            autonomy: None,
        };
        assert!(spec.validate().is_ok());
        for runtime in [
            "/bin/sh",
            "sh -c malicious",
            "https://unknown.invalid",
            "",
            "bad\nvalue",
        ] {
            spec.runtime = runtime.into();
            assert!(spec.validate().is_err());
        }
        spec.runtime = "sdk:example-reviewer".into();
        assert!(spec.validate().is_ok());
        spec.instructions = "x".repeat(MAX_INSTRUCTIONS_BYTES + 1);
        assert!(spec.validate().is_err());
        let mut value = serde_json::to_value(spec).unwrap();
        value["command"] = serde_json::json!("sh");
        assert!(serde_json::from_value::<AgentSpec>(value).is_err());
        for value in ["0".repeat(63), "A".repeat(64), "/".repeat(64)] {
            assert!(!valid_id(&value));
        }
        assert!(valid_id(&request_id().unwrap()));
    }
}
