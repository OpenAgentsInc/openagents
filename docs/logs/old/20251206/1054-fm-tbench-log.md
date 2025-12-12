# 1054 FM Terminal-Bench Implementation Log

## Summary

Implementing FM Terminal-Bench loop end-to-end testing. Key issues found and fixed:

## Issues Found

### 1. SkillMatch Export Bug (Pre-existing)
- **Error**: `export 'SkillMatch' not found in './schema.js'`
- **Root cause**: Bun requires interfaces to be re-exported with `export type` syntax
- **Fix**: Updated `src/skills/index.ts` to use `export type { SkillFilter, SkillQuery, SkillMatch }`

### 2. FM Writing to Wrong Path
- **Error**: Model was writing to `/tmp/hello.txt` instead of workspace
- **Root cause**: FM didn't understand workspace context; tool execution used `resolve()` which preserves absolute paths
- **Fix**: Added `normalizePath()` helper that converts absolute paths to relative using `basename()`

### 3. FM Context Window Exceeded
- **Error**: `Exceeded model context window size` on first turn
- **Root cause**: Apple FM has very small context window; system prompt was too verbose
- **Fix**: Shortened FM_BASE_PROMPT significantly:
  ```
  You are a coding assistant. Use relative paths only.
  Tools (use <tool_call>...):
  - write_file(path, content)
  - read_file(path) 
  - edit_file(path, old_text, new_text)
  - run_command(command)
  Say TASK_COMPLETE when done.
  ```

## Current Status
Testing simplified FM prompt with hello-world task.

## Files Modified
- `src/skills/index.ts` - Fixed type exports
- `src/bench/model-adapter.ts` - Shortened FM prompt, added normalizePath(), added workspace context
- `docs/terminal-bench.md` - Added FM documentation

## Commits
- `ec8f74a`: fix: export interface types properly in skills/index.ts
- `59e3e80`: feat: FM Terminal-Bench loop with learning layers

