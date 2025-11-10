 GPTOSS 20B Integration Plan

     Overview

     Create comprehensive specification and implementation roadmap for integrating GPT-OSS 20B (via MLX Swift) as a new native agent provider in
     OpenAgents, with intelligent task routing between Foundation Models (lightweight) and GPTOSS (heavy reasoning).

     Files to Create

     1. Main Specification (docs/gptoss/gptoss-integration-spec.md)

     Purpose: Comprehensive technical specification for GPTOSS integration

     Contents:
     - Executive Summary: Why GPTOSS, what it replaces/complements
     - Architecture Overview: GPTOSSAgentProvider as native agent, relationship to existing providers
     - Task Allocation Matrix: What stays in Foundation Models vs moves to GPTOSS
       - FM: Titles, summaries, routing decisions, meta questions (2-3 sentences, <140 tokens)
       - GPTOSS: Code generation, multi-step reasoning, documentation, complex analysis
     - Model Specifications:
       - Model: mlx-community/gpt-oss-20b-MXFP4-Q8
       - Size: ~12.1 GB, Active params: 3.6B (MoE)
       - Requirements: macOS only, 16 GB RAM minimum, 24 GB recommended
     - Integration Pattern: Follow MLXEmbeddingProvider pattern
       - Actor-based for thread safety
       - Hub.snapshot for resumable downloads
       - ChatSession for streaming
       - ACP integration for UI updates
     - Delegation Flow: FM orchestrator → GPTOSS execution (like existing codex.run tool)
     - Package Dependencies: MLXLLM, MLXLMCommon, Tokenizers from mlx-swift-examples
     - Implementation Phases: 6 phases (Foundation → Integration → Routing → UI → Advanced)
     - Testing Strategy: Unit, integration, memory profiling, golden tests
     - Success Metrics: Latency, quality, memory usage, user satisfaction
     - Risk Mitigation: Memory pressure, download failures, performance, safety

     2. GitHub Issues (docs/gptoss/issues/)

     Will create 10 detailed implementation issues:

     Issue 1: 001-mlx-llm-dependencies.md

     Add MLXLLM Dependencies to Package.swift
     - Add mlx-swift-examples products (MLXLLM, MLXLMCommon)
     - Update target dependencies
     - Verify build on macOS
     - Phase: 1 (Foundation)
     - Priority: P0 (blocking)

     Issue 2: 002-gptoss-agent-provider.md

     Implement GPTOSSAgentProvider Core
     - Create GPTOSSAgentProvider.swift following AgentProvider protocol
     - Implement model loading via Hub.snapshot with progress tracking
     - Basic text generation using ChatSession
     - Memory requirement checks (16 GB minimum)
     - Phase: 1 (Foundation)
     - Priority: P0

     Issue 3: 003-model-download-ui.md

     Implement Model Download UI
     - Download progress indicator in Settings
     - Disk space check (25 GB free recommended)
     - Resume support for interrupted downloads
     - Model verification (SHA256 checksums)
     - Phase: 4 (UI & Polish)
     - Priority: P1

     Issue 4: 004-acp-streaming-integration.md

     Implement GPTOSS Streaming to ACP
     - Token-by-token streaming via SessionUpdateHub
     - Convert ChatSession AsyncSequence to ACP agentMessageChunk
     - Handle cancellation (cancel button in UI)
     - Phase: 2 (Integration)
     - Priority: P0

     Issue 5: 005-agent-registry-registration.md

     Register GPTOSS in Agent Registry
     - Add ACPSessionModeId.gptoss_20b enum case
     - Register GPTOSSAgentProvider in DesktopWebSocketServer
     - Add session/set_mode support for "gptoss_20b"
     - Implement isAvailable() check (macOS, memory)
     - Phase: 2 (Integration)
     - Priority: P0

     Issue 6: 006-fm-delegation-tool.md

     Add gptoss.generate Tool to Foundation Models
     - Create FMTool_GPTOSSGenerate following FMTool_CodexRun pattern
     - Implement FM → GPTOSS delegation via local RPC
     - Update OpenAgentsLocalProvider decision rubric
     - Phase: 3 (Routing)
     - Priority: P1

     Issue 7: 007-task-routing-logic.md

     Implement Intelligent Task Routing
     - Define complexity heuristics (lightweight vs heavy)
     - Update FM instructions for delegation decisions
     - Add routing metrics/logging
     - Implement fallback chain (GPTOSS → Codex → Claude Code)
     - Phase: 3 (Routing)
     - Priority: P1

     Issue 8: 008-settings-ui.md

     Add GPTOSS Settings UI (macOS)
     - Model selection dropdown (GPTOSS 20B, disable if <16 GB)
     - Temperature/top-p controls
     - Memory usage indicator
     - Model status (not loaded / loading / ready / error)
     - Unload model button (free memory)
     - Phase: 4 (UI & Polish)
     - Priority: P2

     Issue 9: 009-memory-management.md

     Memory Management and Performance
     - Implement auto-unload on idle (configurable timeout)
     - Memory pressure monitoring (os_signpost)
     - Warn user if approaching memory limits
     - Profile generation latency (target <2s first token)
     - Batch processing optimizations
     - Phase: 4 (UI & Polish)
     - Priority: P1

     Issue 10: 010-testing-documentation.md

     Testing, Validation, and Documentation
     - Unit tests: Model loading, streaming, ACP conversion
     - Integration tests: FM → GPTOSS delegation flow
     - Memory profiling tests (16 GB, 32 GB, 64 GB Macs)
     - Golden tests: Sample tasks with expected outputs
     - User documentation: When to use GPTOSS vs other agents
     - ADR-0010: GPTOSS Integration Architecture
     - Phase: 5 (Advanced) & 6 (Documentation)
     - Priority: P1

     Key Decisions Documented

     Task Allocation (Foundation Models vs GPTOSS)

     Foundation Models (Stay):
     - Conversation titles (3-5 words)
     - Session summaries (1-2 sentences)
     - Meta questions ("who are you?", "what can you do?")
     - Routing decisions (which agent to invoke)
     - Classification/tagging
     - Temperature: 0.1-0.15, Tokens: <140

     GPTOSS (New):
     - Code generation (functions, classes, modules)
     - Complex refactoring (multi-file)
     - Architectural planning (system design)
     - Documentation generation (README, API docs)
     - Repository analysis (codebase understanding)
     - Long-form explanations
     - Temperature: 0.7 (configurable), Tokens: unlimited

     External Agents (Existing):
     - Codex: Specialized coding agent (fallback for GPTOSS)
     - Claude Code: Advanced reasoning (fallback)

     Integration Pattern

     Following MLXEmbeddingProvider pattern (proven, working):
     - Actor for thread safety
     - Hub.snapshot for downloads with progress/resume
     - Availability checking (platform, memory, model state)
     - Graceful degradation (fallback to other agents)
     - macOS-only compilation (#if os(macOS))

     Following OpenAgentsLocalProvider delegation pattern:
     - Foundation Models as orchestrator/router
     - Tool-based invocation (gptoss.generate)
     - Local RPC for in-process calls (localSessionSetMode, localSessionPrompt)
     - ACP streaming for UI updates

     Why This Architecture

     1. Separation of concerns: FM for routing, GPTOSS for execution
     2. Proven patterns: Reuse successful embeddings + delegation infrastructure
     3. Flexibility: Users can invoke GPTOSS directly OR let FM decide
     4. Performance: Native Swift actor (no IPC overhead like CLI agents)
     5. Memory efficiency: Load/unload model as needed
     6. Future-proof: Easy to swap models (20B → 40B → custom)

     Implementation Timeline

     - Phase 1-2 (Weeks 1-3): Core provider + registration → Working GPTOSS agent
     - Phase 3 (Week 3-4): Routing logic → Automatic FM → GPTOSS delegation
     - Phase 4 (Week 4-5): UI + memory mgmt → Polished user experience
     - Phase 5-6 (Week 5-6+): Advanced features + docs → Production ready

     Open Issue Integration

     - #1469 (FM codex.run): Pattern to follow for gptoss.generate tool
     - #1468 (Embeddings audit): Ongoing MLX work, same dependencies
     - #1467 (Embeddings impl): Direct precedent for MLX + Hub integration

     Next Steps

     After approval, I will:
     1. Create docs/gptoss/gptoss-integration-spec.md with full technical specification
     2. Create 10 issue files in docs/gptoss/issues/ with detailed acceptance criteria
     3. Include code snippets, architecture diagrams (text-based), and cross-references
     4. Ensure consistency with existing ADRs and codebase conventions

     All files will be in Markdown format, ready for review and conversion to actual GitHub issues.
