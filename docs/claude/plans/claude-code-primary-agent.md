# Plan: Switch MechaCoder to Claude Code Primary Agent

## Goal
Make Claude Code the primary agent for MechaCoder's Golden Loop, with Grok/OpenRouter as backup only.

## Current State Summary

**What's already done:**
- Claude Code detection, wrapper, MCP tools, router (oa-cc01-06)
- Orchestrator architecture with 9-phase flow (oa-pi13-16)
- Subagent router with fallback logic (runBestAvailableSubagent)
- SDK harmonization layer
- E2E test with CC (mocked/fallback)

**What's missing:**
1. Entrypoints (`do-one-task.ts`, `overnight.ts`) still use direct Grok-based `agentLoop()`
2. Minimal subagent prompt not extracted (oa-pi17 still open)
3. Error recovery in progress (oa-d313a1)
4. No CLI command that uses orchestrator

---

## Task Changes

### NEW TASKS TO CREATE

| ID | Title | Priority | Type | Description |
|----|-------|----------|------|-------------|
| oa-cc-switch-01 | Wire orchestrator into do-one-task.ts | P0 | task | Make orchestrator the default path, but **keep old Grok-based agentLoop as backup**. Add `--legacy` flag or `useOrchestrator: false` config to use old path when needed. |
| oa-cc-switch-02 | Add orchestrator CLI command | P0 | task | Create `bun run mechacoder:orchestrator --dir . --once` entry that uses runOrchestrator. This becomes the new default entrypoint. |
| oa-cc-switch-03 | Wire orchestrator into overnight.ts | P1 | task | Update overnight multi-task runner to use orchestrator flow instead of direct agentLoop. |
| oa-cc-switch-04 | E2E validation with real CC tasks | P1 | task | Run 2-3 real tasks through orchestrator+CC path, verify commit/push/close works end-to-end. Document any issues found. |
| oa-cc-switch-05 | Update project.json defaults for CC | P1 | task | Set claudeCode.enabled=true, claudeCode.preferForComplexTasks=true as defaults. Ensure fallbackToMinimal=true. |

### TASKS TO REPRIORITIZE

| Task ID | Current Priority | New Priority | Reason |
|---------|------------------|--------------|--------|
| oa-pi17 | P1 | **P0** | CRITICAL: Minimal subagent is fallback when CC unavailable |
| oa-d313a1 | P1 (in_progress) | **P0** | CRITICAL: CC error recovery needed for production use |
| oa-pi12 | P0 | P0 | Keep as-is: Epic for orchestrator work |
| oa-e0d033 | P1 | **P0** | Elevate: CC integration epic should track switchover |
| oa-820036 | P1 | **P2** | Deprioritize: Test fix not blocking switchover |
| oa-pi02 | P1 | **P2** | Deprioritize: Cross-provider transform not needed for CC |
| oa-pi03 | P1 | **P2** | Deprioritize: Benchmarking nice-to-have |
| oa-44834c | P1 | **P2** | Deprioritize: Tool install useful but not blocking |
| oa-0d6425 | P1 | **P3** | Deprioritize: Pi-port parity not needed for CC |
| oa-17d0cd | P1 | **P3** | Deprioritize: SDK harmonization epic not blocking |
| oa-5dc986 | P3 (in_progress) | **P4** | Deprioritize: SDK agent schemas not blocking |

### TASKS TO CLOSE

| Task ID | Reason |
|---------|--------|
| oa-e0d033 | After switchover complete, close as parent epic |
| oa-pi12 | After oa-pi17 lands, close this epic |

---

## Implementation Order

### Phase 1: Core Switchover (P0 tasks)
Execute in this order:

1. **oa-pi17** - Create minimal coding subagent prompt
   - File: `src/agent/orchestrator/subagent.ts`
   - Already has placeholder SUBAGENT_SYSTEM_PROMPT
   - Extract to ~50 token prompt per pi-mono pattern
   - Tests exist

2. **oa-d313a1** - Complete CC error recovery
   - File: `src/agent/orchestrator/claude-code-subagent.ts`
   - Already has retry logic, timeout handling
   - Need: AbortController integration, mid-task recovery
   - Ensure fallback triggers correctly

3. **oa-cc-switch-01** - Wire orchestrator into do-one-task.ts
   - File: `src/agent/do-one-task.ts`
   - Replace `agentLoop()` with `runOrchestrator()`
   - Honor project.json claudeCode settings
   - Add `useOrchestrator` flag (default true)

4. **oa-cc-switch-02** - Add orchestrator CLI
   - File: `src/cli/mechacoder.ts` (new or extend existing)
   - Entry: `bun run mechacoder:orchestrator --dir . --once`
   - Use runOrchestrator directly

### Phase 2: Integration (P1 tasks)

5. **oa-cc-switch-05** - Update project.json defaults
   - File: `src/tasks/project.ts`
   - Set: `claudeCode.enabled: true`, `claudeCode.preferForComplexTasks: true`

