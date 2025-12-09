# Plan: Full Docker Support for TB2 Tests

## Executive Summary

**Goal**: Replace all local TB2 test execution with proper Docker-based verification. Remove all hacks that game the benchmark (e.g., `/app/` path replacement).

**Key Insight**: OpenAgents already has robust container infrastructure in `src/sandbox/` that supports Docker and macOS containers. Harbor provides the canonical execution model. We need to unify these.

---

## Current State Analysis

### What's Broken

1. **E2E test is gaming** (`src/hillclimber/e2e-regex-log.test.ts:81-85`):
   ```typescript
   // BAD: Replacing /app/ paths locally
   content = content.replace(/\/app\//g, `${workspace}/`);
   ```

2. **TB2 Docker runner uses wrong image** (`src/bench/tb2-docker-runner.ts:89`):
   ```typescript
   // Uses generic python:3.11-slim instead of task-specific image
   "python:3.11-slim"
   ```
   - Each TB2 task has its own Docker image: `alexgshaw/<task-id>:20251031`
   - Task Dockerfiles install specific dependencies, pre-stage data
   - Using generic image breaks tasks that need specific setup

3. **Verification shows "0/1 tests"**: Docker runner parses output but tests aren't running correctly because environment isn't set up properly.

### What Works

1. **Sandbox infrastructure** (`src/sandbox/`):
   - `ContainerBackend` interface with `run()` and `build()` methods
   - Docker backend with volume mounts, resource limits, streaming
   - macOS container backend for Apple Silicon
   - Auto-detection layer (`autoDetectLayer`)
   - Credential injection from macOS Keychain

2. **Harbor integration** (`src/harbor/`):
   - `MechaCoderAgent` adapter for Harbor execution
   - ATIF trajectory format support
   - Task import utilities

---

## Harbor's Execution Model (Reference)

Harbor uses this task structure:
```
task-id/
├── task.toml           # cpus, memory_mb, storage_mb, docker_image
├── instruction.md      # Task description
├── environment/
│   └── Dockerfile      # Task-specific container
├── tests/
│   └── test_outputs.py # Pytest verification
└── solution/
    └── solve.sh        # Reference solution
```

**Execution Flow**:
1. Build or pull task's Docker image
2. Start container with `/app/` as workdir
3. Agent writes solution files to `/app/`
4. Run `pytest tests/ -v` in container
5. Parse results, return pass/fail + progress

---

## Implementation Plan

### Phase 1: Proper Task Image Support

**Goal**: Use task-specific Docker images instead of generic `python:3.11-slim`.

**Files to modify**:
- `src/bench/tb2-docker-runner.ts`
- `src/bench/terminal-bench.ts` (task loading)

**Changes**:

1. **Parse `docker_image` from task.toml** or task.json:
   ```typescript
   interface TB2Task {
     // ... existing fields
     environment?: {
       docker_image?: string;  // e.g., "alexgshaw/regex-log:20251031"
       cpus?: number;
       memory?: string;
       build_timeout_sec?: number;
     };
   }
   ```

2. **Use task image in Docker runner**:
   ```typescript
   const dockerArgs = [
     "run", "--rm",
     "-v", `${dockerContext}:/app`,
     "-w", "/app",
     task.environment?.docker_image || "python:3.11-slim",  // Fallback
     "sh", "-c", "pytest tests/ -v 2>&1"
   ];
   ```

3. **Handle image pulling**: Pre-pull images or build from Dockerfile if needed.

### Phase 2: Integrate with Existing Sandbox Infrastructure

**Goal**: Unify TB2 Docker runner with `src/sandbox/` infrastructure.

**Files to modify**:
- `src/bench/tb2-docker-runner.ts` → Refactor to use `ContainerBackend`
- `src/hillclimber/evaluator.ts` → Use unified container execution

**Changes**:

1. **Create TB2-specific container config**:
   ```typescript
   // src/bench/tb2-container.ts
   import { ContainerBackend, ContainerConfig } from "../sandbox/index.js";

   export function createTB2ContainerConfig(
     task: TerminalBenchTask,
     workspace: string
   ): ContainerConfig {
     return {
       image: task.environment?.docker_image || "python:3.11-slim",
       workdir: "/app",
       volumeMounts: [
         { hostPath: workspace, containerPath: "/app" },
       ],
       memoryLimit: task.environment?.memory || "2G",
       cpuLimit: task.environment?.cpus || 1,
       timeoutMs: (task.verification?.timeout || 120) * 1000,
       env: { PYTHONUNBUFFERED: "1" },
     };
   }
   ```

