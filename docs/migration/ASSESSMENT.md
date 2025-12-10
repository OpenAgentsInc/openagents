# Migration Assessment: TypeScript/Effect → Rust

> **Date:** 2025-12-10
> **Status:** Analysis Complete
> **Purpose:** Gap analysis for completing the TypeScript to Rust migration

---

## Executive Summary

**Current State:**
- TypeScript: ~633 files, ~186,000 lines across 42+ modules
- Rust: ~404 files, ~152,000 lines across 19+ crates (~82% line coverage)

**Key Finding:** The **core execution infrastructure is fully ported** (HillClimber, TestGen, Orchestrator, Tools, Tasks, CLI). What remains are:
1. **Learning/Memory systems** (Skills, Memory, Reflexion, Archivist) - the "lifelong learning" stack
2. **Multi-provider LLM abstraction** - currently Rust has skeleton, TypeScript has 6 providers
3. **Desktop/UI infrastructure** - Effuse components, Desktop app shell, WebSocket HUD
4. **Agent orchestration details** - subagent system, context compaction, session management

**Recommendation:** The learning systems (Skills, Memory, Reflexion) should be **deprioritized or omitted** for v1. They add complexity without being essential to the Terminal-Bench #1 goal. Focus on completing the LLM providers and UI/Desktop stack.

---

## What's Been Ported (✅ Complete)

### Core Agent Infrastructure
| Rust Crate | TypeScript Module | Status | Notes |
|------------|-------------------|--------|-------|
| `orchestrator` | `src/agent/orchestrator/` | ✅ Complete | Golden Loop, task decomposition, verification |
| `parallel` | N/A (new) | ✅ Complete | Git worktree isolation for multi-agent |
| `tasks` | `src/tasks/` | ✅ Complete | Task CRUD, dependencies, SQLite |
| `tools` | `src/tools/` | ✅ Complete | bash, read, write, edit, grep, find |
| `cli` | `src/cli/` | ✅ Complete | Task, MechaCoder, Session commands |
| `config` | various | ✅ Complete | Project configuration |

### HillClimber / Terminal-Bench
| Rust Crate | TypeScript Module | Status | Notes |
|------------|-------------------|--------|-------|
| `hillclimber` | `src/hillclimber/` | ✅ Complete | MAP architecture, decomposer, evaluator, monitor |
| `testgen` | `src/fm/testgen/` | ✅ Complete | Test generation, evolution, pytest formatting |
| `fm-bridge` | `src/llm/foundation-models.ts` | ✅ Complete | Apple FM HTTP client |

### ATIF / Trajectory
| Rust Crate | TypeScript Module | Status | Notes |
|------------|-------------------|--------|-------|
| `atif` | `src/atif/` | ✅ Complete | Trajectory schema, types |
| `atif-store` | `src/atif/service.ts` | ✅ Complete | SQLite persistence |

### UI / Visualization
| Rust Crate | TypeScript Module | Status | Notes |
|------------|-------------------|--------|-------|
| `hud` | `src/hud/` | ✅ Complete | GPUI graph visualization, APM widget |
| `theme` | CSS/inline styles | ✅ Complete | Centralized color system |
| `unit` | `src/flow/` | ✅ Complete | Visual programming framework |
| `commander` | `src/effuse/commander/` | 🔶 Partial | Library exports, needs components |
| `gym` | N/A (new) | ✅ Complete | Multi-view Terminal-Bench dashboard |

---

## What's NOT Ported (❌ TypeScript Only)

### Priority 1: Required for Feature Parity

#### 1. **Multi-Provider LLM Layer** (`src/llm/`)
**TypeScript LOC:** ~8,000
**Rust Status:** Skeleton only (`crates/llm/`)
**Gap:**
- Provider implementations: Anthropic, OpenAI, Gemini, Ollama, OpenRouter, custom
- Model selector with fallback chains
- OAuth/API key management
- Token accounting and cost tracking
- Extended thinking support
- Partial tool argument parsing

**Recommendation:** ✅ **PORT** - Essential for production use beyond Apple FM
**Effort:** Medium (2-3 weeks)

#### 2. **Desktop Application Shell** (`src/desktop/`)
**TypeScript LOC:** ~1,500
**Rust Status:** Not ported
**Gap:**
- webview-bun native window
- HTTP server for static files
- Worker thread for server
- Desktop-HUD protocol

**Recommendation:** ✅ **REPLACE with GPUI native window** - No webview needed
**Effort:** Low (1 week) - GPUI handles windowing natively

#### 3. **Effuse UI Components** (`src/effuse/`)
**TypeScript LOC:** ~4,000
**Rust Status:** Partially ported to GPUI
**Gap:**
- 15+ Effect-native widgets
- StateCell → Entity (already done conceptually)
- html tagged template → GPUI element builders
- DomService, StateService, SocketService

