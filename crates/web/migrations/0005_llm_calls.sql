-- LLM API call logging
-- Tracks all AI completions for billing and debugging

CREATE TABLE IF NOT EXISTS llm_calls (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    stripe_customer_id TEXT,
    model TEXT NOT NULL,
    prompt_tokens INTEGER DEFAULT 0,
    completion_tokens INTEGER DEFAULT 0,
    total_tokens INTEGER DEFAULT 0,
    request_messages TEXT,  -- JSON of input messages
    response_content TEXT,  -- Assistant response
    finish_reason TEXT,
    error TEXT,
    duration_ms INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_llm_calls_user ON llm_calls(user_id);
CREATE INDEX IF NOT EXISTS idx_llm_calls_created ON llm_calls(created_at);
