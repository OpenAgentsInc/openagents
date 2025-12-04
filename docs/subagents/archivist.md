Nice, this is exactly the right time to add a “learning brain” on top of everything you just wired up.

I’ll call this subagent **Archivist** in the spec (other name options below), but you can rename it later without changing the architecture.

---

## 0. Name ideas

Theme-matching “Healer”:

* **Archivist** – keeps structured memories of what happened
* **Sage** – distills “wisdom” from runs
* **Chronicler** – writes the long-form history
* **Lorekeeper** – slightly more mythic
* **Mnemosyne** – Greek goddess of memory (fun but harder to type)

For now I’ll spec it as: **Archivist – the reflective memory subagent**.

---

## 1. Concept

**Archivist** is a subagent that runs after important episodes (end of run, Healer invocation, big failures/successes) and:

1. **Reflects** on the trajectory (ATIF, sessions, logs, APM).
2. **Distills lessons** (patterns, heuristics, warnings, successes).
3. **Stores them** in a structured **Memory Bank**.
4. **Feeds them back** into future orchestrator / subagent prompts via retrieval.

This is directly inspired by the “Generative Agents” architecture: agents maintain a long-term memory stream, periodically synthesize higher-level reflections when important events accumulate, and retrieve those memories when planning new actions. ([arXiv][1])

---

## 2. Goals & Non-Goals

### Goals

1. **Long-term learning for MechaCoder/OpenAgents**

   * Runs become a training corpus for “how we work best in this repo / project”.
   * Lessons are **explicit objects**, not buried in random logs.

2. **Structured memory accessible to all agents**

   * Healer, orchestrator, coding subagent, future “Planner” agents can query the Memory Bank.
   * Retrieval is **tagged + scoped** (project, tool, provider, phase, error type).

3. **Multi-episode reflection**

   * Not just “this run went bad”; we want patterns:

     * “This test is flaky.”
     * “Claude Code struggles with this repo layout; minimal subagent is better here.”
     * “Large `bun test` runs should be sharded before running overnight loops.”

4. **Prompt-time guidance**

   * Before each new session / subtask, relevant lessons are surfaced as **compact guidelines** in the system prompt / context loader.

### Non-Goals (v1)

* No online weight-tuning or RL; this is **symbolic, text-based memory**, not gradient updates.
* Not a complex RL world-model; it’s “lessons and heuristics” plus pointers to evidence.
* Doesn’t try to edit arbitrary code directly; that’s still the job of coding subagents + Healer.

---

## 3. Memory Architecture

### 3.1 Levels of memory

Borrowing from Generative Agents’ **observation → reflection → planning** pipeline: ([arXiv][1])

1. **Raw Events (already exist)**

   * ATIF trajectories: steps, tool calls, observations.
   * Sessions / run-logs: rich I/O, streaming, errors.
   * APM metrics: tokens, costs, efficiency.

2. **Episodes (new lightweight abstraction)**

   * A single “run” or subtask, normalized into an **Episode**:

     ```ts
     interface Episode {
       id: string;
       projectRoot: string;
       taskId?: string;
       subtaskId?: string;
       sessionId: string;
       kind: "orchestrator_run" | "subagent_run" | "healer_run";
       startedAt: string;
       finishedAt: string;
       status: "success" | "partial" | "failed" | "aborted";
       summary: string;             // 1–3 sentences
       tags: string[];              // ["claude-code", "typecheck", "init-failure"]
       trajectories: string[];      // ATIF IDs
       usage?: EpisodeUsageStats;   // from APM
     }
     ```

