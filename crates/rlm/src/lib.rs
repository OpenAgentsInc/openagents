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

pub mod cli;
mod command;
mod context;
mod engine;
mod error;
mod executor;
mod mock_executor;
mod prompts;
mod python_executor;
mod subquery;

pub use command::{Command, RunArgs};
pub use context::{Context, ContextType, FileEntry, SearchResult};
pub use engine::{ExecutionLogEntry, RlmConfig, RlmEngine, RlmResult};
pub use error::RlmError;
pub use executor::{ExecutionEnvironment, ExecutionResult, ExecutorCapabilities};
pub use mock_executor::MockExecutor;
pub use prompts::{
    system_prompt_with_context, BASIC_SYSTEM_PROMPT, CONTEXT_SYSTEM_PROMPT, SYSTEM_PROMPT,
};
pub use python_executor::PythonExecutor;
