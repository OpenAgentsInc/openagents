# Issue #011: Demo Preparation

**Component**: Demo & Documentation
**Priority**: P2 (Medium)
**Estimated Effort**: 2-3 days
**Dependencies**: #001-#010 (all components working)
**Assignee**: TBD

---

## Overview

Prepare for video recording: demo script, sample repo setup, expected outputs, PR examples, upgrade JSON visualization.

**Location**: `private/overnight/demo-script.md`

---

## Deliverables

### 1. Demo Script (demo-script.md)

Step-by-step guide for recording demo video:

**Part 1: Setup (2 minutes)**
- Show clean repo state
- Load upgrade manifest (`nightly-refactor.json`)
- Start scheduler with `openagents scheduler start --upgrade examples/nightly-refactor.json`
- Show iOS app connected

**Part 2: First Cycle (3 minutes)**
- Wait for first scheduled wake-up
- Show FM decision process (thinking, analyzing session history)
- Show task queued in iOS app
- Show agent (Claude Code) starting work
- Show tool calls streaming in real-time
- Show completion

**Part 3: PR Creation (2 minutes)**
- Show branch created
- Show commits generated from tool calls
- Show PR created on GitHub
- Show PR preview in iOS app with approve/cancel

**Part 4: Second Cycle (2 minutes)**
- Show next wake-up
- Different agent (Codex) selected
- Different task (test generation)
- Another PR created

**Part 5: Results & Vision (2 minutes)**
- Show morning view: multiple PRs created overnight
- Show quality of work (code changes, tests)
- Show upgrade JSON manifest (deterministic logic)
- Explain future: Nostr marketplace, reputation, Bitcoin payments

**Total**: ~11 minutes

### 2. Sample Repo Setup

Create test repo with:
- Some files needing refactoring (complex error handling)
- Low test coverage areas
- Recent commits showing user patterns
- `.openagents/` directory with upgrade manifests

### 3. Expected Outputs

Document what the demo should produce:
- **PR #1**: "Refactor BridgeManager error handling with Swift Result types"
  - ~150 lines changed
  - Proper error propagation
  - Tests still passing

- **PR #2**: "Add comprehensive tests for DesktopWebSocketServer"
  - ~200 lines of test code
  - Coverage increased from 65% to 85%
  - All tests passing

- **PR #3**: "Improve SessionUpdateHub concurrency safety"
  - Actor isolation
  - No race conditions
  - Tests updated

### 4. Visualization

Create diagrams:
- System architecture (macOS orchestration + iOS monitoring)
- Overnight timeline (wake-ups, decisions, agent work, PRs)
- Upgrade manifest structure (JSON → execution)
- Future vision (Nostr marketplace)

### 5. Talking Points

**Key Messages**:
1. "Agents work for you while you sleep"
2. "On-device Apple Intelligence makes smart decisions"
3. "Wake up to quality PRs, not busywork"
4. "All logic is deterministic JSON - shareable, verifiable, monetizable"
5. "Future: marketplace of overnight upgrades, earn Bitcoin for contributions"

**Demo Hooks**:
- Show real Foundation Models deciding in real-time
- Show two agents working in parallel (Claude + Codex)
- Show iOS app updating while agents work on macOS
- Show upgrade JSON is simple, readable, portable

---

## Pre-Flight Checklist

**Before Recording**:
- [ ] Clean repo (no uncommitted changes)
- [ ] gh CLI authenticated
- [ ] Foundation Models available (macOS 26+)
- [ ] iOS app paired with macOS via bridge
- [ ] Sample upgrade manifests tested
- [ ] Screen recording software ready
- [ ] Good lighting and audio setup

**Dry Run**:
- [ ] Run full demo 2-3 times without recording
- [ ] Time each section
- [ ] Fix any issues
- [ ] Prepare backup plan for live failures

---

## Testing

1. Run demo end-to-end without recording
2. Verify all PRs created successfully
3. Verify FM decisions make sense
4. Verify iOS app updates correctly
5. Time total duration (should be ~11 min)

---

## Acceptance Criteria

- [ ] Demo script written with timestamps
- [ ] Sample repo prepared
- [ ] Expected outputs documented
- [ ] Visualizations created
- [ ] Talking points finalized
- [ ] Dry run succeeds 3x in a row
- [ ] Recording produces high-quality video

---

## References

- README.md - Demo flow section
