-- OpenAgents SQLite Schema v1.3.0
-- Migration: Trajectories table for storing ATIF trajectories and test generation sessions

-- ============================================================================
-- Trajectories Table
-- ============================================================================
-- Stores all trajectories (ATIF format, test generation, agent runs, etc.)

CREATE TABLE IF NOT EXISTS trajectories (
  -- Primary key
  session_id TEXT PRIMARY KEY,

  -- Extracted metadata (for efficient querying)
  schema_version TEXT NOT NULL,  -- e.g., "ATIF-v1.4", "testgen-v1.0"
  agent_name TEXT NOT NULL,      -- e.g., "mechacoder-orchestrator", "testgen", "hillclimber"
  agent_version TEXT,
  model_name TEXT,
  step_count INTEGER NOT NULL DEFAULT 0,

  -- Computed fields
  first_step_at TEXT,
  last_step_at TEXT,
  total_cost_usd REAL DEFAULT 0,
  total_tokens INTEGER DEFAULT 0,

  -- Full trajectory JSON (for complete data)
  trajectory JSON NOT NULL,

  -- Vector embedding for semantic search (optional, stored as BLOB)
  embedding BLOB,

  -- Timestamps
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  indexed_at TEXT
);

-- ============================================================================
-- Test Generation Trajectories
-- ============================================================================
-- Specialized table for test generation sessions (extends trajectories)

CREATE TABLE IF NOT EXISTS testgen_trajectories (
  -- Primary key (same as session_id in trajectories)
  session_id TEXT PRIMARY KEY REFERENCES trajectories(session_id) ON DELETE CASCADE,

  -- Task information
  task_id TEXT NOT NULL,
  task_description TEXT NOT NULL,

  -- Test generation metadata
  total_tests INTEGER NOT NULL,
  total_rounds INTEGER NOT NULL,
  category_rounds JSON,  -- Record<TestCategory, number>
  comprehensiveness_score REAL,  -- 1-10 scale
  total_tokens_used INTEGER NOT NULL,
  duration_ms INTEGER NOT NULL,

  -- Generated tests (full JSON array)
  tests JSON NOT NULL,  -- Array of GeneratedTest

  -- Reflections (full JSON array)
  reflections JSON,  -- Array of reflection objects

  -- Environment context
  environment JSON,  -- EnvironmentInfo object

  -- Uncertainties
  uncertainties JSON,  -- Array of strings

  -- Timestamps
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============================================================================
-- Indexes for Performance
-- ============================================================================

-- Trajectories indexes
CREATE INDEX IF NOT EXISTS idx_trajectories_agent ON trajectories(agent_name);
CREATE INDEX IF NOT EXISTS idx_trajectories_model ON trajectories(model_name);
CREATE INDEX IF NOT EXISTS idx_trajectories_created_at ON trajectories(created_at);
CREATE INDEX IF NOT EXISTS idx_trajectories_step_count ON trajectories(step_count);
CREATE INDEX IF NOT EXISTS idx_trajectories_cost ON trajectories(total_cost_usd);

-- Composite index for agent + date queries
CREATE INDEX IF NOT EXISTS idx_trajectories_agent_created ON trajectories(agent_name, created_at);

-- Test generation indexes
CREATE INDEX IF NOT EXISTS idx_testgen_task ON testgen_trajectories(task_id);
CREATE INDEX IF NOT EXISTS idx_testgen_created_at ON testgen_trajectories(created_at);
CREATE INDEX IF NOT EXISTS idx_testgen_score ON testgen_trajectories(comprehensiveness_score);

-- ============================================================================
-- Full-Text Search
-- ============================================================================

-- Full-text search on trajectory content
CREATE VIRTUAL TABLE IF NOT EXISTS trajectories_fts USING fts5(
  session_id UNINDEXED,
  agent_name UNINDEXED,
  content  -- All step messages concatenated
);

-- FTS sync triggers for trajectories
CREATE TRIGGER IF NOT EXISTS trajectories_fts_insert AFTER INSERT ON trajectories BEGIN
  INSERT INTO trajectories_fts(rowid, session_id, agent_name, content)
  VALUES (NEW.rowid, NEW.session_id, NEW.agent_name, json_extract(NEW.trajectory, '$.steps[*].message') || ' ' || json_extract(NEW.trajectory, '$.steps[*].tool_calls[*].name'));
END;

CREATE TRIGGER IF NOT EXISTS trajectories_fts_update AFTER UPDATE ON trajectories BEGIN
  INSERT INTO trajectories_fts(trajectories_fts, rowid, session_id, agent_name, content)
  VALUES ('delete', OLD.rowid, OLD.session_id, OLD.agent_name, '');
  INSERT INTO trajectories_fts(rowid, session_id, agent_name, content)
  VALUES (NEW.rowid, NEW.session_id, NEW.agent_name, json_extract(NEW.trajectory, '$.steps[*].message') || ' ' || json_extract(NEW.trajectory, '$.steps[*].tool_calls[*].name'));
END;

CREATE TRIGGER IF NOT EXISTS trajectories_fts_delete AFTER DELETE ON trajectories BEGIN
  INSERT INTO trajectories_fts(trajectories_fts, rowid, session_id, agent_name, content)
  VALUES ('delete', OLD.rowid, OLD.session_id, OLD.agent_name, '');
END;

-- ============================================================================
-- Metadata Comments
-- ============================================================================

-- This schema supports storing all types of trajectories:
-- - ATIF trajectories (from orchestrator, subagents, etc.)
-- - Test generation trajectories (from testgen service)
-- - HillClimber trajectories (from overnight optimization)
--
-- Key patterns:
-- - trajectories: Generic table for all trajectory types
-- - testgen_trajectories: Specialized table for test generation with test-specific fields
-- - Full-text search via trajectories_fts for searching trajectory content
-- - Vector embeddings (optional) for semantic search
--
-- Integration points:
-- - ATIF service saves to trajectories table
-- - TestGen service saves to both trajectories and testgen_trajectories
-- - HillClimber can save to trajectories table
