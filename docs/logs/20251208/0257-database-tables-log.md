# 0257 Database Tables Log

## Summary
Documented all tables in `.openagents/openagents.db` SQLite database.

## Database Tables

### Core Tables

1. **`_schema_version`**
   - Tracks database schema version
   - Fields: `version` (TEXT PRIMARY KEY), `applied_at` (TEXT)

2. **`tasks`**
   - Main task tracking table
   - Primary key: `id` (TEXT)
   - Core fields: `title`, `description`, `status`, `priority`, `type`
   - JSON fields: `labels`, `commits`, `comments`, `pending_commit`
   - Extended fields: `design`, `acceptance_criteria`, `notes`, `estimated_minutes`
   - Source tracking: `source_repo`, `source_discovered_from`, `source_external_ref`
   - Timestamps: `created_at`, `updated_at`, `closed_at`
   - Soft delete: `deleted_at`

3. **`task_dependencies`**
   - Tracks relationships between tasks
   - Composite primary key: `(task_id, depends_on_task_id)`
   - Fields: `task_id`, `depends_on_task_id`, `dependency_type`, `created_at`
   - Dependency types: `blocks`, `related`, `parent-child`, `discovered-from`

4. **`task_deletions`**
   - Tracks soft-deleted tasks
   - Primary key: `task_id` (TEXT)
   - Fields: `task_id`, `deleted_at`, `deleted_by`, `reason`

5. **`inferences`**
   - Stores LLM inference requests and responses
   - Primary key: `id` (INTEGER AUTOINCREMENT)
   - Request fields: `model`, `request_id`, `request_messages` (JSON), `request_options` (JSON)
   - Response fields: `response_data` (JSON), `response_id`, `response_model`, `response_content`
   - Usage metrics: `prompt_tokens`, `completion_tokens`, `total_tokens`, `cost_usd`
   - Timestamp: `created_at`

6. **`hillclimber_configs`**
   - Stores hillclimber optimization configurations
   - Primary key: `id` (INTEGER AUTOINCREMENT)
   - Fields: `task_id`, `hint`, `use_skills`, `max_turns_override`, `config_hash`, `is_current`, `created_at`
   - Unique constraint: `(task_id, config_hash)`

7. **`hillclimber_runs`**
   - Stores hillclimber run results
   - Primary key: `id` (INTEGER AUTOINCREMENT)
   - Fields: `run_id` (UNIQUE), `task_id`, `config_id`, `passed`, `turns`, `duration_ms`, `step_summary`, `error_message`
   - Meta-reasoning: `meta_model`, `proposed_change`, `change_accepted`
   - Scoring: `score`, `is_best`
   - Timestamp: `created_at`

8. **`hillclimber_best_configs`**
   - Tracks best performing configurations per task
   - Primary key: `task_id` (TEXT)
   - Fields: `task_id`, `config_id`, `run_id`, `score`, `pass_count`, `total_runs`, `updated_at`

### Full-Text Search (FTS5) Tables

9. **`tasks_fts`**
   - Virtual table for full-text search on tasks
   - Indexed fields: `title`, `description`
   - Content table: `tasks`

10. **`tasks_fts_data`**, **`tasks_fts_idx`**, **`tasks_fts_docsize`**, **`tasks_fts_config`**
    - FTS5 auxiliary tables for `tasks_fts`

11. **`inferences_fts`**
    - Virtual table for full-text search on inferences
    - Indexed field: `response_content`
    - Content table: `inferences`

12. **`inferences_fts_data`**, **`inferences_fts_idx`**, **`inferences_fts_docsize`**, **`inferences_fts_config`**
    - FTS5 auxiliary tables for `inferences_fts`

### System Tables

13. **`sqlite_sequence`**
    - SQLite system table for AUTOINCREMENT sequences
    - Fields: `name`, `seq`

## Indexes

The database includes numerous indexes for performance:
- Task indexes: status, priority, type, assignee, timestamps, composite indexes
- Task dependency indexes: task_id, depends_on_task_id
- Inference indexes: model, response_model, request_id, created_at, cost
- Hillclimber indexes: task_id, config_id, is_current, is_best, created_at

## Triggers

- FTS triggers for automatic updates to `tasks_fts` and `inferences_fts` when base tables change

## Total Tables: 20

- 8 core application tables
- 8 FTS5 auxiliary tables (4 for tasks, 4 for inferences)
- 1 schema version table
- 1 SQLite system table
- 2 FTS5 virtual tables

