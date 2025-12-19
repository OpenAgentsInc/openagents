# BlackBox 12-Hour Example: Autonomous Autopilot Session

**Goal**: Create a best-case scenario example of Autopilot running autonomously for 12 hours on the OpenAgents platform repo. This serves as:
1. Target behavior for our coding agents to aim at
2. Test fixture for replay simulator
3. Documentation of what productive autonomous work looks like

---

## CONTEXT

**Repo**: OpenAgents/platform (simulated from current state)
**Agent**: Autopilot
**Duration**: 12 hours (720 minutes)
**Mode**: Full Auto
**Objective**: Move platform forward - complete priorities, add features, harden, simplify
**Issues**: Fictional but realistic, based on actual codebase gaps

**Current Platform State** (from exploration):
- 6 crates: server, autopilot, chat_service, llm, daytona, storybook
- Full Auto mode works but limited to ~10 min SSE timeout
- Issue system with priority + lease-based claiming
- Admin dashboard (/aiur) exists but incomplete
- Tests exist but service layer under-tested

---

## MISSING PRIMITIVES (to add to BlackBox format)

### 1. Session Lifecycle
```
@start id=sess_12h_001 budget=$50 duration=12h
@checkpoint hour=4 tokens=45000 cost=$12.30
@pause reason="waiting for CI"
@resume
@end summary="Completed 8 issues, 3 PRs merged"
```

### 2. Time Awareness
```
# t=00:00:00 (session start)
# t=04:23:17 (4 hours 23 minutes in)
# t=11:58:00 (near end)
```

### 3. Budget Tracking
```
# budget: $50.00 remaining=$47.23 tokens=12847
```

### 4. Priority Assessment
```
@assess →
  P0: Admin dashboard (roadmap Phase 1)
  P1: Lease expiration background job
  P2: Service layer tests
  P3: Error handling consistency
```

### 5. Human Touchpoints
```
@notify "PR #47 ready for review"
@wait-approval pr=47 timeout=2h → approved
@escalate "Tests failing on CI, may need human help"
```

### 6. Multi-Issue Coordination
```
@batch [issue-142, issue-156] reason="related billing changes"
```

---

## 12-HOUR SESSION STRUCTURE

| Hour | Focus | Expected Output |
|------|-------|-----------------|
| 0-1 | Orientation | Analyze repo, recall memories, create plan |
| 1-4 | High Priority | Admin dashboard monitoring (Phase 1 roadmap) |
| 4-5 | Checkpoint | Review progress, adjust priorities |
| 5-7 | Hardening | Lease expiration job, issue service tests |
| 7-8 | Simplification | Remove dead code, consolidate |
| 8-10 | Features | Chat persistence, better error messages |
| 10-11 | Polish | Documentation, cleanup |
| 11-12 | Wrap-up | Final PRs, handoff notes |

---

## EXAMPLE DOCUMENT STRUCTURE

### File: `docs/examples/12h-session-adrian.bbox`

```
---
id: sess_12h_adrian_001
mode: auto
model: sonnet-4
agent: adrian
repo: OpenAgentsInc/platform
duration: 12h
budget: $50
skills: []
mcp: [github]
---

# ═══════════════════════════════════════════════
# HOUR 0: ORIENTATION (00:00 - 01:00)
# ═══════════════════════════════════════════════

# t=00:00:00
@start budget=$50

# Check for memories from past sessions
r: "platform" "priorities" → [2 matches]
  sess_20250617: discussed Phase 1 roadmap
  sess_20250615: reviewed issue system design

# Load current state
c:github.issues state=open → [12 issues]
t:read docs/decisions/userstory.md → [19 lines]
t:read docs/decisions/agent-algorithms.md → [348 lines]

# ... (first 10 minutes detailed)
# ... (rest summarized or in separate files)
```

---

## FIRST 10 MINUTES (~200 lines, every action)

Fully detailed showing every tool call and decision:

1. **Session initialization** (t=0:00-0:30)
   - @start with budget and duration
   - Load session metadata
   - a: "Starting 12-hour autonomous session..."

