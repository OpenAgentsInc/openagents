//! Method implementations for RLM paper replication.
//!
//! This crate provides implementations of the methods from the RLM paper:
//! - Base: Direct LLM call with full context
//! - Summary Agent: Iterative context summarization
//! - CodeAct+BM25: ReAct-style with BM25 retrieval
//! - RLM Full: Recursive Language Model with llm_query
//! - RLM No Sub-calls: Ablation without recursive sub-queries
//!
//! # Example
//!
//! ```rust,ignore
//! use rlm_methods::BaseMethod;
//! use lm_router::LmRouter;
//! use std::sync::Arc;
//!
//! let router = Arc::new(LmRouter::new());
//! let method = BaseMethod::new(router, "model-name");
//! let result = method.solve(&task).await?;
//! ```

mod error;
mod base;
mod summary_agent;
pub mod prompts;

pub use error::{Error, Result};
pub use base::BaseMethod;
pub use summary_agent::SummaryAgentMethod;

// Re-export Method trait from bench-harness
pub use bench_harness::{Method, MethodResult};
