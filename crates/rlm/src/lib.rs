//! Recursive Language Model (RLM) execution engine.
//!
//! RLMs enable LLMs to programmatically examine, decompose, and recursively
//! call themselves over input data. This replaces the canonical `llm.complete()`
//! call with an iterative prompt-execute-loop pattern.
//!
//! # Architecture
//!
//! ```text
//! User Query → RlmEngine → FM Bridge (LLM) → Parse Commands → Executor
//!                 ↑                                              ↓
//!                 └──────────── Execution Result ←───────────────┘
//!                               (loop until FINAL)
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use rlm::{RlmEngine, MockExecutor};
//! use fm_bridge::FMClient;
//!
//! let client = FMClient::new()?;
//! let executor = MockExecutor::new();
//! let engine = RlmEngine::new(client, executor);
//!
//! let result = engine.run("What is 2 + 2?").await?;
//! ```

pub mod chunking;
pub mod cli;
mod command;
mod context;
mod engine;
mod error;
mod executor;
mod mock_executor;
pub mod orchestrator;
mod prompts;
mod python_executor;
mod subquery;

pub use chunking::{chunk_by_structure, detect_structure, Chunk, DocumentStructure, DocumentType, Section};
pub use command::{Command, RunArgs};
pub use context::{Context, ContextType, FileEntry, SearchResult};
pub use engine::{ExecutionLogEntry, RlmConfig, RlmEngine, RlmResult, StuckDetector, StuckType};
pub use error::RlmError;
pub use executor::{ExecutionEnvironment, ExecutionResult, ExecutorCapabilities};
pub use mock_executor::MockExecutor;
pub use orchestrator::{AnalysisResult, ChunkSummary, EngineOrchestrator, OrchestratorConfig};
pub use prompts::{
    continuation_prompt_with_reminder, error_prompt_with_reminder, system_prompt_for_tier,
    system_prompt_for_tier_no_context, system_prompt_with_context, PromptTier,
    BASIC_SYSTEM_PROMPT, CONTEXT_SYSTEM_PROMPT, GUIDED_SYSTEM_PROMPT, MINIMAL_SYSTEM_PROMPT,
    SYSTEM_PROMPT,
};
pub use python_executor::PythonExecutor;