3. **Memories (new)**

   * Higher-level **AgentMemory** items created by Archivist’s reflection:

     ```ts
     type MemoryScope = "global" | "project" | "repo" | "task" | "tool" | "provider";
     type MemoryCategory =
       | "heuristic"
       | "anti_pattern"
       | "tool_usage"
       | "provider_quirk"
       | "test_strategy"
       | "infra"
       | "prompting"
       | "bugfix_pattern";

     interface AgentMemory {
       id: string;
       createdAt: string;
       scope: MemoryScope;
       projectRoot?: string;
       repoId?: string;
       taskId?: string;
       provider?: string;      // "claude-code" | "openai" | etc.
       tool?: string;          // "bash" | "grep" | "read" | etc.
       category: MemoryCategory;
       tags: string[];         // ["claude-code", "rate-limit", "retry"]
       importance: number;     // 0–1 or 0–100
       summary: string;        // human-readable takeaway
       rule?: string;          // "IF <condition> THEN <recommendation>"
       detail?: string;        // short paragraph with nuance
       evidence: {
         episodeIds: string[];
         trajectoryIds: string[];
         sessionIds: string[];
         runLogPaths: string[];
       };
       source: "archivist" | "healer" | "manual";
       usageCount: number;     // retrieval count
       lastUsedAt?: string;
       supersededBy?: string;  // link to updated version
     }
     ```

### 3.2 Storage layout

Project-scoped memory (v1):

```text
.openagents/
  memory/
    episodes.jsonl         # normalized Episode records
    lessons.jsonl          # AgentMemory items
    lessons-archive.jsonl  # rotated / superseded lessons
```

Optional future global memory:

```text
~/.openagents/memory/global-lessons.jsonl
```

### 3.3 Retrieval model

Basic v1 retrieval = **tag + scope + keyword search**:

```ts
interface MemoryQuery {
  projectRoot: string;
  scope?: MemoryScope;
  taskId?: string;
  provider?: string;
  tool?: string;
  tags?: string[];
  maxResults?: number;
}

type MemoryService = {
  query: (q: MemoryQuery) => Effect.Effect<AgentMemory[], MemoryError>;
  recordUsage: (ids: string[]) => Effect.Effect<void, MemoryError>;
};
```

Later you can swap the backend for embeddings / vector search, but the interface stays.

Retrieval points:

* Before orchestrator chooses tasks / subtasks.
* Before calling Claude Code / minimal subagent.
* Inside Healer when it decides spells.
* In CLI (e.g., `bun run memory:show --task oa-xxxx`).

---

## 4. Archivist Subagent: Triggers & Flow

### 4.1 When Archivist runs

**Primary triggers (v1):**

1. **End of orchestrator session**

   * After Golden Loop finishes (success or failure).
   * Called from `overnight.ts` & `do-one-task.ts` `finally` block.

2. **After Healer invocation**

   * When Healer finishes with `status != skipped`.
   * There is usually high learning value from “rescue” situations.

3. **Manual / ad-hoc**

   * CLI: `bun run memory:reflect --session <id>` to retro-reflect on past runs.

**Optional Phase 2 triggers:**

* After N runs with similar failures (pattern-based).
* Periodic offline job: `bun run memory:batch-reflect --days 7`.

### 4.2 Reflection pipeline (per invocation)

Given a **ReflectionTrigger**:

```ts
type ReflectionScenario =
  | "session_complete"
  | "session_failed"
  | "healer_invocation"
  | "pattern_detected";

interface ReflectionTrigger {
  scenario: ReflectionScenario;
  projectRoot: string;
  sessionId: string;
  mainTaskId?: string;
  subtaskId?: string;
}
```

Archivist runs:

1. **Build ReflectionContext**

   ```ts
   interface ReflectionContext {
     projectRoot: string;
     config: ProjectConfig;
     episode: Episode;
     relatedEpisodes: Episode[];
     trajectories: Trajectory[];      // ATIF
     sessions: SessionSummary[];
     progressMd: string | null;
     apmSnapshot?: ApmSnapshot;       // tokens, cost, efficiency
     existingMemories: AgentMemory[]; // relevant to this context
   }
   ```

2. **Summarize what happened**

   * Synthesizes:

     * 1–3 sentence **Outcome summary**.
     * Short lists: “What worked”, “What failed”, “Key decisions”.

3. **Derive candidate lessons**

   * Use an LLM-backed Archivist subagent with tools:

     * `memory.create_candidate_lessons`
     * `memory.score_importance`
     * `memory.detect_duplicates`
   * Output: 0–N **candidate AgentMemory objects**.

