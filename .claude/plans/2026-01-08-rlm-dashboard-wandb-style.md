# RLM Dashboard Implementation Plan

Extend `/rlm` from demo-only visualization to full W&B-style experiment tracking.

## Current State

- **Web UI** (`crates/web/client/src/views/rlm.rs`): Demo trace playback, 4 phases, inspector
- **Web Worker** (`crates/web/worker/src/routes/rlm.rs`): Static HTML only, no API
- **Pylon CLI** (`crates/pylon/src/cli/rlm.rs`): Does NOT use FRLM, no trace events
- **FRLM** (`crates/frlm/src/trace.rs`): TraceEmitter exists but no persistence

## Architecture

```
Pylon CLI ──► FRLM TraceEmitter ──► Local SQLite
                                        │
                                        ▼ (opt-in sync)
                               D1 (Cloudflare) ◄── Web API
                                        │
                                        ▼
                               Dashboard UI ◄── WebSocket (live)
```

**Key decisions:**
- Local-first: SQLite in Pylon, sync optional
- `pylon rlm` must be updated to use FRLM
- Durable Object for live WebSocket streaming

---

## Phase 1: Local Storage

**Goal:** `pylon rlm` stores traces locally

### Tasks

1. **Add RLM database module**
   - File: `crates/pylon/src/db/rlm.rs`
   - Tables: `runs`, `trace_events`, `experiments`
   - Migration in `crates/pylon/src/db/migrations/`

2. **Add SQLite sink to TraceEmitter**
   - File: `crates/frlm/src/trace_db.rs`
   - Batch writes, flush on completion

3. **Update `pylon rlm` to use FRLM**
   - File: `crates/pylon/src/cli/rlm.rs`
   - Wrap execution with TraceEmitter
   - Add `--log` flag (default on)

4. **Add `pylon rlm history` command**
   - List runs from local SQLite
   - Show status, query, cost, duration

### Deliverables
- `pylon rlm "query"` stores trace to `~/.openagents/pylon/rlm.db`
- `pylon rlm history` lists past runs

---

## Phase 2: Web API

**Goal:** Sync runs to cloud, API for dashboard

### Tasks

1. **D1 migration**
   - File: `crates/web/migrations/0009_rlm_runs.sql`
   - Tables: `rlm_runs`, `rlm_trace_events`, `rlm_experiments`

2. **API routes**
   - File: `crates/web/worker/src/routes/rlm.rs`
   - `GET /api/rlm/runs` - list user's runs
   - `GET /api/rlm/runs/:id` - run detail
   - `POST /api/rlm/runs/sync` - upload local run
   - `GET /api/rlm/runs/:id/trace` - trace events

3. **Durable Object for live streaming**
   - File: `crates/web/worker/src/rlm_do.rs`
   - WebSocket relay: Pylon → Browser
   - Based on existing `TunnelRelay` pattern

4. **Sync command**
   - `pylon rlm sync [run-id]`
   - Signs with Nostr key, POSTs to API

### Deliverables
- `pylon rlm sync` uploads to cloud
- `/api/rlm/runs` returns paginated list

---

## Phase 3: Dashboard UI

**Goal:** Full dashboard with live data

### Tasks

