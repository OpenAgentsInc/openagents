use crate::solver::types::{Change, ChangeError, ChangeResult};
use anyhow::Result;
use serde::Deserialize;

/// Response from LLM for change generation
#[derive(Debug, Deserialize)]
struct ChangeResponse {
    changes: Vec<ChangeBlock>,
    reasoning: String,
}

/// A block of changes from the LLM
#[derive(Debug, Deserialize)]
struct ChangeBlock {
    path: String,
    search: String,
    replace: String,
    #[serde(skip)]
    #[allow(dead_code)]
    reason: String,
}