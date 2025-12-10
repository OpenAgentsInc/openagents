# User Stories Rust Implementation Analysis & Plan

## Executive Summary

**Total User Stories:** 210 stories across 9 categories
**Currently Implemented in Rust:** ~25% (52/210)
**Partially Implemented:** ~10% (21/210)
**Not Implemented:** ~65% (137/210)

---

## Implementation Status by Category

### 1. Desktop Application (HUD) - 43 Stories

| Section | Stories | Rust Status | Coverage |
|---------|---------|-------------|----------|
| App Launch & Lifecycle | HUD-001..006 | **Partial** | 50% |
| Canvas Interactions | HUD-010..017 | **Implemented** | 90% |
| Node Display | HUD-020..029 | **Partial** | 40% |
| Real-Time Updates | HUD-030..040 | **Implemented** | 80% |
| APM Widget | HUD-050..057 | **Partial** | 50% |
| Error Handling | HUD-060..064 | **Implemented** | 90% |

**Implemented in Rust:**
- `crates/hud/` - GraphView with pan/zoom/reset (HUD-010-017)
- `crates/hud_test/` - Protocol types, message factories, test fixtures
- `crates/hud/src/tests/` - 42 tests covering HUD-001, HUD-002, HUD-010-017, HUD-030-040, HUD-060-064

**Missing:**
- Node hierarchy display (HUD-020-029) - needs Unit dataflow integration
- Historical APM (HUD-055-056) - time window tracking
- Inertial scrolling (HUD-013), trackpad gestures (HUD-017)

---

### 2. Task System - 26 Stories

| Section | Stories | Rust Status | Coverage |
|---------|---------|-------------|----------|
| Task Creation | TASK-001..006 | **Not Implemented** | 0% |
| Task Listing | TASK-010..016 | **Not Implemented** | 0% |
| State Management | TASK-020..026 | **Not Implemented** | 0% |
| Dependencies | TASK-030..034 | **Not Implemented** | 0% |

**Current Location:** TypeScript in `src/tasks/`

**Needed for Rust:**
- Task storage layer (SQLite or JSONL parsing)
- CRUD operations for tasks.jsonl
- Priority queue logic
- Dependency resolution

---

### 3. Agent Orchestrator (Golden Loop) - 38 Stories

| Section | Stories | Rust Status | Coverage |
|---------|---------|-------------|----------|
| Task Selection | ORCH-001..005 | **Not Implemented** | 0% |
| Subagent Execution | ORCH-010..017 | **Not Implemented** | 0% |
| Verification | ORCH-020..025 | **Not Implemented** | 0% |
| Git Operations | ORCH-030..035 | **Not Implemented** | 0% |
| Session Management | ORCH-040..046 | **Not Implemented** | 0% |
| Error Recovery | ORCH-050..053 | **Not Implemented** | 0% |
| Safe Mode | ORCH-070..072 | **Not Implemented** | 0% |
| Sandbox | ORCH-080..082 | **Not Implemented** | 0% |

**Note:** `crates/hillclimber/` is related but implements MAP optimization, not task orchestration.

**Current Location:** TypeScript in `src/agent/orchestrator/`

---

### 4. CLI Commands - 22 Stories

| Section | Stories | Rust Status | Coverage |
|---------|---------|-------------|----------|
| Task CLI | CLI-001..007 | **Not Implemented** | 0% |
| MechaCoder CLI | CLI-010..015 | **Not Implemented** | 0% |
| Session CLI | CLI-020..026 | **Not Implemented** | 0% |
| Worktree CLI | CLI-030..032 | **Not Implemented** | 0% |

**Current Location:** TypeScript `bun run` scripts

**Partial Rust:** `fm-bridge` has CLI for FM inference (`fm complete`, `fm models`)

---

### 5. LLM Provider Integration - 13 Stories

| Section | Stories | Rust Status | Coverage |
|---------|---------|-------------|----------|
| Provider Selection | LLM-001..005 | **Partial** | 20% |
| Chat & Tool Calling | LLM-010..015 | **Partial** | 40% |
| Token Accounting | LLM-020..024 | **Not Implemented** | 0% |

**Implemented in Rust:**
- `crates/fm-bridge/` - Apple Foundation Model client (LLM-001 partial)
- Chat completions, model listing, health checks
- Token tracking (basic)

