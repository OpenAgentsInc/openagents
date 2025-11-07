# Tool Calling Implementation Audit — November 6, 2025

## What's in this directory

**tool-calling-audit.md**
- High-level analysis of current tool calling implementation
- Identifies what works and what's broken
- Recommends priority improvements
- Suitable for architects and product leads

**implementation-guide.md**
- Detailed, task-by-task implementation plan
- Code snippets and file locations
- Acceptance criteria for each task
- Suitable for coding agents and developers

## TL;DR

We defined FM tools with proper `@Generable` arguments and `@Guide` constraints, and we register them on the session. But we don't let the model call them. Instead, we ask the model to generate a TEXT plan, then manually parse it with regex to extract operations. This defeats the entire purpose of tool calling.

**The fix**: Let the model invoke tools directly via Foundation Models' native tool calling loop. Remove the 400+ lines of manual parsing code. Add persistent sessions with transcript management, streaming, and token budgeting.

**Impact**: More robust orchestration, multi-turn reasoning, and elimination of brittle text parsing.

## Quick Start

### For Reviewers

1. Read `tool-calling-audit.md` for the big picture
2. Review recommended improvements and priorities
3. Assess implementation roadmap and risks

### For Implementers

1. Read `tool-calling-audit.md` to understand the problems
2. Follow `implementation-guide.md` task by task
3. Run tests after each task
4. Use feature flag for gradual rollout

### For Swift Agents

You can provide these instructions to a Swift agent system:

> Read all files in docs/audit/20251106/. Implement the improvements outlined in implementation-guide.md, starting with Task 1 (persistent session). After each task, run tests and verify acceptance criteria. Use the feature flag approach to avoid breaking existing functionality.

## Timeline

**Current state**: Working but brittle. Text-plan parsing is fragile, no multi-turn reasoning, no session persistence.

**After Phase 1** (native tool calling): Robust tool invocation, no manual parsing, multi-turn reasoning enabled.

**After Phase 2** (persistent session): Context carried across runs, prewarming for better latency.

**After Phase 3** (streaming): Progressive updates to iOS during generation.

**After Phase 4** (token management): Safe operation on long sessions, automatic transcript pruning.

**After Phase 5** (structured outputs): Type-safe tool results, better model reasoning.

**Estimated total time**: 6-7 days of focused implementation.

## Key Files

**Current implementation**:
- `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ExploreOrchestrator.swift` (1030 lines)
- `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/FMTools.swift` (172 lines)
- `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ToolExecutor.swift` (169 lines)
- `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/SessionTools.swift` (447 lines)

**Documentation**:
- `docs/foundation-models/tool-calling.md` - Apple FM tool calling guide
- `docs/foundation-models/stateful-sessions.md` - Session management guide
- `docs/adr/0006-foundation-models.md` - ADR for FM adoption

**To be added**:
- `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ExploreOrchestratorTests.swift` (new)

## Success Criteria

✅ **Before audit**:
- [x] FM tools defined with @Generable + @Guide
- [x] Tools registered on session
- [x] Tools execute via ToolExecutor
- [x] ACP updates streamed to iOS

❌ **Current problems**:
- [ ] Model generates TEXT plans, not tool calls
- [ ] 400+ lines of brittle regex parsing
- [ ] No session persistence
- [ ] No transcript management
- [ ] No streaming
- [ ] No token budgeting

✅ **After implementation**:
- [ ] Native FM tool calling loop
- [ ] Zero manual plan parsing
- [ ] Persistent session with transcript
- [ ] Multi-turn reasoning
- [ ] Streaming partial updates
- [ ] Token budget enforcement
- [ ] Structured tool outputs
- [ ] All tests passing

## Questions?

- **Why is this important?** Text-plan parsing is brittle and defeats the purpose of tool calling. We built the foundation but aren't using it correctly.
- **Is it risky?** No. Native tool calling is Apple's recommended approach. We're moving from a fragile custom solution to the standard pattern.
- **Can we roll back?** Yes. Feature flag enables gradual rollout with fallback to current behavior.
- **Do we break anything?** No, if we use the feature flag approach. Old path remains available during transition.

## References

- [Agent Client Protocol (ACP)](https://agentclientprotocol.com/)
- [ADR-0002: Agent Client Protocol](../../adr/0002-agent-client-protocol.md)
- [ADR-0006: Foundation Models](../../adr/0006-foundation-models.md)
- [Apple Foundation Models Documentation](https://developer.apple.com/documentation/foundationmodels)

---

**Audit conducted by**: AI Agent (Claude Code)
**Date**: 2025-11-06
**Status**: Complete, awaiting implementation
