//! Agent execution requirements.
//!
//! Requirements define what an agent needs to run - the execution environment,
//! compute resources, model access, and sandbox configuration.
//!
//! This enables:
//! - **Matching**: Find suitable hosts for agent execution
//! - **Resource allocation**: Reserve appropriate compute
//! - **Security**: Sandbox agents appropriately
//! - **Cost estimation**: Predict resource costs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// What an agent needs to run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentRequirements {
    /// Execution environment.
    #[serde(default)]
    pub environment: ExecutionEnvironment,

    /// Resource requirements.
    #[serde(default)]
    pub resources: ResourceRequirements,

    /// Model requirements (if agent uses an LLM).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelRequirement>,

    /// Sandbox configuration.
    #[serde(default)]
    pub sandbox: SandboxConfig,

    /// Required environment variables.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env_vars: HashMap<String, EnvVarRequirement>,

    /// Required runtime dependencies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<Dependency>,
}

impl AgentRequirements {
    /// Create new requirements with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a builder for requirements.
    pub fn builder() -> AgentRequirementsBuilder {
        AgentRequirementsBuilder::default()
    }

    /// Check if these requirements can be satisfied by a host.
    pub fn can_run_on(&self, host: &HostCapabilities) -> bool {
        // Check environment compatibility
        if !self.environment.compatible_with(&host.environment) {
            return false;
        }

        // Check resource availability
        if !self.resources.satisfied_by(&host.resources) {
            return false;
        }

        // Check model availability
        if let Some(model_req) = &self.model {
            if !host.available_models.iter().any(|m| model_req.satisfied_by(m)) {
                return false;
            }
        }

        true
    }
}

/// Builder for AgentRequirements.
#[derive(Default)]
pub struct AgentRequirementsBuilder {
    requirements: AgentRequirements,
}

impl AgentRequirementsBuilder {
    /// Set the execution environment.
    pub fn environment(mut self, env: ExecutionEnvironment) -> Self {
        self.requirements.environment = env;
        self
    }

    /// Set resource requirements.
    pub fn resources(mut self, resources: ResourceRequirements) -> Self {
        self.requirements.resources = resources;
        self
    }

    /// Set model requirement.
    pub fn model(mut self, model: ModelRequirement) -> Self {
        self.requirements.model = Some(model);
        self
    }

    /// Set sandbox configuration.
    pub fn sandbox(mut self, sandbox: SandboxConfig) -> Self {
        self.requirements.sandbox = sandbox;
        self
    }

    /// Add a required environment variable.
    pub fn env_var(mut self, name: impl Into<String>, requirement: EnvVarRequirement) -> Self {
        self.requirements.env_vars.insert(name.into(), requirement);
        self
    }

    /// Add a dependency.
    pub fn dependency(mut self, dep: Dependency) -> Self {
        self.requirements.dependencies.push(dep);
        self
    }

    /// Build the requirements.
    pub fn build(self) -> AgentRequirements {
        self.requirements
    }
}

/// Where the agent runs.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionEnvironment {
    /// Run locally on device.
    ///
    /// Uses local compute resources (CPU, GPU, Apple Neural Engine).
    /// Best for privacy-sensitive workloads or offline operation.
    #[default]
    Local,

    /// Run on a cloud provider.
    ///
    /// Uses cloud compute (Anthropic, OpenAI, etc.).
    /// Best for high-performance workloads.
    Cloud {
        /// Cloud provider identifier.
        provider: CloudProvider,
    },

    /// Run on the swarm compute network.
    ///
    /// Uses distributed compute from other devices.
    /// Best for cost-effective bulk workloads.
    Swarm,

    /// Can run either locally or in the cloud.
    ///
    /// Falls back to cloud if local resources insufficient.
    Hybrid {
        /// Preferred execution location.
        prefer: ExecutionPreference,
    },

    /// Run in an OANIX container.
    ///
    /// Sandboxed execution with filesystem-based capabilities.
    Oanix {
        /// Namespace configuration.
        namespace: OanixNamespace,
    },
}