1. **Refactor rlm.rs into views/**
   - `crates/web/client/src/views/rlm/mod.rs`
   - `crates/web/client/src/views/rlm/list.rs` - run list
   - `crates/web/client/src/views/rlm/detail.rs` - run detail

2. **Fetch from API**
   - Add API client calls
   - Toggle demo mode vs live mode

3. **WebSocket for live runs**
   - Connect to RlmRunDO
   - Real-time trace event updates

4. **New routes**
   - `/rlm` - run list (dashboard home)
   - `/rlm/runs/:id` - run detail
   - `/rlm/demo` - demo mode (preserve existing)

### Deliverables
- `/rlm` shows user's runs
- Click run to see real trace playback
- Live runs update in real-time

---

## Phase 4: Experiment Comparison

**Goal:** W&B-style experiment tracking

### Tasks

1. **Experiment management**
   - Create/edit experiments
   - Group runs into experiments

2. **ComparisonTable component**
   - Side-by-side metrics
   - Sortable columns

3. **ProviderLeaderboard component**
   - Aggregate stats: latency, success rate, cost

4. **Export**
   - CSV export of metrics
   - JSON export of traces

---

## Files to Create

```
crates/pylon/src/db/rlm.rs
crates/frlm/src/trace_db.rs
crates/web/migrations/0009_rlm_runs.sql
crates/web/worker/src/db/rlm.rs
crates/web/worker/src/rlm_do.rs
crates/web/client/src/views/rlm/mod.rs
crates/web/client/src/views/rlm/list.rs
crates/web/client/src/views/rlm/detail.rs
```

## Files to Modify

```
crates/pylon/src/cli/rlm.rs      # Use FRLM, add history/sync
crates/pylon/src/db/mod.rs       # Export rlm module
crates/pylon/Cargo.toml          # Add frlm dependency
crates/frlm/src/trace.rs         # Add SQLite sink option
crates/frlm/src/lib.rs           # Export trace_db
crates/web/worker/src/lib.rs     # Add RLM routes
crates/web/worker/wrangler.toml  # Add RlmRunDO
crates/web/client/src/state.rs   # Extend RlmVizState
crates/web/client/src/app.rs     # Add new routes
```

---

## MVP Option

If we want a faster first deliverable, we can ship **Phase 1 only**:
- Local-only storage
- `pylon rlm history` shows past runs
- No cloud sync, no web dashboard changes
- ~3-5 days of work

Then iterate from there.

---

## Agent Documentation / Execution Log

To keep a reliable trail, all agents should update this file while working on this plan.

**Rules:**
- Append new log entries at the end of this file (do not rewrite earlier entries).
- If a task or decision deviates from this plan, add a short inline note in the relevant section using `[NOTE YYYY-MM-DD]` and reference the log entry.
- Always record tests run; if none, write `not run` with a short reason.

**Log template (copy/paste):**
```
### Log - YYYY-MM-DD - <agent>
- Scope:
- Changes:
- Files:
- Commands/Tests:
- Decisions/Notes:
- Follow-ups:
```

**Log (append entries below):**

### Log - 2026-01-08 - codex
- Scope: Phase 1 local RLM storage + FRLM integration for pylon rlm
- Changes: Added FRLM SQLite trace writer; added RLM SQLite schema/store; refactored pylon rlm to use FRLM with local/swarm submitters, trace logging, and history subcommand
- Files: crates/frlm/src/trace_db.rs, crates/frlm/src/lib.rs, crates/frlm/Cargo.toml, crates/pylon/src/db/migrations/004_rlm.sql, crates/pylon/src/db/rlm.rs, crates/pylon/src/db/mod.rs, crates/pylon/src/cli/rlm.rs, crates/pylon/Cargo.toml
- Commands/Tests: not run (not requested)
- Decisions/Notes: FRLM trace logging uses a background thread with a dedicated connection; local-only uses LocalSubmitter to preserve fragment fanout
- Follow-ups: consider surfacing fanout/quorum controls; verify local fallback behavior with real swarm and backends

### Log - 2026-01-08 - codex
- Scope: Phase 2 cloud sync + API + live relay DO
- Changes: Added D1 migration for rlm runs/trace/experiments; added worker DB helpers and RLM API endpoints; added RlmRunDO and WebSocket route; added pylon rlm sync command with Nostr-signed payload
- Files: crates/web/migrations/0009_rlm_runs.sql, crates/web/worker/src/db/rlm.rs, crates/web/worker/src/db/mod.rs, crates/web/worker/src/db/users.rs, crates/web/worker/src/routes/rlm.rs, crates/web/worker/src/rlm_do.rs, crates/web/worker/src/lib.rs, crates/web/wrangler.toml, crates/pylon/src/db/rlm.rs, crates/pylon/src/cli/rlm.rs, crates/pylon/Cargo.toml
- Commands/Tests: not run (not requested)
- Decisions/Notes: Sync payload signed with Nostr key over JSON string; browser WS requires session + run ownership; pylon WS requires signature over run_id
- Follow-ups: consider pagination cursors and streaming client integration in pylon

### Log - 2026-01-08 - codex
- Scope: Phase 3 dashboard UI (list/detail/demo) and client runtime wiring
- Changes: Split RLM views into list/detail/demo modules; added list view with run rows and empty/error states; added detail view with run summary, output, and trace log + live status; added client runtime fetchers for /api/rlm runs and traces plus WebSocket listener; updated app routing and input handling for RLM modes; updated worker HTML to set RLM_MODE and RLM_RUN_ID; added routes for /rlm/demo and /rlm/runs/:id
- Files: crates/web/client/src/views/rlm/mod.rs, crates/web/client/src/views/rlm/list.rs, crates/web/client/src/views/rlm/detail.rs, crates/web/client/src/views/rlm/demo.rs, crates/web/client/src/views/mod.rs, crates/web/client/src/rlm_runtime.rs, crates/web/client/src/app.rs, crates/web/client/src/lib.rs, crates/web/client/src/state.rs, crates/web/worker/src/routes/rlm.rs, crates/web/worker/src/lib.rs
- Commands/Tests: not run (not requested)
- Decisions/Notes: preserved existing RLM visualization as demo-only route; live detail view shows trace log entries with summarized FRLM events; WebSocket handler accepts either raw TraceEvent JSON or {event,...} payloads
- Follow-ups: map FRLM trace events into the demo visualization for richer playback; consider explicit WS event schema/versioning

### Log - 2026-01-08 - codex
- Scope: Phase 4 experiment comparison, export, provider leaderboard
- Changes: Added experiment-run mapping table and DB helpers; added experiment list/detail/provider endpoints and HTML routes; added experiment comparison/leaderboard views and runtime fetchers; added experiment export (CSV metrics, JSON with trace events); added client state + routing for new views; added local experiment grouping types in rlm crate
- Files: crates/web/migrations/0010_rlm_experiment_runs.sql, crates/web/worker/src/db/rlm.rs, crates/web/worker/src/routes/rlm.rs, crates/web/worker/src/routes/export.rs, crates/web/worker/src/lib.rs, crates/web/client/src/state.rs, crates/web/client/src/rlm_runtime.rs, crates/web/client/src/app.rs, crates/web/client/src/views/rlm/experiments.rs, crates/web/client/src/views/rlm/experiment_detail.rs, crates/web/client/src/views/rlm/providers.rs, crates/web/client/src/views/rlm/mod.rs, crates/web/client/src/views/mod.rs, crates/rlm/src/experiment.rs, crates/rlm/src/lib.rs
- Commands/Tests: not run (not requested)
- Decisions/Notes: Provider leaderboard derived from subquery_execute/subquery_return events; export JSON includes per-run trace events
- Follow-ups: consider pagination and server-side sort filters for experiment list/provider stats
