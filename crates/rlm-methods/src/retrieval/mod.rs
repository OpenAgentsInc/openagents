//! Retrieval module for CodeAct+BM25 method.
//!
//! Provides BM25-based retrieval over context for the ReAct-style agent.

pub mod bm25;

pub use bm25::Bm25Index;
