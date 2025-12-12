//! Agent capabilities model.
//!
//! Capabilities define what an agent can do - the tools it can use,
//! the job types it can handle, and the skills it possesses.
//!
//! This model enables:
//! - **Discovery**: Find agents by capability
//! - **Matching**: Route jobs to capable agents
//! - **Permission**: Control what agents are allowed to do
//! - **Composition**: Combine agents with complementary capabilities

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

/// What an agent can do.
///
/// Capabilities are declarative - they describe the agent's abilities
/// without specifying how they're implemented.
///
/// # Example
///
/// ```rust,ignore
/// let capabilities = AgentCapabilities::builder()
///     .add_tool(ToolCapability {
///         name: "read_file".into(),
///         description: "Read contents of a file".into(),
///         input_schema: json!({
///             "type": "object",
///             "properties": {
///                 "path": { "type": "string" }
///             }
///         }),
///         requires_permission: true,
///     })
///     .add_job_kind(KIND_JOB_TEXT_GENERATION)
///     .add_skill("code-generation")
///     .add_skill("code-review")
///     .build();
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentCapabilities {
    /// Tools this agent can use.
    ///
    /// Tools are discrete functions the agent can invoke,
    /// like reading files, making HTTP requests, or running commands.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolCapability>,

    /// NIP-90 job kinds this agent can handle.
    ///
    /// These correspond to DVM job request kinds (5000-5999).
    /// For example, 5050 for text generation, 5001 for summarization.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub job_kinds: Vec<u16>,

    /// Supported input types.
    ///
    /// What kinds of input the agent can accept.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<InputCapability>,

    /// Supported output types.
    ///
    /// What kinds of output the agent can produce.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<OutputCapability>,

    /// Skill categories.
    ///
    /// High-level descriptions of what the agent is good at.
    /// Used for discovery and matching.
    #[serde(default, skip_serializing_if = "HashSet::is_empty")]
    pub skills: HashSet<String>,

    /// Model capabilities.
    ///
    /// If the agent uses an LLM, this describes the model's capabilities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelCapability>,

    /// Maximum concurrent jobs.
    ///
    /// How many jobs this agent can process simultaneously.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_concurrent_jobs: Option<u32>,

    /// Additional capability metadata.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub extra: Value,
}

impl AgentCapabilities {
    /// Create a new empty capabilities set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a builder for constructing capabilities.
    pub fn builder() -> AgentCapabilitiesBuilder {
        AgentCapabilitiesBuilder::default()
    }

    /// Check if this agent can handle a specific job kind.
    pub fn can_handle_job(&self, kind: u16) -> bool {
        self.job_kinds.contains(&kind)
    }

    /// Check if this agent has a specific tool.
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.iter().any(|t| t.name == name)
    }

    /// Check if this agent has a specific skill.
    pub fn has_skill(&self, skill: &str) -> bool {
        self.skills.contains(skill)
    }

    /// Get a tool by name.
    pub fn tool(&self, name: &str) -> Option<&ToolCapability> {
        self.tools.iter().find(|t| t.name == name)
    }

    /// Check if capabilities are empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
            && self.job_kinds.is_empty()
            && self.inputs.is_empty()
            && self.outputs.is_empty()
            && self.skills.is_empty()
    }

    /// Merge with another set of capabilities.
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

/// Builder for AgentCapabilities.
#[derive(Default)]
pub struct AgentCapabilitiesBuilder {
    capabilities: AgentCapabilities,
}

impl AgentCapabilitiesBuilder {
    /// Add a tool capability.
    pub fn add_tool(mut self, tool: ToolCapability) -> Self {
        self.capabilities.tools.push(tool);
        self
    }

    /// Add a job kind this agent can handle.
    pub fn add_job_kind(mut self, kind: u16) -> Self {
        if !self.capabilities.job_kinds.contains(&kind) {
            self.capabilities.job_kinds.push(kind);
        }
        self
    }

    /// Add an input capability.
    pub fn add_input(mut self, input: InputCapability) -> Self {
        self.capabilities.inputs.push(input);
        self
    }

    /// Add an output capability.
    pub fn add_output(mut self, output: OutputCapability) -> Self {
        self.capabilities.outputs.push(output);
        self
    }

    /// Add a skill.
    pub fn add_skill(mut self, skill: impl Into<String>) -> Self {
        self.capabilities.skills.insert(skill.into());
        self
    }

    /// Set model capabilities.
    pub fn model(mut self, model: ModelCapability) -> Self {
        self.capabilities.model = Some(model);
        self
    }

    /// Set maximum concurrent jobs.
    pub fn max_concurrent_jobs(mut self, max: u32) -> Self {
        self.capabilities.max_concurrent_jobs = Some(max);
        self
    }

    /// Build the capabilities.
    pub fn build(self) -> AgentCapabilities {
        self.capabilities
    }
}

