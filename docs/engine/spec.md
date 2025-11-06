# Semantic + Lexical Search Primitives — Swift‑Only (macOS + iOS) — v0.1

**Status:** Draft (proposed)

**Owner:** OpenAgents — Search Working Group

**Scope:** End‑to‑end, headless system for local code/document search that combines lexical (FTS/grep‑class) and semantic (embeddings + ANN) retrieval, with fusion ranking and optional reranking. macOS hosts the engine; clients (AI agents, desktop components, optional iOS companion) invoke it programmatically via RPC/SDK. No end‑user search UI is required. Swift‑only implementation; no Rust/Tauri/Expo.

> Update — 2025‑11‑06
> Refocused from an end‑user search UI to agent‑facing search primitives. UI components are optional diagnostics only. Added `includeText` support in search APIs and a `content.getSpan` RPC for precise snippet retrieval.

---

## 1. Goals

1. **High‑recall developer search primitives** on local repos/docs with <300ms p95 for lexical and <700ms p95 for hybrid (semantic+lexical) on medium repos (~50k–150k lines).
2. **Offline‑first, private**: all indexing & inference runs locally on macOS; clients invoke searches programmatically; iOS companion is optional for diagnostics.
3. **Hybrid retrieval**: semantic + lexical **fusion** beats either alone.
4. **Agent‑aware**: capture traces (queries, openings, accepted edits) to improve ranking over time.
5. **Single‑language developer flow**: Swift for engine, service, client, and tests.
6. **Simple install/upgrade**: single macOS service (LaunchAgent capable); iOS companion optional.

### Non‑Goals

* Cloud‑hosted search, cross‑org multi‑user sharing, or collaborative features.
* Full "code understanding" (refactors, call graphs) in v0.1. Symbol awareness is constrained to lightweight indexing.
* On‑device iOS full indexing (iOS runs a minimal client‑side subset only).

---

## 2. Architecture Overview

### Components

* **SearchKitCore (Swift package)** — Indexing, storage, lexical search (FTS5), semantic embedding calls, ANN search, fusion ranking, telemetry. Linked by both macOS and iOS targets, but the full engine runs on macOS.
* **SearchKitService (macOS target)** — A background service inside the macOS app bundle:

  * Hosts a **WebSocket + HTTP** interface (SwiftNIO) for control and queries.
  * Exposes a pairing flow (local network) for the iOS app.
  * Manages the on‑disk index directory.
* **SearchKitClient (Swift package)** — Lightweight, typed client SDK for invoking the service (JSON‑RPC/WebSocket) from agents and apps. Provides request/response models and helpers. No end‑user UI.

### Process Model

* Single macOS process (app) running the **service** (foreground or as a LaunchAgent for background availability).
* Clients (agent runtimes, desktop app components, optional iOS companion) connect over local network (Bonjour discovery → WebSocket).
* All search and indexing happens on macOS. Clients send queries and consume streamed results; no end‑user search UI is required.

### Data Flow (Happy Path)

1. User adds a folder to index in macOS app.
2. Indexer walks files → chunker produces chunks → store text in SQLite FTS and vectors in SQLite (BLOB) or vec‑table.
3. An AI agent (or client) issues a query via RPC/SDK.
4. Engine executes **lexical** and **semantic** sub‑queries → **Fusion** → optional **Rerank** → stream top‑K.
5. User clicks a result → macOS opens file at span; telemetry logged.

---

## 3. Storage & Index Format

**Root:** `~/Library/Application Support/OpenAgents/SearchIndex/`

**Per‑Repo Directory:**

```
<index_root>/<repo_slug>/
  ├── index.sqlite            # FTS5 tables, metadata
  ├── vectors.sqlite          # vector table (or same db as index.sqlite)
  ├── blobs/                  # optional: compressed text snapshots
  ├── state.json              # index config & stats
  └── .lock
```

### SQLite Schema (v0)

