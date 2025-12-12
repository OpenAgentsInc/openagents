//! Agent capabilities model.
//!
//! Capabilities define what an agent can do - the tools it can use,
//! the job types it can handle, and the skills it possesses.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

/// What an agent can do.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentCapabilities {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolCapability>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub job_kinds: Vec<u16>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<InputCapability>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<OutputCapability>,

    #[serde(default, skip_serializing_if = "HashSet::is_empty")]
    pub skills: HashSet<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelCapability>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_concurrent_jobs: Option<u32>,

    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub extra: Value,
}

impl AgentCapabilities {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn builder() -> AgentCapabilitiesBuilder {
        AgentCapabilitiesBuilder::default()
    }

    pub fn can_handle_job(&self, kind: u16) -> bool {
        self.job_kinds.contains(&kind)
    }

    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.iter().any(|t| t.name == name)
    }

    pub fn has_skill(&self, skill: &str) -> bool {
        self.skills.contains(skill)
    }

    pub fn tool(&self, name: &str) -> Option<&ToolCapability> {
        self.tools.iter().find(|t| t.name == name)
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
            && self.job_kinds.is_empty()
            && self.inputs.is_empty()
            && self.outputs.is_empty()
            && self.skills.is_empty()
    }

    pub fn merge(&mut self, other: AgentCapabilities) {
        self.tools.extend(other.tools);
        self.job_kinds.extend(other.job_kinds);
        self.inputs.extend(other.inputs);
        self.outputs.extend(other.outputs);
        self.skills.extend(other.skills);

        if self.model.is_none() {
            self.model = other.model;
        }

        if self.max_concurrent_jobs.is_none() {
            self.max_concurrent_jobs = other.max_concurrent_jobs;
        }
    }
}

#[derive(Default)]
pub struct AgentCapabilitiesBuilder {
    capabilities: AgentCapabilities,
}

impl AgentCapabilitiesBuilder {
    pub fn add_tool(mut self, tool: ToolCapability) -> Self {
        self.capabilities.tools.push(tool);
        self
    }

    pub fn add_job_kind(mut self, kind: u16) -> Self {
        if !self.capabilities.job_kinds.contains(&kind) {
            self.capabilities.job_kinds.push(kind);
        }
        self
    }

    pub fn add_input(mut self, input: InputCapability) -> Self {
        self.capabilities.inputs.push(input);
        self
    }

    pub fn add_output(mut self, output: OutputCapability) -> Self {
        self.capabilities.outputs.push(output);
        self
    }

    pub fn add_skill(mut self, skill: impl Into<String>) -> Self {
        self.capabilities.skills.insert(skill.into());
        self
    }

    pub fn model(mut self, model: ModelCapability) -> Self {
        self.capabilities.model = Some(model);
        self
    }

    pub fn max_concurrent_jobs(mut self, max: u32) -> Self {
        self.capabilities.max_concurrent_jobs = Some(max);
        self
    }

    pub fn build(self) -> AgentCapabilities {
        self.capabilities
    }
}

/// A tool the agent can use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCapability {
    pub name: String,
    pub description: String,

    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub input_schema: Value,

    #[serde(default)]
    pub requires_permission: bool,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<ToolCategory>,

    #[serde(default)]
    pub dangerous: bool,

    #[serde(default)]
    pub has_network_access: bool,

    #[serde(default)]
    pub can_modify_filesystem: bool,
}

impl ToolCapability {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema: Value::Null,
            requires_permission: false,
            categories: Vec::new(),
            dangerous: false,
            has_network_access: false,
            can_modify_filesystem: false,
        }
    }

    pub fn with_schema(mut self, schema: Value) -> Self {
        self.input_schema = schema;
        self
    }

    pub fn requires_permission(mut self) -> Self {
        self.requires_permission = true;
        self
    }

    pub fn dangerous(mut self) -> Self {
        self.dangerous = true;
        self.requires_permission = true;
        self
    }

    pub fn with_network_access(mut self) -> Self {
        self.has_network_access = true;
        self
    }

    pub fn with_filesystem_access(mut self) -> Self {
        self.can_modify_filesystem = true;
        self
    }

    pub fn with_category(mut self, category: ToolCategory) -> Self {
        self.categories.push(category);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCategory {
    Filesystem,
    Execution,
    Network,
    Search,
    Code,
    Git,
    Database,
    Ai,
    Communication,
    System,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputCapability {
    pub input_type: InputType,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mime_types: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_size: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    Text,
    Url,
    Event,
    Job,
    Binary,
    Image,
    Audio,
    Video,
}

impl InputType {
    pub fn as_str(&self) -> &'static str {
        match self {
            InputType::Text => "text",
            InputType::Url => "url",
            InputType::Event => "event",
            InputType::Job => "job",
            InputType::Binary => "binary",
            InputType::Image => "image",
            InputType::Audio => "audio",
            InputType::Video => "video",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputCapability {
    pub mime_type: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(default)]
    pub streaming: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapability {
    pub model: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,

    #[serde(default)]
    pub supports_vision: bool,

    #[serde(default)]
    pub supports_tools: bool,

    #[serde(default)]
    pub supports_streaming: bool,
}

pub const KIND_JOB_TEXT_EXTRACTION: u16 = 5000;
pub const KIND_JOB_SUMMARIZATION: u16 = 5001;
pub const KIND_JOB_TRANSLATION: u16 = 5002;
pub const KIND_JOB_TEXT_GENERATION: u16 = 5050;
pub const KIND_JOB_IMAGE_GENERATION: u16 = 5100;
pub const KIND_JOB_SPEECH_TO_TEXT: u16 = 5250;

pub const SKILL_CODE_GENERATION: &str = "code-generation";
pub const SKILL_CODE_REVIEW: &str = "code-review";
pub const SKILL_DEBUGGING: &str = "debugging";
pub const SKILL_REFACTORING: &str = "refactoring";
pub const SKILL_TESTING: &str = "testing";
pub const SKILL_DOCUMENTATION: &str = "documentation";
pub const SKILL_RESEARCH: &str = "research";
pub const SKILL_ANALYSIS: &str = "analysis";
pub const SKILL_SUMMARIZATION: &str = "summarization";
pub const SKILL_TRANSLATION: &str = "translation";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capabilities_builder() {
        let caps = AgentCapabilities::builder()
            .add_tool(ToolCapability::new("read_file", "Read a file"))
            .add_job_kind(KIND_JOB_TEXT_GENERATION)
            .add_skill(SKILL_CODE_GENERATION)
            .add_skill(SKILL_CODE_REVIEW)
            .max_concurrent_jobs(4)
            .build();

        assert_eq!(caps.tools.len(), 1);
        assert!(caps.can_handle_job(KIND_JOB_TEXT_GENERATION));
        assert!(caps.has_skill(SKILL_CODE_GENERATION));
        assert!(caps.has_tool("read_file"));
        assert!(!caps.has_tool("write_file"));
    }

    #[test]
    fn test_capabilities_serialization() {
        let caps = AgentCapabilities::builder()
            .add_job_kind(KIND_JOB_TEXT_GENERATION)
            .add_skill("coding")
            .build();

        let json = serde_json::to_string(&caps).unwrap();
        let deserialized: AgentCapabilities = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.job_kinds, caps.job_kinds);
        assert_eq!(deserialized.skills, caps.skills);
    }
}
