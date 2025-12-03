# Pi-Mono Integration: Lessons Learned and Future Directions

A deep analysis of what we've ported from Mario Zechner's pi-mono, what remains, and how we should evolve the integration for MechaCoder's overnight automation loop.

## Executive Summary

We have successfully ported the foundational infrastructure from pi-mono to Effect TypeScript:
- Multi-provider LLM abstraction (OpenRouter, Anthropic, Gemini, OpenAI)
- Model registry generation from models.dev/OpenRouter metadata
- Core tools (read, edit, write, bash, grep, find, ls)
- Agent loop with verification state tracking
- Session management with JSONL persistence
- Token/cost accounting

**The critical insight:** Pi-mono and MechaCoder solve fundamentally different problems.

| Aspect | Pi-mono (Copilot) | MechaCoder (Automation) |
|--------|-------------------|-------------------------|
| Human in loop | Always present | Never present |
| Session length | Single context window | Hours/days across windows |
| Error recovery | Human steers | Must self-recover |
| Orchestration | Human provides | Agent must provide |
| Verification | Human judges | Must self-verify |

Pi-mono's minimalism works because **the human is the orchestrator**. MechaCoder runs overnight without supervision—it needs structure that pi-mono offloads to the user.

The solution isn't to bloat the coding agent's prompt. It's to **split orchestration from execution** using a multi-agent architecture, as Anthropic's ["Effective Harnesses for Long-Running Agents"](https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents) recommends.

---

## The Copilot vs Automation Distinction

### Why Pi-Mono Can Be Minimal

Mario's agent succeeds with 4 tools and ~200 tokens of system prompt because:

1. **The human provides context** - User knows what needs to be done
2. **The human provides verification** - User sees if output is correct
3. **The human provides course-correction** - User can say "no, try this instead"
4. **Single context window** - No need to bridge sessions

Pi's "YOLO by default" philosophy makes sense when a human is watching. The human is the orchestrator, the planner, the verifier.

### Why MechaCoder Needs More Structure

MechaCoder runs via cron/launchd overnight. No human will see failures until morning. This changes everything:

1. **Must self-orient** - What's the state of the repo? What was done last session?
2. **Must self-plan** - Which task is highest priority? What are the subtasks?
3. **Must self-verify** - Did the code actually work? Did tests pass?
4. **Must bridge sessions** - Leave artifacts for the next context window

The Anthropic article identifies the exact failure modes we've seen:
- Agent tries to "one-shot" too much and runs out of context mid-implementation
- Agent declares victory prematurely
- Agent leaves environment in broken state for next session
- Agent marks features done without proper testing

---

## The Multi-Agent Architecture

Anthropic's solution is a two-agent architecture. We should adopt this pattern:

### Orchestrator Agent

The orchestrator runs at the start of each session and handles:

1. **Orientation** - Read progress files, git log, task state
2. **Task Selection** - Pick the highest-priority ready task
3. **Subtask Decomposition** - Break task into implementable chunks
4. **Verification Coordination** - Ensure tests run after changes
5. **Session Cleanup** - Commit progress, update state files

The orchestrator's prompt can be larger because it runs once per session, not per turn.

### Coding Subagent

The coding subagent is invoked per-subtask and should be **minimal like pi-mono**:

```markdown
You are an expert coding assistant. Complete the following subtask:

{subtask_description}

Tools: read, write, edit, bash

When done, output SUBTASK_COMPLETE.
```

This is almost identical to pi-mono's prompt. The model knows how to code. We don't need to explain Effect patterns, git conventions, or verification steps in the coding prompt—the orchestrator handles all that.

### Architecture Diagram

```
┌─────────────────────────────────────────────────────────┐
│                    Orchestrator Agent                    │
│  - Reads .openagents/tasks.jsonl                        │
│  - Reads progress.md / git log                          │
│  - Picks next task, decomposes into subtasks            │
│  - Coordinates verification (typecheck, tests)          │
│  - Commits and updates state                            │
└─────────────────────────────────────────────────────────┘
                           │
                           ▼
          ┌────────────────┴────────────────┐
          │                                 │
          ▼                                 ▼
┌─────────────────────┐          ┌─────────────────────┐
│  Coding Subagent    │          │  Coding Subagent    │
│  (Subtask 1)        │          │  (Subtask 2)        │
│  - Minimal prompt   │          │  - Minimal prompt   │
│  - 4 tools          │          │  - 4 tools          │
│  - Single focus     │          │  - Single focus     │
└─────────────────────┘          └─────────────────────┘
```