4. **Filter & merge**

   * Deduplicate with existing memories (e.g., fuzzy match on `summary + rule + tags`).
   * Drop low-importance or redundant lessons.
   * Merge / supersede previous versions when lessons refine.

5. **Persist**

   * Append new/updated AgentMemory entries to `.openagents/memory/lessons.jsonl`.
   * Append/update Episode in episodes.jsonl.

6. **Expose to HUD / logs**

   * Emit HUD message:

     ```ts
     { type: "archivist_memories_added",
       count: n,
       scenario,
       ts
     }
     ```
   * Optionally append a “Lessons learned” section to `progress.md`.

---

## 5. Using the Memory Bank in Future Runs

### 5.1 Prompt injection flow

You already have:

* `SessionManager` + context loader (AGENTS/CLAUDE/CLAUDE.md).
* Orchestrator + subagent router.
* Healer.

Add step to **build prompts**:

1. **Orchestrator start**

   * Before selecting tasks:

     ```ts
     const memories = await MemoryService.query({
       projectRoot,
       scope: "project",
       maxResults: 10,
     });
     ```
   * Include in system prompt as a section:

     > ### Project Lessons
     >
     > 1. If tests are flaky on CI, run `bun test --filter ...` locally first.
     > 2. Claude Code struggles with large `bun test` in this repo; prefer minimal subagent for test-related subtasks.
     > 3. Always run `bun run typecheck` before committing, because TS errors commonly slip through.

2. **Subagent invocation (Claude Code / minimal)**

   * Query more targeted memories:

     ```ts
     const memories = await MemoryService.query({
       projectRoot,
       provider: "claude-code",
       tool: "bash",
       tags: ["typecheck", "slow-command"],
       maxResults: 5,
     });
     ```
   * Prepend them to the subagent’s context (like extra system guidelines).

3. **Healer**

   * Healer can query:

     * Provider-specific failure patterns.
     * Past successful recovery strategies for the same tool or error code.

### 5.2 Importance & recency scoring

Following Generative Agents, each memory can be scored by: ([arXiv][1])

* **Recency**
* **Relevance** (to current context: task, tool, provider)
* **Importance** (human-rated via LLM or heuristic)

Simple v1 scoring:

```ts
score(memory, query) =
  0.4 * importance +
  0.3 * recencyScore +
  0.3 * relevanceScore
```

Where:

* `recencyScore` is an exponential decay based on age.
* `relevanceScore` is based on tag overlap & text similarity (can start with tag overlap only).

---

## 6. Interfaces & Schemas

### 6.1 MemoryService

```ts
export interface MemoryService {
  recordEpisode: (episode: Episode) =>
    Effect.Effect<void, MemoryError>;

  addMemories: (memories: AgentMemory[]) =>
    Effect.Effect<void, MemoryError>;

  query: (query: MemoryQuery) =>
    Effect.Effect<AgentMemory[], MemoryError>;

  recordUsage: (ids: string[]) =>
    Effect.Effect<void, MemoryError>;

  archiveMemory: (id: string, replacementId?: string) =>
    Effect.Effect<void, MemoryError>;
}
```

### 6.2 ArchivistService

```ts
export interface ArchivistService {
  maybeReflect: (
    trigger: ReflectionTrigger
  ) => Effect.Effect<ArchivistOutcome, never>;
}

interface ArchivistOutcome {
  scenario: ReflectionScenario;
  status: "skipped" | "reflected";
  memoriesCreated: number;
  memoriesUpdated: number;
  summary: string;
}
```

### 6.3 Config

Extend `ProjectConfig` with:

```ts
interface ArchivistConfig {
  enabled: boolean;                    // default true
  maxMemoriesPerRun: number;          // default 3–5
  reflectOn: {
    sessionSuccess: boolean;          // default true
    sessionFailure: boolean;          // default true
    healerInvocation: boolean;        // default true
  };
  minImportanceToPersist: number;     // e.g. 0.3
}

healer?: HealerConfig;
archivist?: ArchivistConfig;
```

