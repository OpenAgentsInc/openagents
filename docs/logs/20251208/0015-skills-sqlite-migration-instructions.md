# Skills Store: JSONL to SQLite Migration Instructions

**Date**: 2025-12-08 00:15 CT
**Purpose**: Instructions for migrating `src/skills/store.ts` from JSONL to SQLite
**Reference**: Follow the pattern established in `src/storage/database.ts` (tasks)

---

## Current State

### JSONL Implementation (`src/skills/store.ts`)
- Stores skills in `.openagents/skills/library.jsonl`
- Maintains a separate index file `.openagents/skills/index.json`
- Uses in-memory Map with full file rewrites on update
- Auto-seeds from `src/skills/library/index.ts` (71 bootstrap skills)

### Problems with JSONL
1. Full file rewrite on every update (O(n) writes)
2. No transactions - concurrent access unsafe
3. Separate index file can get out of sync
4. Embeddings stored as JSON arrays (inefficient)
5. No query optimization

---

## Target State

### SQLite Implementation
Follow the pattern in `src/storage/database.ts`:
- Use `bun:sqlite` for database operations
- Store in `.openagents/openagents.db` (same DB as tasks, or separate `.openagents/skills.db`)
- Effect-based service layer
- Proper migrations support

---

## Schema Design

### Skills Table

```sql
CREATE TABLE skills (
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
CREATE INDEX idx_skills_category ON skills(category);
CREATE INDEX idx_skills_status ON skills(status);
CREATE INDEX idx_skills_status_category ON skills(status, category);
CREATE INDEX idx_skills_success_rate ON skills(success_rate DESC);
CREATE INDEX idx_skills_usage_count ON skills(usage_count DESC);

-- Full-text search for skill retrieval
CREATE VIRTUAL TABLE skills_fts USING fts5(
  id UNINDEXED,
  name,
  description,
  code,
  content=skills,
  content_rowid=rowid
);

-- FTS sync triggers
CREATE TRIGGER skills_fts_insert AFTER INSERT ON skills BEGIN
  INSERT INTO skills_fts(rowid, id, name, description, code)
  VALUES (NEW.rowid, NEW.id, NEW.name, NEW.description, NEW.code);
END;

CREATE TRIGGER skills_fts_update AFTER UPDATE ON skills BEGIN
  INSERT INTO skills_fts(skills_fts, rowid, id, name, description, code)
  VALUES ('delete', OLD.rowid, OLD.id, OLD.name, OLD.description, OLD.code);
  INSERT INTO skills_fts(rowid, id, name, description, code)
  VALUES (NEW.rowid, NEW.id, NEW.name, NEW.description, NEW.code);
END;

CREATE TRIGGER skills_fts_delete AFTER DELETE ON skills BEGIN
  INSERT INTO skills_fts(skills_fts, rowid, id, name, description, code)
  VALUES ('delete', OLD.rowid, OLD.id, OLD.name, OLD.description, OLD.code);
END;
```

### Skill Tags Table (Optional - for better tag queries)

```sql
CREATE TABLE skill_tags (
  skill_id TEXT NOT NULL,
  tag TEXT NOT NULL,
  PRIMARY KEY (skill_id, tag),
  FOREIGN KEY (skill_id) REFERENCES skills(id) ON DELETE CASCADE
);

CREATE INDEX idx_skill_tags_tag ON skill_tags(tag);
```

---

## Files to Modify/Create

### 1. Create `src/skills/migrations.ts`

Similar to `src/storage/migrations.ts`:

```typescript
import { Database } from "bun:sqlite";
import { Effect } from "effect";
import { SkillStoreError } from "./store.js";

export const SKILLS_SCHEMA_SQL = `
  -- Schema from above
`;

export const migrateSkillsDatabase = (db: Database): Effect.Effect<void, SkillStoreError> => {
  // Check if skills table exists
  // If not, run SKILLS_SCHEMA_SQL
  // Handle version migrations
};
```

