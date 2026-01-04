//! FRLM core types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A fragment of input data for sub-query processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fragment {
    /// Unique identifier for this fragment.
    pub id: String,
    /// The fragment content.
    pub content: String,
    /// Metadata about the fragment (source, offset, etc.).
    pub metadata: HashMap<String, String>,
}

impl Fragment {
    /// Create a new fragment.
    pub fn new(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            content: content.into(),
            metadata: HashMap::new(),
        }
    }

    /// Create a fragment with metadata.
    pub fn with_metadata(
        id: impl Into<String>,
        content: impl Into<String>,
        metadata: HashMap<String, String>,
    ) -> Self {
        Self {
            id: id.into(),
            content: content.into(),
            metadata,
        }
    }

    /// Get the size in bytes.
    pub fn size_bytes(&self) -> usize {
        self.content.len()
    }
}

/// A sub-query to be executed on a fragment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubQuery {
    /// Unique identifier for this sub-query.
    pub id: String,
    /// The prompt to send to the LLM.
    pub prompt: String,
    /// Optional fragment this query operates on.
    pub fragment_id: Option<String>,
    /// Model preference (if any).
    pub model: Option<String>,
    /// Maximum tokens for response.
    pub max_tokens: Option<u32>,
}

impl SubQuery {
    /// Create a new sub-query.
    pub fn new(id: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            prompt: prompt.into(),
            fragment_id: None,
            model: None,
            max_tokens: None,
        }
    }

    /// Create a sub-query for a specific fragment.
    pub fn for_fragment(
        id: impl Into<String>,
        prompt: impl Into<String>,
        fragment_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            prompt: prompt.into(),
            fragment_id: Some(fragment_id.into()),
            model: None,
            max_tokens: None,
        }
    }

    /// Set the model preference.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the max tokens.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }
}

/// Result from a sub-query execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubQueryResult {
    /// The sub-query ID this result is for.
    pub query_id: String,
    /// The result content.
    pub content: String,
    /// Provider that executed this query.
    pub provider_id: Option<String>,
    /// Execution venue.
    pub venue: Venue,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Cost in satoshis.
    pub cost_sats: u64,
    /// Whether execution succeeded.
    pub success: bool,
    /// Error message if failed.
    pub error: Option<String>,
    /// Additional metadata (attestations, signatures, etc.).
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl SubQueryResult {
    /// Create a successful result.
    pub fn success(
        query_id: impl Into<String>,
        content: impl Into<String>,
        venue: Venue,
        duration_ms: u64,
    ) -> Self {
        Self {
            query_id: query_id.into(),
            content: content.into(),
            provider_id: None,
            venue,
            duration_ms,
            cost_sats: 0,
            success: true,
            error: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a failed result.
    pub fn failure(query_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            query_id: query_id.into(),
            content: String::new(),
            provider_id: None,
            venue: Venue::Unknown,
            duration_ms: 0,
            cost_sats: 0,
            success: false,
            error: Some(error.into()),
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the result.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Set the provider ID.
    pub fn with_provider(mut self, provider_id: impl Into<String>) -> Self {
        self.provider_id = Some(provider_id.into());
        self
    }

    /// Set the cost in sats.
    pub fn with_cost(mut self, cost_sats: u64) -> Self {
        self.cost_sats = cost_sats;
        self
    }
}

/// Execution venue for sub-queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Venue {
    /// Local execution via FM Bridge.
    Local,
    /// Swarm execution via NIP-90.
    Swarm,
    /// Datacenter API (e.g., Crusoe).
    Datacenter,
    /// Unknown venue.
    #[default]
    Unknown,
}

impl std::fmt::Display for Venue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Venue::Local => write!(f, "local"),
            Venue::Swarm => write!(f, "swarm"),
            Venue::Datacenter => write!(f, "datacenter"),
            Venue::Unknown => write!(f, "unknown"),
        }
    }
}

/// An FRLM program to execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrlmProgram {
    /// Unique run ID.
    pub run_id: String,
    /// The root query/task.
    pub query: String,
    /// Input fragments to process.
    pub fragments: Vec<Fragment>,
    /// Context variables.
    pub context: HashMap<String, String>,
}

impl FrlmProgram {
    /// Create a new FRLM program.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            run_id: uuid::Uuid::new_v4().to_string(),
            query: query.into(),
            fragments: Vec::new(),
            context: HashMap::new(),
        }
    }

    /// Add fragments to process.
    pub fn with_fragments(mut self, fragments: Vec<Fragment>) -> Self {
        self.fragments = fragments;
        self
    }

    /// Add a context variable.
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }
}

/// Result from an FRLM execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrlmResult {
    /// The run ID.
    pub run_id: String,
    /// The final output.
    pub output: String,
    /// Total iterations/steps.
    pub iterations: u32,
    /// Total sub-queries executed.
    pub sub_queries_executed: usize,
    /// Total cost in satoshis.
    pub total_cost_sats: u64,
    /// Total duration in milliseconds.
    pub total_duration_ms: u64,
    /// Individual sub-query results.
    pub sub_query_results: Vec<SubQueryResult>,
}

impl FrlmResult {
    /// Create a new result.
    pub fn new(run_id: impl Into<String>, output: impl Into<String>) -> Self {
        Self {
            run_id: run_id.into(),
            output: output.into(),
            iterations: 0,
            sub_queries_executed: 0,
            total_cost_sats: 0,
            total_duration_ms: 0,
            sub_query_results: Vec::new(),
        }
    }
}

/// Status of a sub-query in the scheduler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubQueryStatus {
    /// Pending submission.
    Pending,
    /// Submitted to provider.
    Submitted { job_id: String },
    /// Being executed by provider.
    Executing { provider_id: String },
    /// Completed successfully.
    Complete { result: String, duration_ms: u64 },
    /// Failed.
    Failed { error: String },
    /// Timed out.
    Timeout,
}

impl SubQueryStatus {
    /// Check if this status is terminal (complete, failed, or timeout).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            SubQueryStatus::Complete { .. }
                | SubQueryStatus::Failed { .. }
                | SubQueryStatus::Timeout
        )
    }

    /// Check if this status is successful.
    pub fn is_success(&self) -> bool {
        matches!(self, SubQueryStatus::Complete { .. })
    }
}
