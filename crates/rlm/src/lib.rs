//! Recursive Language Model (RLM) execution engine.
//!
//! RLMs enable LLMs to programmatically examine, decompose, and recursively
//! call themselves over input data. This replaces the canonical `llm.complete()`
//! call with an iterative prompt-execute-loop pattern.
//!
//! # Architecture
//!
//! ```text
//! User Query → RlmEngine → LlmClient (any backend) → Parse Commands → Executor
//!                 ↑                                                      ↓
//!                 └──────────── Execution Result ←───────────────────────┘
//!                               (loop until FINAL)
//! ```
//!
//! # Example
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
//! let result = engine.run("What is 2 + 2?").await?;
//! ```

pub mod chunking;
mod client;
#[cfg(feature = "fm-bridge")]
pub mod cli;
mod command;
mod context;
mod engine;
mod error;
mod executor;
mod lm_router_adapter;
mod mock_executor;
pub mod orchestrator;
mod prompts;
mod python_executor;
mod subquery;

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

pub use chunking::{chunk_by_structure, detect_structure, Chunk, DocumentStructure, DocumentType, Section};
pub use client::{LlmChoice, LlmClient, LlmMessage, LlmResponse, LlmUsage};
pub use command::{Command, RunArgs};
pub use context::{Context, ContextType, FileEntry, SearchResult};
pub use engine::{ExecutionLogEntry, RlmConfig, RlmEngine, RlmResult, StuckDetector, StuckType};
pub use error::RlmError;
pub use executor::{ExecutionEnvironment, ExecutionResult, ExecutorCapabilities};
pub use lm_router_adapter::LmRouterClient;
pub use mock_executor::MockExecutor;
pub use orchestrator::{AnalysisResult, ChunkSummary, EngineOrchestrator, OrchestratorConfig};
pub use prompts::{
    continuation_prompt_with_reminder, error_prompt_with_reminder, system_prompt_for_tier,
    system_prompt_for_tier_no_context, system_prompt_with_context, PromptTier,
    BASIC_SYSTEM_PROMPT, CONTEXT_SYSTEM_PROMPT, GUIDED_SYSTEM_PROMPT, MINIMAL_SYSTEM_PROMPT,
    SYSTEM_PROMPT,
};
pub use python_executor::PythonExecutor;

// Provenance exports (always available)
pub use span::{SpanRef, SpanRefBuilder};

// DSPy re-exports (optional)
#[cfg(feature = "dspy")]
pub use dspy_bridge::{
    configure_dspy_lm, configure_dspy_with_lm, create_lm_for_local, create_lm_for_openrouter,
    get_dspy_lm, example, prediction, sign, field, hashmap,
    Example, LM, Module, Optimizable, Optimizer, Predict, Prediction, Predictor,
    COPRO, MIPROv2, Evaluator, Signature,
    ChatAdapter, configure, get_lm, LMResponse, LmUsage, Chat, Message, MetaSignature,
    // LmRouter bridge
    LmRouterDspyBridge, LmRouterDspyConfig,
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
    RlmTool, ToolConfig, ToolError, ToolResult,
    GrepTool, GrepHit, ReadLinesTool, ReadResult,
    ListFilesTool, FileInfo, SymbolsTool, SymbolInfo, SymbolKind,
};