**Recommendation:** ✅ **PORT remaining components to GPUI** - Already started
**Effort:** Medium (2-3 weeks for remaining components)

**Components to port:**
| Component | Priority | Complexity | Notes |
|-----------|----------|------------|-------|
| TBControls | P0 | Medium | Test suite controls |
| TBResults | P0 | High | Results table with sorting |
| CategoryTree | P1 | High | Task hierarchy tree |
| ContainerPanes | P1 | Medium | Layout containers |
| HFTrajectoryList | P2 | Medium | HuggingFace integration |
| ATIFDetails | P2 | Low | Trajectory detail view |
| ATIFThread | P2 | Medium | Message thread view |
| IntroCard | P3 | Low | Welcome widget |
| ThreeBackground | P4 | High | 3D animated background (skip?) |

#### 4. **Sandbox/Container Execution** (`src/sandbox/`)
**TypeScript LOC:** ~2,000
**Rust Status:** Not ported
**Gap:**
- Docker backend implementation
- macOS container support
- Credential extraction from Keychain
- Image building, bootstrapping

**Recommendation:** ✅ **PORT** - Critical for safe agent execution
**Effort:** Medium (2 weeks)

#### 5. **Session Management** (`src/sessions/`, `src/agent/session.ts`)
**TypeScript LOC:** ~1,500
**Rust Status:** ATIF-store covers persistence, but session lifecycle missing
**Gap:**
- JSONL session format (different from ATIF)
- Session resume/replay
- Context compaction for long conversations

**Recommendation:** 🔶 **CONSOLIDATE with ATIF** - Unify formats
**Effort:** Low (1 week)

---

### Priority 2: Nice to Have for v1

#### 6. **Healer (Self-Healing Subagent)** (`src/healer/`)
**TypeScript LOC:** ~2,500
**Rust Status:** Not ported
**Gap:**
- Stuck detection
- Recovery spells (rewind, restart, escalate)
- Git status analysis
- Error pattern detection

**Recommendation:** 🔶 **DEFER to v2** - Orchestrator handles basic recovery
**Effort:** Medium (2 weeks)

#### 7. **Guardrails** (`src/guardrails/`)
**TypeScript LOC:** ~500
**Rust Status:** Not ported
**Gap:**
- Safety rule definitions
- Blocked file patterns
- Resource limits

**Recommendation:** 🔶 **PORT minimal version** - Critical for safety
**Effort:** Low (3-5 days)

#### 8. **Bench Harness** (`src/bench/`)
**TypeScript LOC:** ~3,000
**Rust Status:** HillClimber covers most functionality
**Gap:**
- Reporter formatting
- Baseline comparisons
- Metrics calculation

**Recommendation:** 🔶 **CONSOLIDATE into HillClimber** - Avoid duplication
**Effort:** Low (1 week)

#### 9. **HuggingFace Integration** (`src/huggingface/`)
**TypeScript LOC:** ~1,000
**Rust Status:** Not ported
**Gap:**
- Model downloads
- Dataset integration
- Trajectory uploads

**Recommendation:** 🔶 **PORT for trajectory sharing** - Useful for TB leaderboard
**Effort:** Low (1 week)

---

### Priority 3: Omit from v1 (Lifelong Learning Stack)

These modules implement research papers (Voyager, Generative Agents, Reflexion) that add significant complexity without being essential to Terminal-Bench #1.

#### 10. **Skills Library** (`src/skills/`)
**TypeScript LOC:** ~3,000
**Description:** Voyager-style procedural skill accumulation with embeddings
**Recommendation:** ❌ **OMIT from v1** - Nice research feature, not essential
**Rationale:** TestGen + HillClimber already learn patterns; skills add DX complexity

#### 11. **Memory System** (`src/memory/`)
**TypeScript LOC:** ~2,000
**Description:** Episodic, semantic, procedural memory with importance weighting
**Recommendation:** ❌ **OMIT from v1** - ATIF trajectories provide sufficient history
**Rationale:** Adds retrieval complexity; simple context window is sufficient

#### 12. **Reflexion** (`src/reflexion/`)
**TypeScript LOC:** ~1,500
**Description:** Post-failure self-critique generating verbal reinforcement
**Recommendation:** ❌ **OMIT from v1** - Interesting but unproven benefit
**Rationale:** HillClimber's iteration loop provides similar improvement signal

#### 13. **Archivist** (`src/archivist/`)
**TypeScript LOC:** ~2,000
**Description:** Pattern extraction from trajectories into lessons
**Recommendation:** ❌ **OMIT from v1** - Meta-learning without clear ROI
**Rationale:** Focus on direct task performance, not meta-extraction

