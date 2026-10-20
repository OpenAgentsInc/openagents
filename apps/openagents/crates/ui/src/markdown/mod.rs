//! The markdown renderer is `openagents_chat_view::markdown`, shared with the
//! web (#12216). This module re-exports it under its old path and adds the
//! desktop's Mermaid diagrams, which the transcript supplies through
//! [`render::MediaUi`].

pub use openagents_chat_view::markdown::*;

pub mod mermaid;
pub(crate) mod mermaid_cache;
