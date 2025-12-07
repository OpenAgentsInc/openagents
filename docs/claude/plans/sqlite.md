# JSONL to SQLite Migration Plan - Complete Conversion in One Session

## Executive Summary

**Goal**: Migrate entire task system from JSONL to SQLite with 100% test coverage and zero data loss.

**Scope**:
- 12 core files in `src/tasks/` to modify
- 11 test files to update
- 567 existing tasks to migrate
- 31 dependent files (likely no changes needed)
- New `src/storage/` layer to create

**Strategy**: Bottom-up - Build database layer first, then replace JSONL I/O in service layer, update tests, migrate data, validate.

**No dual-write phase** - Complete replacement with rollback capability.

---

## Critical Design Decisions

### ✅ Decision 1: Keep ProjectConfig as JSON
- **Rationale**: Simple, human-editable, version-control friendly
- **File**: `.openagents/project.json` stays as-is
- **Impact**: Only migrate tasks to SQLite

### ✅ Decision 2: Preserve Two-Phase Commit Pattern
- **Rationale**: SQLite transactions don't prevent crashes between git commit and DB update
- **Implementation**: Keep `pendingCommit` JSON field and `commit_pending` status
- **Impact**: Crash recovery in orchestrator continues to work

### ✅ Decision 3: Soft Delete with Tombstones
- **Rationale**: Enable recovery, audit trail, analytics on deleted tasks
- **Implementation**: `deleted_at` timestamp column + `task_deletions` table
- **Impact**: Replaces archive functionality with filtered queries

### ✅ Decision 4: Deprecate but Keep merge.ts
- **Rationale**: SQLite ACID reduces conflicts, but parallel agents still need merge support
- **Implementation**: Mark deprecated, update for SQLite, remove in Phase 2
- **Impact**: Graceful transition, can remove later

### ✅ Decision 5: API Surface Stays Identical
- **Rationale**: Minimize changes to 31 dependent files
- **Implementation**: Service functions keep same signatures, add DatabaseService dependency
- **Impact**: Transparent migration for consumers

---

## Implementation Phases

### Phase 1: Database Foundation (4 hours, HIGH RISK)

#### 1.1 Create DatabaseService Layer
**File**: `src/storage/database.ts` (NEW, ~500 lines)

**Purpose**: Low-level SQLite operations with Effect integration

**Key Components**:
```typescript
export class DatabaseService extends Context.Tag("DatabaseService")<
  DatabaseService,
  {
    // Core operations
    readonly db: Database;
    readonly migrate: () => Effect.Effect<void, DatabaseError>;

    // Task CRUD
    readonly insertTask: (task: Task) => Effect.Effect<void, DatabaseError>;
    readonly updateTask: (id: string, update: Partial<Task>) => Effect.Effect<void, DatabaseError>;
    readonly getTask: (id: string) => Effect.Effect<Task | null, DatabaseError>;
    readonly listTasks: (filter?: TaskFilter) => Effect.Effect<Task[], DatabaseError>;
    readonly deleteTask: (id: string, soft: boolean) => Effect.Effect<void, DatabaseError>;

    // Dependencies
    readonly addDependency: (taskId: string, depId: string, type: DependencyType) => Effect.Effect<void, DatabaseError>;
    readonly getDependencies: (taskId: string) => Effect.Effect<Dependency[], DatabaseError>;

    // Specialized queries
    readonly getReadyTasks: (sort: SortPolicy) => Effect.Effect<Task[], DatabaseError>;
    readonly findTasksWithPendingCommit: () => Effect.Effect<Task[], DatabaseError>;
    readonly getTaskStats: () => Effect.Effect<TaskStats, DatabaseError>;

    // Transactions
    readonly runInTransaction: <A, E>(effect: Effect.Effect<A, E>) => Effect.Effect<A, E | DatabaseError>;
  }
>() {}
```

**Error Handling**:
- Map SQLite errors to `DatabaseError` with reason enums
- Preserve stack traces for debugging
- Propagate through Effect error channel

**Complexity**: HIGH - Foundation for everything
**Risk**: HIGH - Bugs cascade to all consumers

---

#### 1.2 Create Migration SQL
**File**: `.openagents/migrations/001_initial_schema.sql` (NEW, ~200 lines)