**Missing:**
- OpenRouter/Anthropic/OpenAI providers
- Tool calling (LLM-011-012)
- Streaming (LLM-013-014)
- Cost accounting (LLM-023)

---

### 6. Core Tools - 16 Stories

| Section | Stories | Rust Status | Coverage |
|---------|---------|-------------|----------|
| File Reading | TOOL-001..004 | **Not Implemented** | 0% |
| File Writing | TOOL-010..013 | **Not Implemented** | 0% |
| File Search | TOOL-020..023 | **Not Implemented** | 0% |
| Shell Execution | TOOL-030..033 | **Not Implemented** | 0% |

**Current Location:** TypeScript in `src/tools/` and `src/skills/`

---

### 7. Project Configuration - 14 Stories

| Section | Stories | Rust Status | Coverage |
|---------|---------|-------------|----------|
| Basic Config | CONF-001..005 | **Not Implemented** | 0% |
| Safety Config | CONF-010..013 | **Not Implemented** | 0% |
| Claude Code Config | CONF-020..024 | **Not Implemented** | 0% |
| Sandbox Config | CONF-030..033 | **Not Implemented** | 0% |

**Current Location:** TypeScript config loading

---

### 8. Parallel Execution & Worktrees - 10 Stories

| Section | Stories | Rust Status | Coverage |
|---------|---------|-------------|----------|
| Parallel Orchestrator | PAR-001..005 | **Not Implemented** | 0% |
| Worktree Isolation | PAR-010..013 | **Not Implemented** | 0% |

**Current Location:** TypeScript-based

---

### 9. TerminalBench Command Center - 18 Stories

| Section | Stories | Rust Status | Coverage |
|---------|---------|-------------|----------|
| Dashboard | TBCC-001..005 | **Partial** | 30% |
| Task Browser | TBCC-010..014 | **Partial** | 30% |
| Run Browser | TBCC-020..024 | **Partial** | 30% |
| Settings | TBCC-030..033 | **Not Implemented** | 0% |

**Implemented in Rust:**
- `crates/storybook/` - Visual component showcase
- ATIF trajectory rendering

**Missing:** Full TBCC dashboard, task/run browsers, settings

---

## Implementation Priority Matrix

### Phase 1: Foundation (P0 Stories) - Est. ~40 stories

**Goal:** Core functionality to run MechaCoder autonomously in Rust

| Category | Key Stories | Crate | Effort |
|----------|-------------|-------|--------|
| Task System | TASK-001..003, 010..012, 020..022 | `tasks` (new) | Medium |
| Orchestrator | ORCH-001..002, 010..011, 020..022, 030..032, 040..041, 050 | `orchestrator` (new) | Large |
| Config | CONF-001..002, 010..011 | `config` (new) | Small |
| Tools | TOOL-001..002, 010..011, 020..021, 030..031 | `tools` (new) | Medium |
| LLM | LLM-010..012 | `llm` (new) | Medium |

### Phase 2: Enhanced HUD (P0-P1 Stories) - Est. ~30 stories

**Goal:** Complete HUD visualization

| Category | Key Stories | Crate | Effort |
|----------|-------------|-------|--------|
| Node Display | HUD-020..024 | `hud` | Medium |
| APM Widget | HUD-050..054 | `hud` | Small |
| Real-time | HUD-035..040 | `hud` | Small |

### Phase 3: CLI & Integration (P1 Stories) - Est. ~35 stories

**Goal:** Full CLI interface, session management

| Category | Key Stories | Crate | Effort |
|----------|-------------|-------|--------|
| Task CLI | CLI-001..007 | `cli` (new) | Medium |
| MechaCoder CLI | CLI-010..015 | `cli` | Medium |
| Session CLI | CLI-020..026 | `cli` | Medium |
| Session Replay | ORCH-045..046 | `orchestrator` | Small |

### Phase 4: Advanced Features (P1-P2 Stories) - Est. ~40 stories

**Goal:** Parallel execution, sandbox, advanced LLM

| Category | Key Stories | Crate | Effort |
|----------|-------------|-------|--------|
| Parallel | PAR-001..013 | `parallel` (new) | Large |
| Sandbox | ORCH-080..082, CONF-030..033 | `sandbox` (new) | Large |
| LLM Providers | LLM-001..005, 020..024 | `llm` | Medium |
| TBCC | TBCC-001..033 | `commander` | Large |