### Why This Works

1. **Coding subagent stays minimal** - Embraces pi-mono's lesson
2. **Orchestration is explicit** - Not hidden in a mega-prompt
3. **Clean separation of concerns** - Each agent does one thing well
4. **Debugging is easier** - Can inspect orchestrator decisions separately
5. **Extensible** - Can add testing agent, cleanup agent later

---

## Coordination Artifacts

Following Anthropic's approach, we need structured files for cross-session coordination:

### Feature/Subtask List (JSON)

```json
{
  "taskId": "oa-abc123",
  "subtasks": [
    {
      "id": "sub-001",
      "description": "Add TaskService.getById method",
      "status": "done",
      "verifiedAt": "2025-12-03T08:30:00Z"
    },
    {
      "id": "sub-002", 
      "description": "Add tests for getById",
      "status": "in_progress",
      "startedAt": "2025-12-03T09:00:00Z"
    },
    {
      "id": "sub-003",
      "description": "Update CLI to use getById",
      "status": "pending"
    }
  ]
}
```

Using JSON (not Markdown) because models are less likely to corrupt structured data.

### Progress File

```markdown
# Session 2025-12-03T09:00:00Z

## Orientation
- Task: oa-abc123 "Add TaskService.getById"
- Previous session completed sub-001
- Tests passing, no broken state

## This Session
- Working on sub-002: Add tests for getById
- Added test file src/tasks/service.test.ts
- Tests passing

## Next Session Should
- Complete sub-003: Update CLI to use getById
- Run full test suite before marking task done
```

### Init Script

```bash
#!/bin/bash
# .openagents/init.sh - Run at start of each session

# Start any dev servers
bun run dev &

# Run quick sanity check
bun test --bail 2>/dev/null || echo "WARNING: Tests failing at session start"

# Show current state
git status
cat .openagents/progress.md | tail -20
```

---

## What Pi-Mono Got Right (Still Relevant)

### 1. Minimal Coding Prompt

Pi's prompt works because the model is RL-trained for coding. We should keep coding subagent prompts minimal.

### 2. Four Core Tools

For the coding subagent: read, write, edit, bash. Let the model compose grep/find via bash.

The orchestrator may need additional tools for task management.

### 3. Structured Tool Results

Pi's `{output, details}` pattern is valuable for HUD rendering:

```typescript
return {
  output: `Created file src/foo.ts`,  // For LLM
  details: { path: "src/foo.ts", lines: 42, bytes: 1024 }  // For HUD
};
```

### 4. Cross-Provider Context Handoff

Still valuable for model escalation (cheap model for exploration, expensive for implementation).

---

## What We Should Port

### 1. Tools Manager (Auto-Install rg/fd)

Pi's `tools-manager.ts` auto-downloads ripgrep and fd binaries when missing:

```typescript
export async function ensureTool(tool: "fd" | "rg"): Promise<string | null> {
  const existingPath = getToolPath(tool);
  if (existingPath) return existingPath;
  return await downloadTool(tool);
}
```

Our grep/find tools assume `rg` exists in PATH. For MechaCoder running headlessly, this is fragile.

**Recommendation:** Port `tools-manager.ts` with Effect-based download/extract/chmod.

### 2. Session Branching

Pi supports creating branched sessions from a specific message index:

```typescript
createBranchedSession(state, branchFromIndex): string
```

This enables "what if" explorations and rollback to clean states. Our `SessionManager` only supports linear append.

**Recommendation:** Add branching support for MechaCoder retries after failed verification.

### 3. Slash Commands

Pi's slash command system loads markdown templates with argument substitution:

```markdown
---
description: Run a code review sub-agent
---
Spawn yourself as a sub-agent via bash to do a code review: $@
```

