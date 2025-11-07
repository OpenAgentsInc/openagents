---

title: "ADR-0007: Modular Bridge Server Components"
date: "2025-11-07"
status: "In Review" # Proposed | Draft | In Review | Accepted | Rejected | Deprecated | Superseded
authors:
  - "Claude (AI Agent)"
reviewers: ["AtlantisPleb"] # When ready for review, add GitHub handles as strings

---

## 1. Context and Problem Statement

The November 2025 audit flagged `DesktopWebSocketServer.swift` as a **1,817-line monolith** and the #2 hotspot for defect risk in the codebase. The file mixes 13+ distinct responsibilities:

- WebSocket transport (NWListener, connection handling)
- JSON-RPC routing (15+ method dispatch)
- Agent process orchestration (Claude, Codex)
- CLI discovery (recursive binary search)
- Stdout/JSONL parsing and translation
- Tinyvex persistence
- File tailing with polling
- Session mode management
- Thread/history API endpoints
- File system operations
- Terminal command execution
- Orchestration (Foundation Models integration)
- Bonjour service advertisement

**The real problem:** We can't ship agent provider registration (needed for "OpenAgents Coder" native agent) without first untangling this monolith. The switch statement for agent launching is buried in 450 lines of JSON-RPC dispatch logic. Adding a new agent requires touching 10+ locations across this file.

**Audit finding:** "Modularize DesktopWebSocketServer into smaller components (handshake + JSON-RPC router + agent process/tailer + threads listing) with unit tests per component." Marked as **P0 priority**.

## 2. Decision Drivers

**Immediate pain points forcing this decision:**

1. **Agent Provider Blocked:** Can't implement pluggable agent system (issue #1431 context) without extracting process management from the monolith
2. **Testing Difficulty:** 500 lines of tests but only 30% coverage because everything is coupled
3. **Review Overhead:** Every PR touching this file requires reviewing unrelated code (Tinyvex + agent launch + file tailing in one file)
4. **Concurrency Bugs:** Manual `DispatchQueue` usage instead of actors leads to race conditions
5. **Duplicate Logic:** CLI discovery code duplicated for each agent (Claude, Codex)
6. **Logging Noise:** 160+ print statements make debugging impossible

**What happens if we don't do this:** Agent provider registration adds another 300 lines to the monolith, making it 2,100+ lines. The file becomes unmaintainable and we're stuck with CLI-only agents forever.

## 3. Considered Options

### Option 1: Extract into Modular Actors (Chosen)

**Description:** Split into focused components with clear boundaries:
- `SessionUpdateHub` - Actor for canonical persistence+broadcast
- `HistoryApi` - Actor for Tinyvex read operations
- `JsonRpcRouter` - Handler registration pattern for method dispatch
- `AgentProcessRunner` - (Future) Process launching and management
- `ServerTransport` - (Future) NWListener wrapper

Each component has its own tests. DesktopWebSocketServer becomes a ~200-line coordinator.

**Real-world impact:**
- Agent provider registration becomes independent extraction
- Each component can be tested in isolation
- Tinyvex changes don't force reading agent launch code

**Pros:**
- Each component <300 lines, easy to understand
- Actor isolation fixes race conditions
- Can mock components for testing
- Future-proofs for agent provider system