impl ExecutionEnvironment {
    /// Check if this environment is compatible with another.
    pub fn compatible_with(&self, other: &ExecutionEnvironment) -> bool {
        match (self, other) {
            // Local can only run on local
            (ExecutionEnvironment::Local, ExecutionEnvironment::Local) => true,
            (ExecutionEnvironment::Local, ExecutionEnvironment::Hybrid { .. }) => true,

            // Cloud can run on matching cloud or hybrid
            (ExecutionEnvironment::Cloud { provider: p1 }, ExecutionEnvironment::Cloud { provider: p2 }) => p1 == p2,
            (ExecutionEnvironment::Cloud { .. }, ExecutionEnvironment::Hybrid { .. }) => true,

            // Swarm can run on swarm
            (ExecutionEnvironment::Swarm, ExecutionEnvironment::Swarm) => true,
            (ExecutionEnvironment::Swarm, ExecutionEnvironment::Hybrid { .. }) => true,

            // Hybrid is flexible
            (ExecutionEnvironment::Hybrid { .. }, _) => true,

            // OANIX requires OANIX
            (ExecutionEnvironment::Oanix { .. }, ExecutionEnvironment::Oanix { .. }) => true,

            _ => false,
        }
    }
}

/// Cloud provider identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudProvider {
    /// Anthropic (Claude).
    Anthropic,
    /// OpenAI (GPT).
    OpenAi,
    /// Google (Gemini).
    Google,
    /// AWS Bedrock.
    AwsBedrock,
    /// Azure OpenAI.
    Azure,
    /// Custom cloud provider.
    Custom(String),
}

/// Preference for hybrid execution.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPreference {
    /// Prefer local execution, fall back to cloud.
    #[default]
    PreferLocal,
    /// Prefer cloud execution, fall back to local.
    PreferCloud,
    /// Prefer swarm execution.
    PreferSwarm,
    /// Lowest cost option.
    LowestCost,
    /// Fastest option.
    Fastest,
}

/// OANIX namespace configuration.
///
/// OANIX provides Plan 9-style filesystem-based capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct OanixNamespace {
    /// Mount points for capabilities.
    #[serde(default)]
    pub mounts: Vec<OanixMount>,

    /// Allowed capability paths.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// OANIX mount point.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OanixMount {
    /// Source (e.g., "http", "ws", "nostr").
    pub source: String,
    /// Mount point (e.g., "/cap/http").
    pub target: String,
    /// Mount options.
    #[serde(default)]
    pub options: HashMap<String, String>,
}

/// Resource requirements.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceRequirements {
    /// Minimum memory in bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_memory: Option<u64>,

    /// Maximum memory in bytes (for cost control).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_memory: Option<u64>,

    /// CPU requirements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<CpuRequirement>,

    /// GPU requirements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu: Option<GpuRequirement>,

    /// Network access requirements.
    #[serde(default)]
    pub network: NetworkAccess,

    /// Filesystem access requirements.
    #[serde(default)]
    pub filesystem: FilesystemAccess,

    /// Disk space requirements in bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_space: Option<u64>,

    /// Maximum execution time in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_execution_time: Option<u64>,
}

impl ResourceRequirements {
    /// Check if these requirements are satisfied by available resources.
    pub fn satisfied_by(&self, available: &AvailableResources) -> bool {
        // Check memory
        if let Some(min_mem) = self.min_memory {
            if available.memory < min_mem {
                return false;
            }
        }

        // Check GPU
        if let Some(gpu_req) = &self.gpu {
            match &available.gpu {
                Some(gpu) => {
                    if let Some(min_vram) = gpu_req.min_vram {
                        if gpu.vram < min_vram {
                            return false;
                        }
                    }
                }
                None if gpu_req.required => return false,
                None => {}
            }
        }

        // Check disk
        if let Some(disk_req) = self.disk_space {
            if available.disk_space < disk_req {
                return false;
            }
        }

        true
    }
}

/// CPU requirements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuRequirement {
    /// Minimum number of cores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_cores: Option<u32>,

    /// Minimum clock speed in MHz.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_clock_mhz: Option<u32>,

    /// Required CPU features (e.g., "avx2", "neon").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
}

/// GPU requirements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuRequirement {
    /// Whether GPU is required (vs optional).
    #[serde(default)]
    pub required: bool,

    /// Minimum VRAM in bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_vram: Option<u64>,

    /// Acceptable GPU types.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<GpuType>,

    /// Required compute capability (for CUDA).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_compute_capability: Option<String>,
}

