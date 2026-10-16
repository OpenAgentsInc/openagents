//! Private, revisioned agent definitions. Saving a definition never starts work.
use crate::{CancellationToken, Client, Deployment, Error, Session};
use reqwest::Method;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Runtime IDs resolve only to implementations installed and authorized by a host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSpec {
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub runtime: String,
    pub model: Option<String>,
    pub access: Access,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    ReadOnly,
    WorkspaceWrite,
}

impl AgentSpec {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.name.trim().is_empty()
            || self.name.len() > 80
            || self.name.chars().any(char::is_control)
        {
            return Err("Enter a short name.");
        }
        if self.description.len() > 512 || self.description.chars().any(char::is_control) {
            return Err("Enter a shorter description.");
        }
        if self.instructions.trim().is_empty()
            || self.instructions.len() > 16_384
            || self.instructions.contains('\0')
        {
            return Err("Add instructions or shorten the text.");
        }
        if self.runtime.is_empty()
            || self.runtime.len() > 128
            || !self
                .runtime
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
        {
            return Err("Choose a supported runtime.");
        }
        if self
            .model
            .as_ref()
            .is_some_and(|m| m.is_empty() || m.len() > 256 || m.chars().any(char::is_control))
        {
            return Err("Choose a supported model.");
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentDefinition {
    pub id: String,
    pub revision: u64,
    pub spec: AgentSpec,
    pub deleted: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateAgent {
    pub request_id: String,
    pub spec: AgentSpec,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateAgent {
    pub request_id: String,
    pub expected_revision: u64,
    pub spec: AgentSpec,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteAgent {
    pub request_id: String,
    pub expected_revision: u64,
}
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

pub fn valid_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub fn request_id() -> Result<String, Error> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| Error::InvalidReference)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

pub struct Agents<'a> {
    client: &'a Client,
    session: &'a Session,
}
impl Client {
    pub fn agents<'a>(&'a self, session: &'a Session) -> Result<Agents<'a>, Error> {
        self.check_session(session)?;
        Ok(Agents {
            client: self,
            session,
        })
    }
}
impl Agents<'_> {
    pub fn create_request(&self, spec: AgentSpec) -> Result<AgentRequest<CreateAgent>, Error> {
        spec.validate().map_err(|_| Error::InvalidReference)?;
        Ok(self.bind(CreateAgent {
            request_id: request_id()?,
            spec,
        }))
    }
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
    pub fn delete_request(
        &self,
        expected_revision: u64,
    ) -> Result<AgentRequest<DeleteAgent>, Error> {
        Ok(self.bind(DeleteAgent {
            request_id: request_id()?,
            expected_revision,
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
        spec.instructions = "x".repeat(16385);
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
