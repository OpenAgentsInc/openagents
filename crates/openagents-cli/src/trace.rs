//! Trace ingestion, redaction, and recording commands

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTrace {
    pub session_id: String,
    pub agent_name: String,
    pub step_count: usize,
    pub created_at: u64,
}

pub struct TraceStore {
    pub traces: Vec<SessionTrace>,
}

impl TraceStore {
    pub fn new() -> Self {
        Self { traces: Vec::new() }
    }

    pub fn scan_foreign_sessions() -> Vec<SessionTrace> {
        vec![
            SessionTrace {
                session_id: "claude_sess_01".to_string(),
                agent_name: "claude-code".to_string(),
                step_count: 42,
                created_at: 1724600000,
            },
            SessionTrace {
                session_id: "codex_sess_01".to_string(),
                agent_name: "codex-cli".to_string(),
                step_count: 18,
                created_at: 1724600100,
            },
        ]
    }

    pub fn redact_trace(input: &str) -> String {
        input.replace("sk-", "[REDACTED_KEY]")
            .replace("oa_pat_", "[REDACTED_PAT]")
    }
}
