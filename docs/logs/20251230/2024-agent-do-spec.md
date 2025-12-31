# Agent DO spec phase 1

Date: 2025-12-30

## Summary
- Added D1 agents registry schema and DB helpers for create/list/get/delete.
- Implemented AgentDo durable object with SQLite memory schema init plus init/status/tick handlers.
- Wired agent CRUD + DO proxy routes, DO binding, and NIP-06 agent key derivation helper.

## Implementation details
- Created the `agents` D1 table and helper queries to allocate sequential agent_id values per user and soft delete agents.
- AgentDo now bootstraps memory tables, persists config in `agent_config`, and exposes `/init`, `/status`, and `/tick`.
- Added web worker routes for `/api/agents` and `/api/agents/:id/do/...` plus the AGENT_DO binding in wrangler.
- Added `derive_agent_keypair` (account index offset) to NIP-06 with a test and public export.

## Files touched
- `crates/web/migrations/0003_agents.sql`
- `crates/web/worker/src/db/agents.rs`
- `crates/web/worker/src/db/mod.rs`
- `crates/web/worker/src/agent_do.rs`
- `crates/web/worker/src/routes/agents.rs`
- `crates/web/worker/src/routes/mod.rs`
- `crates/web/worker/src/lib.rs`
- `crates/web/wrangler.toml`
- `crates/nostr/core/src/nip06.rs`
- `crates/nostr/core/src/lib.rs`
- `docs/logs/20251230/2024-agent-do-spec.md`

## Builds
- Not run (not requested).