**Cons:**
- Incremental migration creates temporary duplication (both new handlers and old switch)
- More files to navigate (7 new files vs 1 monolith)
- Requires discipline to finish migration (can't leave it half-done)
- 4-6 PRs to complete vs 1 big rewrite

### Option 2: Keep Monolith, Add Comments

**Description:** Leave structure as-is, add `// MARK:` comments for organization.

**Real-world impact:** Each new feature still touches 1,817 lines.

**Pros:**
- Zero migration cost
- All logic in one place

**Cons:**
- Audit P0 violation stays unfixed
- Agent provider system impossible to implement cleanly
- Test coverage stays at 30%
- Race conditions persist
- Can't onboard new contributors to this file

### Option 3: Big Rewrite (Extract Everything at Once)

**Description:** One massive PR that splits all 13 responsibilities into components.

**Real-world impact:** High-risk PR, long review time, blocks other work.

**Pros:**
- No half-migrated state
- Clean final architecture immediately

**Cons:**
- Risky: breaks everything if bugs introduced
- Blocks concurrent work (agent provider PR would conflict)
- Long PR review cycle (1000+ line diff)
- Hard to revert if problems found
- Discourages experimentation (all-or-nothing)

## 4. Decision Outcome

**Chosen Option:** Extract into Modular Actors (Option 1)

**Rationale:**

We chose incremental extraction because **agent provider registration can't wait for a big rewrite**. The audit was clear: this is P0. Doing it incrementally lets us:

1. Ship each component with tests as we go (de-risk)
2. Keep main branch stable (other agents can work concurrently)
3. Get early feedback on the handler pattern (adjust before it's set in stone)
4. Extract agent launching logic specifically to unblock provider registration

The switch-to-handler migration provides a clean seam: old logic stays in switch as fallback, new logic uses registered handlers. Once all handlers migrate, delete the switch. This is safer than rewriting everything in one shot.

**Trade-off we're accepting:** 4-6 PRs with temporary duplication. But each PR ships something useful and keeps the build green.

## 5. Consequences

### What We Get (The Good Stuff)

- **Unblocks Agent Provider System:** Can extract `launchAgentProcess()` into `AgentProvider` implementations without touching 1,817 lines
- **Better Test Coverage:** 1,189 lines of tests written (vs 500 before), components individually testable
- **Faster Reviews:** Changes to Tinyvex history don't require reviewing agent launch code
- **Actual Actors:** Fix race conditions with proper Swift concurrency instead of manual DispatchQueue
- **Easier Onboarding:** New contributors can understand SessionUpdateHub (177 lines) vs DesktopWebSocketServer (1,817 lines)

### What It Costs (The Real Trade-offs)

- **6 new files:** Navigation overhead (7 files vs 1 monolith)
- **Migration discipline:** Must finish the extraction or we're stuck in half-migrated state forever
- **Temporary duplication:** Both router handlers and switch statement exist during transition
- **Integration complexity:** Components need wiring in coordinator (init boilerplate)
- **4-6 PRs:** Can't ship it all at once, incremental means we're "in progress" for weeks

### What Changes (The Cultural Impact)

- New pattern: handler registration instead of switch statements
- Agent work must coordinate with bridge refactoring (both touching same code)
- Future features should extract components first, not add to monolith
- Tests become requirement, not nice-to-have (actors enforce isolation)

**Bottom line:** We're trading 4-6 PRs of migration overhead for the ability to ship agent provider registration without rewriting the entire bridge. The audit said this is P0, so we're biting the bullet.

## 6. Validation Plan

**Success indicators (does this actually help?):**

- ✅ Agent provider PR can extract `launchAgentProcess()` without touching 1,000+ lines
- ✅ Test coverage for bridge code reaches 70%+ (vs 30% before)
- ✅ DesktopWebSocketServer drops to <300 lines
- ✅ Audit P0 finding resolved
- ⏱️ New agent registration (e.g., "OpenAgents Coder") takes <100 lines of new code

**Failure indicators (time to reconsider):**

- ❌ Still in "half-migrated" state after 2 months
- ❌ Test coverage drops because component isolation is too hard
- ❌ Integration bugs increase (components don't communicate properly)
- ❌ Agent provider PR still can't land because of coupling
- ❌ More files created but total complexity doesn't decrease

**Timeline for re-evaluation:** 2026-01-07 (2 months from decision)

If by then we haven't removed the switch statement fallback and reduced DesktopWebSocketServer to <500 lines, abandon this approach and either:
- Revert to monolith with better comments
- Commit to big-rewrite approach instead

**Current status (as of 2025-11-07):**
- ✅ Phase 1 complete: SessionUpdateHub extracted (177 lines + 278 test lines)
- ✅ Phase 2 complete: HistoryApi extracted (165 lines + 451 test lines)
- ✅ Phase 3a complete: JsonRpcRouter extracted (211 lines + 460 test lines)
- ✅ Phase 3b complete: Router integrated, handlers registered
- ⏳ Remaining: AgentProcessRunner, ServerTransport, BridgeLogging, remove switch

## 7. References

- **Audit Report:** `docs/audits/20251107/findings.md` (P0 finding: "Oversized Monoliths")
- **Issue Tracking:** GitHub issue #1431 (Agent Registration System Refactoring)
- **Related ADRs:**
  - ADR-0002: Agent Client Protocol (all updates must be ACP)
  - ADR-0004: iOS ↔ Desktop WebSocket Bridge (this refactors the server side)
- **Implementation Commits:**
  - `3335fad1` - Phase 1: SessionUpdateHub
  - `decffd1b` - Phase 2: HistoryApi
  - `e47ed5bc` - Phase 3a: JsonRpcRouter foundation
  - `7c5a45b9` - Phase 3b: Router integration
- **Coordinating Work:** Agent provider registration (separate PR) will build on this extraction
