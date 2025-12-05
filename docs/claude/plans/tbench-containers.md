# Terminal-Bench Container Integration & Output Streaming (REVISED)

## Problem Statement

When Terminal-Bench runs from desktop server:
1. ✅ **Authentication works** (after switching to `stdout: "pipe"`)
2. ✅ **Real-time output works** (Phase 1 implemented with async stream readers)
3. ❌ **No sandboxing** - Agent runs directly on host without isolation

## Architectural Separation (CORRECTED)

**Existing files serve different purposes:**

1. **`src/cli/tbench-local.ts`** - Local development mode
   - Runs directly on host
   - Uses Claude Code SDK (`query()`)
   - Takes TB suite format (`--suite`, `--tasks`)
   - **Should NOT be modified for container support**

2. **`src/cli/tbench.ts`** - Harbor/ATIF evaluation mode
   - Different purpose (Harbor integration)
   - Takes single `--instruction` string
   - Outputs ATIF format (events.jsonl, trajectory.json, metrics.json)
   - Uses `claude` CLI directly
   - **Not for desktop sandbox mode**

3. **NEW: `src/cli/tbench-sandbox.ts`** - Container/sandbox mode
   - Hybrid execution: SDK on HOST, setup/verify in CONTAINER
   - Reuses TB suite format like `tbench-local.ts`
   - Integrates with `src/sandbox/` infrastructure
   - Handles credential mounting and container lifecycle

## Goals

1. **Add container support** - Create new `tbench-sandbox.ts` using `src/sandbox/` infrastructure
2. ~~**Fix output streaming**~~ - ✅ Already done in Phase 1
3. **Credential injection** - Use MechaCoder's proven `createCredentialMount()` approach
4. **Backward compatible** - Keep `tbench-local.ts` unchanged for development

## Proposed Architecture

### Option A: Container Integration via New File (CORRECT APPROACH)

```
Desktop Server
  └─> handlers.ts decides mode based on `sandbox` flag:
       ├─> sandbox === false → spawns tbench-local.ts (current behavior)
       └─> sandbox === true  → spawns tbench-sandbox.ts (new file)
            └─> For each TB task:
                 ├─> Create credential mount (host temp dir)
                 ├─> If task has setup → run in CONTAINER
                 ├─> Run Claude Code SDK on HOST (accesses workspace)
                 └─> If task has verification → run in CONTAINER
                 └─> Cleanup credential mount
```

**Benefits**:
- Clean separation of concerns
- `tbench-local.ts` stays pure local (no container code)
- `tbench-sandbox.ts` focused on hybrid container execution
- `tbench.ts` remains Harbor-specific

**Why SDK runs on HOST:**
- Avoids container-in-container complexity
- MCP servers accessible (host-only)
- Workspace already accessible (mounted)
- Setup/verification isolated in container (security)

### ~~Option B: Direct Spawn with Output Streaming~~ ✅ DONE (Phase 1)

This was implemented and committed. Real-time output now works via async stream readers in `handlers.ts`.

## Implementation Plan

### ~~Phase 1: Fix Output Streaming~~ ✅ COMPLETED

**Completed in commit `<commit-sha>`:**
- Added async stream readers in `handlers.ts:161-195`
- Reads stdout/stderr with `getReader()` and `TextDecoder`
- Writes to `process.stdout.write()` for real-time visibility
- Tested and verified working

### Phase 2: Create tbench-sandbox.ts

**New file: `src/cli/tbench-sandbox.ts`**

Takes same args as `tbench-local.ts`:
```typescript
interface TBenchSandboxArgs {
  suite: string;        // Path to TB suite JSON
  tasks?: string;       // Comma-separated task IDs
  output: string;       // Output directory
  timeout?: number;     // Per-task timeout
  maxTurns?: number;    // Max agent turns
  sandboxBackend?: "docker" | "macos-container";  // Container backend
  sandboxImage?: string;  // Container image
}
```

**High-level flow:**
1. Parse CLI args (same as `tbench-local.ts`)
2. Load TB suite JSON
3. Filter tasks by IDs (if specified)
4. For each task:
   a. Create credential mount with `createCredentialMount()`
   b. If task has setup commands → run in container via `runInContainer()`
   c. Run SDK query on HOST (import `runClaudeCodeSubagent` or similar)
   d. If task has verification → run in container via `runInContainer()`
   e. Record outcome
   f. Cleanup credential mount with `cleanupCredentialMount()`