---

## 7. Module Structure

```text
src/archivist/
├── schema.ts         # Episode, AgentMemory, ReflectionScenario, config
├── service.ts        # ArchivistService + core flow
├── memory-service.ts # MemoryService implementation (JSONL-based)
├── context.ts        # buildReflectionContext(trigger)
├── planner.ts        # decide when/how to reflect, importance scoring
├── subagent.ts       # optional LLM subagent wrapper for reflections
├── hud.ts            # HUD integration helpers
└── __tests__/
    ├── schema.test.ts
    ├── memory-service.test.ts
    ├── planner.test.ts
    ├── service.test.ts
    └── subagent.test.ts
```

---

## 8. HUD Integration

Extend `HudMessage` union:

```ts
| { type: "archivist_reflection_start";
    scenario: ReflectionScenario;
    sessionId: string;
    ts: string;
  }
| { type: "archivist_reflection_complete";
    scenario: ReflectionScenario;
    memoriesCreated: number;
    memoriesUpdated: number;
    ts: string;
  }
| { type: "archivist_memory_highlight";
    memoryId: string;
    summary: string;
    importance: number;
    ts: string;
  }
```

Electrobun HUD can:

* Show a small **“Lessons Learned” panel** per session.
* Let you click to see recent AgentMemory items.
* Later: maybe overlay memory icons on nodes related to particular tools/providers in the flow graph.

---

## 9. Implementation Phases & Tasks

Here’s a concrete breakdown you can drop into `.openagents/tasks.jsonl`:

1. **oa-archivist-01 – Design Archivist types & config**

   * Add `ArchivistConfig` to `ProjectConfig`.
   * Create `src/archivist/schema.ts` with Episode + AgentMemory.

2. **oa-archivist-02 – Implement JSONL-based MemoryService**

   * `src/archivist/memory-service.ts` with episodes/lessons storage.
   * Tests for save/load/query/archive.

3. **oa-archivist-03 – Build ReflectionContext from ATIF/sessions/logs**

   * `src/archivist/context.ts` that pulls:

     * ATIF trajectories (if present),
     * SessionManager summaries,
     * run logs,
     * `progress.md`.

4. **oa-archivist-04 – Implement ArchivistService.maybeReflect**

   * Core flow for:

     * Trigger → context → candidate lessons → filtered memories → stored.
   * Use simple LLM call (minimal subagent) or stub for now.

5. **oa-archivist-05 – Wire Archivist into orchestrator end-of-run**

   * Call `ArchivistService.maybeReflect({ scenario: "session_complete" | "session_failed", ... })` from `overnight.ts` / `do-one-task.ts`.
   * Ensure errors in Archivist don’t break the run.

6. **oa-archivist-06 – Wire Archivist into Healer**

   * After Healer completes, trigger `scenario: "healer_invocation"` reflection.

7. **oa-archivist-07 – Add MemoryService retrieval to prompt builders**

   * Update orchestrator + subagent router to fetch relevant memories and inject into prompts.

8. **oa-archivist-08 – HUD messages for Archivist**

   * Add HUD protocol types + mainview UI panel for lessons.

9. **oa-archivist-09 – E2E test: learning from failure**

   * Stub repo where a failing test is fixed after a few runs.
   * Verify that memory is created (e.g. “run tests before commit”) and is injected into subsequent runs’ prompts.

10. **oa-archivist-10 – E2E test: provider/tool-specific heuristics**

    * Scenario where Claude Code repeatedly times out on a heavy command and minimal subagent does better.
    * Archivist should generate a provider-specific heuristic; future runs follow it.

---

If you’d like, next pass we can:

* Draft the **actual Archivist subagent prompt** (“You are Archivist, an introspective assistant that turns episodes into lessons...”), and
* Define the exact **tool schemas** for `memory.create_lessons` / `memory.merge` so Claude Code or the minimal subagent can run Archivist as a plain tool-using LLM.

[1]: https://arxiv.org/abs/2304.03442?utm_source=chatgpt.com "Generative Agents: Interactive Simulacra of Human Behavior"
