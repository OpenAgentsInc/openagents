//! Shared agent definitions and NIP-AGENT payload validation. No execution or I/O.
use serde::{Deserialize, Serialize};
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
    HostDefault,
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
        if self.runtime == "sdk" && (self.model.is_none() || self.access != Access::HostDefault) {
            return Err("Choose an installed runner and this computer's access settings.");
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
pub fn valid_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub const SCHEMA: &str = "openagents.agent.definition.v1";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: String,
    pub definition: AgentDefinition,
}
impl Profile {
    pub fn new(definition: AgentDefinition) -> Result<Self, &'static str> {
        let p = Self {
            schema: SCHEMA.into(),
            definition,
        };
        p.validate()?;
        Ok(p)
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != SCHEMA || !valid_id(&self.definition.id) || self.definition.revision == 0
        {
            return Err("Invalid agent profile.");
        }
        self.definition.spec.validate()
    }
}