5. Write summary JSON

**Key imports needed:**
```typescript
import { runInContainer, isContainerAvailable, autoDetectLayer, createCredentialMount, cleanupCredentialMount } from "../sandbox/index.js";
import { query } from "@anthropic-ai/claude-agent-sdk"; // For SDK execution
import { Effect } from "effect";
```

**Container config pattern (from `sandbox-runner.ts`):**
```typescript
const containerConfig = {
  image: args.sandboxImage || "oven/bun:latest",
  workspaceDir: taskWorkspace,  // Task's workspace dir
  workdir: "/workspace",
  volumeMounts: [
    credentialMount.volumeMount,  // e.g., "/tmp/creds:/root/.claude:ro"
  ],
  timeoutMs: (args.timeout || 3600) * 1000,
  autoRemove: true,
};
```

**Reuse TB suite loading:**
Extract common suite loading logic into `src/cli/tbench-common.ts` (optional) or copy from `tbench-local.ts`.

### Phase 3: Update handlers.ts to Switch Scripts

**File**: `src/desktop/handlers.ts`

Current code spawns `tbench-local.ts` unconditionally. Change to:

```typescript
export async function startTBRun(options: StartTBRunRequest): Promise<{ runId: string }> {
  // ... existing setup ...

  // Decide which script to run
  const scriptPath = options.sandbox
    ? join(PROJECT_ROOT, "src/cli/tbench-sandbox.ts")
    : join(PROJECT_ROOT, "src/cli/tbench-local.ts");

  // Build args (both scripts use similar interface)
  const args = [
    scriptPath,
    "--suite", options.suitePath,
    "--output", outputDir,
    "--timeout", String(options.timeout || 3600),
    "--max-turns", String(options.maxTurns || 300),
  ];

  if (options.taskIds && options.taskIds.length > 0) {
    args.push("--tasks", options.taskIds.join(","));
  }

  // If sandbox mode, pass container options
  if (options.sandbox) {
    if (options.sandboxBackend) {
      args.push("--sandbox-backend", options.sandboxBackend);
    }
    if (options.sandboxImage) {
      args.push("--sandbox-image", options.sandboxImage);
    }
  }

  // Spawn subprocess
  activeTBRun = spawn({
    cmd: [process.execPath, ...args],
    cwd: PROJECT_ROOT,
    stdout: "pipe",
    stderr: "pipe",
    stdin: "ignore",
    env: process.env,
  });

  // Async stream readers (already implemented in Phase 1)
  // ...

  return { runId };
}
```

**Lines to modify**: `handlers.ts:120-152` (approx)

### Phase 4: Update Protocol (Already Done)

**File**: `src/desktop/protocol.ts`

Already has `sandbox`, `sandboxBackend`, `sandboxImage` fields added to `StartTBRunRequest` interface (lines 50-60).

### Phase 5: Add UI Controls (Optional)

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

**File**: `src/mainview/index.ts`

Wire sandbox options to WebSocket request.

## Critical Files to Create/Modify

| File | Action | Purpose | Lines |
|------|--------|---------|-------|
| `src/cli/tbench-sandbox.ts` | **CREATE** | New hybrid container execution mode | ~400 |
| `src/desktop/handlers.ts` | **MODIFY** | Switch between local vs sandbox scripts | ~15 |
| ~~`src/desktop/protocol.ts`~~ | ~~MODIFY~~ | ✅ Already done | - |
| ~~`src/cli/tbench-local.ts`~~ | ~~MODIFY~~ | ❌ **DO NOT MODIFY** (keep pure local) | - |
| `src/mainview/index.html` | MODIFY (optional) | Add sandbox UI controls | ~20 |
| `src/mainview/index.ts` | MODIFY (optional) | Wire sandbox options | ~15 |

**Total**: ~450 lines (excluding optional UI).

## Phased Rollout

### ~~Phase 1: Output Streaming Only~~ ✅ COMPLETED
- **Status**: Committed and working
- **Benefit**: Real-time visibility for all modes

