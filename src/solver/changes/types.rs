use serde::Deserialize;

/// Response from LLM for change generation
#[derive(Debug, Deserialize)]
pub struct ChangeResponse {
    pub changes: Vec<ChangeBlock>,
    pub reasoning: String,
}

/// A block of changes from the LLM
#[derive(Debug, Deserialize, Clone)]
pub struct ChangeBlock {
    pub path: String,
    pub search: String,
    pub replace: String,
    #[serde(skip)]
    #[allow(dead_code)]
    pub reason: String,
}
