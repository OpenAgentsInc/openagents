# GPTOSS 20B Integration Documentation

This directory contains comprehensive documentation for integrating GPT-OSS 20B (via MLX Swift) as a native agent provider in OpenAgents.

## Quick Start

**New to this integration?** Start here:
1. Read [gptoss-integration-spec.md](./gptoss-integration-spec.md) for the full technical specification
2. Review [next-steps-20251110.md](./next-steps-20251110.md) for gaps and revisions
3. Check [research.md](./research.md) for MLX Swift implementation details
4. Browse [issues/](./issues/) for implementation tasks

## Document Index

### Specifications

- **[gptoss-integration-spec.md](./gptoss-integration-spec.md)** - Complete technical specification
  - Architecture overview
  - Task allocation strategy (FM vs GPTOSS)
  - Integration patterns
  - Implementation phases
  - Testing strategy
  - Success metrics
  - **Start here for comprehensive understanding**

- **[next-steps-20251110.md](./next-steps-20251110.md)** - Gaps, revisions, and execution order
  - Identifies gaps to close (Harmony compliance, download UX, memory guardrails)
  - Proposes revisions to spec and plan
  - Concrete 2-3 week execution order
  - Interface sketches
  - **Read this for implementation-ready guidance**

- **[research.md](./research.md)** - Original MLX Swift research
  - Model selection rationale (GPTOSS 20B MXFP4-Q8)
  - MLX Swift integration patterns
  - Code examples for loading and inference
  - Download and caching details
  - **Reference for MLX-specific implementation**

- **[claudeplan.md](./claudeplan.md)** - Previous integration planning (historical)
  - Earlier Claude-based planning discussions
  - May contain useful context and alternatives considered

### Implementation Issues

All implementation tasks are in [issues/](./issues/), organized by phase:

#### Phase 1: Foundation (P0, Blocking)
- **[001-mlx-llm-dependencies.md](./issues/001-mlx-llm-dependencies.md)** - Add MLXLLM to Package.swift
- **[002-gptoss-agent-provider.md](./issues/002-gptoss-agent-provider.md)** - Core provider implementation

#### Phase 2: Integration (P0, Blocking)
- **[004-acp-streaming-integration.md](./issues/004-acp-streaming-integration.md)** - Token streaming to UI
- **[005-agent-registry-registration.md](./issues/005-agent-registry-registration.md)** - Register in agent system

#### Phase 3: Routing (P1)
- **[006-fm-delegation-tool.md](./issues/006-fm-delegation-tool.md)** - FM gptoss.generate tool
- **[007-task-routing-logic.md](./issues/007-task-routing-logic.md)** - Intelligent task routing

#### Phase 4: UI & Polish (P1-P2)
- **[003-model-download-ui.md](./issues/003-model-download-ui.md)** - Download progress UI
- **[008-settings-ui.md](./issues/008-settings-ui.md)** - Settings screen
- **[009-memory-management.md](./issues/009-memory-management.md)** - Memory lifecycle

#### Phase 5-6: Advanced & Documentation (P1)
- **[010-testing-documentation.md](./issues/010-testing-documentation.md)** - Testing, validation, docs

## Key Concepts

### Task Allocation

**Foundation Models** (lightweight, <140 tokens):
- Conversation titles and summaries
- Meta questions ("who are you?")
- Routing decisions
- Quick explanations

**GPTOSS 20B** (heavyweight, unlimited tokens):
- Code generation
- Documentation
- Complex reasoning
- Long-form content
- Multi-step planning

**External Agents** (Codex, Claude Code):
- User explicitly requests them
- GPTOSS unavailable (fallback)
- Specialized features

### Architecture

```
┌──────────────────┐
│ Foundation Models│  (Orchestrator/Router)
│ OpenAgents       │
└────────┬─────────┘
         │ Uses gptoss.generate tool
         ↓
┌──────────────────┐
│ GPTOSS 20B       │  (Heavy Execution)
│ AgentProvider    │
└──────────────────┘
         │
         ↓ Streams tokens
┌──────────────────┐
│ ACP Updates      │
│ → UI             │
└──────────────────┘
```

