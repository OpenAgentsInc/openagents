# Convex Integration Audit (self‑hosted persistence for OpenAgents)

Goal
- Evaluate running a self‑hosted Convex backend alongside the Codex bridge to provide synchronized, queryable persistence for Projects, Threads, Messages, and Skills across multiple clients (mobile, web), while preserving Codex JSONL rollouts for compatibility.

## What Convex provides (from convex-backend)
- Reactive database and server function runtime with strong consistency and live WebSocket subscriptions.
- Storage backends: SQLite/Postgres engines (via crates `sqlite`, `postgres`, `database`, `storage`).
- Sync and live query layer: crates `sync`, `log_streaming`, `events`, `application`, `runtime`.
- Search + vectors:
  - Full‑text: crates `text_search`, `search` (see `memory_index`).
  - Vector: crate `vector` wraps Qdrant’s segment libs (HNSW index, mmap storage), supports filters and ANN queries.
- Auth + HTTP: crates `authentication`, `http_client`, `health_check`.
- Local backend entrypoints: crates `convex`, `local_backend`, `function_runner`, `node_executor`.

References
- Repo: ~/code/convex-backend
- Self‑hosting guide: self-hosted/README.md
- Crates dir sampling:
  - `sqlite/`, `postgres/`, `database/`, `storage/`
  - `sync/` (replication & subscriptions), `runtime/`
  - `search/`, `text_search/`, `vector/` (qdrant segments; HNSW)
  - `local_backend/`, `convex/` (binaries + orchestration)

## Fit for our use case
- Multi‑client sync: Yes. Convex’s reactive WS queries and replication layer are designed for multiple concurrent clients; typed functions control mutations.
- Local‑first topology: The backend can run on the user’s desktop (our source of truth), and mobile/web connect over LAN/VPN (e.g., Tailscale).
- Rich querying: Beyond JSONL traversal, we can maintain normalized tables for Projects/Threads/Messages; index text/vector for search and similarity.
- JSONL compatibility: We can keep Codex rollouts authoritative for resume while mirroring into Convex for querying.

## Proposed architecture with Codex bridge
- Processes
  - Codex bridge (Axum WS on :8787) remains the agent transport and JSONL forwarder.
  - Convex backend runs as a sibling process (preferred) or embedded binary, bound to loopback (e.g., :7788).
- Data flow (live)
  1) Bridge streams Codex JSONL → parses items (already implemented in `history.rs`).
  2) For persisted sessions, the bridge upserts Convex documents:
     - projects(id, name, workingDir, repo, createdAt, updatedAt)
     - threads(id, rolloutPath, source, projectId?, title, resumeId, createdAt/updatedAt)
     - messages(threadId, ts, role, kind, text, idx)
     - skills(id, source: user|registry, meta)
  3) Optional: emit embeddings for assistant/user message text and index via `vector` crate (either via background job or on write).
- Data flow (historical)
  - Backfill: scan `$CODEX_HOME/sessions/**` and write thread+message rows with rolloutPath + offsets. Keep Codex JSONL untouched.
- Client access
  - Mobile/web can either:
    - Query via Convex JS client (direct to :7788) for lists/search, or
    - Continue using our bridge WS for history/thread (low‑risk path), while we progressively move queries to Convex.

## Schema (initial)
- projects(id TEXT PK, name, workingDir, repo JSON, createdAt, updatedAt)
- threads(id TEXT PK, rolloutPath, source ENUM('Cli','Exec','VSCode'), projectId?, title, resumeId?, createdAt, updatedAt)
- messages(id AUTOINC, threadId FK, ts, role ENUM('assistant','user'), kind ENUM('message','reason','cmd'), text, idx)
- skills(id TEXT PK, source ENUM('user','registry'), name, description, license?, allowedTools?, metadata JSON)
- vector index: (threadId, messageId) → embedding (f32[]), filterable by projectId/threadId/role

## Mapping from Codex JSONL
- `thread.started` → create threads row; set resumeId; rollup `rolloutPath` captured from `session_configured`.
- `item.completed` with type `agent_message` → messages row (role=assistant, kind=message, text).
- `item.completed` with type `user_message` → messages row (role=user).
- `response_item/reasoning` or `event_msg/agent_reasoning` → messages row (kind=reason).
- `command_execution` → messages row (kind=cmd) with summarized payload.

## Sync & subscriptions
- Convex WS queries can subscribe clients to:
  - Recent threads list for a project (order by updatedAt desc)
  - Single thread messages (paginate by idx, or recent tail)
  - Skills list
- This lets mobile + web reflect updates with minimal glue code. Our current bridge‑WS view can remain as a compatibility path.

## Auth & security
- Run Convex bound to 127.0.0.1 by default; optionally advertise on LAN/Tailscale.
- Bridge can mint short‑lived access tokens or enforce a local static secret while prototyping.
- Long‑term: per‑device keys and explicit allowlists.

## Deployment & ops
- Self‑hosted binary or Docker compose; point to local SQLite (default) or a managed Postgres for persistence.
- Bridge launches Convex on demand (or verifies it’s reachable) and reports status in the app header.

## Keep JSONL as source of truth
- We do not replace Codex persistence; the Convex mirror is for queryability and multi‑device sync.
- Each thread stores `rolloutPath` → “resume by id/path” remains reliable.
- Optional user setting: “Publish sessions to Codex resume” (see chat‑persistence addendum) remains orthogonal.

