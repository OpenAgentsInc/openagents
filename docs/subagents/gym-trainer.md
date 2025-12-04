Trainer is fun, because it turns your little agent guild into something that actually *trains* between adventures instead of just grinding in production.

I’ll spec **Trainer** and the **Gym** concept in the same style as Healer/Archivist, and show how it plugs into Terminal-Bench, APM, parallel worktrees, HUD, etc.

---

# Trainer Subagent & Gym Spec

> **Trainer**: a subagent that can pull any agent/subagent into a safe **Gym environment**, run benchmark suites (Terminal-Bench & friends), and evolve them via prompt/config changes and learned policies—without breaking production workflows.

---

## 1. Goals & Non-Goals

### Goals

1. **Systematic agent improvement**

   * Trainer runs structured evaluation suites (Terminal-Bench, internal “MechaBench” scenarios, tool microbenchmarks) to measure how well each agent/subagent performs under controlled conditions.
   * It produces **versioned improvements**: new prompts, config tweaks, routing rules, and AgentMemory lessons.

2. **Gym as a safe playground**

   * All training happens in **sandboxed worktrees/containers**, not the live project.
   * Trainer can run lots of experiments (A/B prompts, model choices, tool sets) while keeping Golden Loop invariants intact.

3. **Unified training pipeline**

   * Same Gym infrastructure works for:

     * Coding subagent (Terminal-Bench)
     * Orchestrator strategies (task selection/verification policies)
     * Healer strategies (recovery scenarios)
     * Archivist reflection quality
   * Over time, you can plug in new “bench adapters” easily.

4. **Visible progress**

   * HUD shows which agents are in the Gym, which environments they’re in, current metrics, and why Trainer put them there.
   * Trainer writes readable reports + structured data that Analyst/Archivist can consume.

### Non-Goals (v1)

* No gradient-based RL or live weight training—this is **evaluation + config/prompt evolution**, not training neural nets.
* Trainer does **not** directly change production code without going through normal verification + commit rules.
* Trainer does not schedule itself autonomously for now; initial flow is manual / config-scheduled.

---

## 2. Gym Concept

Think of the Gym as a set of **environments** and **benchmarks** with consistent APIs.

### 2.1 Gym Environment

```ts
type GymEnvironmentKind =
  | "terminal_bench"
  | "mecha_bench"        // internal Golden Loop–style tasks
  | "tool_microbench"
  | "healer_bench"
  | "archivist_bench"
  | "custom_script";

interface GymEnvironment {
  id: string;                      // "terminal-bench-v2", "healer-chaos-01"
  kind: GymEnvironmentKind;
  description: string;
  repoTemplatePath: string;        // fixture repo / scenario template
  setupScript?: string;            // optional setup (e.g., seed data)
  benchmarkAdapter: string;        // e.g., "terminal_bench_adapter"
  tags: string[];                  // ["coding", "cli", "refactor"]
  defaultEpisodeLimit: number;     // max runs per training session
}
```

Examples:

* `terminal-bench-v2`: uses `src/bench/terminal-bench.ts` & `harness.ts`.
* `healer-chaos-01`: synthetic repo with deliberate test + infra failures, used to train Healer.
* `archivist-reflection-01`: offline set of ATIF trajectories & sessions used to evaluate reflection quality.

### 2.2 Benchmarks & Episodes

A **GymEpisode** is a single run of a particular agent profile in an environment:

```ts
interface AgentProfile {
  id: string;                  // "coding-subagent-minimal-v3"
  agentKind: "orchestrator" | "subagent" | "healer" | "archivist";
  config: Record<string, unknown>;   // model, prompt variant, flags
  description: string;
  source: "manual" | "trainer" | "baseline";
  parentProfileId?: string;    // for evolved variants
}

interface GymEpisode {
  id: string;
  environmentId: string;
  agentProfileId: string;
  startedAt: string;
  finishedAt: string;
  status: "success" | "failed" | "timeout" | "partial";
  seed?: number;
  metrics: BenchMetrics;       // from src/bench/metrics.ts + APM
  trajectoryIds: string[];     // ATIF IDs
  logsPath: string;            // .openagents/gym/logs/...
}
```

`BenchMetrics` would wrap what you already track:

