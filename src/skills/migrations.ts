import { Database } from "bun:sqlite";
import { Effect } from "effect";
import { SkillStoreError } from "./store.js";

/**
 * Skills database schema SQL
 */
export const SKILLS_SCHEMA_SQL = `
-- Schema version tracking
CREATE TABLE IF NOT EXISTS _schema_version (
  version TEXT PRIMARY KEY,
  applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Skills table
CREATE TABLE IF NOT EXISTS skills (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  version TEXT NOT NULL,
  description TEXT NOT NULL,
  category TEXT NOT NULL CHECK (category IN (
    'file_operations', 'testing', 'debugging', 'refactoring',
    'git', 'shell', 'search', 'documentation', 'security',
    'performance', 'meta', 'api', 'effect'
  )),
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN (
    'active', 'archived', 'draft', 'failed'
  )),

  -- The actual skill code/pattern
  code TEXT NOT NULL,

  -- JSON fields
  parameters JSON,        -- Array of SkillParameter
  prerequisites JSON,     -- Array of skill IDs
  postconditions JSON,    -- Array of strings
  examples JSON,          -- Array of SkillExample
  tags JSON,              -- Array of strings
  languages JSON,         -- Array of strings (e.g., ["typescript"])
  frameworks JSON,        -- Array of strings (e.g., ["effect", "react"])
  learned_from JSON,      -- Array of episode IDs
  verification JSON,      -- SkillVerification object

  -- Embedding vector (stored as BLOB for efficiency)
  embedding BLOB,

  -- Stats
  success_rate REAL,
  usage_count INTEGER DEFAULT 0,
  last_used TEXT,

  -- Metadata
  source TEXT CHECK (source IN ('bootstrap', 'learned', 'manual')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_skills_category ON skills(category);
CREATE INDEX IF NOT EXISTS idx_skills_status ON skills(status);
CREATE INDEX IF NOT EXISTS idx_skills_status_category ON skills(status, category);
CREATE INDEX IF NOT EXISTS idx_skills_success_rate ON skills(success_rate DESC);
CREATE INDEX IF NOT EXISTS idx_skills_usage_count ON skills(usage_count DESC);

-- Full-text search for skill retrieval
CREATE VIRTUAL TABLE IF NOT EXISTS skills_fts USING fts5(
  id UNINDEXED,
  name,
  description,
  code,
  content=skills,
  content_rowid=rowid
);

-- FTS sync triggers
CREATE TRIGGER IF NOT EXISTS skills_fts_insert AFTER INSERT ON skills BEGIN
  INSERT INTO skills_fts(rowid, id, name, description, code)
  VALUES (NEW.rowid, NEW.id, NEW.name, NEW.description, NEW.code);
END;

CREATE TRIGGER IF NOT EXISTS skills_fts_update AFTER UPDATE ON skills BEGIN
  INSERT INTO skills_fts(skills_fts, rowid, id, name, description, code)
  VALUES ('delete', OLD.rowid, OLD.id, OLD.name, OLD.description, OLD.code);
  INSERT INTO skills_fts(rowid, id, name, description, code)
  VALUES (NEW.rowid, NEW.id, NEW.name, NEW.description, NEW.code);
END;

CREATE TRIGGER IF NOT EXISTS skills_fts_delete AFTER DELETE ON skills BEGIN
  INSERT INTO skills_fts(skills_fts, rowid, id, name, description, code)
  VALUES ('delete', OLD.rowid, OLD.id, OLD.name, OLD.description, OLD.code);
END;

-- Insert initial schema version
INSERT OR IGNORE INTO _schema_version (version) VALUES ('1.0.0');
`;

/**
 * Check if skills table exists
 */
const tableExists = (db: Database, tableName: string): boolean => {
  try {
    const stmt = db.prepare(
      "SELECT name FROM sqlite_master WHERE type='table' AND name=?",
    );
    return !!stmt.get(tableName);
  } catch {
    return false;
  }
};

/**
 * Migrate skills database - creates schema if it doesn't exist
 */
export const migrateSkillsDatabase = (
  db: Database,
): Effect.Effect<void, SkillStoreError> =>
  Effect.try({
    try: () => {
      // Check if skills table exists
      if (!tableExists(db, "skills")) {
        // Run schema SQL
        db.exec(SKILLS_SCHEMA_SQL);
      }
    },
    catch: (e) =>
      new SkillStoreError(
        "migration",
        `Failed to migrate skills database: ${e}`,
        e,
      ),
  });
