//! Agent execution requirements.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionEnvironment {
    #[default]
    Local,
    Cloud { provider: CloudProvider },
    Swarm,
    Hybrid { prefer: ExecutionPreference },
    Oanix { namespace: OanixNamespace },
}

impl ExecutionEnvironment {
    pub fn compatible_with(&self, other: &ExecutionEnvironment) -> bool {
        match (self, other) {
            (ExecutionEnvironment::Local, ExecutionEnvironment::Local) => true,
            (ExecutionEnvironment::Local, ExecutionEnvironment::Hybrid { .. }) => true,
            (ExecutionEnvironment::Cloud { provider: p1 }, ExecutionEnvironment::Cloud { provider: p2 }) => p1 == p2,
            (ExecutionEnvironment::Cloud { .. }, ExecutionEnvironment::Hybrid { .. }) => true,
            (ExecutionEnvironment::Swarm, ExecutionEnvironment::Swarm) => true,
            (ExecutionEnvironment::Swarm, ExecutionEnvironment::Hybrid { .. }) => true,
            (ExecutionEnvironment::Hybrid { .. }, _) => true,
            (ExecutionEnvironment::Oanix { .. }, ExecutionEnvironment::Oanix { .. }) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudProvider {
    Anthropic,
    OpenAi,
    Google,
    AwsBedrock,
    Azure,
    Custom(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPreference {
    #[default]
    PreferLocal,
    PreferCloud,
    PreferSwarm,
    LowestCost,
    Fastest,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct OanixNamespace {
    #[serde(default)]
    pub mounts: Vec<OanixMount>,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OanixMount {
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub options: HashMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentRequirements {
    #[serde(default)]
    pub environment: ExecutionEnvironment,
    #[serde(default)]
    pub resources: ResourceRequirements,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelRequirement>,
    #[serde(default)]
    pub sandbox: SandboxConfig,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env_vars: HashMap<String, EnvVarRequirement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<Dependency>,
}

impl AgentRequirements {
    pub fn new() -> Self { Self::default() }
    pub fn builder() -> AgentRequirementsBuilder { AgentRequirementsBuilder::default() }
}

#[derive(Default)]
pub struct AgentRequirementsBuilder {
    requirements: AgentRequirements,
}

impl AgentRequirementsBuilder {
    pub fn environment(mut self, env: ExecutionEnvironment) -> Self {
        self.requirements.environment = env;
        self
    }
    pub fn build(self) -> AgentRequirements { self.requirements }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceRequirements {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_memory: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_memory: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<CpuRequirement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu: Option<GpuRequirement>,
    #[serde(default)]
    pub network: NetworkAccess,
    #[serde(default)]
    pub filesystem: FilesystemAccess,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_space: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_execution_time: Option<u64>,
}

impl ResourceRequirements {
    pub fn satisfied_by(&self, available: &AvailableResources) -> bool {
        if let Some(min_mem) = self.min_memory {
            if available.memory < min_mem { return false; }
        }
        if let Some(disk_req) = self.disk_space {
            if available.disk_space < disk_req { return false; }
        }
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuRequirement {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_cores: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_clock_mhz: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuRequirement {
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_vram: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<GpuType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_compute_capability: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuType {
    Cuda,
    Metal,
    AppleNeuralEngine,
    Rocm,
    IntelArc,
    Vulkan,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkAccess {
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_domains: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_domains: Vec<String>,
    #[serde(default)]
    pub websocket: bool,
    #[serde(default)]
    pub nostr: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FilesystemAccess {
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_paths: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub write_paths: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequirement {
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_context_length: Option<u32>,
    #[serde(default)]
    pub requires_vision: bool,
    #[serde(default)]
    pub requires_tools: bool,
}

impl ModelRequirement {
    pub fn satisfied_by(&self, model: &AvailableModel) -> bool {
        if self.model.ends_with('*') {
            let prefix = &self.model[..self.model.len() - 1];
            if !model.name.starts_with(prefix) { return false; }
        } else if self.model != model.name {
            return false;
        }
        if let Some(required_provider) = &self.provider {
            if model.provider.as_ref() != Some(required_provider) { return false; }
        }
        if let Some(min_ctx) = self.min_context_length {
            if model.context_length.unwrap_or(0) < min_ctx { return false; }
        }
        if self.requires_vision && !model.supports_vision { return false; }
        if self.requires_tools && !model.supports_tools { return false; }
        true
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SandboxConfig {
    #[serde(default)]
    pub mode: SandboxMode,
    #[serde(default)]
    pub network_isolation: bool,
    #[serde(default)]
    pub filesystem_isolation: bool,
    #[serde(default)]
    pub process_isolation: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_limits: Option<ResourceLimits>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxMode {
    None,
    #[default]
    Light,
    Full,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_memory: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cpu_time: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_processes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_open_files: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVarRequirement {
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub secret: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default)]
    pub kind: DependencyKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    #[default]
    System,
    Python,
    Node,
    Rust,
    Container,
}

#[derive(Debug, Clone, Default)]
pub struct HostCapabilities {
    pub environment: ExecutionEnvironment,
    pub resources: AvailableResources,
    pub available_models: Vec<AvailableModel>,
}

#[derive(Debug, Clone, Default)]
pub struct AvailableResources {
    pub memory: u64,
    pub gpu: Option<AvailableGpu>,
    pub disk_space: u64,
}

#[derive(Debug, Clone)]
pub struct AvailableGpu {
    pub gpu_type: GpuType,
    pub vram: u64,
}

#[derive(Debug, Clone)]
pub struct AvailableModel {
    pub name: String,
    pub provider: Option<String>,
    pub context_length: Option<u32>,
    pub supports_vision: bool,
    pub supports_tools: bool,
}
