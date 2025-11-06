# Engine & Orchestrator Test Coverage Plan (v0.2)

Date: 2025-11-06

This plan enumerates tests to write for full coverage of the v0.2 agent-only spec (search primitives, orchestrator, RPC, policy, and performance). File paths reflect current repo layout and should live under the existing XCTest targets.

**Search Primitives (Unit)**
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Search/FTSQueryTests.swift — BM25 queries, highlight offsets, tokenizer config.
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Search/ChunkingHeuristicsTests.swift — SwiftSyntax block-aware chunking; windowed fallback; dedupe by fingerprint.
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Search/EmbeddingProviderTests.swift — Core ML embedder outputs shape, normalization, batching.
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Search/SemanticBruteForceTests.swift — vDSP dot-product top‑K, ties, stability.
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Search/FusionRRFAndMMRTests.swift — RRF math, boosts (path/symbol/recency), optional MMR.
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Search/ContentGetSpanTests.swift — bounds, context expansion, truncation, encoding/EOL handling.

**Indexing & Storage (Unit)**
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Index/SQLiteSchemaMigrationTests.swift — create/migrate (WAL on), views, constraints.
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Index/FileWatcherAndIncrementalIndexTests.swift — FSEvents integration (mock), idempotent reindex.
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Index/VectorStoreLayoutTests.swift — dim verification, blob packing, corruption recovery.

**RPC Surface (Unit + Integration)**
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/RPC/JSONRPCApiComplianceTests.swift — request/response shapes for `search.*`, `content.getSpan`, `index.*`.
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/RPC/StreamingPartialsTests.swift — `search.partial` batching, order, completion flags.
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/RPC/VersionNegotiationTests.swift — Bonjour TXT version mismatch handling, graceful errors.
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/RPC/ErrorMappingTests.swift — 4xx/5xx mapping to structured errors with retryable flag.

**Orchestrator & Typed Streaming (Integration)**
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestrator/ExploreOrchestratorStreamingTests.swift — `orchestrate.explore.start` end‑to‑end; plan snapshots; event stream.
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestrator/PlanSnapshotMonotonicityTests.swift — snapshot ordering, de‑dupe `nextOps`.
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestrator/CmdExecutionQueueTests.swift — backpressure, max concurrency, fairness, cancellation.
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestrator/AbortAndRecoveryTests.swift — abort RPC, cleanup, terminal events.
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestrator/ResultTypingAndDecodeTests.swift — `ResultAny` type tags, schema versioning.

**Workspace & Git (Integration)**
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Workspace/PathAllowlistEnforcementTests.swift — symlink traversal, normalization, deny outside roots.
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Workspace/GitCloneCheckoutTests.swift — clone to workspace, branch checkout, state.json updates (use local bare repo fixture).
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Workspace/GitStatusAndDirtyTreeTests.swift — working tree vs HEAD; exclusions respected.
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Workspace/SubmodulesAndLFSTests.swift — excluded from indexing by default; disk caps applied.

**Security & Policy (Integration)**
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Security/ExternalLLMPolicyTests.swift — `LLMCall` gated by policy; provider/model allowlist.
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Security/NonceAndRateLimitTests.swift — nonce signing, single in‑flight FM/LLM call, rate limits.
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Security/PrivacyAndRedactionTests.swift — redact secrets in logs, trace payload limits.

**Foundation Models (Unit + Integration)**
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Foundation/PlanSnapshotStreamingTests.swift — `@Generable` partials, field coercion, fallbacks when unavailable.
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Foundation/AvailabilityFallbackTests.swift — `.modelNotReady` behavior; deterministic local fallback path.

**Telemetry & Replay (Integration)**
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Telemetry/TraceEventLoggingTests.swift — log all AgentCmd/Evt, rotation.
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Telemetry/ReplayDeterminismTests.swift — `replay(traceId)` reproducibility on golden corpus.

**Performance (Bench-like Tests)**
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Performance/SearchLatencyTargetsTests.swift — p95 targets for lexical/semantic/hybrid on 100k‑chunk fixture.
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Performance/IndexThroughputTargetsTests.swift — ≥5 MB/s; memory watermark assertions.

**Default Workspace E2E**
- ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/E2E/ExploreDefaultWorkspaceTests.swift — run explore against `/Users/christopherdavid/code/openagents`; assert `ExploreSummary` fields and path scoping.

Notes
- Where direct network/git access is undesirable in CI, use local fixtures (bare repos, file trees) and conditional skips.
- Keep payloads bounded; use temp dirs under `/tmp` or process sandbox when possible.
- Align names and folders to existing XCTest conventions in this repo; adjust paths if SearchKit is split into dedicated SwiftPM packages later.