## Feasibility
- High. The convex-backend repo exposes the pieces we need:
  - Storage (sqlite/postgres), sync layer, vector indexing, and a local backend entrypoint.
  - Our bridge already parses JSONL; writing normalized rows and embeddings is straightforward.
- Complexity trade‑offs
  - Adds another service to manage. Start with loopback + SQLite, opt‑in.
  - Indexing/embeddings require background jobs or opportunistic compute.

## Phased plan
- P0: Spike with loopback Convex
  - Run Convex locally (Docker or binary) with SQLite.
  - Create tables + basic functions (projects, threads, messages, skills).
  - Bridge writes rows on stream; app can fetch Skills/Projects from Convex (read‑only) while keeping existing WS for history/thread.
- P1: Reactive queries + search
  - Switch History drawer to query Convex (subscribe to latest N threads).
  - Backfill rollouts → rows.
  - Add full‑text search over messages.
- P2: Vectors
  - Add embeddings pipeline (OpenAI local or model of choice). Store in vector index (HNSW via `vector` crate).
  - Implement “Similar messages”/“Find threads like this”.
- P3: Multi‑device auth and exposure
  - LAN/Tailscale exposure, tokens, simple device pairing.
  - Optional web client that talks to Convex directly.

## Open questions
- How much of Convex we embed vs run as a sibling process? (sibling recommended)
- License (FSL‑1.1‑Apache‑2.0) is acceptable for local bundling; confirm distribution terms.
- Canonical schema ownership: keep in this repo and generate Convex schema/functions at runtime or check in a minimal app within a subfolder.

## Recommendation
Adopt a staged integration: keep JSONL rollouts for Codex compatibility, add Convex as an opt‑in local persistence service to enable live sync, rich querying, and vector search. Start with loopback SQLite, then layer on full‑text and vectors, and eventually reactive UI backed by Convex subscriptions.

---

Addendum: Deployment options (Docker vs. binary) and “one command” UX

Target UX
- End user runs a single command on desktop, e.g. `cargo bridge` or `cargo bridge --with-convex`, and everything required starts locally with SQLite. No Docker, no Postgres.

Options
1) Standalone Convex binary (preferred near‑term)
   - Shape: download or build the official Convex backend binary once, store under `~/.openagents/bin/convex-backend`, and spawn it from our bridge when needed.
   - Pros: minimal coupling; we track an upstream release tag; easy to replace/upgrade; works with SQLite; preserves Convex’s operational model.
   - Cons: separate artifact management (download/verify); platform‑specific builds.
   - UX: add `cargo run -p codex-bridge -- --start-convex` flag (aliased in `cargo bridge`), or a Settings toggle “Start local Convex automatically”. Health‑check on `127.0.0.1:7788`.

2) Git submodule + small runner crate in this repo
   - Shape: add `get-convex/convex-backend` as a git submodule; create `crates/oa-convex` that depends on minimal internal crates (e.g., `local_backend`) to produce a trimmed runner binary (e.g. `oa-convex`). SQLite only, loopback bind by default.
   - Pros: one Cargo workspace build; single `cargo bridge` can orchestrate both binaries; no external installer.
   - Cons: heavy monorepo dependency; internal crate APIs are not guaranteed stable; build times may increase; licensing diligence required (FSL‑1.1‑Apache‑2.0 is okay for local use, confirm redistribution policy).
   - UX: add alias `cargo convex` to run `oa-convex`, and `cargo bridge` can spawn/monitor it.

3) Git dependency (direct) on convex-backend crates
   - Shape: reference Convex crates via `git = "https://github.com/get-convex/convex-backend"` at a pinned `rev` and compile a runner.
   - Pros: no submodule maintenance; pin to a commit.
   - Cons: similar to (2) but potentially more brittle; complex workspace graph; large compile surface; upstream refactors can break our build.

4) Write a “Convex Bridge” wrapper binary (thin supervisor)
   - Shape: our `crates/convex-bridge` that only supervises a Convex server process and applies local defaults (SQLite path under `~/.openagents/convex`, port 7788, loopback bind), plus health checks and logs.
   - Pros: small and stable; hides process mgmt behind a clean CLI; pairs well with (1) for the actual server.
   - Cons: still needs a Convex server binary from (1) or (2).

Data layout and defaults
- DB root: `~/.openagents/convex/data` (SQLite files live here). Config/env under `~/.openagents/convex/config` if needed.
- Port: `127.0.0.1:7788` by default; configurable in Settings; Tailscale exposure optional.
- Health: use `health_check` endpoint from the server; bridge retries/backoff and shows status in header.

Recommended path
- Start with (1) + (4): supervise an upstream Convex backend binary from our bridge. Keep the “one command” UX by making `cargo bridge` start Convex automatically when the “Use local Convex persistence (beta)” toggle is enabled.
- If we want everything self‑contained later, move to (2) with a submodule + `crates/oa-convex` runner (SQLite only) and add `cargo convex` as a helper alias.

Notes
- Postgres is not required; SQLite is sufficient for P0/P1.
- We can keep JSONL rollouts authoritative and gradually cut over read paths (history/thread lists) to Convex subscriptions.
- For upgrades, bridge checks the Convex binary version at startup and prompts to update if incompatible.
