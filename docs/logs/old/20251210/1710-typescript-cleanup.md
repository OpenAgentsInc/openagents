# TypeScript Cleanup Progress

**Date:** 2025-12-10 17:10 (Updated 18:10)
**Status:** In Progress

---

## Summary

Deleted 101 TypeScript files that have been ported to Rust.
Restored files that were incorrectly deleted (not ported).

## Files Deleted

### HillClimber (43 files → 0)
Entire `src/hillclimber/` directory removed. Replaced by `crates/hillclimber/`.

Key files deleted:
- decomposer.ts → decomposer.rs
- evaluator.ts → evaluator.rs
- monitor.ts → monitor.rs
- scoring.ts → scoring.rs
- runner.ts → runner.rs
- store.ts → store.rs
- types.ts → types.rs
- executor.ts → orchestrator.rs
- map-orchestrator.ts → orchestrator.rs
- parallel-sampler.ts → sampler.rs
- testgen-*.ts → crates/testgen/

### Sandbox (12 files → 0)
Entire `src/sandbox/` directory removed. Replaced by `crates/sandbox/`.

Key files deleted:
- docker.ts → docker.rs
- macos-container.ts → macos.rs
- detect.ts → detect.rs
- credentials.ts → credentials.rs
- backend.ts → backend.rs
- schema.ts → config.rs + error.rs

### TBCC (10 files → 0)
Entire `src/effuse/components/tb-command-center/` removed. Replaced by `crates/gym/src/tbcc/`.

Key files deleted:
- tbcc-dashboard.ts → dashboard.rs
- tbcc-task-browser.ts → task_browser.rs
- tbcc-run-browser.ts → run_browser.rs
- tbcc-settings.ts → settings.rs
- tbcc-shell.ts → shell.rs
- tbcc-testgen.ts → testgen/ module

### LLM (5 core files)
From `src/llm/`, core files removed:
- openai.ts → openai.rs
- anthropic.ts → anthropic.rs
- retry.ts → retry.rs
- models.ts → models.rs
- model-types.ts → (merged into models.rs)

## Before/After

| Metric | Before | After | Change |
|--------|--------|-------|--------|
| Total TS files | 633 | 534 | -99 |
| HillClimber TS | 43 | 0 | -43 |
| Sandbox TS | 12 | 0 | -12 |
| Tools TS | 19 | 0 | -19 |
| HUD TS | 14 | 0 | -14 |
| TBench-HUD TS | 6 | 0 | -6 |
| LLM TS (core) | 5 | 0 | -5 |

## Files Restored (Not Ported)

Restored after incorrectly deleting:
- src/atif/ - Has crates/atif but not fully ported
- src/fm/ - Has crates/fm-bridge but not fully ported
- src/effuse/ - UI framework replaced by GPUI but not deleted
- src/mainview/ - Frontend for web UI
- src/archivist/ - Not ported
- src/guardrails/ - Not ported
- src/reflexion/ - Not ported
- And other utility directories

## Rust Crate Stats

| Crate | Files | Lines |
|-------|-------|-------|
| hillclimber | 14 | 7,181 |
| sandbox | 8 | 1,393 |
| gym | 40 | 9,183 |
| llm | 10 | 3,851 |
| testgen | 11 | ~2,000 |

## Remaining TypeScript (563 files)

Major directories still in TypeScript:
- src/agent/ (102 files) - MechaCoder agent
- src/effuse/ (~80 files) - UI framework
- src/llm/ (~35 files) - Additional providers (gemini, openrouter, ollama)
- src/cli/ (30 files) - CLI utilities
- src/tasks/ (24 files) - Task management
- src/healer/ (24 files) - Healer CLI

## Validation

All Rust crates build and tests pass:
```bash
cargo build -p hillclimber -p gym -p sandbox -p llm  # OK
cargo test -p hillclimber -p gym -p sandbox -p llm   # All pass
cargo run -p hillclimber -- stats                     # Works
```

## ATIF Module Analysis (18:10)

Analyzed `src/atif/` vs `crates/atif/` and `crates/atif-store/`.

**Rust crates:**
- `crates/atif/` - Core data types (Agent, Step, Trajectory, etc.) ✅
- `crates/atif-store/` - SQLite storage layer ✅

**TypeScript atif/ (12 files) - ALL MUST STAY:**

| File | Purpose | Why Keep |
|------|---------|----------|
| schema.ts | Effect Schema types | Used by 17+ TS files |
| collector.ts | In-memory trajectory builder | TS agent specific |
| service.ts | File-based storage | TS uses JSON, Rust uses SQLite |
| streaming-writer.ts | JSONL streaming | TS agent specific |
| validation.ts | ATIF v1.4 validation | TS agent specific |
| adapter.ts | OrchestratorEvent → ATIF | TS agent specific |
| integration.ts | TS agent loop hooks | TS agent specific |
| recovery.ts | JSONL recovery | TS agent specific |
| hud-emitter.ts | HUD emission | TS HUD (GPUI replaces) |
| hud-streaming.ts | HUD streaming | TS HUD |
| sdk-adapter.ts | SDK adapters | TS agent specific |
| index.ts | Exports | Required |

**Files importing schema.ts (cannot delete):**
- src/effuse/ (socket, tb-command-center, hf-trajectory-*)
- src/desktop/handlers.ts
- src/cli/tbench-local.ts
- src/agent/do-one-task.ts
- src/healer/ (service, types, atif, stuck)
- src/huggingface/openthoughts.ts

**Conclusion:** Keep ALL atif/*.ts files until TypeScript agent code is fully ported.

## Next Steps

1. Port remaining LLM providers (gemini, ollama, openrouter)
2. Port agent module for MechaCoder
3. Port CLI utilities
4. Remove Effuse UI framework (replaced by GPUI)
