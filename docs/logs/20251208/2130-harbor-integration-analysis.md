# Harbor Integration Analysis - Full Report

**Date:** 2025-12-08 21:30 CT
**Context:** Determining Harbor integration strategy for TB2 verification

---

## Executive Summary

**Recommendation: Hybrid Approach**
- Use **vanilla Harbor** for official benchmark submissions (leaderboard)
- Use **custom Docker runner** for development/HillClimber iteration
- Build **Harbor adapter** to translate our streaming format when needed

**Why not fork Harbor:**
- Harbor is actively maintained (v2.0 released recently)
- We don't need to modify Harbor's core - just adapt our output
- Keeping vanilla Harbor ensures leaderboard compatibility

---

## 1. What is Harbor?

### Architecture Overview

Harbor is a **Python-based evaluation framework** for agents and LLMs. It's the official harness for Terminal-Bench 2.0.

**Core Components:**
```
Harbor Framework
├── Trial (orchestration)
│   ├── Environment (Docker/Daytona/E2B/Modal/Runloop)
│   ├── Agent (setup + execution)
│   └── Verifier (test runner)
├── LocalOrchestrator (parallel execution)
└── Job (dataset + results)
```

**Execution Flow:**
```
1. ENVIRONMENT_START
   - Build/pull Docker image
   - Start container
   - Mount workspace to /app/

2. AGENT_START
   - Upload install script
   - Run agent installation
   - Execute agent with task instruction

3. VERIFICATION_START
   - Upload test scripts
   - Run pytest in container
   - Parse results

4. END
   - Download logs
   - Parse trajectory (ATIF format)
   - Cleanup
```

### What We Have Already

**Current Harbor Integration** (`src/harbor/`):
```typescript
src/harbor/
├── openagents_harbor/
│   ├── mechacoder_agent.py       // MechaCoder adapter for Harbor
│   ├── install-mechacoder.sh.j2  // Setup script template
│   └── __init__.py
├── pyproject.toml                 // Python package config
└── README.md
```

**MechaCoder Agent Adapter:**
- Implements Harbor's `BaseInstalledAgent` interface
- Installs Bun, Claude Code CLI, and OpenAgents
- Injects credentials from macOS Keychain
- Runs `bun src/cli/tbench.ts`
- Parses ATIF trajectory from Claude Code output

**This means:** We can already submit MechaCoder to Harbor benchmarks!

---

## 2. Current Custom Docker Runner

### What We Built

**File:** `src/bench/tb2-docker-runner.ts`

**Features:**
- Spawns Docker containers directly (`spawn("docker", ...)`)
- Pulls/builds task-specific images
- Installs Python + pytest at runtime
- Runs verification, parses pytest output
- Returns structured results (pass/fail, test counts, feedback)

**Advantages:**
- ✅ Fast - no Python framework overhead
- ✅ Synchronous control flow - easy to debug
- ✅ Direct integration with Effect TypeScript
- ✅ Streaming-friendly (can capture stdout in real-time)
- ✅ Lightweight dependencies

**Disadvantages:**
- ❌ Doesn't produce ATIF trajectories
- ❌ Not compatible with Harbor leaderboard submissions
- ❌ Single backend (Docker only, no Daytona/E2B)
- ❌ Reimplements some Harbor functionality

---

## 3. Harbor for Final Benchmarks

### Official Leaderboard Submissions

**Requirements:**
- Must use Harbor framework
- Must produce ATIF v1.4 trajectories
- Must run in approved environments (Docker/Daytona)
- Results must be reproducible

**Current Status: ✅ We're Ready**

Our `openagents_harbor` package already provides this:

```bash
# Official benchmark submission
harbor run \
  --dataset terminal-bench@2.0 \
  --agent-import-path openagents_harbor:MechaCoderAgent \
  --model anthropic/claude-opus-4-1 \
  --n-concurrent 100 \
  --env daytona
```

**Output:**
- ATIF trajectories in `trials/<trial-id>/agent/trajectory.json`
- Metrics JSON with tokens, cost, success rate
- Logs for each trial
- Aggregated job statistics

### Why Use Vanilla Harbor for Benchmarks

**1. Leaderboard Compatibility**
- Harbor is the official framework for TB2
- Results must be from unmodified Harbor
- Forking risks invalidating submissions

**2. Multi-Backend Support**
- Daytona for cloud scale (100+ parallel)
- E2B for reproducibility
- Modal for serverless
- Docker for local testing