#### 14. **Learning Orchestrator** (`src/learning/`)
**TypeScript LOC:** ~4,000
**Description:** TRM, SOAR, TTT, EMA - sophisticated meta-learning strategies
**Recommendation:** ❌ **OMIT from v1** - Research-grade complexity
**Rationale:** These are experimental; HillClimber is the proven approach

#### 15. **Trainer/Gym** (`src/trainer/`)
**TypeScript LOC:** ~2,500
**Description:** Training loop with skill/memory integration
**Recommendation:** ❌ **OMIT from v1** - Depends on Skills/Memory
**Rationale:** HillClimber IS the training system for TB

---

### Priority 4: Utility/Support Modules

These are small utilities that may or may not need porting:

| Module | LOC | Recommendation | Notes |
|--------|-----|----------------|-------|
| `src/telemetry/` | ~300 | PORT minimal | Basic metrics only |
| `src/health/` | ~500 | PORT | System diagnostics |
| `src/interop/` | ~500 | DEFER | MCP bridge - low priority |
| `src/flow/` | ~2,000 | PORTED | Now in `unit` crate |
| `src/schemas/sdk/` | ~1,500 | EVALUATE | SDK types if needed |
| `src/storage/` | ~500 | PORTED | Covered by ATIF-store |
| `src/usage/` | ~400 | DEFER | Usage tracking |
| `src/dashboard/` | ~300 | SKIP | Replaced by GPUI |

---

## Migration Effort Summary

### Phase 1: Complete Core (2-3 weeks)
Focus: Finish what's needed for Terminal-Bench #1 with multiple LLM providers

| Task | Effort | Priority |
|------|--------|----------|
| LLM providers (Anthropic, OpenAI) | 2 weeks | P0 |
| Guardrails minimal | 3 days | P0 |
| Session consolidation | 1 week | P1 |

### Phase 2: Desktop/UI (2-3 weeks)
Focus: Native GPUI desktop app replacing webview

| Task | Effort | Priority |
|------|--------|----------|
| GPUI window shell | 1 week | P0 |
| Remaining Effuse→GPUI components | 2 weeks | P0 |
| WebSocket HUD client | 1 week | P1 |

### Phase 3: Production Hardening (2 weeks)
Focus: Safe execution, monitoring

| Task | Effort | Priority |
|------|--------|----------|
| Sandbox/Docker | 2 weeks | P0 |
| Health monitoring | 3 days | P1 |
| HuggingFace integration | 1 week | P2 |

### Total: 6-8 weeks to full parity (excluding learning stack)

---

## What to Delete After Migration

Once Rust is feature-complete, these TypeScript directories can be removed:

```
src/
├── agent/          # → orchestrator, parallel
├── atif/           # → atif, atif-store
├── bench/          # → hillclimber
├── cli/            # → cli
├── config/         # → config (partial)
├── desktop/        # → commander (GPUI native)
├── effuse/         # → hud, commander, gym
├── flow/           # → unit
├── fm/             # → fm-bridge, hillclimber, testgen
├── hillclimber/    # → hillclimber
├── hud/            # → hud
├── llm/            # → llm (needs completion)
├── mainview/       # → commander (GPUI native)
├── sandbox/        # → (needs port)
├── sessions/       # → atif-store
├── tasks/          # → tasks
├── tools/          # → tools
├── tbench-*        # → hillclimber, gym
```

**Keep (no Rust equivalent needed):**
```
src/
├── skills/         # ❌ Omit from v1
├── memory/         # ❌ Omit from v1
├── reflexion/      # ❌ Omit from v1
├── archivist/      # ❌ Omit from v1
├── learning/       # ❌ Omit from v1
├── trainer/        # ❌ Omit from v1
├── healer/         # 🔶 Defer to v2
├── guardrails/     # 🔶 Port minimal
├── huggingface/    # 🔶 Defer
├── interop/        # 🔶 Defer
```

---

## Architecture Decisions

### 1. Effect Patterns → Rust Idioms
**Decision:** Use native Rust patterns, not Effect ports (per `effect-patterns-in-rust.md`)

| Effect Pattern | Rust Equivalent |
|----------------|-----------------|
| `Effect<T, E, R>` | `async fn() -> Result<T, E>` |
| `Effect.gen` | `async { }` |
| `Context.Tag` | Trait bounds + generics |
| `Layer` | Builder pattern |
| `Stream` | `tokio-stream` |
| Tagged errors | `thiserror` enums |

### 2. UI Framework
**Decision:** GPUI replaces Effuse completely (per `gpui-complete-guide.md`)

| Effuse | GPUI |
|--------|------|
| `Component<S, E>` | `impl Render` |
| `StateCell<T>` | `Entity<T>` |
| `html` template | Element builders |
| `ctx.dom.delegate()` | `cx.listener()` |