This is a powerful UX pattern for common workflows without hardcoding them.

**Recommendation:** Port slash command loader for CLI/HUD integration.

### 4. HTML Transcript Export

Pi's `export-html.ts` (50KB!) generates beautiful session exports. This is valuable for:
- Debugging failed runs
- Sharing solutions
- Building a corpus for fine-tuning

**Recommendation:** Port as a post-processing utility for run logs.

---

## Effect-Specific Advantages

The Effect runtime gives us capabilities pi-mono doesn't have:

### 1. Composable Agent Loops

Orchestrator and subagent can share infrastructure:

```typescript
const orchestratorLoop = agentLoop.pipe(
  Effect.provideService(Tools, orchestratorTools),
  Effect.provideService(Prompt, orchestratorPrompt)
);

const codingLoop = agentLoop.pipe(
  Effect.provideService(Tools, minimalTools),
  Effect.provideService(Prompt, minimalPrompt)
);
```

### 2. Scoped Resource Management

Session artifacts clean up automatically:

```typescript
const withProgressFile = Effect.acquireRelease(
  createProgressFile(taskId),
  (file) => finalizeAndCommit(file)
);
```

### 3. Structured Concurrency

Run verification in parallel with cleanup:

```typescript
yield* Effect.all([
  runTypecheck(),
  runTests(),
  updateProgressFile()
], { concurrency: "unbounded" });
```

### 4. Observability

Effect's tracing powers the HUD naturally:

```typescript
const instrumentedSubagent = codingLoop.pipe(
  Effect.withSpan("subagent.coding", { attributes: { subtaskId } })
);
```

---

## HUD Integration Points

The Flow HUD (oa-b78d3f through oa-924781) should visualize the multi-agent architecture:

1. **Orchestrator state** - Current task, subtask decomposition, progress
2. **Subagent timeline** - Which subtasks ran, duration, outcome
3. **Verification badges** - Green/red for typecheck/tests per subtask
4. **Cross-session history** - What happened in previous context windows
5. **Live progress file** - Real-time view of coordination artifacts

---

## Prioritized Recommendations

### P0 - Critical for Long-Running Automation

| Task | Rationale |
|------|-----------|
| **Orchestrator/Subagent Architecture** | Core pattern for overnight runs |
| **Subtask Decomposition** | Prevent "one-shot" failures |
| **Progress File Infrastructure** | Bridge context windows |
| **Tools Manager** | Auto-install rg/fd for headless runs |

### P1 - Valuable for Reliability

| Task | Rationale |
|------|-----------|
| **Init Script Runner** | Verify clean state at session start |
| **Session Branching** | Retry from checkpoint on failure |
| **Cross-Provider Transform** | Model escalation support |
| **Benchmarking Harness** | Measure improvements |

### P2 - Nice to Have

| Task | Rationale |
|------|-----------|
| **HTML Export** | Debug/share transcripts |
| **Slash Commands** | Workflow shortcuts for interactive use |
| **Streaming Tool Results** | Real-time bash output |
| **OpenTelemetry Integration** | Production monitoring |

---

## Key Insight: Separation of Concerns

The fundamental lesson is **not** that prompts should be minimal. It's that:

1. **Coding prompts should be minimal** - The model knows how to code
2. **Orchestration should be explicit** - Not hidden in a mega-prompt
3. **Coordination artifacts matter** - Progress files, subtask lists, init scripts
4. **Verification must be systematic** - Not optional, not skipped

Pi-mono's minimalism works because Mario is the orchestrator. MechaCoder needs an orchestrator agent to fill that role.

---

## Conclusion

The pi-mono port gave us solid infrastructure. The next phase should:

1. **Split do-one-task into orchestrator + subagent** - Adopt Anthropic's pattern
2. **Add coordination artifacts** - Progress files, subtask lists, init scripts
3. **Keep coding subagent minimal** - Embrace pi-mono's lesson where it applies
4. **Invest in verification** - Self-testing before marking complete

The goal is not to copy pi-mono's interface (copilot) but to adopt its insight (minimalism) where appropriate—in the coding subagent—while building the orchestration layer that overnight automation requires.