```sql
-- Files tracked
CREATE TABLE file (
  id INTEGER PRIMARY KEY,
  path TEXT NOT NULL,
  lang TEXT,
  size_bytes INTEGER,
  mtime_ns INTEGER,
  sha256 TEXT,
  indexed_at INTEGER
);
CREATE UNIQUE INDEX file_path_uq ON file(path);

-- Text chunks (code/doc segments)
CREATE TABLE chunk (
  id INTEGER PRIMARY KEY,
  file_id INTEGER NOT NULL REFERENCES file(id) ON DELETE CASCADE,
  start_line INTEGER,
  end_line INTEGER,
  byte_range_start INTEGER,
  byte_range_end INTEGER,
  text TEXT NOT NULL,
  fingerprint TEXT, -- e.g., rolling hash for dedupe
  tokens INTEGER,
  created_at INTEGER
);

-- Full‑text index (content=chunk for shadow table)
CREATE VIRTUAL TABLE chunk_fts USING fts5(
  text, path UNINDEXED, lang UNINDEXED, token="unicode61 remove_diacritics 2"
);

-- Backref to make FTS joins easy
CREATE VIEW chunk_search AS
SELECT c.id, c.text, f.path, f.lang, c.start_line, c.end_line
FROM chunk c JOIN file f ON f.id = c.file_id;

-- Vector table (normalized cosine vectors)
CREATE TABLE chunk_vec (
  chunk_id INTEGER PRIMARY KEY REFERENCES chunk(id) ON DELETE CASCADE,
  dim INTEGER NOT NULL,
  vec BLOB NOT NULL,      -- contiguous Float32[dim], L2‑normalized
  created_at INTEGER
);

-- Lightweight symbols (optional in v0)
CREATE TABLE symbol (
  id INTEGER PRIMARY KEY,
  file_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  kind TEXT,
  start_line INTEGER,
  end_line INTEGER
);

-- Telemetry for learning
CREATE TABLE trace_event (
  id INTEGER PRIMARY KEY,
  session_id TEXT,
  ts INTEGER,
  kind TEXT,              -- query|open|accept|reject|click
  payload TEXT            -- JSON
);
```

**Notes**

* Keep FTS and vectors in one DB initially to simplify distribution; split later if needed.
* All vectors are **pre‑normalized**; cosine distance becomes dot product.

---

## 4. Indexing Pipeline

### 4.1 File Discovery & Watch

* Add/watch root folders. Exclude `node_modules`, `.git`, `build/`, `DerivedData`, and large binaries by extension.
* Hash files (SHA‑256) to skip unchanged content.
* Persist per‑file `mtime_ns` and `sha256`.

### 4.2 Language Detection

* Heuristics by extension, plus simple shebang scans. Store `lang` in `file`.

### 4.3 Chunking (v0)

* **Heuristic chunks** per file:

  * Prefer function/class/struct blocks if reliably detected (SwiftSyntax for `.swift`); else fall back to sliding windows of ~80–200 lines with ~⅓ overlap.
  * Hard caps: max text per chunk ~6–8KB; avoid enormous tokens.
* Record `start_line`, `end_line`, `byte_range_*` to support precise previews.
* Deduplicate identical chunks via `fingerprint` (rolling hash) to avoid indexing repeated blobs.

### 4.4 Tokenization & Stop‑words (FTS)

* `unicode61` tokenizer; tune to avoid splitting on underscores. No stemming in v0.

### 4.5 Embedding Generation

* **On macOS:** Use Core ML model packaged as `.mlmodel` implementing `embed(texts:[String]) -> [[Float]]`.
* Normalize vectors with Accelerate (vDSP). Store in `chunk_vec`.
* Batch size tuned by model throughput (e.g., 32–128 sequence equivalents).

### 4.6 Backpressure & Scheduling

* Indexer runs with a small worker queue; pause/resume via API. Avoid starving UI threads; use `Task` + cooperative cancellation.

---

## 5. Query Execution

### 5.1 Query Parsing

* Input supports:

  * **Free text** (semantic + lexical by default)
  * **Operators**: `path:`, `lang:`, `symbol:` (when symbol table exists), `type:code|doc`, `since:` (recency bias)
  * **Mode selectors** (client‑provided hints): `All | Code | Docs | Symbols | Recent`

### 5.2 Sub‑Queries

* **Lexical**: FTS5 query against `chunk_fts` with BM25 scoring. Extract highlight offsets for preview.
* **Semantic**: Embed the query string to `q ∈ R^d`; compute top‑K via:

  * v0: **brute‑force dot product** against `chunk_vec` with vDSP (fast enough for ≤100k chunks)
  * v1: add an **ANN index** (e.g., IVF‑Flat/HNSW) as a Swift module; store centroids/graph on disk.

### 5.3 Fusion (Required)

* Compute **Reciprocal Rank Fusion (RRF)** across the two ranked lists:

  * `score_i = Σ 1 / (k + rank_i)` with `k ≈ 60` (tunable)
  * Add tie‑break features: path match boosts, recent edits, symbol name matches, query term overlap.
* Return a single ranked stream.

### 5.4 Reranking (Optional v1)

* A small cross‑encoder in Core ML for top‑50 to reorder based on pair similarity.
* Can be toggled off; served via the same embedding provider abstraction.

### 5.5 Streaming

* Results are streamed in batches (e.g., 20 at a time) over WebSocket as soon as each sub‑query completes; final fused order sent when both finish.

---

## 6. Service & IPC (Swift‑Only)

