-- OpenAgents SQLite Schema v1.2.0
-- Migration: HillClimber tables for overnight benchmark optimization

-- ============================================================================
-- HillClimber Configs Table
-- ============================================================================
-- Stores task configurations (the "knobs" we're tuning)

CREATE TABLE IF NOT EXISTS hillclimber_configs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id TEXT NOT NULL,
  hint TEXT,                           -- Task-specific hint (main knob to tune)
  use_skills INTEGER DEFAULT 0,        -- Boolean: enable skill injection
  max_turns_override INTEGER DEFAULT 30,
  config_hash TEXT NOT NULL,           -- SHA256 of config for comparison/dedup
  is_current INTEGER DEFAULT 0,        -- Boolean: is this the current config for this task?
  created_at TEXT NOT NULL DEFAULT (datetime('now')),

  UNIQUE(task_id, config_hash)
);

-- ============================================================================
-- HillClimber Runs Table
-- ============================================================================
-- Stores every execution attempt with results and meta-reasoning

CREATE TABLE IF NOT EXISTS hillclimber_runs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id TEXT NOT NULL UNIQUE,         -- "hc-{timestamp}-{random}"
  task_id TEXT NOT NULL,
  config_id INTEGER NOT NULL REFERENCES hillclimber_configs(id),

  -- Results
  passed INTEGER NOT NULL,             -- Boolean: did the task pass verification?
  turns INTEGER NOT NULL,              -- Number of FM turns used
  duration_ms INTEGER NOT NULL,        -- Total execution time
  step_summary TEXT,                   -- Last 3 StepSummary entries (JSON array)
  error_message TEXT,                  -- Error if failed

  -- Meta-reasoning
  meta_model TEXT,                     -- Model used for reasoning (e.g., "x-ai/grok-4.1-fast:free")
  proposed_change TEXT,                -- What hint change was proposed
  change_accepted INTEGER DEFAULT 0,   -- Boolean: was the proposed change applied?

  -- Scoring
  score INTEGER NOT NULL,              -- Computed score (higher = better)
  is_best INTEGER DEFAULT 0,           -- Boolean: is this the best run for this task?

  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============================================================================
-- HillClimber Best Configs Table
-- ============================================================================
-- Quick lookup table for best config per task (for export and stats)

CREATE TABLE IF NOT EXISTS hillclimber_best_configs (
  task_id TEXT PRIMARY KEY,
  config_id INTEGER NOT NULL REFERENCES hillclimber_configs(id),
  run_id INTEGER NOT NULL REFERENCES hillclimber_runs(id),
  score INTEGER NOT NULL,
  pass_count INTEGER DEFAULT 0,        -- How many times this config passed
  total_runs INTEGER DEFAULT 0,        -- Total runs with this config
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============================================================================
-- Indexes for Performance
-- ============================================================================

-- Config lookups
CREATE INDEX IF NOT EXISTS idx_hc_configs_task ON hillclimber_configs(task_id);
CREATE INDEX IF NOT EXISTS idx_hc_configs_current ON hillclimber_configs(task_id, is_current) WHERE is_current = 1;
CREATE INDEX IF NOT EXISTS idx_hc_configs_hash ON hillclimber_configs(config_hash);

-- Run lookups
CREATE INDEX IF NOT EXISTS idx_hc_runs_task ON hillclimber_runs(task_id);
CREATE INDEX IF NOT EXISTS idx_hc_runs_config ON hillclimber_runs(config_id);
CREATE INDEX IF NOT EXISTS idx_hc_runs_best ON hillclimber_runs(task_id, is_best) WHERE is_best = 1;
CREATE INDEX IF NOT EXISTS idx_hc_runs_created ON hillclimber_runs(created_at);

-- Composite index for recent runs by task
CREATE INDEX IF NOT EXISTS idx_hc_runs_task_created ON hillclimber_runs(task_id, created_at);

-- ============================================================================
-- Metadata Comments
-- ============================================================================

-- This schema supports the HillClimber overnight optimization loop:
-- - hillclimber_configs: Stores all tried configurations (hint, skills, turns)
-- - hillclimber_runs: Records every execution attempt with results
-- - hillclimber_best_configs: Quick lookup for best performing config per task
--
-- Key patterns:
-- - Config deduplication via config_hash (SHA256)
-- - Current config tracking via is_current flag
-- - Best run tracking via is_best flag in runs + best_configs table
-- - Pass rate tracking via pass_count/total_runs in best_configs
--
-- Integration points:
-- - InferenceStore for meta-reasoning call tracking
-- - hints.ts for exporting learned hints
