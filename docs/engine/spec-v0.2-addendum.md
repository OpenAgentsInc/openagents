# Spec v0.2 Addendum — Questions & Considerations

Date: 2025-11-06

This addendum highlights open questions, risks, and concrete implementation considerations for the v0.2 agent-only spec.

**Typed Streaming Semantics**
- Snapshot Ordering: Clarify whether `ExplorePlan.PartiallyGenerated` snapshots are monotonic (append-only) and how clients de-dupe `nextOps` across snapshots.
- Snapshot IDs: Include a monotonically increasing `snapshotId` and `planId` to correlate events and support partial replay.
- Result Typing: Prefer explicit `type` discriminators over `mime` in `ResultAny`, and include a schema version to guard decode.

**Backpressure & Concurrency**
- Work Queue Policy: Define max in-flight ops and per-op concurrency (e.g., grep parallelism vs embeddings). Expose `orchestrate.explore.setConcurrency`.
- Preemption: Specify whether newer snapshots can cancel queued ops; define fairness when long ops are running.

**Search Payload Bounds**
- `includeText` Limits: Cap bytes and lines for inlined text; return truncation markers. Recommend default `contextLines=2`.
- Encoding & EOL: Normalize to UTF-8 with `\n` line endings; document behavior for mixed encodings.

**Indexing & Storage**
- WAL Mode: Enable SQLite WAL; document vacuum/compaction and migration strategy for `index.sqlite`.
- Multi-Workspace: Add `workspaceId` and index version tagging; ensure isolation between workspaces and clear upgrade path.
- Symbols: Confirm minimal symbol table fields for cross-language parity and when to index them (M4 gate).

**Git & Workspace**
- Submodules/LFS: Decide default behavior (ignore vs fetch); ensure exclusions in indexing and disk caps.
- Trust Model: Validate remote origin; restrict non-ASCII paths if needed; treat `.git` as non-readable for content APIs.
- Dirty Trees: Define policy for uncommitted changes; prefer indexing working tree; expose `gitStatus` deltas in summary.

**Security & Policy**
- External LLMs: Require explicit provider allowlist and model IDs; redact secrets in prompts; log only hashes/metadata.
- FS Allowlist: Enforce path normalization and symlink resolution to prevent traversal beyond roots.
- Session Keys: Align with bridge pairing (nonce signing); document expiration/refresh and per-session rate limits.

**Reliability & Replay**
- Event IDs: Add `eventId` and `prevEventId` for ordering; include `sessionId` on every `AgentEvt`.
- Durable Traces: Consider rotating logs and compression; add `replay(traceId, untilEventId)`.
- Crash Recovery: On restart, resume orphaned sessions or mark as aborted with a terminal event.

**Foundation Models**
- Availability: Specify fallback cadence when `modelNotReady`; avoid blocking orchestration by queueing non-FM ops first.
- Structured Output: Validate partial objects against a schema; define strict coercion rules and reject ambiguous fields.

**API Versioning**
- Envelope Version: Add a `protocolVersion` field to `AgentCmd/AgentEvt`; bump with breaking changes and fail fast at connect.
- Feature Flags: Advertise supported ops in handshake; orchestrator adapts plan to available tools.

**Performance Targets**
- Streaming Budget: Ensure first tokens for plan snapshots <150ms p50; partial search results start <300ms p95.
- Memory Caps: Document indexing memory high-water marks and backpressure behavior on low-memory systems.

**Testability**
- Golden Traces: Ship canonical traces for the default workspace; assert deterministic replay and summaries.
- Fault Injection: Provide knobs to force ANN disabled, embedder delays, and FS errors for resilience tests.

If any of the above need to be ratified as normative, I can propose a narrow PR to spec-v0.2 with concrete fields and limits (IDs, sizes, versioning, and error codes).

