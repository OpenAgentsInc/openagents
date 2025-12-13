//! OpenAgents: Unified agent type system for billions of agents.
//!
//! This crate provides foundational types for defining, discovering, and executing
//! agents across local, cloud, and swarm compute environments.
//!
//! # Architecture Overview
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                         Agent Ecosystem                              │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │                                                                      │
//! │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐          │
//! │  │   Identity   │    │   Manifest   │    │  Economics   │          │
//! │  │  (Nostr key) │───▶│ (Declarative)│───▶│  (Bitcoin)   │          │
//! │  └──────────────┘    └──────────────┘    └──────────────┘          │
//! │         │                   │                   │                   │
//! │         ▼                   ▼                   ▼                   │
//! │  ┌─────────────────────────────────────────────────────────┐       │
//! │  │                    Agent Runtime                         │       │
//! │  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐    │       │
//! │  │  │  Local  │  │  Cloud  │  │  Swarm  │  │  Hybrid │    │       │
//! │  │  │Executor │  │Executor │  │Executor │  │Executor │    │       │
//! │  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘    │       │
//! │  └─────────────────────────────────────────────────────────┘       │
//! │                              │                                      │
//! │                              ▼                                      │
//! │  ┌─────────────────────────────────────────────────────────┐       │
//! │  │                    NIP-90 Protocol                       │       │
//! │  │         (Job Requests → Results → Payments)              │       │
//! │  └─────────────────────────────────────────────────────────┘       │
//! │                                                                      │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Key Concepts
//!
//! - **AgentId**: Universal identity based on Nostr public key (npub)
//! - **AgentManifest**: Declarative definition of agent capabilities
//! - **AgentExecutor**: Runtime trait for executing jobs
//! - **AgentEconomics**: Bitcoin wallet and payment configuration

pub mod core;

pub use core::{
    AgentAuthor,
    // Capabilities
    AgentCapabilities,
    AgentCategory,
    // Economics
    AgentEconomics,
    // Traits
    AgentError,
    // Events
    AgentEvent,
    AgentExecutor,
    AgentFactory,
    // Identity
    AgentId,
    AgentIdError,
    AgentKeypair,
    // Manifest
    AgentManifest,
    AgentRegistry,
    // Requirements
    AgentRequirements,
    AgentResult,
    AgentSession,
    // State
    AgentState,
    AgentStats,
    AgentStore,
    AvailableGpu,
    AvailableModel,
    AvailableResources,
    CloudProvider,
    CpuRequirement,
    Dependency,
    DependencyKind,
    EnvVarRequirement,
    EventSeverity,
    ExecutionEnvironment,
    ExecutionPreference,
    FilesystemAccess,
    GpuRequirement,
    GpuType,
    HostCapabilities,
    InputCapability,
    InputType,
    JobCostEstimate,
    JobInput,
    JobInvoice,
    JobParam,
    JobRequest,
    JobResult,
    KIND_JOB_IMAGE_GENERATION,
    KIND_JOB_SPEECH_TO_TEXT,
    KIND_JOB_SUMMARIZATION,
    KIND_JOB_TEXT_EXTRACTION,
    KIND_JOB_TEXT_GENERATION,
    KIND_JOB_TRANSLATION,
    MessageRole,
    ModelCapability,
    ModelRequirement,
    NetworkAccess,
    OanixMount,
    OanixNamespace,
    OutputCapability,
    PaymentMethod,
    PaymentReceipt,
    PermissionHandler,
    PermissionResult,
    PriceTier,
    PricingModel,
    RefundPolicy,
    ResourceLimits,
    ResourceRequirements,
    RevenueShare,
    RevenueSharing,
    SATS_TO_MILLISATS,
    SKILL_ANALYSIS,
    SKILL_CODE_GENERATION,
    SKILL_CODE_REVIEW,
    SKILL_DEBUGGING,
    SKILL_DOCUMENTATION,
    SKILL_REFACTORING,
    SKILL_RESEARCH,
    SKILL_SUMMARIZATION,
    SKILL_TESTING,
    SKILL_TRANSLATION,
    SandboxConfig,
    SandboxMode,
    SessionMessage,
    SessionState,
    ShareType,
    ToolCapability,
    ToolCategory,
    WalletConfig,
    WalletType,
};
