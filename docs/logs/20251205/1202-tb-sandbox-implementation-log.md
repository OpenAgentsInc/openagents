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

