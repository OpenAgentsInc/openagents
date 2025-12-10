-- OpenAgents SQLite Schema v1.1.0
-- Migration: Add inferences table to track all OpenRouter inference requests and responses

-- ============================================================================
-- Inferences Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS inferences (
  -- Auto-increment primary key
  id INTEGER PRIMARY KEY AUTOINCREMENT,

  -- Request metadata
  model TEXT NOT NULL,
  request_id TEXT, -- OpenRouter response id (e.g., "gen-1765180827-U9reySaHTymW5eUcIN90")

  -- Request data (stored as JSON for flexibility)
  request_messages JSON NOT NULL, -- Array of ChatMessage objects
  request_options JSON, -- Optional request parameters (temperature, maxTokens, etc.)

  -- Response data (stored as JSON to capture all fields)
  response_data JSON NOT NULL, -- Full ChatResponse object with all metadata

  -- Extracted fields for easy querying
  response_id TEXT, -- From response_data.id
  response_model TEXT, -- From response_data.model (especially important for openrouter/auto)
  response_content TEXT, -- First choice message content (for search)

  -- Usage metrics (extracted from response_data.usage)
  prompt_tokens INTEGER,
  completion_tokens INTEGER,
  total_tokens INTEGER,
  cost_usd REAL, -- From response_data.usage.cost if available

  -- Timestamps
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============================================================================
-- Indexes for Performance
-- ============================================================================

-- Index for querying by model
CREATE INDEX IF NOT EXISTS idx_inferences_model ON inferences(model);
CREATE INDEX IF NOT EXISTS idx_inferences_response_model ON inferences(response_model);

-- Index for querying by request_id (for deduplication/lookup)
CREATE INDEX IF NOT EXISTS idx_inferences_request_id ON inferences(request_id);

-- Index for querying by timestamp
CREATE INDEX IF NOT EXISTS idx_inferences_created_at ON inferences(created_at);

-- Index for cost analysis
CREATE INDEX IF NOT EXISTS idx_inferences_cost ON inferences(cost_usd);

-- Composite index for model + timestamp queries
CREATE INDEX IF NOT EXISTS idx_inferences_model_created ON inferences(model, created_at);

-- ============================================================================
-- Full-Text Search (FTS5) for response content
-- ============================================================================

CREATE VIRTUAL TABLE IF NOT EXISTS inferences_fts USING fts5(
  id UNINDEXED,
  response_content,
  content=inferences,
  content_rowid=rowid
);

-- Triggers to keep FTS table in sync
CREATE TRIGGER IF NOT EXISTS inferences_fts_insert AFTER INSERT ON inferences BEGIN
  INSERT INTO inferences_fts(rowid, id, response_content)
  VALUES (NEW.rowid, NEW.id, NEW.response_content);
END;

CREATE TRIGGER IF NOT EXISTS inferences_fts_update AFTER UPDATE ON inferences BEGIN
  UPDATE inferences_fts SET response_content = NEW.response_content
  WHERE rowid = NEW.rowid;
END;

CREATE TRIGGER IF NOT EXISTS inferences_fts_delete AFTER DELETE ON inferences BEGIN
  DELETE FROM inferences_fts WHERE rowid = OLD.rowid;
END;

-- ============================================================================
-- Metadata Comments
-- ============================================================================

-- This table stores complete inference history for:
-- - Cost tracking and analysis
-- - Model performance comparison
-- - Debugging and troubleshooting
-- - Learning from past interactions
-- - Audit trail of all AI interactions

-- Performance notes:
-- - JSON fields store complete request/response for full fidelity
-- - Extracted fields enable fast queries without JSON parsing
-- - FTS5 enables fast text search on response content
-- - Indexes support common query patterns (by model, time, cost)

-- Data retention:
-- - No automatic deletion (preserve all history)
-- - Can add cleanup policy later if needed
-- - Consider partitioning by date for very large datasets





