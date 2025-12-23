# Audit (Extended): 2025-12-23 Codebase

## Scope
- Range reviewed: cc4a00ca3..061ebac31 plus current HEAD state.
- Areas expanded: parallel autopilot containers + MCP wiring, autopilot GUI data paths, Docker image/compose, database concurrency, workspace membership hygiene, GPT-OSS tooling, and stub inventory across wallet/marketplace/compute/ACP.
- Sources: code inspection, targeted `rg` searches, and rlogs in `docs/logs/20251223/*.rlog`.

## Method
- Searched for not-implemented paths and hard error returns in core flows.
- Reviewed container build/compose assets for correctness and operational drift.
- Inspected DB initialization for concurrency readiness.
- Cross-checked CLI vs runtime behavior for mismatches.

## Findings (Severity Ordered)
### Critical
- Parallel autopilot containers cannot run issue MCP: `--with-issues` writes `.mcp.json` to run `cargo run -p issues-mcp`, but the Docker image does not install Cargo and the entrypoint always uses `--with-issues`. Issue tooling cannot start inside containers. (`crates/autopilot/src/main.rs:1156`, `crates/autopilot/src/main.rs:1967`, `docker/autopilot/Dockerfile:18`, `docker/autopilot/Dockerfile:62`)
- Shared issues DB is not actually used in containers: compose sets `ISSUES_DB=/shared/autopilot.db` and mounts `/shared/autopilot.db`, but autopilot never reads `ISSUES_DB` and the entrypoint does not pass `--issues-db`, so default DB is `/workspace/autopilot.db` per worktree. This breaks shared scheduling/claiming. (`docker/autopilot/docker-compose.yml:23`, `docker/autopilot/Dockerfile:62`, `crates/autopilot/src/cli.rs:67`, `crates/autopilot/src/lib.rs:1060`)
- d-012 violations still block core flows across wallet/marketplace/compute/ACP/GUI, preventing real functionality via default CLI/GUI. (`src/gui/routes/wallet.rs:14`, `src/gui/routes/marketplace.rs:15`, `src/gui/routes/gitafter.rs:13`, `src/gui/routes/acp.rs:224`, `crates/wallet/src/cli/bitcoin.rs:5`, `crates/marketplace/src/compute/consumer.rs:258`, `crates/compute/src/services/relay_service.rs:81`)
- Bifrost threshold ECDH/signing remains unimplemented, and tests expect failure, so cryptographic core is non-functional end-to-end. (`crates/frostr/src/bifrost/aggregator.rs:183`, `crates/frostr/src/bifrost/node.rs:556`, `crates/frostr/tests/bifrost_e2e.rs:116`)

### High
- Autopilot CLI advertises `gpt-oss`, but runtime only supports `claude` or `codex`, causing `Unknown agent` errors for documented usage. (`crates/autopilot/src/cli.rs:31`, `crates/autopilot/src/main.rs:1199`)
- Autopilot GUI uses fixed DB paths and runs blocking rusqlite queries inside async handlers, risking event-loop stalls and empty UI data when DBs live elsewhere. (`crates/autopilot-gui/src/server/routes.rs:46`, `crates/autopilot-gui/src/server/parallel.rs:242`, `crates/autopilot-gui/src/sessions.rs:38`)
- SQLite concurrency not configured for parallel agents: WAL and busy_timeout are not set in issues/marketplace/metrics DBs, risking "database is locked" under concurrent writes. (`crates/issues/src/db.rs:20`, `crates/marketplace/src/db.rs:14`, `crates/autopilot/src/metrics/mod.rs:379`)
- Docker healthcheck will fail: container relies on `pgrep` but `procps` is not installed. (`docker/autopilot/Dockerfile:18`, `docker/autopilot/Dockerfile:56`)
- Compose resource limits are likely ignored outside Swarm; `deploy.resources` does not apply in normal `docker-compose` usage. (`docker/autopilot/docker-compose.yml:29`)
- Path traversal risk in GPT-OSS apply_patch when target does not exist (non-canonicalized fallback path). (`crates/gpt-oss-agent/src/tools/apply_patch.rs:79`)

### Medium
- Autopilot runs with `dangerously_skip_permissions(true)` in run/resume paths, so GUI permission rules are not enforced. (`crates/autopilot/src/main.rs:1130`, `crates/autopilot/src/main.rs:1942`)
- Worktree creation failures only warn; containers start anyway, so missing worktrees lead to empty mounts. (`crates/autopilot/src/parallel/worktree.rs:42`, `crates/autopilot/src/parallel/docker.rs:48`)
- Plan-mode "launch swarm" path is explicitly not implemented. (`crates/autopilot/src/planmode.rs:270`)
- TaskComplexity deserialization discards persisted data and defaults, risking misleading analytics. (`crates/autopilot/src/model_selection.rs:478`)
- GPT-OSS default model is set to `gpt-4o-mini`, which does not match GPT-OSS naming. (`crates/gpt-oss/src/client.rs:8`)
- Docker orchestration hard-depends on `docker-compose` instead of Compose v2 (`docker compose`). (`crates/autopilot/src/parallel/docker.rs:73`)