6. **oa-cc-switch-03** - Wire orchestrator into overnight.ts
   - File: `src/agent/overnight.ts`
   - Multi-task loop now calls orchestrator

7. **oa-cc-switch-04** - E2E validation
   - Pick 2-3 open tasks from tasks.jsonl
   - Run via new CLI
   - Verify full loop works

### Phase 3: Cleanup

8. Close **oa-pi12** (orchestrator epic)
9. Close **oa-e0d033** (CC integration epic)
10. Deprioritize remaining pi-* and SDK tasks

---

## Critical Files to Modify

| File | Change |
|------|--------|
| `src/agent/do-one-task.ts` | Replace agentLoop with runOrchestrator |
| `src/agent/overnight.ts` | Use orchestrator for multi-task |
| `src/agent/orchestrator/subagent.ts` | Finalize minimal prompt |
| `src/agent/orchestrator/claude-code-subagent.ts` | Complete error recovery |
| `src/tasks/project.ts` | Update claudeCode defaults |
| `src/cli/mechacoder.ts` | Add orchestrator CLI entry |
| `.openagents/project.json` | Update this repo's config |
| `.openagents/tasks.jsonl` | Add new tasks, reprioritize existing |

---

## Success Criteria

1. `bun run mechacoder:orchestrator --dir . --once`:
   - Picks task from tasks.jsonl
   - Uses Claude Code as primary subagent
   - Falls back to Grok if CC unavailable/fails
   - Runs tests/typecheck
   - Commits and closes task

2. Tests pass: `bun test`

3. Typecheck passes: `bun run typecheck`

4. At least one real task completed through new flow

---

## JSON Patch for tasks.jsonl

### New tasks to add:

```jsonl
{"id":"oa-cc-switch-01","title":"Wire orchestrator into do-one-task.ts","description":"Make orchestrator the default path, but keep old Grok-based agentLoop as backup. Add --legacy flag or useOrchestrator: false config to use old path when needed.","status":"open","priority":0,"type":"task","labels":["claude-code","mechacoder","switchover"],"deps":[{"id":"oa-pi17","type":"blocks"}],"commits":[],"createdAt":"2025-12-03T07:00:00Z","updatedAt":"2025-12-03T07:00:00Z","closedAt":null}
{"id":"oa-cc-switch-02","title":"Add orchestrator CLI command","description":"Create bun run mechacoder:orchestrator --dir . --once entry that uses runOrchestrator directly. This becomes the primary entrypoint for CC-first operation.","status":"open","priority":0,"type":"task","labels":["claude-code","mechacoder","cli","switchover"],"deps":[{"id":"oa-cc-switch-01","type":"blocks"}],"commits":[],"createdAt":"2025-12-03T07:00:00Z","updatedAt":"2025-12-03T07:00:00Z","closedAt":null}
{"id":"oa-cc-switch-03","title":"Wire orchestrator into overnight.ts","description":"Update overnight multi-task runner to use orchestrator flow instead of direct agentLoop.","status":"open","priority":1,"type":"task","labels":["claude-code","mechacoder","switchover"],"deps":[{"id":"oa-cc-switch-01","type":"blocks"}],"commits":[],"createdAt":"2025-12-03T07:00:00Z","updatedAt":"2025-12-03T07:00:00Z","closedAt":null}
{"id":"oa-cc-switch-04","title":"E2E validation with real CC tasks","description":"Run 2-3 real tasks through orchestrator+CC path, verify commit/push/close works end-to-end. Document any issues found.","status":"open","priority":1,"type":"task","labels":["claude-code","mechacoder","testing","switchover"],"deps":[{"id":"oa-cc-switch-02","type":"blocks"}],"commits":[],"createdAt":"2025-12-03T07:00:00Z","updatedAt":"2025-12-03T07:00:00Z","closedAt":null}
{"id":"oa-cc-switch-05","title":"Update project.json defaults for CC","description":"Set claudeCode.enabled=true, claudeCode.preferForComplexTasks=true as defaults. Ensure fallbackToMinimal=true.","status":"open","priority":1,"type":"task","labels":["claude-code","mechacoder","config","switchover"],"deps":[],"commits":[],"createdAt":"2025-12-03T07:00:00Z","updatedAt":"2025-12-03T07:00:00Z","closedAt":null}
```

### Priority updates:

| Task | Change |
|------|--------|
| oa-pi17 | priority: 1 → 0 |
| oa-d313a1 | priority: 1 → 0 |
| oa-e0d033 | priority: 1 → 0 |
| oa-820036 | priority: 1 → 2 |
| oa-pi02 | priority: 1 → 2 |
| oa-pi03 | priority: 1 → 2 |
| oa-44834c | priority: 1 → 2 |
| oa-0d6425 | priority: 1 → 3 |
| oa-17d0cd | priority: 1 → 3 |
| oa-5dc986 | priority: 3 → 4 |
