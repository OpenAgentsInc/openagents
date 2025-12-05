# 0841 Work Log (oa-48215f)
- Migrated edit tool schema to support SDK naming (file_path, old_string/new_string) with legacy aliases.
- Added replace_all flag to allow multi-occurrence replacements; kept unique-match validation when not set.
- Updated edit tests for SDK aliases and new replace_all behavior.
- Tests: `bun run typecheck`, `bun test` (pass).