**Schema** (from docs/database/SCHEMA.md):
```sql
-- Schema version tracking
CREATE TABLE _schema_version (
  version TEXT PRIMARY KEY,
  applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);
INSERT INTO _schema_version (version) VALUES ('1.0.0');

-- Tasks table (matches Task schema)
CREATE TABLE tasks (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  description TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('open', 'in_progress', 'blocked', 'closed', 'commit_pending')),
  priority INTEGER NOT NULL CHECK (priority BETWEEN 0 AND 4),
  type TEXT NOT NULL CHECK (type IN ('bug', 'feature', 'task', 'epic', 'chore')),
  assignee TEXT,
  close_reason TEXT,

  -- JSON fields
  labels JSON,
  commits JSON,
  comments JSON,
  pending_commit JSON,

  -- Extended fields
  design TEXT,
  acceptance_criteria TEXT,
  notes TEXT,
  estimated_minutes REAL,

  -- Source tracking
  source_repo TEXT,
  source_discovered_from TEXT,
  source_external_ref TEXT,

  -- Timestamps
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  closed_at TEXT,
  deleted_at TEXT
);

-- Dependencies (many-to-many)
CREATE TABLE task_dependencies (
  task_id TEXT NOT NULL,
  depends_on_task_id TEXT NOT NULL,
  dependency_type TEXT NOT NULL CHECK (
    dependency_type IN ('blocks', 'related', 'parent-child', 'discovered-from')
  ),
  created_at TEXT NOT NULL DEFAULT (datetime('now')),

  PRIMARY KEY (task_id, depends_on_task_id),
  FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
  FOREIGN KEY (depends_on_task_id) REFERENCES tasks(id) ON DELETE CASCADE
);

-- Deletion tombstones
CREATE TABLE task_deletions (
  task_id TEXT PRIMARY KEY,
  deleted_at TEXT NOT NULL,
  deleted_by TEXT,
  reason TEXT
);

-- Indexes for performance
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_priority ON tasks(priority);
CREATE INDEX idx_tasks_status_priority ON tasks(status, priority);
CREATE INDEX idx_tasks_created_at ON tasks(created_at);
CREATE INDEX idx_tasks_deleted_at ON tasks(deleted_at);

-- Composite index for ready task query (CRITICAL)
CREATE INDEX idx_tasks_ready ON tasks(status, priority, created_at)
  WHERE deleted_at IS NULL;

-- Full-text search
CREATE VIRTUAL TABLE tasks_fts USING fts5(
  id UNINDEXED,
  title,
  description,
  content=tasks,
  content_rowid=rowid
);

-- FTS sync triggers
CREATE TRIGGER tasks_fts_insert AFTER INSERT ON tasks BEGIN
  INSERT INTO tasks_fts(rowid, id, title, description)
  VALUES (NEW.rowid, NEW.id, NEW.title, NEW.description);
END;

CREATE TRIGGER tasks_fts_update AFTER UPDATE ON tasks BEGIN
  UPDATE tasks_fts SET title = NEW.title, description = NEW.description
  WHERE rowid = NEW.rowid;
END;

CREATE TRIGGER tasks_fts_delete AFTER DELETE ON tasks BEGIN
  DELETE FROM tasks_fts WHERE rowid = OLD.rowid;
END;
```

**Critical Features**:
- `commit_pending` status for two-phase commit
- `pending_commit` JSON field for crash recovery
- Composite index on (status, priority, created_at) for ready tasks
- FTS5 for full-text search on title/description
- Soft delete via `deleted_at` timestamp

**Complexity**: MEDIUM - SQL is straightforward
**Risk**: MEDIUM - Schema must match Task type exactly

---

#### 1.3 Create Migration Runner
**File**: `src/storage/migrations.ts` (NEW, ~150 lines)

**Purpose**: Execute migration scripts in order, track applied versions

**Implementation**:
```typescript
export const runMigrations = (
  db: Database,
  migrationsDir: string = ".openagents/migrations",
): Effect.Effect<void, DatabaseError, FileSystem.FileSystem | Path.Path> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;

    // Read all .sql files
    const files = yield* fs.readDirectory(migrationsDir).pipe(
      Effect.map(entries => entries.filter(e => e.endsWith('.sql')).sort())
    );

    // Execute each migration
    for (const file of files) {
      const sql = yield* fs.readFileString(path.join(migrationsDir, file));
      yield* Effect.try({
        try: () => db.exec(sql),
        catch: (e) => new DatabaseError("migration", `Failed to run ${file}: ${e}`)
      });
    }
  });
```

**Complexity**: LOW - Standard migration pattern
**Risk**: LOW - Can be re-run on clean DB

---

#### 1.4 Create JSONL Import Script
**File**: `src/storage/import-jsonl.ts` (NEW, ~300 lines)