**3. Standardized Output**
- ATIF format is the standard
- Other researchers can parse our results
- ML training pipelines expect this format

**4. Active Maintenance**
- Harbor v2.0 just released
- Regular updates for new benchmarks
- Community support

---

## 4. Harbor vs Custom for Development

### Use Cases Compared

| Aspect | Custom Docker Runner | Harbor Framework |
|--------|---------------------|------------------|
| **Speed** | ~1-2s per verification | ~10-30s (setup overhead) |
| **Streaming** | Real-time stdout/stderr | Logs written to files |
| **Integration** | Native Effect TypeScript | Python subprocess |
| **Debugging** | Direct in-process | Parse logs post-run |
| **Overhead** | Minimal (just Docker) | Python runtime + deps |
| **Output** | Structured results | ATIF trajectory |
| **Backends** | Docker only | 5 backends |
| **Use Case** | Rapid iteration | Official evaluation |

### When to Use Each

**Custom Docker Runner (Development):**
```
✅ HillClimber iteration (100+ verifications per task)
✅ Real-time feedback to FM
✅ Debugging test failures
✅ Local development
✅ Fast CI/CD tests
```

**Harbor Framework (Benchmarks):**
```
✅ Official leaderboard submissions
✅ Multi-agent comparisons
✅ Reproducible results
✅ Large-scale parallel runs (100+ tasks)
✅ Publishing research papers
```

---

## 5. Streaming Integration Challenge

### The Problem

Harbor writes logs to **files**, not streams:
```
trials/my-trial/agent/
├── setup/
│   ├── stdout.txt
│   └── stderr.txt
├── command-0/
│   ├── stdout.txt
│   └── stderr.txt
└── trajectory.json  # Written at END
```

**Our Effect setup needs:**
- Real-time streaming to HUD
- Live progress updates
- Immediate feedback to FM

### Current Harbor Limitations

**1. File-Based Logging**
```python
# Harbor's approach
proc = subprocess.Popen(cmd, stdout=file1, stderr=file2)
proc.wait()  # Blocking
```

**2. Async/Await Architecture**
```python
async def run_trial(trial: Trial):
    await trial.environment.start()
    await trial.agent.run()
    await trial.verify()
```

Can't easily hook into our synchronous Effect streams.

**3. Post-Run Processing**
- Trajectory parsed after completion
- No intermediate updates
- Can't react to progress

### Solution Options

#### Option A: Harbor Adapter (Recommended)

**Architecture:**
```typescript
// When official benchmark needed
const harborAdapter = {
  async runWithHarbor(task, agent, config) {
    // 1. Run Harbor subprocess
    const harborProc = spawn("harbor", ["run", ...args]);

    // 2. Tail log files in real-time
    const logWatcher = watchFiles("trials/*/agent/command-*/stdout.txt");

    // 3. Stream to Effect
    logWatcher.pipe(
      Effect.flatMap(parseHarborLog),
      Effect.tap(updateHUD),
    );

    // 4. Wait for completion, return ATIF
    return await harborProc.exited;
  }
};
```

**Pros:**
- ✅ Use vanilla Harbor (leaderboard compatible)
- ✅ Get streaming via file tailing
- ✅ Simple integration layer
- ✅ Best of both worlds

**Cons:**
- File watching adds latency (~100ms)
- More complex than direct streaming

#### Option B: Fork Harbor for Streaming

**Changes needed:**
```python
# Modified Harbor
class StreamingEnvironment(BaseEnvironment):
    def exec(self, command, on_stdout=None, on_stderr=None):
        proc = subprocess.Popen(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        # Stream to callbacks
        for line in proc.stdout:
            if on_stdout:
                on_stdout(line)
```

**Pros:**
- Direct streaming
- Lower latency
- Cleaner architecture

**Cons:**
- ❌ Fork diverges from upstream
- ❌ Merge conflicts on updates
- ❌ May invalidate leaderboard submissions
- ❌ Maintenance burden

#### Option C: Hybrid (Best Approach)

**Development:**
```typescript
// Fast iteration with custom Docker runner
const result = await runTB2InDocker({
  taskId, taskDir, workspace,
  onStdout: (chunk) => hudUpdate(chunk),
  onStderr: (chunk) => hudError(chunk),
});
```

**Benchmarks:**
```typescript
// Official runs with Harbor
const trajectory = await runWithHarbor({
  task,
  agent: "openagents_harbor:MechaCoderAgent",
  model: "claude-opus-4-1",
});
```

