# Issue #012: Documentation & ADR

**Component**: Documentation
**Priority**: P2 (Medium)
**Estimated Effort**: 2-3 days
**Dependencies**: #001-#011 (all components complete)
**Assignee**: TBD

---

## Overview

Write comprehensive documentation and create ADR (Architectural Decision Record) for overnight agent orchestration system.

**Locations**:
- ADR: `docs/adr/0007-overnight-agent-orchestration.md`
- API docs: Inline Swift documentation
- User guide: `docs/overnight-agents-guide.md`

---

## Deliverables

### 1. ADR-0007: Overnight Agent Orchestration Architecture

**Title**: Overnight Agent Orchestration Architecture

**Status**: Accepted

**Context**:
- Users want agents to work autonomously overnight
- Need to demonstrate control of Claude Code and OpenAI Codex
- Foundation Models should make orchestration decisions
- Logic must be deterministic and shareable (upgrade JSON)
- Future: marketplace via Nostr, reputation, Bitcoin payments

**Decision**:
We implement a macOS-only orchestration layer with:
1. **SchedulerService**: Cron-based wake-up with constraint checking
2. **DecisionOrchestrator**: FM-powered task selection and agent assignment
3. **TaskQueue**: Persistent work queue with SQLite/Tinyvex
4. **AgentCoordinator**: Multi-agent session manager
5. **PRAutomationService**: GitHub integration via gh CLI
6. **UpgradeExecutor**: JSON manifest runtime
7. **PolicyEnforcer**: AUP compliance and resource limits

iOS app provides monitoring UI via WebSocket bridge.

All logic encapsulated as declarative JSON "upgrade" manifests, designed for future transfer via Nostr (kind 30051).

**Consequences**:

*Positive*:
- Full autonomous overnight operation
- On-device FM decisions (privacy-first)
- Deterministic, shareable logic (JSON manifests)
- Real GitHub PRs from agents
- Foundation for compute marketplace

*Negative*:
- macOS-only execution (iOS = monitoring)
- Requires macOS 26+ for FM features
- Depends on `gh` CLI for PR creation
- No retry logic in MVP (fail-fast)

**Alternatives Considered**:
1. Cloud-based orchestration - Rejected (privacy, cost)
2. Hard-coded orchestration logic - Rejected (not shareable)
3. iOS execution - Rejected (Apple compliance, background limits)

**References**:
- ADR-0002: Agent Client Protocol
- ADR-0004: iOS ↔ Desktop WebSocket Bridge
- ADR-0006: Foundation Models for On-Device Intelligence
- docs/compute/issues/upgrades.md
- private/20251108-upgrades-convo/01.md

---

### 2. API Documentation

Add Swift documentation to all public types:

```swift
/// macOS-only background scheduler service for orchestrating overnight agent work.
///
/// Provides cron-based wake-up with constraint checking (power, network, CPU, user activity).
/// Integrates with `UpgradeExecutor` to run declarative JSON manifests.
///
/// Example usage:
/// ```swift
/// let scheduler = SchedulerService()
/// let manifest = try await UpgradeExecutor().load(manifestURL)
/// try await scheduler.start(upgrade: manifest)
/// ```
///
/// - Note: macOS 13.0+ only. iOS builds will no-op.
/// - SeeAlso: `UpgradeExecutor`, `DecisionOrchestrator`
@available(macOS 13.0, *)
actor SchedulerService {
    // ...
}
```

Apply to:
- SchedulerService
- DecisionOrchestrator
- TaskQueue
- AgentCoordinator
- PRAutomationService
- UpgradeExecutor
- PolicyEnforcer

---

### 3. User Guide

**File**: `docs/overnight-agents-guide.md`

**Sections**:
1. **Introduction**: What is overnight orchestration?
2. **Prerequisites**: macOS 26+, gh CLI, Foundation Models
3. **Installation**: Enable orchestration features
4. **Quick Start**: Run first overnight cycle
5. **Creating Upgrade Manifests**: JSON schema, examples
6. **Monitoring on iOS**: Bridge connection, real-time updates
7. **Troubleshooting**: Common issues and solutions
8. **Advanced**: Custom operations, scheduling strategies

---

### 4. Update Existing ADRs

**ADR-0002 (Agent Client Protocol)**:
- Add note about overnight orchestration using ACP for all agent communication

**ADR-0004 (iOS ↔ Desktop Bridge)**:
- Add new JSON-RPC methods: `orchestration/*`

**ADR-0006 (Foundation Models)**:
- Add note about DecisionOrchestrator using FM for task selection

---

## Documentation Standards

Follow `docs/adr/AGENTS.md` guidelines:
- Direct, honest tone (no marketing fluff)
- Specific examples from OpenAgents codebase
- Document trade-offs (both positive and negative consequences)
- Include code snippets where helpful
- Reference related ADRs

---

## Testing

1. Spell check all documents
2. Verify all code examples compile
3. Verify all links work
4. Peer review for clarity and completeness

---

## Acceptance Criteria

- [ ] ADR-0007 written and merged
- [ ] All public APIs have Swift doc comments
- [ ] User guide complete with examples
- [ ] Existing ADRs updated with references
- [ ] All docs pass spell check and link check
- [ ] Peer review approved

---

## References

- docs/adr/AGENTS.md (guidelines for AI agents writing ADRs)
- docs/adr/new.sh (script to create new ADR)
