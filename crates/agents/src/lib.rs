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
    // Identity
    AgentId, AgentIdError, AgentKeypair,
    // Capabilities
    AgentCapabilities, ToolCapability, ToolCategory, InputCapability, InputType,
    OutputCapability, ModelCapability, KIND_JOB_TEXT_EXTRACTION, KIND_JOB_SUMMARIZATION,
    KIND_JOB_TRANSLATION, KIND_JOB_TEXT_GENERATION, KIND_JOB_IMAGE_GENERATION,
    KIND_JOB_SPEECH_TO_TEXT, SKILL_CODE_GENERATION, SKILL_CODE_REVIEW, SKILL_DEBUGGING,
    SKILL_REFACTORING, SKILL_TESTING, SKILL_DOCUMENTATION, SKILL_RESEARCH, SKILL_ANALYSIS,
    SKILL_SUMMARIZATION, SKILL_TRANSLATION,
    // Requirements
    AgentRequirements, ExecutionEnvironment, CloudProvider, ExecutionPreference,
    OanixNamespace, OanixMount, ResourceRequirements, CpuRequirement, GpuRequirement,
    GpuType, NetworkAccess, FilesystemAccess, ModelRequirement, SandboxConfig,
    SandboxMode, ResourceLimits, EnvVarRequirement, Dependency, DependencyKind,
    HostCapabilities, AvailableResources, AvailableGpu, AvailableModel,
    // Economics
    AgentEconomics, WalletConfig, WalletType, PricingModel, PriceTier,
    PaymentMethod, RefundPolicy, RevenueSharing, RevenueShare, ShareType,
    JobCostEstimate, JobInvoice, PaymentReceipt, SATS_TO_MILLISATS,
    // State
    AgentState, AgentStats,
    // Events
    AgentEvent, EventSeverity,
    // Manifest
    AgentManifest, AgentAuthor, AgentCategory,
    // Traits
    AgentError, AgentResult, JobRequest, JobInput, JobParam, JobResult,
    AgentExecutor, AgentSession, SessionMessage, MessageRole, SessionState,
    AgentFactory, AgentRegistry, AgentStore, PermissionHandler, PermissionResult,
};
