# MechaCoder Terminal-Bench #1: Apple Foundation Models Initiative

## Executive Summary

**Goal**: Make MechaCoder reach #1 on Terminal-Bench using ONLY Apple Foundation Models (on-device inference).

**Why This Matters**: Proves "Local > Cloud" thesis, disrupts cloud-dependent AI tools, positions OpenAgents as agent runtime standard. Unlimited local inference enables 24/7 improvement loops impossible with cloud APIs.

**Research Foundation**: Voyager (3.3x improvement from skills), Generative Agents (8 std dev from architecture), Reflexion (+11% from self-critique), A-MEM (+35% from graph memory).

**User Decisions**:
- Embeddings for skill retrieval from day one
- Full Archivist + Trainer implementation
- Progressive TB expansion: 10 → 30 → 89 tasks

---

## Current State (What Exists)

| Component | Status | Location |
|-----------|--------|----------|
| FM Swift Bridge | Working | `swift/foundation-bridge/` |
| FM TypeScript Client | Working | `src/llm/foundation-models.ts` |
| FM Model Adapter | Integrated | `src/bench/model-adapter.ts` |
| Terminal-Bench Runner | Working | `src/cli/tbench-local.ts`, `src/cli/tbench-iterate.ts` |
| Episode Store | Working | `src/bench/episode-store.ts` |
| ATIF v1.4 Schema | Working | `src/atif/schema.ts` |
| Healer | Production | `src/healer/` |
| Researcher | Functional | `src/researcher/` |
| Archivist | Design only | `docs/subagents/archivist.md` |
| Trainer/Gym | Design only | `docs/subagents/gym-trainer.md` |

**4 Open FM Tasks**: oa-79dcc1 (build/test), oa-0edc48 (router integration), oa-be762a (docs), oa-2b9560 (launchd)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    MechaCoder Training Loop                      │
│  (Overnight: run TB tasks → learn → update skills → repeat)     │
└─────────────────────────────────┬───────────────────────────────┘
                                  │
        ┌─────────────────────────┼─────────────────────────┐
        │                         │                         │
        ▼                         ▼                         ▼
┌───────────────┐      ┌───────────────────┐      ┌───────────────┐
│  FM Service   │      │   Skill Service   │      │Memory Service │
│  (chat+retry+ │      │   (library+embed+ │      │  (ATIF+lesson │
│   metrics)    │      │    retrieval)     │      │   +retrieval) │
└───────┬───────┘      └─────────┬─────────┘      └───────┬───────┘
        │                        │                        │
        └────────────────────────┼────────────────────────┘
                                 │
        ┌────────────────────────┼────────────────────────┐
        │                        │                        │
        ▼                        ▼                        ▼