/// GPU types.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuType {
    /// NVIDIA CUDA GPU.
    Cuda,
    /// Apple Metal GPU.
    Metal,
    /// Apple Neural Engine.
    AppleNeuralEngine,
    /// AMD ROCm GPU.
    Rocm,
    /// Intel Arc GPU.
    IntelArc,
    /// Vulkan compute.
    Vulkan,
}

/// Network access requirements.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkAccess {
    /// Whether network access is needed.
    #[serde(default)]
    pub required: bool,

    /// Allowed domains (empty = all allowed if required).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_domains: Vec<String>,

    /// Blocked domains.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_domains: Vec<String>,

    /// Whether WebSocket access is needed.
    #[serde(default)]
    pub websocket: bool,

    /// Whether Nostr relay access is needed.
    #[serde(default)]
    pub nostr: bool,
}

/// Filesystem access requirements.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FilesystemAccess {
    /// Whether filesystem access is needed.
    #[serde(default)]
    pub required: bool,

    /// Read-only access to these paths.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_paths: Vec<PathBuf>,

    /// Read-write access to these paths.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub write_paths: Vec<PathBuf>,

    /// Working directory requirement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<PathBuf>,
}

/// Model requirements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequirement {
    /// Required model name or pattern.
    ///
    /// Can be exact ("claude-3-5-sonnet") or pattern ("claude-*").
    pub model: String,

    /// Required provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,

    /// Minimum context length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_context_length: Option<u32>,

    /// Whether vision capability is required.
    #[serde(default)]
    pub requires_vision: bool,

    /// Whether tool use capability is required.
    #[serde(default)]
    pub requires_tools: bool,
}

impl ModelRequirement {
    /// Check if a model satisfies this requirement.
    pub fn satisfied_by(&self, model: &AvailableModel) -> bool {
        // Check model name (simple glob matching)
        if self.model.ends_with('*') {
            let prefix = &self.model[..self.model.len() - 1];
            if !model.name.starts_with(prefix) {
                return false;
            }
        } else if self.model != model.name {
            return false;
        }

        // Check provider
        if let Some(required_provider) = &self.provider {
            if model.provider.as_ref() != Some(required_provider) {
                return false;
            }
        }

        // Check context length
        if let Some(min_ctx) = self.min_context_length {
            if model.context_length.unwrap_or(0) < min_ctx {
                return false;
            }
        }

        // Check capabilities
        if self.requires_vision && !model.supports_vision {
            return false;
        }
        if self.requires_tools && !model.supports_tools {
            return false;
        }

        true
    }
}

/// Sandbox configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Sandbox mode.
    #[serde(default)]
    pub mode: SandboxMode,

    /// Network isolation.
    #[serde(default)]
    pub network_isolation: bool,

    /// Filesystem isolation.
    #[serde(default)]
    pub filesystem_isolation: bool,

    /// Process isolation.
    #[serde(default)]
    pub process_isolation: bool,

    /// Resource limits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_limits: Option<ResourceLimits>,
}

/// Sandbox mode.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxMode {
    /// No sandboxing.
    None,
    /// Light sandboxing (process isolation).
    #[default]
    Light,
    /// Full sandboxing (container-like isolation).
    Full,
    /// Custom sandboxing.
    Custom,
}

/// Resource limits for sandboxed execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum memory in bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_memory: Option<u64>,

    /// Maximum CPU time in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cpu_time: Option<u64>,

    /// Maximum number of processes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_processes: Option<u32>,

    /// Maximum number of open files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_open_files: Option<u32>,
}

/// Environment variable requirement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVarRequirement {
    /// Whether the variable is required.
    #[serde(default)]
    pub required: bool,

    /// Default value if not provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,

    /// Description of what this variable is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Whether this is a secret (should be masked in logs).
    #[serde(default)]
    pub secret: bool,
}

/// Runtime dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    /// Dependency name.
    pub name: String,

    /// Required version (semver).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Dependency type.
    #[serde(default)]
    pub kind: DependencyKind,
}

/// Dependency type.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    /// System package.
    #[default]
    System,
    /// Python package.
    Python,
    /// Node.js package.
    Node,
    /// Rust crate.
    Rust,
    /// Container image.
    Container,
}

// Host capability types (for matching)