/// A tool the agent can use.
///
/// Tools are discrete functions that the agent can invoke during execution.
/// They have a defined input schema and may require user permission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCapability {
    /// Unique name of the tool (e.g., "read_file", "bash", "web_search").
    pub name: String,

    /// Human-readable description of what the tool does.
    pub description: String,

    /// JSON Schema for the tool's input parameters.
    ///
    /// Used for validation and documentation.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub input_schema: Value,

    /// Whether this tool requires explicit user permission.
    ///
    /// Tools that modify state, access sensitive data, or have
    /// side effects should require permission.
    #[serde(default)]
    pub requires_permission: bool,

    /// Categories this tool belongs to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<ToolCategory>,

    /// Whether this tool is dangerous (e.g., can delete files).
    #[serde(default)]
    pub dangerous: bool,

    /// Whether this tool has network access.
    #[serde(default)]
    pub has_network_access: bool,

    /// Whether this tool can modify the filesystem.
    #[serde(default)]
    pub can_modify_filesystem: bool,
}

impl ToolCapability {
    /// Create a new tool capability.
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

    /// Set the input schema.
    pub fn with_schema(mut self, schema: Value) -> Self {
        self.input_schema = schema;
        self
    }

    /// Mark as requiring permission.
    pub fn requires_permission(mut self) -> Self {
        self.requires_permission = true;
        self
    }

    /// Mark as dangerous.
    pub fn dangerous(mut self) -> Self {
        self.dangerous = true;
        self.requires_permission = true; // Dangerous tools always require permission
        self
    }

    /// Mark as having network access.
    pub fn with_network_access(mut self) -> Self {
        self.has_network_access = true;
        self
    }

    /// Mark as modifying filesystem.
    pub fn with_filesystem_access(mut self) -> Self {
        self.can_modify_filesystem = true;
        self
    }

    /// Add a category.
    pub fn with_category(mut self, category: ToolCategory) -> Self {
        self.categories.push(category);
        self
    }
}

/// Categories of tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCategory {
    /// File operations (read, write, delete).
    Filesystem,
    /// Command execution (bash, terminal).
    Execution,
    /// Network operations (HTTP, WebSocket).
    Network,
    /// Search operations (grep, glob, web search).
    Search,
    /// Code analysis and manipulation.
    Code,
    /// Git operations.
    Git,
    /// Database operations.
    Database,
    /// AI/ML operations (embedding, inference).
    Ai,
    /// Communication (chat, notifications).
    Communication,
    /// System operations.
    System,
    /// Custom category.
    Custom,
}

/// Input type the agent can accept.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputCapability {
    /// Type identifier (e.g., "text", "url", "event", "job").
    ///
    /// Corresponds to NIP-90 input types.
    pub input_type: InputType,

    /// MIME types accepted (for "url" type).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mime_types: Vec<String>,

    /// Maximum input size in bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_size: Option<u64>,

    /// Description of what this input is used for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Input type identifiers (NIP-90 compatible).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    /// Direct text input.
    Text,
    /// URL to fetch data from.
    Url,
    /// Nostr event ID.
    Event,
    /// Output from a previous job.
    Job,
    /// Binary data.
    Binary,
    /// Image data.
    Image,
    /// Audio data.
    Audio,
    /// Video data.
    Video,
}

impl InputType {
    /// Get the string representation.
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

/// Output type the agent can produce.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputCapability {
    /// MIME type of the output (e.g., "text/plain", "application/json").
    pub mime_type: String,

    /// Description of the output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Whether output is streamed.
    #[serde(default)]
    pub streaming: bool,
}

/// Model capabilities (if agent uses an LLM).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapability {
    /// Model name or ID.
    pub model: String,

    /// Provider (e.g., "anthropic", "openai", "local").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,

    /// Maximum context length in tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u32>,

    /// Maximum output tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,

    /// Supports vision/images.
    #[serde(default)]
    pub supports_vision: bool,

    /// Supports tool use.
    #[serde(default)]
    pub supports_tools: bool,

    /// Supports streaming.
    #[serde(default)]
    pub supports_streaming: bool,
}

// Common job kinds (NIP-90)
/// Text extraction / OCR
pub const KIND_JOB_TEXT_EXTRACTION: u16 = 5000;
/// Summarization
pub const KIND_JOB_SUMMARIZATION: u16 = 5001;
/// Translation
pub const KIND_JOB_TRANSLATION: u16 = 5002;
/// Text generation / Chat
pub const KIND_JOB_TEXT_GENERATION: u16 = 5050;
/// Image generation
pub const KIND_JOB_IMAGE_GENERATION: u16 = 5100;
/// Speech to text
pub const KIND_JOB_SPEECH_TO_TEXT: u16 = 5250;

// Common skills
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
    use serde_json::json;

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
    fn test_tool_capability() {
        let tool = ToolCapability::new("bash", "Execute bash commands")
            .with_schema(json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string" }
                }
            }))
            .dangerous()
            .with_category(ToolCategory::Execution);

        assert!(tool.dangerous);
        assert!(tool.requires_permission);
        assert!(tool.categories.contains(&ToolCategory::Execution));
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

    #[test]
    fn test_capabilities_merge() {
        let mut caps1 = AgentCapabilities::builder()
            .add_skill("coding")
            .add_job_kind(KIND_JOB_TEXT_GENERATION)
            .build();

        let caps2 = AgentCapabilities::builder()
            .add_skill("research")
            .add_job_kind(KIND_JOB_SUMMARIZATION)
            .build();

        caps1.merge(caps2);

        assert!(caps1.has_skill("coding"));
        assert!(caps1.has_skill("research"));
        assert!(caps1.can_handle_job(KIND_JOB_TEXT_GENERATION));
        assert!(caps1.can_handle_job(KIND_JOB_SUMMARIZATION));
    }
}
