use serde::Deserialize;

/// Response from LLM for change generation
#[derive(Debug, Deserialize)]
pub(crate) struct ChangeResponse {
    pub changes: Vec<ChangeBlock>,
    pub reasoning: String,
}

/// A block of changes from the LLM
#[derive(Debug, Deserialize)]
pub(crate) struct ChangeBlock {
    pub path: String,
    pub search: String,
    pub replace: String,
    #[serde(skip)]
    #[allow(dead_code)]
    pub reason: String,
}