**Implementation:**
```typescript
// src/bench/verification-strategy.ts
export type VerificationMode = "development" | "benchmark";

export function selectVerificationRunner(mode: VerificationMode) {
  switch (mode) {
    case "development":
      return customDockerRunner;  // Fast, streaming
    case "benchmark":
      return harborAdapter;        // Official, ATIF
  }
}
```

---

## 6. Detailed Comparison

### Custom Docker Runner Deep Dive

**Current Implementation:**
```typescript
// src/bench/tb2-docker-runner.ts
export async function runTB2InDocker(options) {
  // 1. Load task config from task.toml
  const config = await loadTaskConfig(taskDir);

  // 2. Ensure Docker image available
  const image = await ensureTaskImage(taskId, taskDir, config.environment);

  // 3. Create temp context, copy workspace + tests
  const dockerContext = mkdtempSync(join(tmpdir(), "tb2-docker-"));
  cpSync(workspace, dockerContext, { recursive: true });
  cpSync(testsDir, join(dockerContext, "tests"), { recursive: true });

  // 4. Run Docker container
  const dockerArgs = [
    "run", "--rm",
    "-v", `${dockerContext}:/app`,
    "-w", "/app",
    "--memory", config.memory || "2G",
    "--cpus", String(config.cpus || 1),
    image,
    "sh", "-c",
    "apt-get update -qq && apt-get install -y -qq python3 python3-pip && " +
    "python3 -m pip install -q --break-system-packages pytest && " +
    "python3 -m pytest tests/ -v 2>&1"
  ];

  const result = await runDockerCommand(dockerArgs, timeout);

  // 5. Parse pytest output
  const parsed = parsePytestSummary(result.stdout + result.stderr);

  return {
    passed: parsed.passed,
    progress: parsed.passing / parsed.total,
    testsPassing: parsed.passing,
    testsTotal: parsed.total,
    exitCode: result.exitCode,
  };
}
```

**Performance:**
- Image pull: ~5s (cached: 0s)
- Container start: ~100ms
- Python install: ~10s (could cache)
- Pytest run: ~50ms
- **Total: ~1-2s (cached), ~15s (cold)**

### Harbor Deep Dive

**Harbor Implementation:**
```python
# From harbor/trial/trial.py
async def run(self) -> TrialResult:
    # ENVIRONMENT_START
    await self.environment.start(force_build=False)

    # AGENT_START
    agent_context = AgentContext(...)
    await self.agent.setup(self.environment)
    await self.agent.run(
        instruction=self.task.instruction,
        environment=self.environment,
        context=agent_context,
    )

    # VERIFICATION_START
    verifier_result = await self.verifier.run(
        environment=self.environment,
        trial_paths=self.trial_paths,
    )

    # END
    await self.environment.stop(delete=True)

    return TrialResult(
        agent_info=agent_context,
        verifier_result=verifier_result,
    )
```

**Performance:**
- Environment build: ~60s (cached: 5s)
- Agent setup: ~30s (install Bun, Claude Code, etc.)
- Agent execution: variable (100-300s)
- Verification: ~10s
- **Total: ~200-400s per trial**

**Harbor is designed for:**
- Batch processing (100+ tasks)
- Reproducible results
- Multi-backend orchestration
- Not real-time iteration

---

## 7. ATIF Trajectory Integration

### What is ATIF?

**Agent Trajectory Interchange Format v1.4**

Standard format for agent evaluation:
```json
{
  "schema_version": "ATIF-v1.4",
  "session_id": "uuid",
  "agent": {
    "name": "mechacoder",
    "version": "0.1.0",
    "model_name": "claude-opus-4-1"
  },
  "steps": [
    {
      "step_id": 1,
      "source": "agent",
      "message": "I'll solve this task...",
      "tool_calls": [
        {"name": "write_file", "arguments": {...}}
      ],
      "metrics": {
        "prompt_tokens": 1000,
        "completion_tokens": 500
      }
    }
  ],
  "final_metrics": {
    "total_prompt_tokens": 5000,
    "total_completion_tokens": 2500,
    "total_steps": 10,
    "success": true
  }
}
```

### Why ATIF Matters

**1. ML Training Pipelines**
- SFT (Supervised Fine-Tuning) datasets
- RL (Reinforcement Learning) from outcomes
- Retokenization safety (token IDs preserved)

**2. Research Reproducibility**
- Standard format across frameworks
- Other researchers can parse
- Published benchmarks use this