2. **Memory recall** (t=0:30-1:30)
   - r: query past sessions about platform
   - r: query previous priorities
   - a: synthesize what was learned

3. **Current state analysis** (t=1:30-4:00)
   - t:read docs/decisions/userstory.md
   - t:read docs/decisions/agent-algorithms.md
   - t:read docs/decisions/blackbox.md
   - c:github.issues state=open (list all open issues)
   - t:git log -10 (recent commits)

4. **Codebase orientation** (t=4:00-7:00)
   - t:grep "TODO\|FIXME" type=rs
   - t:read crates/server/src/services/autopilot_service.rs
   - t:read crates/server/src/services/issue_service.rs
   - x:explore "What are the main architectural components?"

5. **Priority assessment** (t=7:00-9:00)
   - @assess with ranked priority list
   - a: explain reasoning for priorities
   - Identify first task (admin dashboard monitoring)

6. **Plan creation** (t=9:00-10:00)
   - p:create "12-hour session plan"
   - Set checkpoints at hours 4, 8, 12
   - a: "Beginning work on highest priority..."

---

## EXECUTION PLAN

### Step 1: Update BlackBox spec
**File**: `docs/decisions/blackbox.md`

Add new primitives:
```
| `@` | Lifecycle | `@start`, `@checkpoint`, `@end` |
```

Add sections:
- Session lifecycle (@start, @checkpoint, @pause, @resume, @end)
- Time tracking (# t=HH:MM:SS)
- Budget tracking (# budget: $X remaining=$Y)
- Priority assessment (@assess)
- Human touchpoints (@notify, @wait-approval, @escalate)
- Multi-issue batching (@batch)

### Step 2: Create example session
**File**: `docs/examples/12h-autopilot-session.bbox`

Structure:
```
---
[header with session metadata]
---

# ═══════════════════════════════════════════════
# HOUR 0: ORIENTATION (00:00 - 01:00)
# ═══════════════════════════════════════════════

[~200 lines of detailed first 10 minutes]

# ... rest of hour 0 summarized ...

# ═══════════════════════════════════════════════
# HOUR 1-4: ADMIN DASHBOARD (01:00 - 05:00)
# ═══════════════════════════════════════════════

[key moments, PRs created, blockers hit]

# ... hours 5-12 with checkpoints and summaries ...

# ═══════════════════════════════════════════════
# SESSION COMPLETE
# ═══════════════════════════════════════════════

@end summary="..." prs=[201,202,203,204] issues_closed=8
# tokens=487000 cost=$42.17 duration=12h
```

### Step 3: Create examples directory
**Directory**: `docs/examples/`

---

## FILES TO MODIFY/CREATE

| File | Action |
|------|--------|
| `docs/decisions/blackbox.md` | Add new primitives section |
| `docs/examples/12h-autopilot-session.bbox` | Create (~500 lines) |

---

## REALISTIC WORK IN 12 HOURS

Based on exploration, Autopilot could realistically:

### Complete (8-10 items):
1. ✓ Admin dashboard sandbox monitoring panel
2. ✓ Lease expiration background job
3. ✓ Issue service unit tests
4. ✓ Autopilot service refactoring (break up large function)
5. ✓ Chat message persistence verification
6. ✓ Error type consolidation
7. ✓ WebSocket reconnection handling
8. ✓ Documentation for service layer

### Partial/Blocked (2-3 items):
- ◐ LLM provider fallback (needs config decisions)
- ◐ OpenRouter integration (needs API keys)
- ⊘ Billing integration test (needs Stripe test mode)

### PRs Created:
- PR #201: Admin dashboard monitoring
- PR #202: Background job for lease cleanup
- PR #203: Issue service tests
- PR #204: Service layer documentation

---

## SUCCESS CRITERIA

The example document should:
1. Be valid BlackBox format (parseable)
2. Show realistic timing (not too fast, not too slow)
3. Demonstrate all primitives (tools, skills, MCP, subagents, plans)
4. Include failures and recovery (not just happy path)
5. Show human touchpoints (notifications, waiting for approval)
6. End with clear handoff state
