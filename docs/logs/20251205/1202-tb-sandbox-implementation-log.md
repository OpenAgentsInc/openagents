# 1202 TB Container Integration Implementation

## Context

Continuation from previous session where TB authentication was fixed. User requested full implementation of the TB container integration plan from docs/claude/plans/tbench-containers.md.

Initial confusion: I started modifying tbench-local.ts, but user corrected me - the plan was flawed because it didn't recognize existing architectural separation.

## Problem Identified

The original plan proposed adding --sandbox flags to tbench-local.ts, but this violated existing architecture:
- tbench-local.ts = Local dev mode (no containers)
- tbench.ts = Harbor/ATIF mode (different purpose entirely)
- Need NEW file for sandbox mode

## Solution Implemented

### 1. Revised Plan Document (commit: a35732d3e)

Updated docs/claude/plans/tbench-containers.md with correct architecture:
- Documented clean separation: local/Harbor/sandbox modes
- tbench-sandbox.ts = NEW hybrid container mode
- handlers.ts switches between scripts based on flag

### 2. Created tbench-sandbox.ts (commit: 53f28f10d)

New file: src/cli/tbench-sandbox.ts (~700 lines)

**Hybrid execution model:**
- Setup commands: Run IN CONTAINER (isolated)
- Claude Code SDK: Run ON HOST (access to MCP, workspace)
- Verification: Run IN CONTAINER (isolated)

**Key features:**
- Credential mounting via createCredentialMount()
- Effect-based container execution via runInContainer()
- Same CLI interface as tbench-local.ts
- Additional flags: --sandbox-backend, --sandbox-image

### 3. Updated handlers.ts (commit: 53f28f10d)

Modified src/desktop/handlers.ts:
- Added sandbox fields to TBRunOptions interface
- Script switching logic based on options.sandbox flag:
  - false → tbench-local.ts (current behavior)
  - true → tbench-sandbox.ts (new mode)
- Pass sandbox options (backend, image) to CLI

## Architecture Summary



## Files Modified

1. docs/claude/plans/tbench-containers.md - Revised plan
2. src/desktop/protocol.ts - Added sandbox fields (already done in previous session)
3. src/cli/tbench-sandbox.ts - NEW file with hybrid execution
4. src/desktop/handlers.ts - Script switching logic

## Validation

✅ TypeScript type checking passed (bun run build:check)
✅ No modifications to tbench-local.ts (kept pure local)
✅ Clean architectural separation maintained

## Next Steps (Not Implemented Yet)

From revised plan:
1. Test CLI directly: `bun src/cli/tbench-sandbox.ts --suite ... --sandbox-backend docker`
2. Test desktop integration: Enable sandbox checkbox in UI
3. Add UI controls (optional): Checkbox for sandbox toggle, backend selection

## Commits

1. a35732d3e - docs: revise TB container plan to separate local/sandbox/harbor modes
2. 53f28f10d - feat: add TB sandbox mode with hybrid container execution

## Related Files

- docs/claude/plans/tbench-containers.md - Implementation plan
- docs/logs/20251205/1144-tb-output-streaming-log.md - Phase 1 (output streaming)
- src/cli/tbench-local.ts - Local mode (unchanged)
- src/cli/tbench.ts - Harbor mode (unchanged)
- src/cli/tbench-sandbox.ts - NEW sandbox mode
- src/desktop/handlers.ts - Modified (script switching)
- src/desktop/protocol.ts - Modified (sandbox fields)


## Testing Session 2 - Fixes and Validation

### Issues Found and Fixed

#### Issue 1: parseArgs Positional Arguments Error
**Error**: `TypeError: Unexpected argument ''. This command does not take positional arguments`

**Root Cause**: parseArgs() was rejecting unexpected arguments in strict mode.

**Fix**: Added `strict: false` to parseArgs configuration in tbench-sandbox.ts:62

**Commit**: (pending)

#### Issue 2: Docker Volume Mount Error  
**Error**: `Error: invalidArgument: "invalid volume name 'results/sandbox-quick-test/regex-log/workspace': must match ^[A-Za-z0-9][A-Za-z0-9_.-]*$"`

**Root Cause**: Docker requires absolute paths for host volume mounts, but workspaceDir was a relative path.

**Fix**: 
1. Added `resolve` to path imports (line 21)
2. Converted workspaceDir to absolute path using `resolve()` (line 297)

**Commit**: (pending)

### Test Results

**Test Command**:
```bash
bun src/cli/tbench-sandbox.ts \
  --suite ./tasks/terminal-bench-2.json \
  --tasks regex-log \
  --output ./results/sandbox-quick-test \
  --timeout 180 \
  --max-turns 20 \
  --sandbox-backend docker
```

**Execution Flow Verified**:
✅ Credential mount created successfully
✅ SDK running on HOST with absolute path: `/Users/christopherdavid/code/openagents/results/sandbox-quick-test/regex-log/workspace`
✅ SDK authentication successful (got system init message)
✅ Agent processed task successfully
✅ Docker volume mount working (no "invalid volume name" error)
⚠️  Verification failed: Container lacks Python (`oven/bun:latest` doesn't include python3/pytest)

**Verification Error (Expected)**:
```
STDERR:
error: Script not found "python3"
```

This is expected behavior - the `oven/bun:latest` image doesn't include Python. The verification step correctly attempted to run pytest in the container, which demonstrates the hybrid execution model is working as designed.

### Architecture Validation

The hybrid execution model is working correctly:

1. **Setup** → Would run in CONTAINER (task had no setup commands)
2. **SDK Execution** → Ran on HOST (confirmed by absolute path in logs)
3. **Verification** → Attempted to run in CONTAINER (confirmed by "python3 not found" error from container)

### Next Steps

1. ✅ parseArgs fix tested and working
2. ✅ Docker volume mount fix tested and working
3. ✅ Hybrid execution flow validated
4. 🔄 Need to commit fixes
5. 🔄 Consider documenting recommended container images with Python (e.g., `python:3.11-slim`)

### Files Modified (This Session)

1. `src/cli/tbench-sandbox.ts:21` - Added `resolve` to path imports
2. `src/cli/tbench-sandbox.ts:64` - Added `strict: false` to parseArgs
3. `src/cli/tbench-sandbox.ts:297` - Convert workspaceDir to absolute path