### 6.1 Transport

* **SwiftNIO WebSocket** server embedded in macOS app.
* **Bonjour** (NSNetService) advertises `_oasearch._tcp` with TXT records (version, device name).
* iOS discovers & connects over LAN. Fallback: manual host:port + QR code.

### 6.2 Pairing & Security

* **Ephemeral pairing code** shown on Mac; iOS scans QR; both derive a shared key using CryptoKit (Curve25519 key exchange).
* **TLS‑like channel**: use WebSocket over TCP with **noise‑style** handshake at app level or use NWTLS with a pinned self‑signed cert generated at pairing. Store public key fingerprint in iOS Keychain and macOS Keychain.
* **AuthZ**: each request includes a short‑lived nonce signed with the session key (prevents replay).

### 6.3 API (JSON‑RPC style)

**Requests**

```json
{ "id":"uuid", "method":"search.lexical",  "params": {"query":"...","k":50, "filters":{}, "includeText":false, "contextLines":0} }
{ "id":"uuid", "method":"search.semantic", "params": {"query":"...","k":50, "filters":{}, "includeText":false, "contextLines":0} }
{ "id":"uuid", "method":"search.hybrid",  "params": {"query":"...","k":50, "filters":{}, "includeText":true,  "contextLines":2} }
{ "id":"uuid", "method":"index.addRoot",  "params": {"path":"/Users/…/repo"} }
{ "id":"uuid", "method":"index.status",  "params": {} }
{ "id":"uuid", "method":"open.atSpan",     "params": {"path":"…","start":123,"end":145} }
{ "id":"uuid", "method":"content.getSpan",  "params": {"path":"…","start":123,"end":145, "context":2} }
```

**Responses**

```json
{ "id":"uuid", "result": { "status":"ok" } }
{ "id":"uuid", "result": { "items":[ {"path":"…","start":12,"end":24,"preview":"…","text":"…","score":0.83,"kind":"lex"}, … ] } }
{ "id":"uuid", "error": { "code": 400, "message":"…" } }
```

`includeText` returns bounded snippet text (span ± `contextLines`), subject to size limits. For larger or precise ranges, use `content.getSpan` which returns `{ text: "…", start: n, end: m }`.

**Events** (server → client)

```json
{ "method":"index.progress", "params": {"path":"…","filesIndexed":123,"chunks":5231} }
{ "method":"search.partial",  "params": {"corpus":"lex|sem|fused","items":[…],"done":false} }
```

### 6.4 Backward Compatibility & Versioning

* Include `version` in Bonjour TXT and in every response header; refuse mismatches with a graceful error.

---

## 7. Client Integrations (No End‑User UI Required)

- **Agent orchestration**: Coding agents call `search.*` RPCs to retrieve ranked snippets. Use `includeText=true` to inline snippet text or `content.getSpan` for precise ranges/context.
- **Desktop integration**: The OpenAgents desktop app may call the same primitives to enrich flows; a dedicated search UI is not required.
- **Optional diagnostics UIs**: macOS/iOS may include debug/status panels (index status, progress). These are optional and not part of the runtime contract.

---

## 8. Telemetry & Learning (Private)

* **Trace events** logged locally (SQLite `trace_event`).
* For each query, store:

  * Query text, filters, time.
  * Top‑N shown; item clicked; whether user quickly backtracked (implicit dissatisfaction).
* **Lift evaluation**: a local “context bench” (curated pairs of {question, expected spans}).
* **No exfiltration by default.** Opt‑in to export anonymized stats as a file.

---

## 9. Performance Targets & Sizing

* **Index build throughput**: ≥ 5 MB/sec text on Apple Silicon (M1+), single‑thread v0.
* **Query latency (p95)** on a 100k‑chunk corpus:

  * Lexical‑only: < 300ms
  * Semantic brute‑force (vDSP): < 500ms for K=200
  * Hybrid fused: < 700ms
* **Storage overhead**: vectors ~ (4 bytes × dim × chunks). For dim=384 and 100k chunks → ~153 MB.

---

## 10. Testing Strategy (Swift‑only)

* **Unit tests (SearchKitCore)**

  * FTS queries, chunker behaviors, vector normalization, fusion math (RRF), MMR diversity.
* **Golden corpus tests**

  * Small synthetic repos with expected top‑K; assert rankings within tolerance.
* **Property tests** (SwiftCheck)

  * Idempotent reindex; incremental updates preserve results.
* **Integration tests (SearchKitService)**

  * Spin up the WebSocket server; issue queries; assert streamed batches and final fused order.
* **Client integration tests**

  * Validate `search.*` and `content.getSpan` RPCs and streamed partials against a golden corpus.