### Low
- Autopilot GUI token usage is placeholder/demo data rather than computed from logs. (`crates/autopilot-gui/src/server/routes.rs:183`)
- Agent status fields are always None (current issue, uptime). (`crates/autopilot/src/parallel/docker.rs:149`)
- Wallet settings set is unimplemented; FROSTR share export is partial and non-serializable. (`crates/wallet/src/cli/settings.rs:30`, `crates/wallet/src/cli/frostr.rs:88`)
- Marketplace provider/earnings are stubbed at the unified CLI layer. (`src/cli/marketplace.rs:45`)
- `claude-mcp` declares a bin target but has no `src/main.rs` and is not a workspace member, so it is effectively unbuildable. (`crates/claude-mcp/Cargo.toml:10`, `Cargo.toml:3`)

## Expanded Stub Inventory (Selected)
### ACP / Agent Integration
- ACP prompt/cancel endpoints do not invoke live agent connections. (`src/gui/routes/acp.rs:224`, `src/gui/routes/acp.rs:268`)
- Terminal creation is a stub (no PTY spawn). (`crates/acp-adapter/src/client.rs:340`)

### Wallet
- Payment flows in CLI and GUI are hard-bailed pending Breez integration. (`crates/wallet/src/cli/bitcoin.rs:5`, `crates/wallet/src/gui/server.rs:107`)
- Contacts management requires relay integration and returns explicit errors. (`crates/wallet/src/cli/identity.rs:380`)
- FROSTR share import/export is partially implemented only. (`crates/wallet/src/cli/frostr.rs:76`)

### Marketplace / Compute
- Compute job submission and relay integrations are stubbed. (`crates/marketplace/src/compute/consumer.rs:258`, `crates/compute/src/services/relay_service.rs:81`)
- Skills and data browsing are stubbed (Nostr relay integration missing). (`crates/marketplace/src/skills/browse.rs:248`, `crates/marketplace/src/data/discover.rs:289`)
- Ollama service is disabled. (`crates/compute/src/services/ollama_service.rs:1`)

### Autopilot / Metrics
- Context loss analyzer returns empty results. (`crates/autopilot/src/context_analysis.rs:89`)
- Swarm execution is announced but not implemented. (`crates/autopilot/src/planmode.rs:270`)

### GitAfter
- LNURL payment flow not implemented for bounty claims. (`crates/gitafter/src/server.rs:3724`)
- Restack base-layer rebase is not implemented. (`crates/gitafter/src/stacks/restack.rs:194`)

### GPT-OSS Tools
- Browser search tool is placeholder. (`crates/gpt-oss-agent/src/tools/browser.rs:134`)
- Python tool writes temp file but executes inline `python -c`. (`crates/gpt-oss-agent/src/tools/python.rs:92`)

## Testing / Verification Gaps
- Marketplace E2E tests are conceptual and avoid real relay publish/subscribe flows. (`crates/marketplace/tests/compute_e2e.rs:58`, `crates/marketplace/tests/skill_e2e.rs:1`)
- NIP-SA integration tests are missing from `crates/nostr/tests/`, despite directive expectations.
- Autopilot GUI session tests skip on missing DB and do not assert content. (`crates/autopilot-gui/src/sessions.rs:146`)

## Operational / Portability Risks
- Docker Compose resource limits may be ignored in non-Swarm usage. (`docker/autopilot/docker-compose.yml:29`)
- `docker-compose` binary hard dependency in Rust orchestration. (`crates/autopilot/src/parallel/docker.rs:73`)
- `breez-sdk-spark` is a relative path dependency outside the repo, so builds fail without `~/code/spark-sdk`. (`crates/spark/Cargo.toml:38`)

## Recommendations (Extended)
1. Fix parallel autopilot issue tracking: install a known-good `issues-mcp` binary in the image or change MCP config to use a bundled binary, and wire `--issues-db`/`ISSUES_DB` into autopilot.
2. Remove or implement d-012 stubs across wallet/marketplace/compute/ACP; prioritize relay integration to unlock multiple directives.
3. Add SQLite WAL + busy_timeout pragmas to issues/marketplace/metrics databases.
4. Align CLI docs with supported agent list, and add gpt-oss support or remove references.
5. Replace blocking DB calls in GUI handlers with `spawn_blocking` or an async driver; make DB paths configurable.
6. Harden container orchestration (healthcheck dependencies, resource limits, compose v2 support).