---

## Recommended New Crates

```
crates/
├── tasks/              # Task CRUD, JSONL storage, priority queue
├── orchestrator/       # Golden Loop, session management
├── config/             # Project.json parsing, validation
├── tools/              # File, search, shell tools
├── llm/                # Multi-provider LLM abstraction
├── cli/                # Command-line interface
├── parallel/           # Worktree management, parallel execution
└── sandbox/            # Container/Seatbelt isolation
```

---

## Missing User Stories (Suggested Additions)

Based on current codebase, these stories should be added:

### HillClimber / TestGen

| ID | Priority | User Story |
|----|----------|------------|
| HILL-001 | P1 | As a user, I can run overnight optimization |
| HILL-002 | P1 | As a user, I can see evolution progress |
| HILL-003 | P1 | As a user, I can configure MAP parameters |
| TGEN-001 | P1 | As a user, I can generate tests for a category |
| TGEN-002 | P1 | As a user, I can evolve test quality |

### ATIF Trajectories

| ID | Priority | User Story |
|----|----------|------------|
| ATIF-001 | P1 | As a user, I can record agent trajectories |
| ATIF-002 | P1 | As a user, I can replay trajectories |
| ATIF-003 | P2 | As a user, I can export trajectories to HuggingFace |

### Foundation Model Bridge

| ID | Priority | User Story |
|----|----------|------------|
| FM-001 | P0 | As a user, I can use Apple FM for inference |
| FM-002 | P1 | As a user, I can use guided generation |
| FM-003 | P2 | As a user, I can track FM token usage |

---

## Implementation Plan

### Immediate Next Steps (This Sprint)

1. **Create `crates/tasks/`** - Port task system from TypeScript
   - Task struct with serde
   - JSONL read/write
   - Ready task filtering
   - Priority queue

2. **Create `crates/config/`** - Configuration loading
   - Project.json parsing
   - Validation
   - Default values

3. **Enhance `crates/hud/`** - Node display
   - Unit integration for node hierarchy
   - Status color rendering
   - Connection paths

### Medium-term (Next Month)

4. **Create `crates/orchestrator/`** - Golden Loop
   - Session lifecycle
   - Task selection
   - Subtask decomposition
   - Event emission

5. **Create `crates/tools/`** - Core tools
   - File read/write/edit
   - Grep/find
   - Bash execution

6. **Create `crates/llm/`** - LLM abstraction
   - Provider trait
   - Anthropic/OpenAI clients
   - Tool calling support

### Long-term (Next Quarter)

7. **Create `crates/parallel/`** - Parallel execution
8. **Create `crates/sandbox/`** - Isolation
9. **Complete TBCC** - Full Terminal-Bench Command Center
10. **Port CLI** - All CLI commands to Rust

---

## Files to Modify/Create

### New Crates
- `crates/tasks/Cargo.toml`
- `crates/tasks/src/lib.rs`
- `crates/config/Cargo.toml`
- `crates/config/src/lib.rs`
- `crates/orchestrator/Cargo.toml`
- `crates/orchestrator/src/lib.rs`
- `crates/tools/Cargo.toml`
- `crates/tools/src/lib.rs`
- `crates/llm/Cargo.toml`
- `crates/llm/src/lib.rs`

### Existing Crates to Modify
- `crates/hud/src/graph_view.rs` - Node hierarchy
- `crates/hud/src/lib.rs` - Unit integration
- `crates/commander/src/main.rs` - Orchestrator integration
- `Cargo.toml` - Workspace members

### Documentation
- `docs/testing/USER-STORIES.md` - Add HILL/TGEN/ATIF/FM stories
- `docs/rust-migration.md` - New doc for tracking migration

---

## Success Metrics

| Metric | Current | Phase 1 Target | Full Target |
|--------|---------|----------------|-------------|
| Stories Implemented | 52/210 (25%) | 92/210 (44%) | 200/220 (91%) |
| P0 Stories | 30/55 (55%) | 55/55 (100%) | 55/55 (100%) |
| Test Coverage | 78 tests | 150 tests | 300+ tests |
| Crates | 22 | 27 | 30 |
