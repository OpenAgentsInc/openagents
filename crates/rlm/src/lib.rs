//! # RLM: Recursive Language Models (OpenAgents Implementation)
//!
//! ## Design Principle: Symbolic Recursion (per Omar/DSPy)
//!
//! > "The way people tend to implement recursive sub-calls or 'sub-agents' don't work.
//! > In particular, you cannot express sub-agents as tool calls."
//! > — Omar (DSPy/RLM creator)
//!
//! This implementation follows Omar's requirements:
//!
//! 1. **Recursion is symbolic through code, not tool calls**
//!    - The [`EngineOrchestrator`] generates chunks programmatically
//!    - Sub-queries are created by code loops, not LLM verbalization
//!    - The model processes individual chunks, never writes O(N) sub-calls
//!
//! 2. **Context accessible through pointers**
//!    - [`SpanRef`] provides git-aware references (path, lines, bytes, commit)
//!    - Chunks reference positions, not embedded content
//!    - Large contexts (10M+ tokens) handled without fitting in prompts
//!
//! ## Architecture
//!
//! ```text
//! Document (10M chars)
//!     ↓ detect_structure() [CODE]
//! Structure
//!     ↓ chunk_by_structure() [CODE]
//! Vec<Chunk> with position pointers
//!     ↓ extract_from_chunks() [CODE LOOP]
//! LLM processes each chunk individually
//!     ↓ synthesize() [CODE]
//! Final Answer
//! ```
//!
//! The critical insight: **The model never writes the O(N) sub-calls**.
//! Code generates them symbolically.
//!
//! ## Simple REPL Loop (for small queries)
//!
//! ```text
//! User Query → RlmEngine → LlmClient (any backend) → Parse Commands → Executor
//!                 ↑                                                      ↓
//!                 └──────────── Execution Result ←───────────────────────┘
//!                               (loop until FINAL)
//! ```
//!
//! ## Example
//!
//! ```rust,ignore
//! use std::sync::Arc;
//! use rlm::{RlmEngine, MockExecutor, LmRouterClient};
//! use lm_router::LmRouter;
//!
//! // Configure router with any backend (OpenRouter, OpenAI, etc.)
//! let router = Arc::new(LmRouter::builder().build());
//! let client = LmRouterClient::new(router, "model-name");
//! let executor = MockExecutor::new();
//! let engine = RlmEngine::new(client, executor);
//!
//! // For small queries, use REPL loop
//! let result = engine.run("What is 2 + 2?").await?;
//!
//! // For large documents, use orchestrated mode (symbolic recursion)
//! engine.set_context(large_document);
//! let result = engine.run_orchestrated("Find all security issues").await?;
//! ```

pub mod chunking;
pub mod cli;
mod client;
mod command;
mod context;
#[cfg(feature = "dspy")]
pub mod dspy;
mod engine;
mod error;
mod executor;
pub mod experiment;
mod lm_router_adapter;
pub mod mcp_tools;
mod mock_executor;
pub mod orchestrator;
mod prompts;
mod python_executor;

// Provenance tracking
pub mod span;

// DSPy integration (optional)
#[cfg(feature = "dspy")]
pub mod dspy_bridge;
#[cfg(feature = "dspy")]
mod dspy_orchestrator;
#[cfg(feature = "dspy")]
pub mod signatures;
#[cfg(feature = "dspy")]
pub mod tools;

pub use chunking::{
    Chunk, DocumentStructure, DocumentType, Section, chunk_by_structure, detect_structure,
};
pub use client::{LlmChoice, LlmClient, LlmMessage, LlmResponse, LlmUsage};
pub use command::{Command, RunArgs};
pub use context::{Context, ContextType, FileEntry, SearchResult};
pub use engine::{ExecutionLogEntry, RlmConfig, RlmEngine, RlmResult, StuckDetector, StuckType};
pub use error::RlmError;
pub use executor::{ExecutionEnvironment, ExecutionResult, ExecutorCapabilities};
pub use experiment::{ExperimentGroup, ExperimentMetrics, ExperimentRunSummary};
pub use lm_router_adapter::LmRouterClient;
pub use mock_executor::MockExecutor;
pub use orchestrator::{AnalysisResult, ChunkSummary, EngineOrchestrator, OrchestratorConfig};
pub use prompts::{
    BASIC_SYSTEM_PROMPT, CONTEXT_SYSTEM_PROMPT, GUIDED_SYSTEM_PROMPT, MINIMAL_SYSTEM_PROMPT,
    PromptTier, SYSTEM_PROMPT, continuation_prompt_with_reminder, error_prompt_with_reminder,
    system_prompt_for_tier, system_prompt_for_tier_no_context, system_prompt_with_context,
};
pub use python_executor::PythonExecutor;

// MCP tools for integration
pub use mcp_tools::{
    RlmFanoutInput, RlmFanoutOutput, RlmQueryInput, RlmQueryOutput, WorkerResult,
    rlm_tool_definitions,
};

// Provenance exports (always available)
pub use span::{SpanRef, SpanRefBuilder};

// DSPy re-exports (optional)
#[cfg(feature = "dspy")]
pub use dspy_bridge::{
    COPRO,
    Chat,
    ChatAdapter,
    Evaluator,
    Example,
    LM,
    LMResponse,
    // LmRouter bridge
    LmRouterDspyBridge,
    LmRouterDspyConfig,
    LmUsage,
    MIPROv2,
    Message,
    MetaSignature,
    Module,
    Optimizable,
    Optimizer,
    Predict,
    Prediction,
    Predictor,
    Signature,
    configure,
    configure_dspy_lm,
    configure_dspy_with_lm,
    create_lm_for_local,
    create_lm_for_openrouter,
    example,
    field,
    get_dspy_lm,
    get_lm,
    hashmap,
    prediction,
    sign,
};
#[cfg(feature = "dspy")]
pub use dspy_orchestrator::{
    ChunkExtraction, DspyAnalysisResult, DspyOrchestrator, DspyOrchestratorConfig,
    VerificationResult,
};
// Signature types are internal (the macro generates private structs)
// Export only the helper types for parsing signature outputs
#[cfg(feature = "dspy")]
pub use signatures::{CandidateSpan, MissingSpanRequest};
#[cfg(feature = "dspy")]
pub use tools::{
    FileInfo, GrepHit, GrepTool, ListFilesTool, ReadLinesTool, ReadResult, RlmTool, SymbolInfo,
    SymbolKind, SymbolsTool, ToolConfig, ToolError, ToolResult,
};