* task completion rate
* verification pass rate
* token usage
* latency per task
* tool call distribution
* retry rate
* cost (USD)

### 2.3 Training Plan & Run

```ts
interface TrainingPlan {
  id: string;
  projectRoot: string;
  targetAgent: AgentProfile;
  baselines: AgentProfile[];          // baseline profiles for comparison
  environments: GymEnvironment[];
  maxEpisodesPerEnv: number;
  objective: "max_success_rate" | "min_cost" | "pareto";
  searchStrategy: "grid" | "random" | "manual"; // for config/prompt variants
}

interface TrainingRun {
  id: string;
  planId: string;
  startedAt: string;
  finishedAt?: string;
  status: "pending" | "running" | "completed" | "aborted";
  episodes: GymEpisode[];
  summary?: TrainingSummary;
}

interface TrainingSummary {
  bestProfileId: string;
  improvements: string[];       // human-readable bullet points
  metricsComparison: Record<string, BenchMetrics>; // per-profile
}
```

---

## 3. Trainer Subagent Responsibilities

Trainer is the coordinator that:

1. **Chooses when to train**
2. **Selects environments & variants**
3. **Runs Gym episodes through the bench harness**
4. **Analyzes results**
5. **Evolves agents / writes suggestions**
6. **Broadcasts what’s happening to HUD + Memory**

### 3.1 Triggers

**Manual triggers (v1):**

* CLI commands:

  * `bun run trainer:run --agent coding-subagent --env terminal-bench-v2`
  * `bun run trainer:compare --agent coding-subagent --env terminal-bench-v2 --variants minimal,cc-high-thinking`

**Automatic triggers (v2):**

* When **Analyst** / APM detects:

  * drop in success rate on real tasks,
  * cost blow-ups,
  * new model provider available.
* When **Healer** or **Watcher** see repeated classes of error (“Healer is constantly fixing typecheck failures; train the coding subagent on typecheck scenarios”).

### 3.2 Trainer Flow (per TrainingRun)

Given a TrainingPlan:

1. **Plan validation & sandbox prep**

   * Ensure Gym environments exist.
   * Use worktree/sandbox backend to create **ephemeral repos** for each env + seed:

     * `src/sandbox` + `src/worktree` (already implemented).
     * Each GymEpisode runs in its own isolated worktree/container.

2. **Episode scheduling**

   * Use parallel runner (once integrated) or sequential runs:

     * For each env × profile × seed:

       * spawn a GymEpisode:

         * call `src/bench/harness.ts` with the given AgentProfile config:

           * orchestrator vs coding subagent
           * provider & model
           * prompt variants
           * tool set

3. **Execution**

   * During each episode:

     * APM collects metrics.
     * ATIF collects trajectories (when ATIF tasks are done).
     * Bench harness reports pass/fail & metrics.
   * Trainer records results to `.openagents/gym/episodes.jsonl`.

4. **Analysis & comparison**

   * Once episode batch completes:

     * Aggregate metrics by profile & environment.
     * Evaluate objective (success rate, efficiency).
     * Detect statistically meaningful differences (simple thresholds in v1, more robust stats later).

5. **Evolution**

   * Trainer proposes changes:

     * new or updated AgentProfiles with different:

       * system prompts,
       * thinking level,
       * tool gating strategies,
       * queue modes,
       * model selection / fallback rules,
       * safety/perf configs.
   * Writes results in structured form:

     * `trainer-suggestions.json` for each project.
     * Optionally creates tasks: “Apply Trainer suggestion X to coding subagent prompt”.

6. **Publishing**

   * Trainer sends:

     * **HUD messages** about Gym sessions.
     * **Archivist memories** summarizing what worked.
     * **Analyst reports** (longer form) into `docs/apm.md`-style logs or new `docs/trainer/` area.

7. **Optional autopilot (later)**

   * For low-risk changes (e.g., toggling thinking level or provider defaults), Trainer may directly modify project config / prompts behind feature flags, always preserving the previous baseline version.

---

## 4. Trainer–Gym–Existing Infra Integration

### 4.1 Bench harness

You already have:

* `src/bench/harness.ts` — runs tasks with metrics.
* `src/bench/terminal-bench.ts` — Terminal-Bench 2.0 adapter.
* `src/bench/metrics.ts`, `reporter.ts` — metrics & comparisons.

**Trainer** reuses this as its execution engine:

```ts
// Pseudocode:
const episode = await BenchHarness.run({
  environment,
  agentProfile,
  apmCollector,
  sandboxConfig,
});
```

The only difference is:

* harness runs in **Gym mode**: writes to `.openagents/gym/` instead of `.openagents/run-logs/` and uses Gym-specific config.

### 4.2 APM integration

Use the APM system you just built:

* `src/agent/apm.ts` for metrics collection.
* HUD APM widget.

Trainer:

* attaches an `APMCollector` to each GymEpisode.
* sends APM snapshots to HUD **with a “gym” tag** so the widget can show “training runs vs real runs”.

### 4.3 ATIF integration

When ATIF tasks are done (`oa-668316+`):

* Each GymEpisode generates ATIF trajectories with `agent.kind` = `"coding_subagent" | "healer" | "archivist" | "orchestrator"`, `context.role = "gym"`.
* ATIF files can live under:

```text
.openagents/trajectories/gym/YYYYMMDD/<env>-<episode>.atif.json
```

This lets you:

* Use Archivist to train on training runs (meta!), and
* Compare behavior patterns between gym and production.

### 4.4 Memory & Trainer

Trainer writes summary memories like:

* “On Terminal-Bench core suite, minimal coding subagent with `thinking=low` and `bash` enabled beat `thinking=high` by 25% while using 40% fewer tokens.”
* “Healer strategy A outperforms B for typecheck failures; prefer rewinding + marking blocked rather than repeated retries.”

These will be `AgentMemory` entries with `category = "prompting" | "heuristic" | "provider_quirk"`, `scope = "project" | "global"`.

---

## 5. HUD Integration

Add new HUD message types (`src/hud/protocol.ts`):

```ts
| { type: "trainer_run_start";
    runId: string;
    planId: string;
    agents: string[];             // agentProfileIds
    environments: string[];
    ts: string;
  }
| { type: "trainer_episode_start";
    runId: string;
    episodeId: string;
    environmentId: string;
    agentProfileId: string;
    ts: string;
  }
| { type: "trainer_episode_complete";
    runId: string;
    episodeId: string;
    status: "success" | "failed" | "timeout" | "partial";
    successRate?: number;
    ts: string;
  }
| { type: "trainer_run_complete";
    runId: string;
    bestProfileId: string;
    improvementSummary: string;
    ts: string;
  }
```

Electrobun HUD:

* Adds a **“Gym / Trainer” panel**:

  * list of active TrainingRuns
  * per-agent progress bars (“Agent: coding-subagent-minimal-v3 – 12/50 episodes complete”)
  * environment icons (Terminal-Bench, Healer-Bench, etc.)
  * highlight best-performing profiles at the end.

On the flow canvas, you can show:

* A “Gym” node with lines to each agent being trained.
* Tooltips summarizing why Trainer pulled them into Gym (“High cost on real runs; testing cheaper prompt variant”).

---

## 6. Config & CLI

### 6.1 ProjectConfig extension

```ts
interface TrainerConfig {
  enabled: boolean;                 // default false (opt-in)
  maxParallelEpisodes: number;      // reuse parallel-runner infra
  defaultEnvironments: string[];    // e.g., ["terminal-bench-core"]
  autoRunOn:
    | "manual_only"
    | "daily"
    | "on_regression";              // triggered by Analyst drop in metrics
  budget: {
    maxDailyCostUsd?: number;
    maxDailyTokens?: number;
  };
}

trainer?: TrainerConfig;
```

### 6.2 CLI

New CLI entrypoints (e.g. `src/cli/trainer.ts`):

* `bun run trainer:plan`
  Interactively create a TrainingPlan (or from JSON).

* `bun run trainer:run`
  Run Trainer with a plan or config:

  * `--agent-profile coding-subagent-minimal-v3`
  * `--env terminal-bench-core`
  * `--episodes 50`
  * `--compare-with coding-subagent-cc-high-thinking`

* `bun run trainer:list`
  List recent TrainingRuns and top-performing profiles.