2. **Use ContainerBackend for execution**:
   ```typescript
   export const runTB2Verification = (
     task: TerminalBenchTask,
     workspace: string
   ): Effect.Effect<TB2DockerResult, ContainerError, ContainerBackend> =>
     Effect.gen(function* () {
       const backend = yield* ContainerBackend;
       const config = createTB2ContainerConfig(task, workspace);

       const result = yield* backend.run(
         "pytest tests/ -v",
         config,
         { onStdout: (chunk) => { /* capture */ } }
       );

       return parsePytestResult(result);
     });
   ```

### Phase 3: Remove Local Execution Hacks

**Goal**: Delete all code that games the benchmark.

**Files to delete/modify**:
- `src/hillclimber/e2e-regex-log.test.ts` - Remove `/app/` replacement (lines 71-88)
- `src/bench/model-adapter.ts` - Remove path normalization hacks if any

**Test setup should**:
1. Copy workspace files (solution) to temp directory
2. Copy tests from TB2 task directory to workspace
3. Run verification in Docker with proper `/app/` mount
4. NOT modify test file contents

```typescript
// CORRECT test setup
beforeAll(async () => {
  workspace = await mkdtemp(join(tmpdir(), "hillclimber-e2e-"));

  // Copy TB2 environment files to workspace (these become /app/ contents)
  const envDir = join(task.source_path, "environment");
  if (existsSync(envDir)) {
    cpSync(envDir, workspace, { recursive: true, filter: (src) => !src.endsWith("Dockerfile") });
  }

  // Tests stay in task directory - mounted separately or copied to workspace/tests
  // DO NOT modify test file contents
});
```

### Phase 4: Support Both Prebuilt and Build-from-Dockerfile

**Goal**: Handle tasks with prebuilt images AND tasks needing local builds.

**Files to create**:
- `src/bench/tb2-image-manager.ts`

**Logic**:
```typescript
export async function ensureTaskImage(task: TerminalBenchTask): Promise<string> {
  const taskEnv = task.environment;

  // 1. If prebuilt image specified, try to pull it
  if (taskEnv?.docker_image) {
    const pulled = await tryPullImage(taskEnv.docker_image);
    if (pulled) return taskEnv.docker_image;
  }

  // 2. Build from Dockerfile if available
  const dockerfile = join(task.source_path, "environment", "Dockerfile");
  if (existsSync(dockerfile)) {
    const imageTag = `tb2-${task.id}:local`;
    await buildImage(dockerfile, imageTag, {
      memoryLimit: taskEnv?.memory || "2G",
      timeout: (taskEnv?.build_timeout_sec || 600) * 1000,
    });
    return imageTag;
  }

  // 3. Fallback to generic image
  return "python:3.11-slim";
}
```

### Phase 5: Update Evaluator Integration

**Goal**: `evaluateProgressWithDocker()` uses proper container infrastructure.

**Files to modify**:
- `src/hillclimber/evaluator.ts`
- `src/hillclimber/map-orchestrator.ts`

**Changes**:

1. **Evaluator uses ContainerBackend**:
   ```typescript
   export const evaluateProgressWithDocker = (
     task: TerminalBenchTask,
     workspace: string
   ): Effect.Effect<EvaluatorResult, Error, ContainerBackend> =>
     Effect.gen(function* () {
       // Ensure image is available
       const image = yield* Effect.tryPromise(() => ensureTaskImage(task));

       // Run verification in container
       const result = yield* runTB2Verification(task, workspace);

       return {
         passed: result.passed,
         progress: result.progress,
         testsTotal: result.testsTotal,
         testsPassing: result.testsPassing,
         failures: [],  // Blind verification
         rawOutput: result.output,
         durationMs: result.durationMs,
       };
     });
   ```

2. **MAP orchestrator provides ContainerBackend layer**:
   ```typescript
   const evaluation = await Effect.runPromise(
     evaluateProgressWithDocker(task, workspace).pipe(
       Effect.provide(autoDetectLayer)  // Uses Docker or macOS container
     )
   );
   ```

---

## Files to Modify

