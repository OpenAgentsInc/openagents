# Agent Search Primitives & Orchestrator — Swift‑Only (macOS + iOS) — v0.2

**Status:** Draft (proposed)

**Owner:** OpenAgents — Search Working Group

**Scope:** Headless primitives and an orchestration flow that let AI agents explore, index, and retrieve from local code/doc repositories. macOS runs the engine and orchestrator; clients (agents, desktop components, optional iOS companion) invoke it via typed RPC and **streaming structured objects**. No end‑user search UI is required.

> **Update — 2025‑11‑06**
> Refocused on **agent‑facing primitives** only. Added an **Explore Codebase** orchestration, **typed command/event stream**, Git access/clone semantics, and **Foundation Models snapshot streaming** for structured plans. Introduced `orchestrate.*` RPCs and external LLM calls (e.g., Claude Code / Codex) behind a single tool facade.

---

## 1. Goals

1. Provide **agent‑usable primitives** for filesystem, indexing, semantic/lexical search, and span reads.
2. Support a one‑click **Explore Codebase** flow that builds initial understanding of a repo (readme, structure, languages, entrypoints) using the engine + on‑device model.
3. **Offline‑first** on macOS; optional external LLMs allowed via explicit policy.
4. **Typed streaming** of plans, commands, and results (Swift types; snapshots during generation).
5. **Deterministic, testable** behaviors and replayable event logs.
6. Swift‑only implementation across engine, service, orchestrator, and tests.

**Non‑Goals (v0.2)**

* Cloud indexing, cross‑user sharing, or multi‑tenant search.
* Full semantic code understanding (call graphs, refactors). We do light symbols only.
* On‑device iOS indexing. iOS remains an optional controller/viewer.

---

## 2. Architecture Overview

**Packages / Targets**

* **SearchKitCore (SwiftPM)** — Chunking, FTS5, embeddings, ANN (brute‑force v0), fusion (RRF), span reading, symbol stub, telemetry.
* **SearchKitService (macOS app/agent)** — SwiftNIO WebSocket/HTTP server; hosts **Orchestrator** actor; manages indexes and repo workspaces.
* **SearchKitClient (SwiftPM)** — Typed client SDK (JSON‑RPC over WebSocket). Used by desktop app, iOS app, and agent runtimes.

**Process Model**

* macOS app runs the service (foreground or LaunchAgent).
* Clients discover via Bonjour (`_oasearch._tcp`) or connect to host:port.
* All heavy work (indexing, grep, embeddings) runs on macOS. Clients stream typed commands and consume typed events.

**Data Roots**

* Index root: `~/Library/Application Support/OpenAgents/SearchIndex/`
* Default workspace under test: **`/Users/christopherdavid/code/openagents`** (user‑selectable soon).

---

## 3. Workspace & Git Access

**Local Access**

* Request a security‑scoped bookmark for the chosen folder (if sandboxed). Persist and re‑acquire each launch.
* Enforce an **allowlist of root paths**. All file ops must remain within the workspace root.

**Remote GitHub Clone**

* If given `{ remoteURL, branch?, dest? }`, clone into `<index_root>/workspaces/<slug>`; verify checkout; record remote in `state.json`.
* Respect user credentials/keychain/SSH config; no credential prompts from agents.
* Ensure `.gitignore` and large binaries are excluded from indexing.

**Reindex Triggers**

* File watcher (FSEvents) + manual RPC (`index.rebuild`).
* Hash by SHA‑256 + `mtime_ns` to avoid redundant work.

---

## 4. Explore Codebase — Orchestration Flow

A **single button** in the desktop app (or programmatic call) starts exploration. The UI is minimal; orchestration and output are **typed streams** for agents.

**Entry RPC**

```
{ "id":"uuid", "method":"orchestrate.explore.start",
  "params": { "root":"/path/to/workspace",
               "remoteURL": "https://github.com/org/repo.git"?,
               "branch": "main"?,
               "policy": { "allowExternalLLMs": false, "network": false },
               "goals": ["map structure","summarize readmes","identify entrypoints"] } }
```

**High‑Level Steps**

1. **Preflight**: validate path/clone; ensure bookmarks; seed state file; start file watcher.
2. **Index bootstrap**: chunk & FTS; generate embeddings; persist `chunk`, `chunk_fts`, `chunk_vec`.
3. **Plan generation** (Foundation Models on‑device): stream an `ExplorePlan.PartiallyGenerated` describing next **AgentOps** (see §6). We may start executing as steps arrive (prefetch semantics).
4. **Act**: execute ops (grep/semantic/reads) and stream **Event** results; feed summaries back to the model as **facts** (tool output).
5. **Converge**: produce an `ExploreSummary` (files of interest, entrypoints, language histogram, TODOs for deeper passes).

**Termination**

