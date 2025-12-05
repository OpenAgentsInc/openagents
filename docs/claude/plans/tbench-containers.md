# Terminal-Bench Container Integration & Output Streaming

## Problem Statement

When Terminal-Bench runs from desktop server:
1. ✅ **Authentication works** (after switching to `stdout: "pipe"`)
2. ❌ **No real-time output** - All stdout/stderr buffered until process ends
3. ❌ **No sandboxing** - Agent runs directly on host without isolation

## Goals

1. **Add container support** - Use existing `src/sandbox/` infrastructure like MechaCoder
2. **Fix output streaming** - Show agent output in real-time regardless of container vs host
3. **Credential injection** - Reuse MechaCoder's proven credential mounting approach
4. **Backward compatible** - Keep existing direct-spawn path working for development

## Current Architecture

```
Desktop Server (Worker with minimal env)
  └─> spawns TB subprocess (Bun.spawn with pipe)
       └─> TB runner (tbench-local.ts)
            └─> Claude Code SDK (query)
                 └─> Claude CLI subprocess
```

**Issue**: With `stdout: "pipe"`, output buffers in Node/Bun until subprocess exits.

## Proposed Architecture

### Option A: Container Integration (Primary Path)

```
Desktop Server
  └─> spawns TB subprocess with --sandbox flag
       └─> TB runner creates credential mount
            └─> Spawns container with workspace + credentials mounted
                 ├─> Setup commands run in container
                 ├─> Claude Code SDK runs on HOST (mounts workspace)
                 └─> Pytest verification runs in container
```

**Benefits**:
- Security isolation for task code
- Resource limits per task
- Reproducible environment
- Matches Harbor/leaderboard mode

### Option B: Direct Spawn with Output Streaming (Development Path)

```
Desktop Server
  └─> spawns TB subprocess with stream reader
       ├─> Read stdout/stderr chunks asynchronously
       ├─> Forward to WebSocket (HUD events)
       └─> TB runner runs on host (current behavior)
```

**Benefits**:
- Fast iteration (no container startup)
- Simpler debugging
- Works without Docker/Podman

## Implementation Plan

### Phase 1: Fix Output Streaming (Quick Win)

**Goal**: Make real-time output work for current non-container path.

**Changes in `src/desktop/handlers.ts`**:

```typescript
export async function startTBRun(options: TBRunOptions): Promise<{ runId: string }> {
  // ... existing setup ...

  activeTBRun = spawn({
    cmd: [process.execPath, ...args],
    cwd: PROJECT_ROOT,
    stdout: "pipe",  // Keep pipe for SDK compatibility
    stderr: "pipe",
    stdin: "ignore",
    env: process.env,
  });

  // NEW: Stream output asynchronously
  (async () => {
    const reader = activeTBRun.stdout.getReader();
    const decoder = new TextDecoder();

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      const text = decoder.decode(value, { stream: true });
      console.log(text); // Shows in desktop server console

      // TODO: Also emit to HUD via WebSocket
      // emitTBTaskOutput(runId, taskId, text);
    }
  })();

  // Do same for stderr
  // ... similar reader for activeTBRun.stderr ...

  return { runId };
}
```

**Impact**: ~30 lines of code, immediate real-time output visibility.

### Phase 2: Add Container Support

**Goal**: Integrate TB with existing sandbox infrastructure.

#### Step 2.1: Add Sandbox Option to TB CLI

**File**: `src/cli/tbench-local.ts`

Add flag parsing:
```typescript
interface TBOptions {
  // ... existing fields ...
  sandbox?: boolean;
  sandboxBackend?: "docker" | "macos-container";
  sandboxImage?: string;
}

// Parse from args
const sandbox = args["--sandbox"] === true;
const sandboxBackend = args["--sandbox-backend"] || "docker";
```

#### Step 2.2: Integrate Sandbox Runner

**Pattern**: Follow `src/agent/orchestrator/sandbox-runner.ts`.

```typescript
import { createCredentialMount, cleanupCredentialMount } from "../sandbox/credentials.js";
import { runInSandboxContainer } from "../sandbox/index.js";

async function runTaskWithSandbox(task: TBTask, workspace: string, options: TBOptions) {
  // 1. Create credential mount
  const credentialMount = await createCredentialMount();

  try {
    // 2. Build container config
    const containerConfig = {
      image: options.sandboxImage || "ubuntu:22.04",
      backend: options.sandboxBackend,
      workingDir: "/workspace",
      volumeMounts: [
        `${workspace}:/workspace`,
        credentialMount.volumeMount, // e.g., "/tmp/creds:/root/.claude:ro"
      ],
      timeout: task.timeout_seconds,
      env: {
        PYTHONPATH: "/workspace",
      },
    };

    // 3. Run setup commands in container (if any)
    if (task.setup) {
      await runInSandboxContainer(task.setup, containerConfig);
    }

    // 4. Run Claude Code SDK on HOST (not in container)
    //    SDK needs access to host filesystem, MCP servers, etc.
    const result = await runClaudeCodeSubagent(subtask, {
      cwd: workspace,  // Host path
      maxTurns: options.maxTurns,
      permissionMode: "bypassPermissions",
    });

    // 5. Run verification in container
    if (task.verification) {
      const verifyResult = await runInSandboxContainer(
        `cd /workspace && ${task.verification}`,
        containerConfig
      );
      return { ...result, verificationOutput: verifyResult.stdout };
    }

    return result;

  } finally {
    // 6. Cleanup credential mount
    await cleanupCredentialMount(credentialMount);
  }
}
```