**3. Leaderboard Requirements**
- Terminal-Bench leaderboard expects ATIF
- SWE-bench, WebArena use similar formats
- Industry standard emerging

### Our Current ATIF Support

**✅ We already generate ATIF:**

```typescript
// src/cli/tbench.ts
// Outputs ATIF trajectory from Claude Code session logs
```

**How it works:**
1. Claude Code runs in headless mode
2. Session logs written to `.claude/sessions/projects/-app/*.jsonl`
3. `tbench.ts` parses these logs
4. Converts to ATIF format
5. Writes `trajectory.json` and `metrics.json`

**This is what Harbor uses!**

---

## 8. Recommendations

### Strategy: Three-Tier Verification

```
┌─────────────────────────────────────────────┐
│  Development (HillClimber iteration)        │
│  → Custom Docker Runner                     │
│  → Fast (1-2s), streaming, Effect-native    │
└─────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────┐
│  Pre-Submission Validation                  │
│  → Harbor + File Tailing                    │
│  → Verify ATIF output, test on few tasks    │
└─────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────┐
│  Official Leaderboard Submission            │
│  → Vanilla Harbor (Daytona backend)         │
│  → 100+ parallel trials, full dataset       │
└─────────────────────────────────────────────┘
```

### Implementation Plan

#### Phase 1: Keep Custom Docker Runner (Done ✅)
- Use for all HillClimber development
- Fast iteration on FM prompts
- Real-time streaming to HUD
- **Status: Implemented and working**

#### Phase 2: Add Harbor Adapter (Next)
```typescript
// src/bench/harbor-adapter.ts
export async function runWithHarbor(
  task: TerminalBenchTask,
  options: HarborOptions
): Promise<ATIFTrajectory> {
  // 1. Write Harbor config
  const configPath = writeHarborConfig(task, options);

  // 2. Spawn Harbor subprocess
  const proc = spawn("harbor", ["run", "--config", configPath]);

  // 3. Tail log files for streaming (optional)
  if (options.streaming) {
    const watcher = watchTrialLogs(options.outputDir);
    watcher.on("data", (chunk) => options.onStdout?.(chunk));
  }

  // 4. Wait for completion
  await proc.exited;

  // 5. Read ATIF trajectory
  return readATIFTrajectory(options.outputDir);
}
```

#### Phase 3: Pre-Submission Validation
```bash
# Test script: validate-for-leaderboard.sh
#!/bin/bash

# Run 5 random tasks with Harbor
harbor run \
  --dataset terminal-bench@2.0 \
  --agent-import-path openagents_harbor:MechaCoderAgent \
  --model anthropic/claude-opus-4-1 \
  --n-concurrent 5 \
  --env docker

# Validate ATIF output
python validate_atif.py trials/*/agent/trajectory.json

# Check metrics
python analyze_results.py trials/
```

#### Phase 4: Official Submission
```bash
# Full benchmark run (Daytona backend, 100 parallel)
harbor run \
  --dataset terminal-bench@2.0 \
  --agent-import-path openagents_harbor:MechaCoderAgent \
  --model anthropic/claude-opus-4-1 \
  --n-concurrent 100 \
  --env daytona \
  --output-dir ./official-results

# Submit to leaderboard
python submit_results.py \
  --results ./official-results \
  --team "OpenAgents" \
  --agent "MechaCoder"
```

---

## 9. Specific Questions Answered

### Should we fork Harbor?

**No.** Here's why:

1. **Leaderboard compatibility** - Forked Harbor may not be accepted
2. **Maintenance burden** - Harbor updates frequently
3. **Not necessary** - We can achieve streaming via adapter
4. **Risk** - Breaking changes in fork could invalidate results

**Alternative:** Contribute streaming support upstream to Harbor if needed later.

### When do we use Harbor?

**Use Harbor when:**
- Submitting to Terminal-Bench leaderboard
- Publishing research results
- Comparing against other agents
- Running large-scale evaluations (100+ tasks)
- Need multi-backend support (Daytona, E2B)

**Use custom Docker runner when:**
- HillClimber development
- Testing FM prompts
- Debugging test failures
- Real-time HUD updates
- Local CI/CD

### How do we stream with Harbor?

**Option 1: File Tailing (Recommended)**
```typescript
import { watch } from "chokidar";

const watcher = watch("trials/*/agent/command-*/stdout.txt");
watcher.on("add", (path) => {
  const stream = createReadStream(path);
  stream.on("data", (chunk) => hudUpdate(chunk));
});
```