### 2. Rewrite `src/skills/store.ts`

Replace JSONL implementation with SQLite:

```typescript
import { Database } from "bun:sqlite";
import { Context, Effect, Layer } from "effect";
import type { Skill, SkillFilter } from "./schema.js";

export class SkillStoreError extends Error {
  readonly _tag = "SkillStoreError";
  constructor(
    readonly reason: "connection" | "query" | "not_found" | "duplicate" | "migration",
    message: string,
    readonly cause?: unknown,
  ) {
    super(message);
  }
}

// Keep the same ISkillStore interface - just change implementation
export interface ISkillStore {
  readonly get: (id: string) => Effect.Effect<Skill | null, SkillStoreError>;
  readonly list: (filter?: SkillFilter) => Effect.Effect<Skill[], SkillStoreError>;
  readonly add: (skill: Skill) => Effect.Effect<void, SkillStoreError>;
  readonly update: (skill: Skill) => Effect.Effect<void, SkillStoreError>;
  readonly archive: (id: string) => Effect.Effect<void, SkillStoreError>;
  readonly getByCategory: (category: string) => Effect.Effect<Skill[], SkillStoreError>;
  readonly getByTag: (tag: string) => Effect.Effect<Skill[], SkillStoreError>;
  readonly count: () => Effect.Effect<number, never>;
  readonly reload: () => Effect.Effect<void, SkillStoreError>;
  readonly getPath: () => string;
  
  // NEW: Direct DB access for advanced queries
  readonly db: Database;
}

export class SkillStore extends Context.Tag("SkillStore")<SkillStore, ISkillStore>() {}

const makeStore = (projectRoot: string): Effect.Effect<ISkillStore, SkillStoreError> =>
  Effect.gen(function* () {
    const dbPath = join(projectRoot, ".openagents", "skills.db");
    
    // Ensure directory exists
    // Open/create database
    // Run migrations
    // Seed bootstrap skills if empty
    
    // Implement all ISkillStore methods using SQL
  });
```

### 3. Update `src/skills/service.ts`

The service layer should work unchanged if the store interface is preserved.

### 4. Update `src/skills/retrieval.ts`

May need updates if it directly accesses store internals.

### 5. Handle Embedding Storage

Embeddings are float arrays. Store as BLOB:

```typescript
// Serialize embedding to BLOB
const serializeEmbedding = (embedding: number[]): Buffer => {
  const buffer = Buffer.alloc(embedding.length * 4); // 4 bytes per float32
  embedding.forEach((val, i) => buffer.writeFloatLE(val, i * 4));
  return buffer;
};

// Deserialize BLOB to embedding
const deserializeEmbedding = (blob: Buffer): number[] => {
  const result: number[] = [];
  for (let i = 0; i < blob.length; i += 4) {
    result.push(blob.readFloatLE(i));
  }
  return result;
};
```

---

## Migration Steps

### Step 1: Create New Files

1. Create `src/skills/migrations.ts` with schema SQL
2. Create `src/skills/database.ts` if separating DB logic (optional)

### Step 2: Rewrite Store

1. Keep `ISkillStore` interface unchanged
2. Replace JSONL read/write with SQL queries
3. Use prepared statements for performance
4. Handle JSON columns with `JSON.parse`/`JSON.stringify`
5. Handle BLOB columns for embeddings

### Step 3: Bootstrap Seeding

Keep the auto-seed logic but use SQL INSERT:

```typescript
if (skillCount === 0) {
  const { bootstrapSkills } = await import("./library/index.js");
  const insertStmt = db.prepare(`INSERT INTO skills (...) VALUES (...)`);
  
  db.transaction(() => {
    for (const skill of bootstrapSkills) {
      insertStmt.run(/* skill values */);
    }
  })();
  
  console.log(`[SkillStore] Seeded ${bootstrapSkills.length} bootstrap skills`);
}
```

### Step 4: Data Migration