* **Bench harness** (separate target)

  * Measures p50/p95/p99 for lexical/semantic/hybrid across corpora; exports JSON.

---

## 11. Security & Privacy

* All data stays on device by default. No network egress.
* Pairing keys stored in Keychain; ability to revoke devices.
* Signed nonces on every request; reject stale/duplicated nonces.
* Index directories respect macOS file permissions.

---

## 12. Failure Handling & Recovery

* **Corruption**: on SQLite error, auto‑backup and rebuild; do not brick the app.
* **Backpressure**: pause indexing under heavy query load.
* **Graceful degradation**: if vectors missing, run lexical‑only; if FTS missing, run semantic‑only.

---

## 13. Configuration

* Index roots (paths)
* Exclusions (glob patterns)
* Embedding model choice (default/local)
* ANN mode: `BruteForce | ANN` (when available)
* Resource caps: max CPU %, max memory (soft)

---

## 14. Implementation Plan & Milestones

**M0 — Scaffolding (1–2 days)**

* Create Swift packages: SearchKitCore, SearchKitService, SearchKitClient.
* SQLite schema migrations; basic file walker; add/remove roots.

**M1 — Lexical Search (3–5 days)**

* Chunker (heuristic) + FTS5; JSON‑RPC `search.lexical`; streaming results; typed client SDK.

**M2 — Semantic Search (5–7 days)**

* Core ML embedding provider; vector storage (BLOB); brute‑force ANN via vDSP; fusion (RRF); `includeText` results.

**M3 — Pairing & Security (3–5 days)**

* Bonjour discovery; QR pairing; session key & pinned certs; signed requests.

**M4 — Telemetry & Bench (3–4 days)**

* Trace_event logging; built‑in context bench; bench harness target.

**M5 — Polish & Symbol Awareness (5–7 days)**

* SwiftSyntax chunker; path/lang filters; basic symbol table; rerank switch (stub); `content.getSpan` ergonomics.

---

## 15. Public Protocols (Swift) — Key APIs

```swift
// MARK: Embeddings
public protocol EmbeddingProvider {
  var dimension: Int { get }
  func embed(_ texts: [String]) throws -> [[Float]]
}

// MARK: Search Requests
public struct SearchFilters: Codable, Sendable {
  public var pathPrefix: String?
  public var language: String?
  public var type: String? // "code" | "doc"
  public var since: Date?
}

public struct SearchResult: Codable, Sendable {
  public var path: String
  public var startLine: Int
  public var endLine: Int
  public var preview: String
  public var text: String? // optional inlined snippet when requested
  public var score: Double
  public var kind: String // "lex" | "sem" | "fused"
}

public protocol SearchEngine {
  func searchLexical(_ query: String, k: Int, filters: SearchFilters?) async throws -> [SearchResult]
  func searchSemantic(_ query: String, k: Int, filters: SearchFilters?) async throws -> [SearchResult]
  func searchHybrid(_ query: String, k: Int, filters: SearchFilters?) async throws -> [SearchResult]
}

// MARK: Content retrieval
public protocol ContentReader {
  /// Returns the file text covering [startLine, endLine], optionally expanded by context lines.
  func getSpan(path: String, startLine: Int, endLine: Int, context: Int?) throws -> String
}
```

---

## 16. Open Questions

1. Which embedding model ships v0 (dimension, license)?
2. Add near‑duplicate suppression at **merge** time (MMR) vs pre‑index dedupe only?
3. ANN choice for v1 (HNSW vs IVF‑Flat) and Swift implementation strategy.
4. Reranker size & latency budget (target <200ms for top‑50).
5. Do we support multiple concurrent macOS indexes and route queries by “workspace”?

---

## 17. Future Work (Not in v0)

* Cross‑workspace federated search (multiple repos at once).
* Deeper symbol graphs (LSIF‑like) and x‑ref jumps.
* Learning‑to‑rank using local traces with tiny adapters; ship updated weights as model deltas.
* CI integration: headless CLI that answers search queries during PR review.

---

## 18. Appendix — Fusion Details

**Reciprocal Rank Fusion (RRF)**

```
score(d) = Σ_{lists L} 1 / (k + rank_L(d)) ;  k ∈ [10, 60]
```

Tunable boosts:

* +pathPrefix exact match (+0.15)
* +symbol name contains query (+0.10)
* +recent file edit (<7 days) (+0.05)
  Normalize to [0,1] after fusion.

**MMR Diversity (optional)**

```
MMR = argmax_{d∈C\S} [ λ·sim(d, q) − (1−λ)·max_{d'∈S} sim(d, d') ]
```

Use to reduce near‑duplicates in fused top‑K.

---

**End of Spec v0.1**