* Normal: `orchestrate.explore.completed` + summary artifact.
* Abort: `orchestrate.explore.abort` cancels running ops and streams `orchestrate.explore.stopped`.

---

## 5. Storage Schema (unchanged from v0.1, abridged)

* `file(id, path, lang, size_bytes, mtime_ns, sha256, indexed_at)`
* `chunk(id, file_id, start_line, end_line, byte_range_start, byte_range_end, text, fingerprint, tokens, created_at)`
* `chunk_fts` (FTS5 over `text`)
* `chunk_vec(chunk_id, dim, vec BLOB, created_at)` — vectors **L2‑normalized**
* `symbol` (optional stub)
* `trace_event(session_id, ts, kind, payload JSON)`

All vectors stored as contiguous Float32 (dim=384/768) and normalized; cosine becomes dot.

---

## 6. Typed Commands & Events (Agent Stream)

All commands and events are **structured**, Codable, and may be **snapshot‑streamed** when generated by the on‑device model.

### 6.1 Envelope

```swift
public struct AgentEnvelope<Payload: Codable & Sendable>: Codable, Sendable {
  public var id: UUID
  public var parentId: UUID?
  public var ts: Date
  public var kind: String // e.g., "Cmd" | "Evt"
  public var payload: Payload
}
```

### 6.2 Commands (subset)

```swift
public enum AgentCmd: Codable, Sendable {
  case fsListDir(FSListDir)
  case contentGetSpan(ContentGetSpan)
  case indexAddRoot(IndexAddRoot)
  case indexBuild(IndexBuild)
  case searchGrep(SearchGrep)
  case searchSemantic(SearchSemantic)
  case searchHybrid(SearchHybrid)
  case gitClone(GitClone)
  case gitStatus(GitStatus)
  case llmCall(LLMCall) // on‑device or external, governed by policy
}

public struct FSListDir: Codable, Sendable { let path: String; let depth: Int }
public struct ContentGetSpan: Codable, Sendable { let path: String; let startLine: Int; let endLine: Int; let context: Int? }
public struct IndexAddRoot: Codable, Sendable { let path: String }
public struct IndexBuild: Codable, Sendable { let maxConcurrency: Int? }
public struct SearchGrep: Codable, Sendable { let pattern: String; let pathPrefix: String?; let isRegex: Bool; let flags: [String]?; let k: Int }
public struct SearchSemantic: Codable, Sendable { let query: String; let k: Int; let filters: SearchFilters? }
public struct SearchHybrid: Codable, Sendable { let query: String; let k: Int; let filters: SearchFilters?; let includeText: Bool; let contextLines: Int }
public struct GitClone: Codable, Sendable { let remoteURL: String; let branch: String?; let dest: String }
public struct GitStatus: Codable, Sendable { let path: String }

public struct LLMCall: Codable, Sendable {
  public enum Provider: String, Codable { case appleOnDevice, claudeCode, codex }
  public var provider: Provider
  public var instructions: String?
  public var prompt: String
  public var expectsType: String? // name of @Generable type expected
  public var streamStructured: Bool // request snapshot streaming of typed objects
  public var policy: LLMPolicy // temp/maxTokens/provider‑specific
}
```

### 6.3 Events (subset)

```swift
public enum AgentEvt: Codable, Sendable {
  case started(Started)
  case progress(Progress)
  case result(ResultAny)
  case planSnapshot(ExplorePlan.PartiallyGenerated) // see §7
  case completed(ExploreSummary)
  case error(AgentError)
}

public struct Started: Codable, Sendable { let op: String; let details: String? }
public struct Progress: Codable, Sendable { let op: String; let percent: Double }
public struct ResultAny: Codable, Sendable { let op: String; let data: Data; let mime: String }
public struct AgentError: Codable, Sendable { let op: String; let message: String; let retryable: Bool }
```

**Notes**

* `ResultAny` carries typed payloads (e.g., `SearchResult[]`) serialized with type tags.
* All file content results are **bounded**; use `contentGetSpan` for larger ranges.

---

## 7. Foundation Models Integration (On‑Device)

We **prefer structured snapshot streaming**. The on‑device session generates plan objects whose fields fill in over time; the orchestrator consumes snapshots and schedules `AgentCmd` accordingly.

```swift
@Generable
public struct ExplorePlan: Equatable {
  @Guide(description: "High‑level objectives")
  public var goals: [String]
  @Guide(description: "A queue of concrete operations the agent intends to run next")
  public var nextOps: [AgentOp]
}

@Generable
public enum AgentOp: Equatable {
  case indexRepo(path: String)
  case listDir(path: String, depth: Int)
  case grep(pattern: String, pathPrefix: String?)
  case semantic(query: String, k: Int)
  case readSpan(path: String, startLine: Int, endLine: Int, context: Int?)
  case summarize(paths: [String])
}

@Generable
public struct ExploreSummary: Equatable {
  public var repoName: String
  public var languages: [String: Int] // line counts
  public var entrypoints: [String]
  public var topFiles: [String]
  public var followups: [String]
}
```