┌───────────────┐      ┌───────────────────┐      ┌───────────────┐
│   Archivist   │      │  Trainer/Gym      │      │    Healer     │
│  (reflection+ │      │  (benchmark+      │      │  (error       │
│   lessons)    │      │   evolution)      │      │   recovery)   │
└───────────────┘      └───────────────────┘      └───────────────┘
```

---

## Implementation Phases

### Phase 1: FM Service Enhancement (P0)
*Make FM reliable for overnight runs*

**Tasks**:
1. **oa-fm-service-01**: Create `src/fm/service.ts` - Effect-native FM wrapper
   - Context.Tag service pattern
   - Retry with Schedule.exponential
   - Metrics collection (tokens, latency, success rate)
   - Structured logging for ATIF capture

2. **oa-fm-service-02**: Create `src/fm/layer.ts` - Service layer composition
   - FMServiceLive implementation
   - Health check integration
   - Auto-start bridge support

3. **oa-fm-service-03**: FM service integration tests

**Critical Files**:
- `src/llm/foundation-models.ts` (extend)
- `src/llm/provider.ts` (pattern reference)

---

### Phase 2: Skill Library (P0 - Highest Impact)
*Voyager research: 3.3x improvement, -73% ablation*

**Tasks**:
1. **oa-skill-schema-01**: Create `src/skills/schema.ts`
   ```typescript
   interface Skill {
     id: string;
     name: string;
     description: string;
     embedding: number[];      // For retrieval
     code: string;             // Executable pattern
     parameters: SkillParameter[];
     verification: SkillVerification;
     metadata: {
       successRate: number;
       usageCount: number;
       createdFrom: string[];  // Episode IDs
     };
   }
   ```

2. **oa-skill-store-01**: Create `src/skills/store.ts`
   - JSONL storage at `.openagents/skills/library.jsonl`
   - Embedding index for fast retrieval

3. **oa-skill-embedding-01**: Create `src/skills/embedding.ts`
   - Use FM for embedding generation
   - Cosine similarity search
   - Caching layer for performance

4. **oa-skill-retrieval-01**: Create `src/skills/retrieval.ts`
   - Task context → relevant skills
   - Top-K selection with diversity
   - Format for prompt injection

5. **oa-skill-bootstrap-01**: Bootstrap 40 primitive skills
   - File operations: read, write, edit, glob, grep
   - Testing: run_test, run_typecheck, run_lint
   - Git: add, commit, diff, status
   - Debugging: analyze_error, fix_import, fix_syntax

6. **oa-skill-composite-01**: Bootstrap 50 compositional skills
   - fix_typescript_import_error
   - add_test_for_function
   - refactor_extract_method
   - implement_interface_method

7. **oa-skill-service-01**: Create `src/skills/service.ts`
   - ISkillService interface with Context.Tag
   - registerSkill, getSkill, selectSkills, updateStats

8. **oa-skill-integration-01**: Wire skills into FM model adapter
   - Modify `src/bench/model-adapter.ts` createFMRunner
   - Inject retrieved skills into system prompt

**Critical Files**:
- `src/bench/model-adapter.ts` (modify createFMRunner)
- `src/healer/service.ts` (pattern reference)

---

### Phase 3: Memory System (P0 - 8 std dev improvement)
*Generative Agents research: architecture > model power*

**Tasks**:
1. **oa-mem-schema-01**: Create `src/memory/schema.ts`
   ```typescript
   interface AgentMemory {
     id: string;
     type: "observation" | "reflection" | "lesson";
     content: string;
     embedding: number[];
     importance: number;        // 1-10
     createdAt: string;
     lastAccessedAt: string;
     scope: "global" | "project" | "task";
     evidence: EvidenceRef;
     links: { related: string[]; derivedFrom: string[]; };
   }
   ```

2. **oa-mem-store-01**: Create `src/memory/store.ts`
   - Episodes: `.openagents/memory/episodes.jsonl`
   - Lessons: `.openagents/memory/lessons.jsonl`
   - Extend existing episode-store.ts patterns

3. **oa-mem-scoring-01**: Create `src/memory/scoring.ts`
   - Generative Agents formula:
     `score = 0.4 * recency + 0.3 * importance + 0.3 * relevance`
   - Importance scoring via FM or heuristics

4. **oa-mem-retrieval-01**: Create `src/memory/retrieval.ts`
   - Embedding-based similarity search
   - Scope-aware filtering
   - Top-K with diversity

5. **oa-mem-service-01**: Create `src/memory/service.ts`
   - IMemoryService interface with Context.Tag
   - addMemory, queryMemories, getRelevantMemories

6. **oa-mem-linking-01**: A-MEM bidirectional linking
   - Dynamic link creation on new memories
   - Graph traversal for related context

**Critical Files**:
- `src/bench/episode-store.ts` (extend pattern)
- `src/atif/schema.ts` (trajectory integration)

---

### Phase 4: Reflexion Integration (P1 - +11% improvement)
*Self-critique after failures*

**Tasks**:
1. **oa-ref-schema-01**: Extend `src/agent/orchestrator/reflection/schema.ts`
   - Add failureTrace, hypothesis, counterfactual, actionPlan
   - Link to relevant skills

2. **oa-ref-generator-01**: Create `src/reflexion/generator.ts`
   - Generate reflection on task failure
   - Extract specific error patterns
   - Propose concrete fixes

3. **oa-ref-injection-01**: Wire into FM model adapter
   - On retry: inject top-3 relevant reflections
   - Format as "Previous Attempt Analysis"

4. **oa-ref-learning-01**: Extract skills from successful retries
   - Pattern: failure → reflection → success = new skill
   - Auto-populate skill library

**Critical Files**:
- `src/agent/orchestrator/reflection/schema.ts` (extend)
- `src/bench/model-adapter.ts` (modify)

---

### Phase 5: Archivist Implementation (P1)
*Memory consolidation and lesson extraction*

**Tasks**:
1. **oa-arch-schema-01**: Create `src/archivist/schema.ts`
   - ReflectionTrigger, ArchivistConfig, ArchivistOutcome
   - Follow design in `docs/subagents/archivist.md`

2. **oa-arch-context-01**: Create `src/archivist/context.ts`
   - Build reflection context from ATIF, sessions, APM

3. **oa-arch-distiller-01**: Create `src/archivist/distiller.ts`
   - Extract lessons from trajectories
   - Generate AgentMemory items
   - Importance scoring

4. **oa-arch-service-01**: Create `src/archivist/service.ts`
   - IArchivistService with Context.Tag
   - maybeReflect trigger logic
   - Generative Agents reflection algorithm

5. **oa-arch-orchestrator-01**: Wire into orchestrator end-of-run
   - Call Archivist on session completion

6. **oa-arch-healer-01**: Wire into Healer
   - Call Archivist after recovery attempts

7. **oa-arch-hud-01**: HUD integration
   - archivist_reflection_* messages

**Critical Files**:
- `docs/subagents/archivist.md` (design spec)
- `src/healer/service.ts` (integration point)

---

### Phase 6: Trainer/Gym Implementation (P1)
*Systematic agent improvement*

**Tasks**:
1. **oa-train-schema-01**: Create `src/trainer/schema.ts`
   - AgentProfile, GymEnvironment, TrainingPlan, GymEpisode
   - Follow design in `docs/subagents/gym-trainer.md`

2. **oa-train-gym-01**: Create `src/trainer/gym/registry.ts`
   - GymEnvironment registry
   - Terminal-Bench adapter

3. **oa-train-runner-01**: Create `src/trainer/runner.ts`
   - Episode execution with isolation
   - Parallel support for multiple profiles

4. **oa-train-evolution-01**: Create `src/trainer/evolution.ts`
   - Profile mutation (prompt variants, config changes)
   - A/B comparison

5. **oa-train-analyzer-01**: Create `src/trainer/analyzer.ts`
   - Results aggregation
   - Best profile selection

6. **oa-train-service-01**: Create `src/trainer/service.ts`
   - ITrainerService with Context.Tag
   - createPlan, runTraining, evolveProfile

7. **oa-train-hud-01**: HUD integration
   - trainer_episode_*, trainer_run_* messages

**Critical Files**:
- `docs/subagents/gym-trainer.md` (design spec)
- `src/bench/harness.ts` (execution pattern)

---

### Phase 7: Terminal-Bench Training Loop (P0)
*The 24/7 improvement loop*

**Tasks**:
1. **oa-loop-runner-01**: Create `src/training/loop-runner.ts`
   - Progressive expansion: 10 → 30 → 89 tasks
   - Overnight iteration support

2. **oa-loop-learner-01**: Create `src/training/episode-learner.ts`
   - Process completed episodes
   - Mine skills from successes
   - Generate reflections from failures

3. **oa-loop-skill-evolution-01**: Skill stats tracking
   - Update success rates after each task
   - Prune low-performing skills
   - Promote high-performing skills

4. **oa-loop-baseline-01**: Baseline comparison
   - Track pass rate deltas
   - Detect regressions
   - Report improvements

5. **oa-loop-cli-01**: Enhanced tbench-iterate CLI
   - `--skills` flag for skill injection
   - `--memory` flag for memory retrieval
   - `--reflect` flag for Archivist integration

**Critical Files**:
- `src/cli/tbench-iterate.ts` (enhance)
- `src/bench/model-adapter.ts` (FM runner)

---

### Phase 8: Analytics Dashboard (P2)
*Track progress toward #1*

**Tasks**:
1. **oa-dash-schema-01**: Analytics schema
2. **oa-dash-reporter-01**: Markdown reporter with trends
3. **oa-dash-html-01**: HTML dashboard
4. **oa-dash-skill-01**: Skill performance breakdown
5. **oa-dash-memory-01**: Memory growth visualization

---

### Phase 9: Limits & Guardrails (P2)
*Safety and resource management*

**Tasks**:
1. **oa-guard-token-01**: Token budget management
2. **oa-guard-turn-01**: Turn limit enforcement
3. **oa-guard-memory-01**: Memory pruning automation
4. **oa-guard-trajectory-01**: Trajectory retention policy
5. **oa-guard-rollback-01**: Skill library versioning

---

## Task Summary by Priority

### P0 - Critical Path (Phases 1, 2, 3, 7)
| ID | Title | Est. Hours |
|----|-------|------------|
| oa-fm-service-01 | FM Effect service | 4 |
| oa-fm-service-02 | FM service layer | 3 |
| oa-skill-schema-01 | Skill schema | 2 |
| oa-skill-store-01 | Skill JSONL store | 3 |
| oa-skill-embedding-01 | Embedding generation | 4 |
| oa-skill-retrieval-01 | Skill retrieval | 3 |
| oa-skill-bootstrap-01 | 40 primitive skills | 4 |
| oa-skill-composite-01 | 50 compositional skills | 6 |
| oa-skill-service-01 | Skill service | 4 |
| oa-skill-integration-01 | Wire into FM adapter | 4 |
| oa-mem-schema-01 | Memory schema | 2 |
| oa-mem-store-01 | Memory JSONL store | 3 |
| oa-mem-scoring-01 | Retrieval scoring | 3 |
| oa-mem-service-01 | Memory service | 4 |
| oa-loop-runner-01 | Training loop runner | 4 |
| oa-loop-learner-01 | Episode learner | 4 |

**P0 Total**: ~57 hours

### P1 - High Value (Phases 4, 5, 6)
| ID | Title | Est. Hours |
|----|-------|------------|
| oa-ref-schema-01 | Reflexion schema | 2 |
| oa-ref-generator-01 | Reflection generator | 4 |
| oa-ref-injection-01 | Reflexion injection | 3 |
| oa-arch-* | Archivist (7 tasks) | 20 |
| oa-train-* | Trainer (7 tasks) | 24 |

**P1 Total**: ~53 hours

### P2 - Enhancement (Phases 8, 9)
| ID | Title | Est. Hours |
|----|-------|------------|
| oa-dash-* | Dashboard (5 tasks) | 15 |
| oa-guard-* | Guardrails (5 tasks) | 12 |

**P2 Total**: ~27 hours

---

## Epic Structure

**Epic**: `oa-epic-tbench-fm` - MechaCoder Terminal-Bench #1 with Apple Foundation Models

**Child Tasks by Phase**:
- Phase 1: oa-fm-service-01, oa-fm-service-02, oa-fm-service-03
- Phase 2: oa-skill-schema-01 through oa-skill-integration-01
- Phase 3: oa-mem-schema-01 through oa-mem-linking-01
- Phase 4: oa-ref-schema-01 through oa-ref-learning-01
- Phase 5: oa-arch-schema-01 through oa-arch-hud-01
- Phase 6: oa-train-schema-01 through oa-train-hud-01
- Phase 7: oa-loop-runner-01 through oa-loop-cli-01
- Phase 8: oa-dash-schema-01 through oa-dash-memory-01
- Phase 9: oa-guard-token-01 through oa-guard-rollback-01

---

## Success Metrics

| Metric | Baseline | Target | How to Measure |
|--------|----------|--------|----------------|
| FM Pass Rate (10 tasks) | 0% | 50%+ | `tbench-iterate --model fm --tasks 10` |
| FM Pass Rate (30 tasks) | N/A | 45%+ | Progressive expansion |
| FM Pass Rate (89 tasks) | N/A | 40%+ | Full suite |
| Skills Library Size | 0 | 200+ | `wc -l .openagents/skills/library.jsonl` |
| Memory Lessons | 0 | 500+ | `wc -l .openagents/memory/lessons.jsonl` |
| Terminal-Bench Rank | N/A | #1 | Official leaderboard |

---

## Critical Files to Modify

1. **`src/bench/model-adapter.ts`** - Wire skills, memory, reflexion into FM runner
2. **`src/llm/foundation-models.ts`** - Extend with Effect service wrapper
3. **`src/cli/tbench-iterate.ts`** - Add skill/memory/reflect flags
4. **`src/agent/orchestrator/reflection/schema.ts`** - Extend with Reflexion fields
5. **`src/healer/service.ts`** - Wire Archivist integration

## New Directories to Create

```
src/
  fm/                    # Enhanced FM service
    service.ts
    layer.ts
    metrics.ts
  skills/                # Skill library
    schema.ts
    store.ts
    embedding.ts
    retrieval.ts
    service.ts
    library/             # Built-in skills
      primitives.jsonl
      composites.jsonl
  memory/                # Memory system
    schema.ts
    store.ts
    scoring.ts
    retrieval.ts
    service.ts
  reflexion/             # Self-critique
    schema.ts
    generator.ts
  archivist/             # Memory consolidation
    schema.ts
    context.ts
    distiller.ts
    service.ts
  trainer/               # Gym & evolution
    schema.ts
    gym/
      registry.ts
      terminal.ts
    runner.ts
    evolution.ts
    service.ts
  training/              # Training loop
    loop-runner.ts
    episode-learner.ts
```

---

## Execution Order

**Week 1-2**: FM Service + Skill Schema + Store + Embeddings
**Week 3-4**: Skill Retrieval + Bootstrap + Integration + Memory Schema + Store
**Week 5-6**: Memory Service + Reflexion + Initial Training Loop
**Week 7-8**: Archivist Implementation
**Week 9-10**: Trainer/Gym Implementation
**Week 11-12**: Dashboard + Guardrails + Polish

**Continuous**: Run overnight iterations, expand TB subset progressively