### Key Files (When Implemented)

```
ios/OpenAgentsCore/Sources/OpenAgentsCore/
├── GPTOSS/
│   ├── GPTOSSTypes.swift              # Errors, config, state
│   ├── GPTOSSModelManager.swift       # Load/unload, generation
│   ├── GPTOSSMemoryManager.swift      # Memory monitoring
│   ├── GPTOSSAgentProvider.swift      # AgentProvider impl
│   ├── GPTOSSManifest.swift           # Download verification
│   └── GPTOSSMetrics.swift            # Performance telemetry
│
└── Agents/
    ├── ACPSessionModeId.swift         # + .gptoss_20b case
    └── OpenAgentsLocalProvider.swift  # + FMTool_GPTOSSGenerate
```

## System Requirements

### Minimum:
- macOS 13.0+ (Ventura)
- Apple Silicon (M1+)
- 16 GB unified memory
- 25 GB free disk space

### Recommended:
- M2 Pro/Max, M3, or M4
- 24 GB+ memory
- 50 GB+ disk space

### Not Supported:
- iOS/iPadOS (model too large)
- Intel Macs (MLX optimized for Apple Silicon)

## Implementation Timeline

Estimated: **2-3 weeks** for MVP (macOS only)

- Week 1: Phase 1-2 (Dependencies + Core + Integration) → Working GPTOSS agent
- Week 2: Phase 3 (Routing) + Phase 4 start → Automatic delegation
- Week 3: Phase 4 finish (UI + Memory) + Phase 5 (Testing/Docs) → Production ready

## Success Criteria

### Performance:
- First token: <2s p50, <5s p95 (M2+)
- Throughput: >15 tok/sec p50
- Memory: 14-17 GB loaded, <20 GB peak

### Quality:
- Code validity: >95%
- Routing accuracy: >90%
- Download success: >99%

### Adoption:
- >50% of macOS users try GPTOSS within 3 months
- NPS >7/10

## Important Gaps (from next-steps-20251110.md)

Must address:
1. **Harmony Compliance**: Always use tokenizer's chat template, never bypass
2. **Download/Verification**: Persist manifest with checksums, visible resume
3. **Memory Guardrails**: Preflight checks, auto-unload, memory watermark
4. **Cancellation**: Clean propagation to MLX session, no dangling tasks
5. **Routing Rubric**: Concrete heuristics, log routing decisions
6. **Safety/Policy**: License acknowledgement, macOS-only enforcement
7. **Telemetry**: Anonymous metrics in dev builds

## Related Work

- **Issue #1469**: FM codex.run tool wiring (pattern to follow)
- **Issue #1468**: Embeddings audit (MLX integration patterns)
- **Issue #1467**: MLX embeddings implementation (proven pattern)
- **ADR-0006**: Foundation Models integration
- **Embeddings system**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Embeddings/MLXEmbeddingProvider.swift`

## Getting Started (For Developers)

1. **Read the spec**: [gptoss-integration-spec.md](./gptoss-integration-spec.md)
2. **Review gaps**: [next-steps-20251110.md](./next-steps-20251110.md)
3. **Start with Issue #1**: [001-mlx-llm-dependencies.md](./issues/001-mlx-llm-dependencies.md)
4. **Follow phase order**: Complete Phase 1 before Phase 2, etc.
5. **Test incrementally**: Each issue has acceptance criteria and tests

## Questions?

- Check [troubleshooting.md](./troubleshooting.md) (when created)
- Review similar patterns in `MLXEmbeddingProvider.swift`
- Consult MLX Swift Examples: https://github.com/ml-explore/mlx-swift-examples
- Reference GPTOSS model card: https://huggingface.co/mlx-community/gpt-oss-20b-MXFP4-Q8

## License

GPTOSS 20B is licensed under Apache 2.0. Integration code follows OpenAgents license.

---

**Last Updated:** 2025-11-10
**Status:** Specification Complete, Ready for Implementation
**Next Step:** Begin Issue #1 (Add MLX dependencies)