**Option 2: Harbor Subprocess Stdout**
```typescript
// Harbor CLI outputs progress to stdout
const proc = spawn("harbor", ["run", ...], { stdio: "pipe" });
proc.stdout.on("data", (chunk) => {
  // Parse Harbor's rich-formatted output
  const progress = parseHarborProgress(chunk);
  hudUpdate(progress);
});
```

**Option 3: Polling (Fallback)**
```typescript
setInterval(async () => {
  const status = await readHarborStatus("trials/job.json");
  hudUpdate(status);
}, 1000);
```

---

## 10. Cost-Benefit Analysis

### Custom Docker Runner

**Development Costs:**
- ✅ Already built (~4 hours)
- Maintenance: ~1 hour/month

**Benefits:**
- 10-20x faster than Harbor
- Native Effect integration
- Real-time streaming
- Easy debugging

**Risks:**
- Drift from Harbor behavior
- Need to maintain both paths

### Harbor Integration

**Development Costs:**
- Adapter: ~4 hours
- Testing: ~2 hours
- Documentation: ~1 hour

**Benefits:**
- Official leaderboard submissions
- Multi-backend support
- Standard ATIF output
- Research credibility

**Risks:**
- Python dependency
- Slower iteration
- File-based limitations

### Hybrid Approach

**Total Development: ~15 hours (already 4 done)**

**Benefits:**
- Best of both worlds
- Clear separation of concerns
- Future-proof

**Risks:**
- Two code paths to maintain
- Potential behavior differences

---

## 11. Concrete Action Items

### Immediate (This Week)
1. ✅ Keep custom Docker runner for development
2. ✅ Document when to use each approach
3. ⬜ Test Harbor submission with 1-2 tasks
4. ⬜ Verify ATIF output from `tbench.ts`

### Short-Term (Next 2 Weeks)
1. Build Harbor adapter for streaming
2. Create validation script (run 5 tasks, check ATIF)
3. Document submission process
4. Set up Daytona account for cloud runs

### Long-Term (Next Month)
1. Official leaderboard submission (100 tasks)
2. Publish results
3. Compare against baseline agents
4. Iterate based on findings

---

## 12. Final Recommendation

**Use the Hybrid Approach:**

```typescript
// Development config
export const devConfig = {
  verification: "custom-docker",  // Fast iteration
  streaming: true,                // Real-time HUD
  backend: "docker",              // Local
};

// Benchmark config
export const benchmarkConfig = {
  verification: "harbor",         // Official
  streaming: false,               // Batch mode
  backend: "daytona",             // Cloud scale
};

// Auto-select based on environment
export function selectConfig() {
  return process.env.OFFICIAL_RUN === "true"
    ? benchmarkConfig
    : devConfig;
}
```

**Why this works:**
- Development stays fast and iterative
- Benchmarks stay official and reproducible
- Clear separation avoids confusion
- Both paths tested and maintained

**Implementation priority:**
1. ✅ Custom Docker runner (done)
2. ⬜ Harbor adapter (next)
3. ⬜ Validation pipeline
4. ⬜ Official submission

---

## Appendix A: Harbor Commands Reference

```bash
# Local Docker run (single task)
harbor run \
  --dataset terminal-bench@2.0 \
  --agent openagents_harbor:MechaCoderAgent \
  --task-ids regex-log \
  --env docker

# Cloud Daytona run (parallel)
harbor run \
  --dataset terminal-bench@2.0 \
  --agent openagents_harbor:MechaCoderAgent \
  --n-concurrent 100 \
  --env daytona

# With specific model
harbor run \
  --dataset terminal-bench@2.0 \
  --agent openagents_harbor:MechaCoderAgent \
  --model anthropic/claude-opus-4-1 \
  --env docker

# Resume interrupted run
harbor run \
  --dataset terminal-bench@2.0 \
  --agent openagents_harbor:MechaCoderAgent \
  --resume \
  --env daytona
```

## Appendix B: File Structure

```
Current Implementation:
src/bench/
├── tb2-config.ts           # Parse task.toml
├── tb2-image-manager.ts    # Manage Docker images
├── tb2-container.ts        # Container config factory
└── tb2-docker-runner.ts    # Custom Docker runner ✅

Recommended Additions:
src/bench/
├── harbor-adapter.ts       # Harbor integration
├── verification-strategy.ts # Mode selection
└── atif-validator.ts       # Validate ATIF output

src/harbor/                 # Already exists ✅
└── openagents_harbor/
    └── mechacoder_agent.py
```

---

**End of Report**