Create a one-time migration script to import existing JSONL data:

```typescript
// src/skills/import-jsonl.ts
import { Database } from "bun:sqlite";
import { readFileSync, existsSync } from "fs";

const importJsonlToSqlite = (jsonlPath: string, db: Database) => {
  if (!existsSync(jsonlPath)) return;
  
  const content = readFileSync(jsonlPath, "utf-8");
  const lines = content.split("\n").filter(l => l.trim());
  
  const insertStmt = db.prepare(`INSERT OR REPLACE INTO skills (...) VALUES (...)`);
  
  db.transaction(() => {
    for (const line of lines) {
      const skill = JSON.parse(line);
      insertStmt.run(/* map skill to columns */);
    }
  })();
};
```

### Step 5: Update Tests

Update `src/skills/schema.test.ts` and any other tests to work with SQLite.

### Step 6: Cleanup

After migration is verified:
1. Remove JSONL-specific code
2. Remove `.openagents/skills/library.jsonl` and `index.json`
3. Update `.gitignore` if needed

---

## Key Considerations

### 1. Preserve Interface

The `ISkillStore` interface should remain unchanged so that:
- `src/skills/service.ts` works without changes
- `src/skills/retrieval.ts` works without changes
- All consumers of SkillStore work without changes

### 2. Transaction Safety

Use `db.transaction()` for operations that modify multiple rows:

```typescript
db.transaction(() => {
  // Multiple operations
})();
```

### 3. Prepared Statements

Use prepared statements for repeated queries:

```typescript
const getByIdStmt = db.prepare("SELECT * FROM skills WHERE id = ?");
const skill = getByIdStmt.get(id);
```

### 4. JSON Column Handling

SQLite stores JSON as TEXT. Parse on read, stringify on write:

```typescript
// Read
const skill = {
  ...row,
  parameters: JSON.parse(row.parameters || "[]"),
  tags: JSON.parse(row.tags || "[]"),
};

// Write
db.prepare("INSERT INTO skills (parameters, tags) VALUES (?, ?)").run(
  JSON.stringify(skill.parameters),
  JSON.stringify(skill.tags),
);
```

### 5. FTS for Retrieval

The full-text search can improve skill retrieval:

```typescript
// Search skills by description
const searchSkills = (query: string) => {
  return db.prepare(`
    SELECT s.* FROM skills s
    JOIN skills_fts fts ON s.id = fts.id
    WHERE skills_fts MATCH ?
    ORDER BY rank
  `).all(query);
};
```

This could complement or replace the embedding-based search for some use cases.

---

## Testing Checklist

- [ ] `bun test src/skills/` passes
- [ ] Bootstrap skills are seeded on first run
- [ ] `SkillService.selectSkills()` returns relevant skills
- [ ] `SkillService.recordUsage()` updates stats
- [ ] FM runs show `[Skills] Injected N relevant skills`
- [ ] `bun run tbench:fm-mini` still passes

---

## Reference Files

| File | Purpose |
|------|---------|
| `src/storage/database.ts` | SQLite pattern for tasks (REFERENCE) |
| `src/storage/migrations.ts` | Schema migrations (REFERENCE) |
| `src/skills/store.ts` | Current JSONL implementation (REPLACE) |
| `src/skills/schema.ts` | Skill type definitions (KEEP) |
| `src/skills/service.ts` | Service layer (KEEP, minimal changes) |
| `src/skills/retrieval.ts` | Embedding retrieval (KEEP, check internals) |
| `src/skills/library/primitives.ts` | Bootstrap skills (KEEP) |
| `src/skills/library/compositional.ts` | Bootstrap skills (KEEP) |

---

## Estimated Effort

- Schema design: 30 min
- Store rewrite: 2-3 hours
- Migration script: 30 min
- Testing: 1 hour
- **Total: ~4-5 hours**

The main complexity is mapping the Skill type to SQL columns and handling JSON/BLOB serialization correctly.