### Phase 2: Container Integration (~6-8 hours)
- **Step 1**: Create `tbench-sandbox.ts` with hybrid execution (~4 hours)
- **Step 2**: Update `handlers.ts` to switch scripts (~30 min)
- **Step 3**: Test CLI with `--sandbox` flag (~1 hour)
- **Step 4**: Integration testing (~1 hour)

### Phase 3: UI Controls (~2 hours, optional)
- Add sandbox toggle UI
- Wire to desktop server

## Testing Strategy

### Test 1: Existing Local Mode (Regression)
```bash
bun dev  # Start desktop server
# Click "Random" - should work as before (no containers)
```

### Test 2: CLI Sandbox Mode
```bash
bun src/cli/tbench-sandbox.ts \
  --suite ./tasks/terminal-bench-2.json \
  --tasks regex-log \
  --sandbox-backend docker \
  --output ./results/sandbox-test
```

Expected:
- ✅ Container starts (Docker/macOS)
- ✅ Setup commands run in container (if any)
- ✅ SDK query runs on host
- ✅ Verification runs in container
- ✅ Real-time output visible
- ✅ Results written to output dir

### Test 3: Desktop Sandbox Mode
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

## Key Design Decisions

### 1. Why separate `tbench-sandbox.ts` instead of adding flag to `tbench-local.ts`?

**Answer**: Clean separation of concerns. Local mode stays simple and fast. Sandbox mode is opt-in and isolated. Easier to maintain and test independently.

### 2. Why SDK on host instead of in container?

**Answer**:
- MCP servers run on host (not containerized)
- Workspace already accessible via mount
- Avoids container-in-container complexity
- Setup/verification still isolated for security

### 3. Which container backend to default to?

**Answer**:
- macOS: Prefer `macos-container` (no Docker needed)
- Linux/CI: Prefer `docker`
- Use `autoDetectLayer` from `src/sandbox/` to auto-select

### 4. Container image to use?

**Answer**:
- Default: `oven/bun:latest` (matches MechaCoder)
- User can override with `--sandbox-image` flag
- Should have basic tools (bash, git, python, etc.)

## Open Questions

### ~~1. How to handle MCP servers in containers?~~

**Answer**: MCP servers stay on host. SDK accesses them from host environment.

### ~~2. Should we containerize the entire TB subprocess?~~

**Answer**: No. SDK on host, only setup/verification in container.

### 3. Credential injection - temp dir or volume?

**Answer**: Use MechaCoder pattern - temp dir on host mounted read-only to container:
```typescript
const credentialMount = await createCredentialMount();
// Returns: { hostPath: "/tmp/claude-creds-123", volumeMount: "/tmp/claude-creds-123:/root/.claude:ro" }
```

### 4. How to reuse TB suite loading logic?

**Options**:
- A. Copy from `tbench-local.ts` (quick, some duplication)
- B. Extract to `src/cli/tbench-common.ts` (cleaner, more work)
- C. Import from `tbench-local.ts` (coupling)

**Recommendation**: Start with A (copy), refactor to B later if needed.

## Success Criteria

- [ ] `tbench-local.ts` unchanged (pure local mode)
- [ ] `tbench-sandbox.ts` created and working
- [ ] `handlers.ts` switches between scripts based on `sandbox` flag
- [ ] Desktop UI can toggle sandbox mode
- [ ] CLI can run with `--sandbox` flag
- [ ] Real-time output works for both modes
- [ ] Containers clean up properly after runs
- [ ] Credential injection working (SDK authenticates)
- [ ] Setup commands run in container
- [ ] Verification commands run in container
- [ ] Tests pass for both local and sandbox modes

## Related Files

- `src/cli/tbench-local.ts` - Local mode (unchanged)
- `src/cli/tbench.ts` - Harbor/ATIF mode (unchanged)
- `src/cli/tbench-sandbox.ts` - NEW sandbox mode
- `src/desktop/handlers.ts` - Modified (script switching)
- `src/desktop/protocol.ts` - Already updated (Phase 2)
- `src/sandbox/` - Container infrastructure (reused)
- `docs/logs/20251205/1144-tb-output-streaming-log.md` - Phase 1 verification

## Next Steps

1. Create `src/cli/tbench-sandbox.ts` with hybrid execution logic
2. Update `handlers.ts` to switch between local/sandbox scripts
3. Test CLI mode with simple task
4. Test desktop mode with sandbox toggle
5. Add UI controls (optional)
6. Document usage patterns
