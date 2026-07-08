# Session Memory System Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #17 from the Bun/Effect terminal-agent systems list. It defines
how the terminal agent should store, discover, retrieve, update, summarize, and
redact memory across conversations, projects, teams, and active sessions.

## Target

Build a memory system that helps future work without turning the current repo,
transcript, or private logs into unbounded model context.

Memory should be typed, scoped, auditable, editable, and retrievable. It should
not be a pile of always-injected text.

## User-Visible Capability

The user should be able to:

- Ask the agent to remember something.
- Ask the agent to forget something.
- Review and edit memory.
- Keep personal preferences separate from project or team facts.
- Know when memory was used.
- Avoid stale or duplicate memories.
- Keep secrets out of memory.
- Continue long sessions through session-memory summaries.

Memory should improve continuity while remaining under user control.

## Memory Types

Use a closed taxonomy:

- `user`: stable information about the user's role, preferences, and context.
- `feedback`: guidance about how the agent should work or communicate.
- `project`: non-derivable project context, goals, incidents, constraints, or
  decisions.
- `reference`: pointers to external systems or sources of truth.

Do not save information that is trivially derivable from the current codebase,
git history, package metadata, or files the agent can inspect on demand.

## Scope Model

Memory scope should be explicit:

- Session memory.
- User-global memory.
- Project memory.
- Local private project memory.
- Team or organization memory.
- Managed policy memory.
- External reference memory.

Each memory record should carry visibility, owner, source, freshness, and
redaction metadata.

## Core Design

Define a `MemoryService` that owns memory records and retrieval.

Suggested service boundary:

```ts
interface MemoryService {
  save(request: MemorySaveRequest): Effect.Effect<MemoryRecord, MemoryError>
  forget(request: MemoryForgetRequest): Effect.Effect<MemoryChangeReceipt, MemoryError>
  list(request: MemoryListRequest): Effect.Effect<ReadonlyArray<MemoryHeader>, MemoryError>
  retrieve(request: MemoryRetrieveRequest): Effect.Effect<MemoryRetrievalResult, MemoryError>
  summarizeSession(request: SessionMemoryRequest): Effect.Effect<SessionMemorySummary, MemoryError>
}
```

The context assembler should request memory through this service instead of
walking storage directly.

## Record Shape

A memory record should include:

- Memory id.
- Type.
- Scope.
- Title.
- Description.
- Body.
- Created and updated timestamps.
- Source run or user action ref.
- Owner or team ref.
- Freshness and expiry hints.
- Visibility.
- Redaction class.
- Related workspace or artifact refs.
- Supersedes or duplicate refs.

File-backed storage is acceptable, but the runtime should still parse it into
typed records.

## Discovery And Retrieval

Retrieval should be deliberate:

- Load compact indexes and headers eagerly.
- Read full memory bodies only when relevant.
- Prefer semantic or structured selection over keyword-only routing.
- Include freshness notes for stale records.
- Cap the number and size of loaded memories.
- Return exclusion reasons when memory was skipped.
- Avoid loading binary or oversized files.

Indexes should be concise. Detailed memory belongs in individual records.

## Session Memory

Long sessions need a separate session-memory path:

- Extract durable summaries during the session.
- Track the last transcript point summarized.
- Preserve unsummarized recent messages.
- Use session memory as compaction input when available.
- Keep tool-use/result invariants when retaining recent messages.
- Truncate oversized session-memory sections with a visible marker.

Session memory is continuity state, not a substitute for permanent user or
project memory.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for memory operations.
- `Schema` for memory records, headers, retrieval results, and change receipts.
- `Layer` for file-backed, database-backed, and fixture memory stores.
- `Stream` for scanning large memory sets.
- `Queue` for async extraction or sync events.
- `Schedule` for stale-memory refresh and extraction retries.
- `Scope` for memory extraction jobs.

Memory retrieval should be interruptible and should fail soft when optional
memory sources are unavailable.

## Safety Rules

- Never save secrets, credentials, private keys, or raw tokens to memory.
- Do not save negative personal judgments about the user.
- Convert relative dates into absolute dates before saving.
- Do not save duplicate memories when an existing memory can be updated.
- Do not let project memory override managed policy.
- Do not expose private memory in public receipts.
- Do not auto-include external files without explicit policy.
- Do not let included memory files recurse indefinitely.
- Do not treat stale memory as authoritative without a freshness marker.

## Tests

Minimum regression coverage:

- Save each memory type with valid metadata.
- Reject or redact secret-bearing memory.
- Forget a memory and produce a change receipt.
- Scan memory headers without loading full bodies.
- Retrieve relevant memory with size caps.
- Skip irrelevant memory with an exclusion reason.
- Detect duplicates and update instead of adding another copy.
- Preserve session-memory summary boundary during compaction.
- Keep unsummarized recent messages after session-memory compaction.
- Redact private memory from public projections.

## OpenAgents Translation Notes

When promoted, map memory records to OpenAgents memory contracts, policy refs,
capability refs, projection visibility, and artifact receipts. Verify live
issue state before claiming memory behavior is implemented.

## Decision

Memory should be a typed, scoped, retrievable knowledge system. The runtime
should load compact indexes, retrieve relevant records, preserve session
continuity through summaries, and keep private memory out of public outputs.