### 3. Desktop Architecture
**Decision:** Pure GPUI native window, no webview

- **Before:** Bun → webview-bun → HTTP server → WebSocket → Frontend
- **After:** Rust binary → GPUI window → Direct rendering

### 4. Learning Stack
**Decision:** Omit Skills/Memory/Reflexion/Archivist from v1

**Rationale:**
- HillClimber + TestGen provide the core learning loop
- ATIF trajectories provide sufficient history
- Complexity/benefit ratio unfavorable for TB#1 goal
- Can add in v2 if needed

---

## Success Criteria for "Full Port Complete"

### Functional Parity
- [ ] Can run all TB2 tasks through Rust HillClimber
- [ ] Can use Claude (Anthropic) and GPT-4 (OpenAI) from Rust
- [ ] Desktop app launches with GPUI window
- [ ] All CLI commands work (`oa tasks list`, `oa mecha run`, etc.)
- [ ] Trajectories saved and viewable in Gym

### Code Quality
- [ ] All TypeScript test cases have Rust equivalents
- [ ] Zero unsafe Rust (or minimal, documented)
- [ ] `cargo clippy` passes with no warnings
- [ ] Documentation for all public APIs

### Performance
- [ ] HillClimber runs at least as fast as TypeScript
- [ ] UI responsive at 60 FPS
- [ ] Memory usage lower than TypeScript version

### Deletability
- [ ] Can delete `src/` TypeScript directory
- [ ] Can remove Bun from dependencies
- [ ] Can remove Effect-TS from package.json
- [ ] Single `cargo build` produces working binary

---

## Recommended Next Steps

1. **Immediate (this week):**
   - Complete LLM Anthropic provider in Rust
   - Test HillClimber with Anthropic as fallback

2. **Short-term (next 2 weeks):**
   - Add OpenAI provider
   - Port remaining TBCC components to GPUI
   - Implement minimal guardrails

3. **Medium-term (weeks 3-6):**
   - Build GPUI desktop shell
   - Port sandbox/Docker execution
   - Consolidate session management

4. **Stretch (weeks 7-8):**
   - Health monitoring
   - HuggingFace trajectory sync
   - Performance optimization

---

## Appendix: Line Count by Module

### TypeScript Modules (to port or skip)
```
src/agent/          ~8,000 lines   → orchestrator, parallel (DONE)
src/llm/            ~8,000 lines   → llm (PARTIAL)
src/hillclimber/    ~6,000 lines   → hillclimber (DONE)
src/fm/             ~5,000 lines   → fm-bridge, testgen (DONE)
src/effuse/         ~4,000 lines   → hud, commander (PARTIAL)
src/learning/       ~4,000 lines   → SKIP
src/tasks/          ~3,500 lines   → tasks (DONE)
src/skills/         ~3,000 lines   → SKIP
src/bench/          ~3,000 lines   → hillclimber (DONE)
src/healer/         ~2,500 lines   → DEFER
src/trainer/        ~2,500 lines   → SKIP
src/tools/          ~2,500 lines   → tools (DONE)
src/sandbox/        ~2,000 lines   → PORT
src/archivist/      ~2,000 lines   → SKIP
src/memory/         ~2,000 lines   → SKIP
src/flow/           ~2,000 lines   → unit (DONE)
src/desktop/        ~1,500 lines   → REPLACE (GPUI)
src/cli/            ~1,500 lines   → cli (DONE)
src/atif/           ~1,500 lines   → atif (DONE)
src/reflexion/      ~1,500 lines   → SKIP
src/sessions/       ~1,500 lines   → atif-store (DONE)
src/hud/            ~1,500 lines   → hud (DONE)
src/huggingface/    ~1,000 lines   → DEFER
Other small modules ~10,000 lines  → Various
```

### Rust Crates (completed)
```
crates/gpui/           ~50,000 lines  (external)
crates/hillclimber/    ~8,000 lines
crates/hud/            ~6,000 lines
crates/testgen/        ~5,000 lines
crates/orchestrator/   ~4,000 lines
crates/unit/           ~4,000 lines
crates/tools/          ~3,000 lines
crates/tasks/          ~2,500 lines
crates/atif/           ~2,000 lines
crates/atif-store/     ~2,000 lines
crates/cli/            ~2,000 lines
crates/parallel/       ~1,500 lines
crates/llm/            ~1,500 lines
crates/fm-bridge/      ~1,500 lines
crates/config/         ~1,000 lines
crates/theme/          ~800 lines
crates/commander/      ~500 lines
crates/gym/            ~3,000 lines
Other crates           ~10,000 lines
```

---

**Last Updated:** 2025-12-10
**Author:** Migration Assessment Agent
**Status:** Ready for Review