/// Capabilities of a host that can run agents.
#[derive(Debug, Clone, Default)]
pub struct HostCapabilities {
    /// Supported execution environment.
    pub environment: ExecutionEnvironment,
    /// Available resources.
    pub resources: AvailableResources,
    /// Available models.
    pub available_models: Vec<AvailableModel>,
}

/// Available resources on a host.
#[derive(Debug, Clone, Default)]
pub struct AvailableResources {
    /// Available memory in bytes.
    pub memory: u64,
    /// Available GPU.
    pub gpu: Option<AvailableGpu>,
    /// Available disk space in bytes.
    pub disk_space: u64,
}

/// Available GPU on a host.
#[derive(Debug, Clone)]
pub struct AvailableGpu {
    /// GPU type.
    pub gpu_type: GpuType,
    /// VRAM in bytes.
    pub vram: u64,
}

/// Available model on a host.
#[derive(Debug, Clone)]
pub struct AvailableModel {
    /// Model name.
    pub name: String,
    /// Provider.
    pub provider: Option<String>,
    /// Context length.
    pub context_length: Option<u32>,
    /// Supports vision.
    pub supports_vision: bool,
    /// Supports tools.
    pub supports_tools: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_requirements_builder() {
        let reqs = AgentRequirements::builder()
            .environment(ExecutionEnvironment::Local)
            .resources(ResourceRequirements {
                min_memory: Some(1024 * 1024 * 1024), // 1GB
                ..Default::default()
            })
            .sandbox(SandboxConfig {
                mode: SandboxMode::Light,
                ..Default::default()
            })
            .build();

        assert_eq!(reqs.environment, ExecutionEnvironment::Local);
        assert_eq!(reqs.resources.min_memory, Some(1024 * 1024 * 1024));
        assert_eq!(reqs.sandbox.mode, SandboxMode::Light);
    }

    #[test]
    fn test_environment_compatibility() {
        let local = ExecutionEnvironment::Local;
        let cloud = ExecutionEnvironment::Cloud {
            provider: CloudProvider::Anthropic,
        };
        let hybrid = ExecutionEnvironment::Hybrid {
            prefer: ExecutionPreference::PreferLocal,
        };

        assert!(local.compatible_with(&local));
        assert!(local.compatible_with(&hybrid));
        assert!(!local.compatible_with(&cloud));

        assert!(hybrid.compatible_with(&local));
        assert!(hybrid.compatible_with(&cloud));
    }

    #[test]
    fn test_model_requirement_matching() {
        let req = ModelRequirement {
            model: "claude-*".to_string(),
            provider: Some("anthropic".to_string()),
            min_context_length: Some(100000),
            requires_vision: false,
            requires_tools: true,
        };

        let good_model = AvailableModel {
            name: "claude-3-5-sonnet".to_string(),
            provider: Some("anthropic".to_string()),
            context_length: Some(200000),
            supports_vision: true,
            supports_tools: true,
        };

        let bad_model = AvailableModel {
            name: "gpt-4".to_string(),
            provider: Some("openai".to_string()),
            context_length: Some(128000),
            supports_vision: true,
            supports_tools: true,
        };

        assert!(req.satisfied_by(&good_model));
        assert!(!req.satisfied_by(&bad_model));
    }

    #[test]
    fn test_resource_requirements() {
        let reqs = ResourceRequirements {
            min_memory: Some(1024 * 1024 * 1024), // 1GB
            gpu: Some(GpuRequirement {
                required: true,
                min_vram: Some(8 * 1024 * 1024 * 1024), // 8GB
                types: vec![GpuType::Metal],
                min_compute_capability: None,
            }),
            ..Default::default()
        };

        let good_resources = AvailableResources {
            memory: 16 * 1024 * 1024 * 1024, // 16GB
            gpu: Some(AvailableGpu {
                gpu_type: GpuType::Metal,
                vram: 16 * 1024 * 1024 * 1024, // 16GB
            }),
            disk_space: 100 * 1024 * 1024 * 1024,
        };

        let no_gpu_resources = AvailableResources {
            memory: 16 * 1024 * 1024 * 1024,
            gpu: None,
            disk_space: 100 * 1024 * 1024 * 1024,
        };

        assert!(reqs.satisfied_by(&good_resources));
        assert!(!reqs.satisfied_by(&no_gpu_resources));
    }
}