* `bun run trainer:report --run <id>`
  Pretty-print report with charts/tables (reuse `bench/reporter.ts`).

* `bun run trainer:suggest`
  Output only the suggestions (for humans or another agent to apply).

---

## 7. Module Structure

```text
src/trainer/
├── schema.ts         # AgentProfile, GymEpisode, TrainingPlan, TrainingRun, TrainingSummary
├── gym.ts            # GymEnvironment registry + loading
├── service.ts        # TrainerService (orchestrates runs)
├── planner.ts        # builds TrainingPlans (manual + auto)
├── runner.ts         # executes episodes via bench harness + sandbox/worktree
├── analyzer.ts       # aggregates metrics, picks best profiles
├── evolution.ts      # proposes evolved AgentProfiles + config/prompt changes
├── hud.ts            # HUD emission helpers
└── __tests__/...
```

Gym-level module:

```text
src/gym/
├── environments.ts   # built-in GymEnvironment definitions (Terminal-Bench etc.)
├── terminal.ts       # glue to src/bench/terminal-bench.ts
├── healer-bench.ts   # scenarios for Healer training
├── archivist-bench.ts# scenarios for reflection training
└── __tests__/...
```

---

## 8. Implementation Phases & Tasks

Here’s a concrete task breakdown you can drop into `.openagents/tasks.jsonl` later.

### Phase T1 – Core schemas & Gym registry (P1)

1. **oa-trainer-01 – Define Trainer & Gym schemas**

   * `src/trainer/schema.ts` with AgentProfile, GymEnvironment, GymEpisode, TrainingPlan, TrainingRun, TrainingSummary.

2. **oa-trainer-02 – Implement Gym environment registry**

   * `src/gym/environments.ts` with built-in envs:

     * `terminal-bench-core`
     * `mecha-bench-smoke` (small Golden Loop-like tasks)
   * Loader API: `getGymEnvironment(id)`.

3. **oa-trainer-03 – Extend ProjectConfig with TrainerConfig**

   * Add `trainer` section.
   * Defaults + tests.

### Phase T2 – TrainerService & runner (P1)

4. **oa-trainer-04 – Implement TrainerService core**

   * `TrainerService.run(plan)`:

     * validate plan,
     * call runner for episodes,
     * store TrainingRun records in `.openagents/gym/runs.jsonl`.

5. **oa-trainer-05 – Implement Gym runner using bench harness**

   * `src/trainer/runner.ts`:

     * uses `src/bench/harness.ts` & `terminal-bench.ts`.
     * integrates APM + sandbox/worktree for each episode.

6. **oa-trainer-06 – Implement analyzer & summary**

   * `src/trainer/analyzer.ts`:

     * aggregates metrics,
     * chooses best profile per objective,
     * produces TrainingSummary.

### Phase T3 – Evolution & suggestions (P1/P2)

7. **oa-trainer-07 – Implement AgentProfile evolution strategies**

   * `src/trainer/evolution.ts`:

     * generate variants: change thinking level, model choice, tool gating, simple prompt tweaks.
     * no auto-apply yet; just suggestions.

8. **oa-trainer-08 – Wire Trainer into Archivist/Memory**

   * Add memory entries summarizing training results.
   * Tag them for Strategist/Quartermaster.

### Phase T4 – HUD & CLI integration (P2)

9. **oa-trainer-09 – Add Trainer/HUD protocol messages & panel**

   * HUD protocol + mainview Trainer panel.

10. **oa-trainer-10 – Add trainer CLI commands**

    * `trainer:run`, `trainer:list`, `trainer:report`, `trainer:suggest`.

### Phase T5 – Specialized gyms (P2+)

11. **oa-trainer-11 – Healer Gym environments**

    * `healer-chaos-01` env with scripted failures.
    * Bench adapter to evaluate recovery success & time.

12. **oa-trainer-12 – Archivist Gym environments**

    * offline dataset of episodes + evaluation harness for memory quality (LLM-based auto-grader).

---

If you like, next I can:

* Draft the **AgentProfile** schemas & TrainerConfig additions exactly as TS, or
* Design **one specific gym**—e.g. “Healer Chaos Gym v1” or “Terminal-Bench Gym v1”—with concrete metrics & lesson types.