| File | Change | Priority |
|------|--------|----------|
| `src/bench/tb2-config.ts` | NEW: Parse task.toml for environment config | HIGH |
| `src/bench/tb2-docker-runner.ts` | Refactor to use ContainerBackend, support task-specific images | HIGH |
| `src/hillclimber/evaluator.ts` | Update `evaluateProgressWithDocker` to use sandbox infra | HIGH |
| `src/hillclimber/e2e-regex-log.test.ts` | Remove `/app/` replacement hacks, use Docker properly | HIGH |
| `src/hillclimber/map-orchestrator.ts` | Provide ContainerBackend layer | MEDIUM |
| `src/bench/tb2-image-manager.ts` | NEW: Image pull/build management | MEDIUM |
| `src/bench/tb2-container.ts` | NEW: TB2-specific container config factory | MEDIUM |

---

## Files to Delete/Clean

| File/Section | Reason |
|--------------|--------|
| `e2e-regex-log.test.ts` lines 71-88 | Gaming the benchmark with path replacement |
| Any `/app/` → workspace replacement | Benchmark gaming |

---

## Task Configuration Discovery

**Finding**: TB2 source already has `task.toml` with environment config:
```toml
# /Users/christopherdavid/code/terminal-bench-2/regex-log/task.toml
[environment]
docker_image = "alexgshaw/regex-log:20251031"
cpus = 1
memory = "2G"
storage = "10G"
build_timeout_sec = 600.0
```

**Strategy**: Parse `task.toml` at runtime using `source_path` (already in our task JSON).
- NO need to duplicate environment config in `terminal-bench-2.json`
- Read `task.toml` when task is loaded/executed
- Use TOML parser (`@iarna/toml` or similar)

```typescript
// src/bench/tb2-config.ts
import { parse as parseToml } from "@iarna/toml";

interface TB2EnvironmentConfig {
  docker_image?: string;
  cpus?: number;
  memory?: string;
  storage?: string;
  build_timeout_sec?: number;
}

export async function loadTaskEnvironment(sourcePath: string): Promise<TB2EnvironmentConfig> {
  const tomlPath = join(sourcePath, "task.toml");
  if (!existsSync(tomlPath)) return {};

  const content = await Bun.file(tomlPath).text();
  const parsed = parseToml(content);
  return parsed.environment || {};
}
```

---

## Execution Order

### Step 1: TOML Config Loader
1. Create `src/bench/tb2-config.ts` with `loadTaskEnvironment()`
2. Add `@iarna/toml` dependency (or use Bun's built-in if available)
3. Test parsing of `regex-log/task.toml`

### Step 2: Image Manager
1. Create `src/bench/tb2-image-manager.ts`
2. Implement `ensureTaskImage()` with pull/build logic
3. Add tests for image management

### Step 3: Container Config Factory
1. Create `src/bench/tb2-container.ts`
2. Implement `createTB2ContainerConfig()`
3. Map TB2 task config to ContainerConfig

### Step 4: Refactor Docker Runner
1. Update `tb2-docker-runner.ts` to use ContainerBackend
2. Remove hardcoded `python:3.11-slim`
3. Use proper volume mounts

### Step 5: Update Evaluator
1. Update `evaluateProgressWithDocker()` to use new infrastructure
2. Ensure blind verification (no expected values leaked)

### Step 6: Clean Up Gaming Code
1. Remove `/app/` path replacement from e2e test
2. Fix test setup to use proper Docker mounts
3. Verify tests pass with Docker execution

### Step 7: Integration Test
1. Run `bun test src/hillclimber/e2e-regex-log.test.ts`
2. Verify Docker is used (not local execution)
3. Verify tests show correct count (9/9 for regex-log)

---

## Success Criteria

1. **Primary**: TB2 tests run in Docker with task-specific images
2. **Secondary**: No `/app/` path replacement anywhere in codebase
3. **Tertiary**: E2E test shows correct test counts (9/9 for regex-log)
4. **Validation**: `docker ps` shows container running during verification

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Docker not available | Fall back to macOS container, then error gracefully |
| Image pull fails | Build from Dockerfile as fallback |
| Task missing environment config | Use sensible defaults (python:3.11-slim, 2G, 1 CPU) |
| Tests still show 0/0 | Debug pytest output parsing, verify tests copied |

---

## Dependencies

- Docker installed and running (`docker --version`)
- TB2 task images available on Docker Hub (`alexgshaw/*`)
- `src/sandbox/` infrastructure working (already tested)

---

## Harbor Alignment

This plan aligns with Harbor's execution model:
- Uses task-specific Docker images ✓
- Mounts workspace to `/app/` ✓
- Runs pytest in container ✓
- Supports resource limits (CPU, memory) ✓
- Blind verification (no expected values) ✓

Future: Can swap Docker backend for Daytona/E2B for cloud execution.