**Execution Model**

* The orchestrator binds a session with instructions that describe tool affordances and **policy** (allowed ops, allowed paths, external LLMs allowed?).
* Snapshot stream → append `nextOps` to a work queue; de‑dupe; execute with backpressure; stream `AgentEvt.result` as each completes.
* Optional: provide **Tool** definitions for inline “tool calling”, but **typed plan streaming** remains primary.

**Feeding Facts**

* Summaries of results (grep hits, semantic top‑K, file spans) are injected back into the session as **tool output** segments to guide subsequent steps without flooding context.

---

## 8. Search Primitives (recap)

**Lexical (FTS5)** — BM25 over `chunk_fts` with highlight offsets.
**Semantic** — on‑device Core ML embedder; brute‑force dot product (vDSP) for ≤100k chunks.
**Hybrid** — Reciprocal Rank Fusion; optional MMR diversity (top‑K).
**Span Reads** — bounded range via `contentGetSpan` to avoid large payloads.

---

## 9. RPC Surface (JSON‑RPC over WebSocket)

**Search / Content**

```
search.lexical   { query, k, filters, includeText, contextLines }
search.semantic  { query, k, filters, includeText, contextLines }
search.hybrid    { query, k, filters, includeText, contextLines }
content.getSpan  { path, start, end, context }
index.addRoot    { path }
index.status     {}
```

**Orchestration**

```
orchestrate.explore.start { root, remoteURL?, branch?, policy?, goals? }
orchestrate.explore.status { sessionId }
orchestrate.explore.abort  { sessionId }
```

**Streaming Events**

```
index.progress { path, filesIndexed, chunks }
search.partial { corpus:"lex|sem|fused", items:[…], done:false }
orchestrate.event { sessionId, evt: AgentEvt }
```

**Versioning** — TXT records and handshake include `version`. Refuse incompatible clients.

---

## 10. Security & Policy

* **Path allowlist**: all FS ops scoped to declared roots; deny traversal outside.
* **Network policy**: external LLM calls require `allowExternalLLMs=true` and per‑provider config.
* **Privacy**: no exfiltration by default; traces stored locally; explicit export RPC required.
* **Rate limits**: single in‑flight `LanguageModelSession` request; reject concurrent tool calls.

---

## 11. Performance Targets

* Index throughput ≥ 5 MB/s text (M1+).
* p95 latencies on 100k chunks: lexical <300ms; semantic <500ms (K≤200); hybrid <700ms.
* Vector store footprint for dim=384, 100k chunks ≈ 153 MB.

---

## 12. Telemetry & Replay

* Log `AgentEnvelope` to `trace_event` for all commands/events.
* Provide a `replay(traceId)` utility to deterministically re‑run Explore plans against a snapshot.

---

## 13. Testing Strategy

* **Unit**: chunker, FTS queries, embedding normalize, RRF/MMR math.
* **Golden**: small corpora with expected top‑K and fixed spans.
* **Integration**: WebSocket server; exercise `search.*`, `content.getSpan`, `orchestrate.*` with streamed partials.
* **E2E Explore**: workspace default **/Users/christopherdavid/code/openagents**; assert summary fields and that only allowlisted paths were touched.
* **Property tests**: idempotent reindex; incremental updates preserve top‑K stability within tolerance.

---

## 14. Configuration

* Roots (paths), exclusions (globs), embedder selection, ANN mode (`BruteForce|ANN‑future`), resource caps.
* Policy for external LLMs (providers, models, rate caps).

---

## 15. Milestones

**M0 — Scaffolding**
SearchKitCore/Service/Client; SQLite schema; WebSocket skeleton; Bonjour; security‑scoped bookmarks.

**M1 — Search Primitives**
Chunker + FTS5; semantic embeddings; brute‑force ANN; RRF; `search.*` + `content.getSpan` + streaming partials.

**M2 — Orchestrator + Explore**
`orchestrate.explore.start/status/abort`; on‑device session; snapshot **plan streaming**; exec queue; summary artifact.

**M3 — External LLMs (opt‑in)**
`AgentCmd.llmCall` with providers (Claude Code, Codex); proxy integration; policy enforcement; observability.

**M4 — Symbols & Reranker**
SwiftSyntax symbols; light reranker for top‑50 via Core ML; near‑dup suppression (MMR).

---

## 16. Change Log

* Dropped any assumption of end‑user search UI; all surfaces are programmatic.
* Added Workspace/Git access semantics and default test path.
* Added `AgentCmd`/`AgentEvt` types and envelope; defined structured streaming.
* Added `orchestrate.*` RPCs and snapshot‑driven **Explore Codebase** flow.
* Documented policy gates for external LLMs; unified LLM calls under `LLMCall`.

---

**End of Spec v0.2**