**Purpose**: One-time migration of 567 tasks from JSONL to SQLite

**Implementation**:
```typescript
export interface ImportResult {
  tasksImported: number;
  deletionsImported: number;
  errors: string[];
  validationPassed: boolean;
}

export const importTasksFromJsonl = (
  jsonlPath: string,
  dbPath: string,
): Effect.Effect<ImportResult, Error, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const db = new Database(dbPath);

    // 1. Run migrations
    yield* runMigrations(db);

    // 2. Read tasks.jsonl
    const tasks = yield* readTasks(jsonlPath);

    // 3. Import in transaction
    db.transaction(() => {
      for (const task of tasks) {
        // Insert task
        const stmt = db.prepare(`
          INSERT INTO tasks (
            id, title, description, status, priority, type,
            assignee, close_reason, labels, commits, comments,
            created_at, updated_at, closed_at, pending_commit,
            design, acceptance_criteria, notes, estimated_minutes,
            source_repo, source_discovered_from, source_external_ref
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        `);
        stmt.run(
          task.id,
          task.title,
          task.description,
          task.status,
          task.priority,
          task.type,
          task.assignee ?? null,
          task.closeReason ?? null,
          JSON.stringify(task.labels ?? []),
          JSON.stringify(task.commits ?? []),
          JSON.stringify(task.comments ?? []),
          task.createdAt,
          task.updatedAt,
          task.closedAt ?? null,
          task.pendingCommit ? JSON.stringify(task.pendingCommit) : null,
          task.design ?? null,
          task.acceptanceCriteria ?? null,
          task.notes ?? null,
          task.estimatedMinutes ?? null,
          task.source?.repo ?? null,
          task.source?.discoveredFrom ?? null,
          task.source?.externalRef ?? null
        );

        // Insert dependencies
        for (const dep of task.deps ?? []) {
          const depStmt = db.prepare(`
            INSERT INTO task_dependencies (task_id, depends_on_task_id, dependency_type)
            VALUES (?, ?, ?)
          `);
          depStmt.run(task.id, dep.id, dep.type);
        }
      }
    })();

    // 4. Validate row count
    const count = db.prepare("SELECT COUNT(*) as count FROM tasks").get();
    const validationPassed = count.count === tasks.length;

    return {
      tasksImported: tasks.length,
      deletionsImported: 0,
      errors: [],
      validationPassed
    };
  });
```

**Validation Steps**:
1. Row count matches JSONL line count (567)
2. All task IDs are unique
3. All dependencies reference existing tasks
4. Spot check: read 10 random tasks, compare JSON

**Complexity**: MEDIUM - Straightforward data copy
**Risk**: HIGH - Point of no return for data

**CHECKPOINT**: Can run migrations and import 567 tasks successfully

---

### Phase 2: Service Layer Refactoring (6 hours, MEDIUM RISK)

#### 2.1 Update src/tasks/service.ts
**File**: `src/tasks/service.ts` (MODIFY, 1,482 lines → ~1,500 lines)

**Strategy**: Keep identical API surface, replace JSONL I/O with SQLite queries

**Key Changes**:

1. **Add DatabaseService dependency** to all functions:
```typescript
export const readTasks = (
  tasksPath: string, // DEPRECATED but keep for compatibility
): Effect.Effect<Task[], TaskServiceError, DatabaseService> =>
  Effect.gen(function* () {
    const db = yield* DatabaseService;
    return yield* db.listTasks({ deleted: false }).pipe(
      Effect.mapError(e => new TaskServiceError("read_error", e.message))
    );
  });
```

2. **Replace file I/O with SQL**:
   - `readTasks()` → `SELECT * FROM tasks WHERE deleted_at IS NULL`
   - `createTask()` → `INSERT INTO tasks` + `INSERT INTO task_dependencies`
   - `updateTask()` → `UPDATE tasks SET ... WHERE id = ?`
   - `closeTask()` → `UPDATE tasks SET status='closed', closed_at=?, close_reason=? WHERE id=?`
   - `deleteTask()` → `UPDATE tasks SET deleted_at=? WHERE id=?` (soft delete)

3. **Update ready tasks query** (CRITICAL for performance):
```sql
SELECT t.* FROM tasks t
WHERE t.status = 'open'
AND t.deleted_at IS NULL
AND NOT EXISTS (
  SELECT 1 FROM task_dependencies td
  JOIN tasks blocker ON td.depends_on_task_id = blocker.id
  WHERE td.task_id = t.id
  AND td.dependency_type IN ('blocks', 'parent-child')
  AND blocker.status IN ('open', 'in_progress')
  AND blocker.deleted_at IS NULL
)
ORDER BY t.priority ASC, t.created_at ASC
```

4. **Preserve two-phase commit**:
   - Keep `pendingCommit` field in schema
   - Keep `commit_pending` status
   - `findTasksWithPendingCommit()` → `SELECT * FROM tasks WHERE status = 'commit_pending'`

5. **Archive functionality**:
   - DEPRECATE `archiveTasks()` - use soft delete instead
   - `readArchivedTasks()` → `SELECT * FROM tasks WHERE deleted_at IS NOT NULL`

6. **Deletion tombstones**:
   - `recordDeletion()` → `INSERT INTO task_deletions`
   - `readDeletions()` → `SELECT * FROM task_deletions`

**Functions to Update** (18 total):
- ✅ `readTasks` - List all non-deleted tasks
- ✅ `writeTasks` - DEPRECATE (no longer needed)
- ✅ `createTask` - Insert task + dependencies
- ✅ `updateTask` - Update task fields
- ✅ `closeTask` - Mark closed with timestamp
- ✅ `reopenTask` - Clear closed status
- ✅ `listTasks` - List with filters
- ✅ `readyTasks` - Complex EXISTS query
- ✅ `pickNextTask` - Get highest priority ready task
- ✅ `addComment` - Update comments JSON array
- ✅ `listComments` - Extract from JSON
- ✅ `renameTaskPrefix` - Bulk UPDATE
- ✅ `mergeTasksById` - Bulk UPDATE + DELETE
- ❌ `archiveTasks` - DEPRECATE
- ✅ `readArchivedTasks` - Filter deleted_at IS NOT NULL
- ✅ `findTasksWithPendingCommit` - Used by recovery.ts
- ✅ `getTaskStats` - GROUP BY queries
- ✅ `getStaleTasks` - Filter by updated_at

**Complexity**: HIGH - 1,482 lines to refactor
**Risk**: MEDIUM - API surface stays same, internals change

---

#### 2.2 Update src/tasks/repository.ts
**File**: `src/tasks/repository.ts` (MODIFY, ~50 lines)

**Changes**: MINIMAL - Repository is a facade
- Update path resolution to use `.openagents/openagents.db` instead of `tasks.jsonl`
- No other changes if service.ts API stays same

**Complexity**: LOW
**Risk**: LOW

---

#### 2.3 Keep src/tasks/project.ts Unchanged
**File**: `src/tasks/project.ts` (NO CHANGE)

**Decision**: Keep ProjectConfig as JSON file
**Rationale**: Simple, human-editable, version-control friendly

**Complexity**: NONE
**Risk**: NONE

---

#### 2.4 Update src/tasks/integrity.ts
**File**: `src/tasks/integrity.ts` (MODIFY, ~100 lines)

**Changes**:
- Check `.openagents/openagents.db` exists instead of `tasks.jsonl`
- Validate SQLite file integrity with `PRAGMA integrity_check`
- Keep git tracking checks (ls-files, skip-worktree, assume-unchanged)
- Remove JSONL conflict marker detection

**Complexity**: LOW
**Risk**: LOW

---

#### 2.5 Update src/tasks/hooks.ts
**File**: `src/tasks/hooks.ts` (MODIFY, ~100 lines)

**Changes**:
- Update git hooks to validate SQLite instead of JSONL
- Change conflict marker check to query `_schema_version` table
- Update hook scripts to check DB integrity on merge/checkout

**Complexity**: LOW
**Risk**: LOW

---

#### 2.6 Update src/tasks/init.ts
**File**: `src/tasks/init.ts` (MODIFY, ~150 lines)

**Changes**:
1. Create `.openagents/openagents.db` instead of `tasks.jsonl`
2. Run migrations on init
3. Keep `project.json` creation
4. Update git merge driver (or remove if no longer needed)

**Complexity**: MEDIUM
**Risk**: MEDIUM - First-run experience

---

#### 2.7 Update src/tasks/cli.ts
**File**: `src/tasks/cli.ts` (MODIFY, ~50 new lines)

**Changes**:
- Update all commands to use DatabaseService
- Add new commands:
  - `migrate` - Import JSONL to SQLite
  - `db:vacuum` - Compact database
  - `db:integrity` - Run PRAGMA integrity_check
- Keep existing commands with same behavior

**Complexity**: MEDIUM
**Risk**: LOW - CLI is well-tested

---

#### 2.8 Deprecate src/tasks/merge.ts
**File**: `src/tasks/merge.ts` (MODIFY, add deprecation warnings)

**Changes**:
- Add deprecation warnings to all functions
- Update to work with SQLite queries instead of JSONL
- Keep three-way merge logic for transition

**Complexity**: MEDIUM
**Risk**: LOW - Can remove later

**CHECKPOINT**: Service layer compiles without errors, TypeScript typechecks pass

---

### Phase 3: Test Updates (4 hours, MEDIUM RISK)

#### 3.1 Create Test Database Helper
**File**: `src/tasks/test-helpers.ts` (NEW, ~100 lines)

**Purpose**: Factory for in-memory SQLite databases in tests

**Implementation**:
```typescript
export const makeTestDatabase = (): Effect.Effect<
  { db: Database; dbPath: string; cleanup: () => void },
  never,
  FileSystem.FileSystem
> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const tmpDir = yield* fs.makeTempDirectory({ prefix: "test-db" });
    const dbPath = path.join(tmpDir, "test.db");
    const db = new Database(dbPath);

    // Run migrations
    yield* runMigrations(db);

    return {
      db,
      dbPath,
      cleanup: () => {
        db.close();
        fs.remove(tmpDir);
      }
    };
  });
```

**Complexity**: LOW
**Risk**: LOW

---

#### 3.2 Update Test Files (11 files)

**Pattern**: Replace temp JSONL files with in-memory SQLite

**Before**:
```typescript
const { tasksPath } = yield* setup(); // Creates temp dir + tasks.jsonl
```

**After**:
```typescript
const { db, dbPath, cleanup } = yield* makeTestDatabase();
// ... tests ...
cleanup();
```

**Files to Update**:
1. ✅ `src/tasks/service.test.ts` - Core service tests (100+ tests)
2. ✅ `src/tasks/repository.test.ts` - Repository facade tests
3. ❌ `src/tasks/project.test.ts` - NO CHANGE (still uses JSON)
4. ✅ `src/tasks/integrity.test.ts` - SQLite integrity checks
5. ❌ `src/tasks/schema.test.ts` - NO CHANGE (pure schema validation)
6. ✅ `src/tasks/merge.test.ts` - Update for SQLite (or mark deprecated)
7. ✅ `src/tasks/init.test.ts` - Test DB creation
8. ❌ `src/tasks/id.test.ts` - NO CHANGE (pure ID generation)
9. ❌ `src/tasks/beads.test.ts` - NO CHANGE (legacy import)
10. ✅ `src/tasks/cli.integration.test.ts` - CLI commands with SQLite
11. ✅ `src/tasks/hooks.test.ts` - Git hooks with SQLite validation

**Changes Needed**: 7 files
**No Changes**: 4 files

**Complexity**: MEDIUM - Mechanical changes
**Risk**: MEDIUM - Must maintain test coverage

**CHECKPOINT**: All 11 test files pass - `bun test src/tasks/` succeeds

---

### Phase 4: Migration Execution (2 hours, HIGH RISK)

#### 4.1 Create Migration Script
**File**: `scripts/migrate-to-sqlite.ts` (NEW, ~100 lines)

**Purpose**: One-time execution script with backup and validation

**Implementation**:
```typescript
#!/usr/bin/env bun
import { Effect } from "effect";
import { BunContext } from "@effect/platform-bun";
import { importTasksFromJsonl } from "../src/storage/import-jsonl.js";

const migrate = Effect.gen(function* () {
  const rootDir = process.cwd();
  const jsonlPath = `${rootDir}/.openagents/tasks.jsonl`;
  const dbPath = `${rootDir}/.openagents/openagents.db`;
  const backupPath = `${jsonlPath}.backup`;

  console.log("Starting migration...");
  console.log(`  Source: ${jsonlPath}`);
  console.log(`  Target: ${dbPath}`);

  // 1. Backup JSONL
  console.log("\n1. Creating backup...");
  yield* fs.copyFile(jsonlPath, backupPath);
  console.log(`  ✓ Backup created: ${backupPath}`);

  // 2. Import
  console.log("\n2. Importing tasks...");
  const result = yield* importTasksFromJsonl(jsonlPath, dbPath);
  console.log(`  ✓ Imported ${result.tasksImported} tasks`);

  // 3. Validate
  console.log("\n3. Validating...");
  if (!result.validationPassed) {
    throw new Error("Validation failed!");
  }
  console.log("  ✓ Validation passed");

  // 4. Rename original
  console.log("\n4. Renaming original...");
  yield* fs.rename(jsonlPath, `${jsonlPath}.migrated`);
  console.log(`  ✓ Renamed to ${jsonlPath}.migrated`);

  console.log("\n✅ Migration complete!");
});

Effect.runPromise(migrate.pipe(Effect.provide(BunContext.layer)))
  .then(() => process.exit(0))
  .catch((err) => {
    console.error("\n❌ Migration failed:", err);
    process.exit(1);
  });
```

**Complexity**: MEDIUM
**Risk**: HIGH - Point of no return

---

#### 4.2 Execute Migration

**Steps**:
```bash
# 1. Run migration script
bun scripts/migrate-to-sqlite.ts

# 2. Verify import
bun src/tasks/cli.ts list --json | jq 'length'  # Should be 567

# 3. Spot check random tasks
bun src/tasks/cli.ts list --json | jq '.[10]'
bun src/tasks/cli.ts list --json | jq '.[100]'
bun src/tasks/cli.ts list --json | jq '.[500]'

# 4. Verify ready tasks
bun src/tasks/cli.ts ready --json
```

**Rollback if Needed**:
```bash
cd .openagents
mv tasks.jsonl.backup tasks.jsonl
rm openagents.db
git checkout src/
```

**CHECKPOINT**: 567 tasks in SQLite, validation passed

---

### Phase 5: Dependent Files (1 hour, LOW RISK)

#### 5.1 Analyze Impact on Dependent Files

**Files that import from tasks/** (31 files identified):
- `src/agent/do-one-task.ts` - Uses `pickNextTask()`, `updateTask()`
- `src/agent/overnight-parallel.ts` - Uses `createTask()`, `updateTask()`
- `src/agent/orchestrator/recovery.ts` - Uses `findTasksWithPendingCommit()`
- `src/desktop/handlers.ts` - Uses `readyTasks()`
- `src/researcher/tasks.ts` - Uses `listTasks()`
- Plus 26 more...

**Required Changes**: **LIKELY NONE**

**Rationale**:
- Service functions keep identical signatures
- DatabaseService provided at app/entry point level
- All dependent files work transparently

**If changes needed**:
- Add DatabaseService to Effect requirements
- Update call sites to provide DatabaseService layer
- Re-run tests for each affected file

**Complexity**: LOW (if API transparent) / MEDIUM (if explicit dependency)
**Risk**: LOW - Service layer abstracts differences

**CHECKPOINT**: All dependent tests pass - `bun test` succeeds

---

### Phase 6: Final Validation (3 hours, CRITICAL)

#### 6.1 Test Execution Plan

**Order**:
1. Unit tests: `bun test src/tasks/*.test.ts`
2. Integration tests: `bun test src/tasks/cli.integration.test.ts`
3. Dependent tests: `bun test src/agent/**/*.test.ts`
4. Full suite: `bun test`

**Success Criteria** (all must pass):
1. ✅ All 11 task test files pass
2. ✅ All 31 dependent file tests pass
3. ✅ Desktop UI loads 567 tasks correctly
4. ✅ MechaCoder can create/update/close tasks
5. ✅ Two-phase commit recovery works
6. ✅ Ready tasks query returns correct results
7. ✅ Performance: ready tasks <10ms, create <5ms, list <50ms
8. ✅ Git hooks validate SQLite (not JSONL)
9. ✅ CLI commands work (list, ready, next, create, update, close)
10. ✅ No data loss (567 tasks intact)

---

#### 6.2 Performance Benchmarks

**Commands**:
```bash
# Ready tasks query (target: <10ms for 567 tasks)
time bun src/tasks/cli.ts ready --limit 10

# Task creation (target: <5ms)
time bun src/tasks/cli.ts create --title "Benchmark test" --type task

# Full list (target: <50ms for 567 tasks)
time bun src/tasks/cli.ts list
```

**If performance is slow**:
- Run `EXPLAIN QUERY PLAN` on slow queries
- Add missing indexes
- Consider denormalizing "ready" flag

**Complexity**: LOW
**Risk**: MEDIUM - Performance critical for desktop UI

---

#### 6.3 Desktop UI Validation

**Manual Testing**:
1. Start desktop: `bun run desktop`
2. Open mainview HUD
3. Verify MCTasksWidget loads tasks
4. Test task assignment to MechaCoder
5. Verify ready tasks display correctly
6. Test task filtering/sorting

**Expected**: All 567 tasks load in <100ms

---

#### 6.4 MechaCoder Integration Test

**Test**:
```bash
# Run one task end-to-end
bun src/agent/do-one-task.ts --dir .

# Should:
# 1. Pick next ready task
# 2. Mark in_progress
# 3. Complete task
# 4. Commit changes
# 5. Mark closed
# 6. Update DB
```

**Expected**: No errors, task closes successfully

**CHECKPOINT**: All systems operational, production-ready

---

## Critical Files to Modify (Order of Implementation)

### Tier 1: Foundation (Must complete first)
1. `src/storage/database.ts` (NEW, ~500 lines) - DatabaseService implementation
2. `.openagents/migrations/001_initial_schema.sql` (NEW, ~200 lines) - Schema definition
3. `src/storage/migrations.ts` (NEW, ~150 lines) - Migration runner
4. `src/storage/import-jsonl.ts` (NEW, ~300 lines) - JSONL import script

### Tier 2: Service Layer (Depends on Tier 1)
5. `src/tasks/service.ts` (MODIFY, 1,482 → 1,500 lines) - Replace JSONL with SQLite
6. `src/tasks/repository.ts` (MODIFY, ~50 lines) - Update path resolution
7. `src/tasks/integrity.ts` (MODIFY, ~100 lines) - SQLite integrity checks
8. `src/tasks/hooks.ts` (MODIFY, ~100 lines) - Git hooks for SQLite
9. `src/tasks/init.ts` (MODIFY, ~150 lines) - Create DB instead of JSONL
10. `src/tasks/cli.ts` (MODIFY, +50 lines) - Add migration commands
11. `src/tasks/merge.ts` (MODIFY, deprecation warnings) - Mark deprecated

### Tier 3: Testing (Depends on Tier 2)
12. `src/tasks/test-helpers.ts` (NEW, ~100 lines) - Test database factory
13. `src/tasks/service.test.ts` (MODIFY, ~500 lines) - Update for SQLite
14. `src/tasks/repository.test.ts` (MODIFY, ~200 lines) - Update for SQLite
15. `src/tasks/integrity.test.ts` (MODIFY, ~100 lines) - SQLite checks
16. `src/tasks/merge.test.ts` (MODIFY, ~150 lines) - Update for SQLite
17. `src/tasks/init.test.ts` (MODIFY, ~100 lines) - Test DB creation
18. `src/tasks/cli.integration.test.ts` (MODIFY, ~200 lines) - CLI with SQLite
19. `src/tasks/hooks.test.ts` (MODIFY, ~100 lines) - Git hooks tests

### Tier 4: Migration Execution (Depends on Tier 3)
20. `scripts/migrate-to-sqlite.ts` (NEW, ~100 lines) - Migration script

### Tier 5: Validation (Depends on Tier 4)
21. Desktop UI testing (manual)
22. MechaCoder integration test (manual)
23. Performance benchmarks (automated)

---

## Risk Mitigation

### High-Risk Areas

1. **Schema Mismatch**
   - **Risk**: SQL schema doesn't match TypeScript Task schema
   - **Mitigation**: Manual validation, spot checks on import
   - **Fallback**: Regenerate SQL from schema.ts

2. **Two-Phase Commit Breakage**
   - **Risk**: `pendingCommit` field lost or corrupted
   - **Mitigation**: Keep field in schema, test recovery.ts thoroughly
   - **Fallback**: Revert to JSONL if recovery fails

3. **Ready Tasks Performance**
   - **Risk**: Complex EXISTS query is slow (>10ms)
   - **Mitigation**: Composite index on (status, priority, created_at)
   - **Fallback**: Denormalize "ready" flag into separate column

4. **Test Failures**
   - **Risk**: Tests fail after migration
   - **Mitigation**: Fix bugs, re-run import with clean DB
   - **Fallback**: Rollback to JSONL, debug offline

5. **Data Loss**
   - **Risk**: Import corrupts or loses tasks
   - **Mitigation**: Backup before migration, validate row counts
   - **Fallback**: Restore from backup, investigate import script

### Unknowns

1. **FTS5 Performance**
   - Unknown: How fast is full-text search on 567 tasks?
   - Plan: Benchmark after import, add indexes if slow

2. **Concurrent Writes**
   - Unknown: How do parallel agents handle SQLite locks?
   - Plan: Test with overnight-parallel.ts, add retries if needed

3. **Transaction Overhead**
   - Unknown: Do transactions slow down writes?
   - Plan: Benchmark create/update, optimize if >10ms

---

## Rollback Plan

**Abort migration if**:
1. Import validation fails (row count ≠ 567)
2. >5 test files fail after migration
3. Critical bug in DatabaseService (data corruption)
4. Performance regression >10x slower
5. Desktop UI crashes on load

**Rollback steps**:
```bash
cd .openagents
mv tasks.jsonl.backup tasks.jsonl
rm openagents.db
git checkout src/
bun test  # Should pass with JSONL
```

---

## Success Metrics

**Before declaring success, verify ALL 10**:

1. ✅ All 11 task test files pass (`bun test src/tasks/`)
2. ✅ All 31 dependent files work (tests pass)
3. ✅ Desktop UI loads 567 tasks correctly
4. ✅ MechaCoder can create/update/close tasks
5. ✅ Two-phase commit recovery works (test with recovery.ts)
6. ✅ Ready tasks query returns correct results
7. ✅ Performance: ready tasks <10ms, create <5ms, list <50ms
8. ✅ Git hooks validate SQLite (not JSONL)
9. ✅ CLI commands work (list, ready, next, create, update, close)
10. ✅ No data loss (567 tasks intact, spot-checked)

**If all pass**: Commit, push, celebrate! 🎉

**If any fail**: Debug, fix, re-test. Do not merge.

---

## Future Enhancements (Post-Migration)

**After successful migration:**

1. **Remove JSONL support** (breaking change)
   - Delete `readTasks()`, `writeTasks()` JSONL code paths
   - Remove JSONL fallback in service.ts

2. **Optimize queries**
   - Add materialized views for stats
   - Denormalize "ready" flag for performance

3. **Add vector search**
   - Install sqlite-vec extension
   - Add embeddings for semantic task search

4. **Remove merge.ts**
   - SQLite transactions eliminate most conflicts
   - Simplify to LAST_WRITE_WINS for edge cases

5. **Migrate other JSONL files**
   - `memories.jsonl` → SQLite
   - `trajectories/` → SQLite
   - `usage.jsonl` → SQLite

---

## Estimated Timeline

**Assuming full-time focus, one developer:**

| Phase | Time | Cumulative |
|-------|------|------------|
| 1. Database Foundation | 4 hours | 4 hours |
| 2. Service Layer | 6 hours | 10 hours |
| 3. Test Updates | 4 hours | 14 hours |
| 4. Migration Execution | 2 hours | 16 hours |
| 5. Dependent Files | 1 hour | 17 hours |
| 6. Validation & Testing | 3 hours | 20 hours |

**Total**: 20 hours (~2.5 days)
**Buffer**: +50% for debugging = 30 hours (~4 days)
**Realistic**: 1 week with testing

---

## Implementation Notes

### Effect TypeScript Patterns

**Modern Effect.gen** (no adapter):
```typescript
Effect.gen(function* () {
  const db = yield* DatabaseService;
  const task = yield* db.getTask(id);
  return task;
})
```

**Providing Layers**:
```typescript
// ✅ CORRECT
Effect.provide(program, DatabaseLive)

// ❌ WRONG
Layer.provide(DatabaseLive, program)
```

**Error Mapping**:
```typescript
db.listTasks().pipe(
  Effect.mapError(e => new TaskServiceError("read_error", e.message))
)
```

### Bun SQLite API

**Opening Database**:
```typescript
import { Database } from "bun:sqlite";
const db = new Database(".openagents/openagents.db");
```

**Transactions**:
```typescript
db.transaction(() => {
  // All statements here are atomic
  stmt1.run(...);
  stmt2.run(...);
})();
```

**Prepared Statements**:
```typescript
const stmt = db.prepare("SELECT * FROM tasks WHERE id = ?");
const task = stmt.get(taskId);
```

---

## Conclusion

This plan provides a comprehensive, step-by-step approach to migrating the entire task system from JSONL to SQLite in **one session**. The key to success is:

1. **Follow the sequence exactly** - Don't skip phases
2. **Validate at each checkpoint** - Don't proceed if tests fail
3. **Maintain API surface** - Minimize changes to dependent files
4. **Backup before migration** - Always have a rollback plan
5. **Test thoroughly** - All 10 success criteria must pass

**Next Steps**:
1. Review this plan with user
2. Clarify any ambiguities
3. Execute Phase 1: Database Foundation
4. Checkpoint: Can import 567 tasks
5. Continue through phases sequentially
6. Validate all success criteria
7. Commit and celebrate! 🚀
