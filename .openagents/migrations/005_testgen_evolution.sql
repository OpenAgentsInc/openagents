-- OpenAgents SQLite Schema v1.4.0
-- Migration: TestGen Evolution tables for iterative test generation optimization

-- ============================================================================
-- TestGen Configs Table
-- ============================================================================
-- Stores test generation configurations (the "knobs" being tuned)

CREATE TABLE IF NOT EXISTS testgen_configs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  version TEXT NOT NULL,                    -- "1.0.0", "1.0.1", etc.

  -- Generation Parameters
  temperature REAL NOT NULL DEFAULT 0.3,
  max_tokens INTEGER NOT NULL DEFAULT 2048,
  min_tests_per_category INTEGER NOT NULL DEFAULT 2,
  max_tests_per_category INTEGER NOT NULL DEFAULT 5,
  max_rounds_per_category INTEGER NOT NULL DEFAULT 3,

  -- Strategy Weights (0-1)
  environment_weight REAL NOT NULL DEFAULT 0.7,
  anti_cheat_weight REAL NOT NULL DEFAULT 0.8,
  precision_weight REAL NOT NULL DEFAULT 0.6,

  -- Category Order (JSON array)
  category_order JSON NOT NULL DEFAULT '["anti_cheat","existence","correctness","boundary","integration"]',

  -- Prompt Templates (JSON)
  category_prompts JSON,                    -- Record<Category, string>
  anti_cheat_prompt TEXT,
  reflection_prompt TEXT,

  -- Model Selection
  primary_model TEXT NOT NULL DEFAULT 'local',  -- 'local' | 'claude'
  reflection_model TEXT NOT NULL DEFAULT 'local',

  -- Quality Thresholds
  min_comprehensiveness_score REAL NOT NULL DEFAULT 7.0,
  target_comprehensiveness_score REAL NOT NULL DEFAULT 8.5,

  -- Hash for deduplication
  config_hash TEXT NOT NULL,
  is_current INTEGER DEFAULT 0,

  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE(config_hash)
);

-- ============================================================================
-- TestGen Runs Table
-- ============================================================================
-- Records every test generation session with analysis metrics

CREATE TABLE IF NOT EXISTS testgen_runs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id TEXT NOT NULL UNIQUE,              -- "tg-YYYYMMDD-HHMMSS-random"
  session_id TEXT NOT NULL REFERENCES testgen_trajectories(session_id),
  config_id INTEGER NOT NULL REFERENCES testgen_configs(id),
  task_id TEXT NOT NULL,

  -- Results
  total_tests INTEGER NOT NULL,
  comprehensiveness_score REAL,
  duration_ms INTEGER NOT NULL,
  total_tokens INTEGER NOT NULL,

  -- Analysis Metrics (computed post-run)
  category_balance REAL,                    -- 0-1, how balanced across categories
  anti_cheat_coverage REAL,                 -- 0-1, coverage of prohibited tools
  parameter_discovery REAL,                 -- 0-1, coverage of discovered parameters
  reflection_effectiveness REAL,            -- 0-1, how much reflections improved tests
  token_efficiency REAL,                    -- comprehensiveness per 1k tokens

  -- Meta-reasoning
  meta_model TEXT,
  proposed_change TEXT,
  change_accepted INTEGER DEFAULT 0,

  -- Scoring
  score INTEGER NOT NULL,                   -- Computed quality score (0-1000)
  is_best INTEGER DEFAULT 0,

  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============================================================================
-- TestGen Best Configs Table
-- ============================================================================
-- Quick lookup for best config per task type (global default + overrides)

CREATE TABLE IF NOT EXISTS testgen_best_configs (
  task_type TEXT PRIMARY KEY,               -- "_global_" | "conversion" | "implementation" | "debugging" | etc.
  config_id INTEGER NOT NULL REFERENCES testgen_configs(id),
  run_id INTEGER NOT NULL REFERENCES testgen_runs(id),
  score INTEGER NOT NULL,
  pass_count INTEGER DEFAULT 0,
  total_runs INTEGER DEFAULT 0,
  is_override INTEGER DEFAULT 0,            -- 1 if this beats global for this task type
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============================================================================
-- TestGen Evolution History Table
-- ============================================================================
-- Tracks what changed and why (for learning and rollback)

CREATE TABLE IF NOT EXISTS testgen_evolution (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  from_config_id INTEGER REFERENCES testgen_configs(id),
  to_config_id INTEGER REFERENCES testgen_configs(id),

  changes JSON NOT NULL,                    -- What changed
  reasoning TEXT NOT NULL,                  -- Why (from meta-reasoner)
  expected_improvement TEXT,

  -- Results (filled after testing)
  actual_improvement REAL,
  quality_delta REAL,

  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============================================================================
-- Indexes for Performance
-- ============================================================================

-- Config lookups
CREATE INDEX IF NOT EXISTS idx_tg_configs_current ON testgen_configs(is_current) WHERE is_current = 1;
CREATE INDEX IF NOT EXISTS idx_tg_configs_hash ON testgen_configs(config_hash);

-- Run lookups
CREATE INDEX IF NOT EXISTS idx_tg_runs_task ON testgen_runs(task_id);
CREATE INDEX IF NOT EXISTS idx_tg_runs_config ON testgen_runs(config_id);
CREATE INDEX IF NOT EXISTS idx_tg_runs_score ON testgen_runs(score);
CREATE INDEX IF NOT EXISTS idx_tg_runs_created ON testgen_runs(created_at);
CREATE INDEX IF NOT EXISTS idx_tg_runs_session ON testgen_runs(session_id);

-- Best config lookups
CREATE INDEX IF NOT EXISTS idx_tg_best_task_type ON testgen_best_configs(task_type);
CREATE INDEX IF NOT EXISTS idx_tg_best_override ON testgen_best_configs(is_override) WHERE is_override = 1;

-- Evolution history
CREATE INDEX IF NOT EXISTS idx_tg_evolution_from ON testgen_evolution(from_config_id);
CREATE INDEX IF NOT EXISTS idx_tg_evolution_to ON testgen_evolution(to_config_id);
CREATE INDEX IF NOT EXISTS idx_tg_evolution_created ON testgen_evolution(created_at);

-- ============================================================================
-- Metadata Comments
-- ============================================================================

-- This schema supports the TestGen HillClimber evolution loop:
-- - testgen_configs: Stores all tried configurations (params, prompts, weights)
-- - testgen_runs: Records every generation session with analysis metrics
-- - testgen_best_configs: Quick lookup for best performing config per task type
-- - testgen_evolution: Tracks config changes and their impact
--
-- Key patterns:
-- - Config deduplication via config_hash (SHA256)
-- - Current config tracking via is_current flag
-- - Best run tracking via is_best flag in runs + best_configs table
-- - Hybrid config: global default + task-type-specific overrides
-- - Quality scoring: 0-1000 scale (comprehensiveness + balance + coverage + efficiency)
--
-- Integration points:
-- - testgen_trajectories: Links runs to full trajectory data
-- - InferenceStore: Tracks meta-reasoning LLM calls
-- - HillClimber: Can use evolved testgen configs for blind verification