**Key Design Decision**: SDK runs on HOST, not in container. This avoids:
- Container-in-container complexity
- MCP server access issues
- Filesystem access problems

Only setup/verification commands run in container for isolation.

#### Step 2.3: Update Desktop Server Integration

**File**: `src/desktop/protocol.ts`

Add sandbox fields to TBRunOptions:
```typescript
export interface TBRunOptions {
  suitePath: string;
  taskIds?: string[];
  timeout?: number;
  maxTurns?: number;

  // NEW: Container options
  sandbox?: boolean;
  sandboxBackend?: "docker" | "macos-container";
  sandboxImage?: string;
}
```

**File**: `src/desktop/handlers.ts`

Pass through to CLI:
```typescript
const args = [
  tbenchPath,
  "--suite", options.suitePath,
  // ... existing args ...
];

if (options.sandbox) {
  args.push("--sandbox");
  if (options.sandboxBackend) {
    args.push("--sandbox-backend", options.sandboxBackend);
  }
  if (options.sandboxImage) {
    args.push("--sandbox-image", options.sandboxImage);
  }
}
```

#### Step 2.4: Add UI Controls (Optional)

**File**: `src/mainview/index.html`

Add checkbox in TB controls:
```html
<label>
  <input type="checkbox" id="tb-sandbox-enabled" />
  Sandbox (container isolation)
</label>

<select id="tb-sandbox-backend">
  <option value="docker">Docker</option>
  <option value="macos-container">macOS Container</option>
</select>
```

## Critical Files to Modify

| File | Changes | Lines |
|------|---------|-------|
| `src/desktop/handlers.ts` | Add async output streaming | ~30 |
| `src/cli/tbench-local.ts` | Add sandbox flag parsing & integration | ~150 |
| `src/desktop/protocol.ts` | Add sandbox fields to TBRunOptions | ~10 |
| `src/mainview/index.html` | Add sandbox UI controls (optional) | ~20 |
| `src/mainview/index.ts` | Wire sandbox options to RPC (optional) | ~15 |

**Total**: ~225 lines of new code (excluding UI enhancements).

## Phased Rollout

### Phase 1: Output Streaming Only
- **Time**: 1 hour
- **Risk**: Low (no architecture change)
- **Benefit**: Immediate visibility improvement
- **Files**: handlers.ts only

### Phase 2: Container Integration
- **Time**: 4-6 hours
- **Risk**: Medium (new integration points)
- **Benefit**: Security, isolation, reproducibility
- **Files**: tbench-local.ts, protocol.ts, handlers.ts

### Phase 3: UI Controls
- **Time**: 2 hours
- **Risk**: Low (pure UI)
- **Benefit**: User can toggle container mode
- **Files**: index.html, index.ts

## Testing Strategy

### Test 1: Output Streaming
```bash
bun dev  # Start desktop server
# Click "Random" - should see real-time output now
```

### Test 2: Container Integration (CLI)
```bash
bun src/cli/tbench-local.ts \
  --suite ./tasks/terminal-bench-2.json \
  --tasks regex-log \
  --sandbox \
  --sandbox-backend docker
```

### Test 3: Container Integration (Desktop)
```bash
bun dev
# Enable "Sandbox" checkbox in UI
# Click "Random" - should use containers
```

## Fallback Strategy

If container integration has issues:
1. Graceful degradation - warn and fall back to host execution
2. Log clear error messages about missing Docker/Podman
3. Allow mixed mode - some tasks in container, some on host

## Open Questions

1. **Which container backend to default to?**
   - macOS: Prefer `macos-container` (no Docker needed)
   - Linux/CI: Prefer `docker`

2. **Should we containerize the entire TB subprocess or just commands?**
   - Current plan: SDK on host, setup/verification in container
   - Alternative: Entire TB run in container (more isolation, more complexity)

3. **How to handle MCP servers in containers?**
   - Current plan: MCP servers on host (SDK accesses them)
   - Alternative: Mount MCP server paths into container

4. **Container image management?**
   - Use existing images (ubuntu:22.04)?
   - Build custom TB image with dependencies?

## Recommendation

**Start with Phase 1** (output streaming fix) for immediate improvement, then add Phase 2 (container support) as an opt-in feature. This gives both:
- Fast iteration path (no containers)
- Secure evaluation path (with containers)

Both paths will have working authentication and real-time output.
