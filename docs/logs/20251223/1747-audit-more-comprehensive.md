# Audit (More Comprehensive): 2025-12-23 Codebase

## Scope
- Range reviewed: cc4a00ca3..061ebac31 plus current HEAD state.
- Areas expanded: autopilot daemon portability, MCP/issue tracking wiring, container orchestration, DB concurrency, GUI data paths, workspace membership, and stub coverage across wallet/marketplace/compute/ACP.
- Sources: code inspection, targeted `rg` searches, and rlogs in `docs/logs/20251223/*.rlog`.

## Method
- Cataloged explicit not-implemented paths and hard error responses in default CLI/GUI routes.
- Reviewed docker/autopilot image + compose + orchestration code for runtime correctness.
- Verified database initialization pragmas for concurrent access.
- Checked daemon control/IPC for platform portability.

## Findings (Severity Ordered)
### Critical
- Parallel autopilot containers cannot run issue MCP: `--with-issues` writes `.mcp.json` that runs `cargo run -p issues-mcp`, but the Docker image does not include Cargo and entrypoint always uses `--with-issues`. Issue tooling cannot start inside containers. (`crates/autopilot/src/main.rs:1156`, `crates/autopilot/src/main.rs:1967`, `docker/autopilot/Dockerfile:18`, `docker/autopilot/Dockerfile:62`)
- Shared issues DB is not honored in containers: compose sets `ISSUES_DB=/shared/autopilot.db`, but autopilot never reads it and entrypoint omits `--issues-db`, so default DB is `/workspace/autopilot.db` per worktree. Parallel agents do not share issue state. (`docker/autopilot/docker-compose.yml:23`, `crates/autopilot/src/cli.rs:67`, `crates/autopilot/src/lib.rs:1060`)
- d-012 violations still block core flows across wallet/marketplace/compute/ACP/GUI, preventing real functionality in default CLI/GUI paths. (`src/gui/routes/wallet.rs:14`, `src/gui/routes/marketplace.rs:15`, `src/gui/routes/gitafter.rs:13`, `src/gui/routes/acp.rs:224`, `crates/wallet/src/cli/bitcoin.rs:5`, `crates/marketplace/src/compute/consumer.rs:258`, `crates/compute/src/services/relay_service.rs:81`)
- Bifrost threshold ECDH/signing remains unimplemented and tests expect failure, so cryptographic core is non-functional end-to-end. (`crates/frostr/src/bifrost/aggregator.rs:183`, `crates/frostr/src/bifrost/node.rs:556`, `crates/frostr/tests/bifrost_e2e.rs:116`)

### High
- Autopilot daemon is Unix-only with no cfg gating: uses Unix domain sockets and `tokio::signal::unix`, so builds fail on non-Unix targets and portability is broken. (`crates/autopilot/src/daemon/control.rs:9`, `crates/autopilot/src/bin/autopilotd.rs:167`)
- Autopilot CLI advertises `gpt-oss`, but runtime only supports `claude`/`codex`, yielding `Unknown agent` errors for documented usage. (`crates/autopilot/src/cli.rs:31`, `crates/autopilot/src/main.rs:1199`)
- Autopilot GUI uses fixed DB paths and blocking rusqlite in async handlers, risking event-loop stalls and empty dashboards when DBs live elsewhere. (`crates/autopilot-gui/src/server/routes.rs:46`, `crates/autopilot-gui/src/server/parallel.rs:242`, `crates/autopilot-gui/src/sessions.rs:38`)
- SQLite concurrency not configured for parallel agents: WAL and busy_timeout not set in issues/marketplace/metrics DBs, risking "database is locked" under concurrent writes. (`crates/issues/src/db.rs:20`, `crates/marketplace/src/db.rs:14`, `crates/autopilot/src/metrics/mod.rs:379`)
- Autopilot runs with `dangerously_skip_permissions(true)` in run/resume paths, bypassing permission rules. (`crates/autopilot/src/main.rs:1130`, `crates/autopilot/src/main.rs:1942`)
- Docker healthcheck will fail: container relies on `pgrep` but `procps` is not installed. (`docker/autopilot/Dockerfile:18`, `docker/autopilot/Dockerfile:56`)
- Compose resource limits are likely ignored outside Swarm; `deploy.resources` does not apply to normal `docker-compose` usage. (`docker/autopilot/docker-compose.yml:29`)
- Path traversal risk in GPT-OSS apply_patch when target does not exist (non-canonicalized fallback path). (`crates/gpt-oss-agent/src/tools/apply_patch.rs:79`)

### Medium
- Autopilot daemon PID detection uses `/proc`, which is Linux-specific and can mis-detect on other platforms. (`crates/autopilot/src/bin/autopilotd.rs:124`)
- Worktree creation failures only warn; containers start anyway, so missing worktrees lead to empty mounts. (`crates/autopilot/src/parallel/worktree.rs:42`, `crates/autopilot/src/parallel/docker.rs:48`)
- Plan-mode "launch swarm" path is explicitly not implemented. (`crates/autopilot/src/planmode.rs:270`)
- TaskComplexity deserialization discards persisted data and defaults, risking misleading analytics. (`crates/autopilot/src/model_selection.rs:478`)
- GPT-OSS default model is set to `gpt-4o-mini`, which does not match GPT-OSS naming. (`crates/gpt-oss/src/client.rs:8`)
- Docker orchestration hard-depends on `docker-compose` instead of Compose v2 (`docker compose`). (`crates/autopilot/src/parallel/docker.rs:73`)
- `claude-mcp` declares a bin target but has no `src/main.rs` and is not a workspace member, so it is unbuildable and untested. (`crates/claude-mcp/Cargo.toml:10`, `Cargo.toml:3`)
- Docker image uses `autopilot` binary while crate docs say binaries are removed in favor of `openagents`, creating conflicting guidance. (`crates/autopilot/Cargo.toml:10`, `docker/autopilot/Dockerfile:62`)

### Low
- Autopilot GUI token usage is placeholder/demo data rather than computed from logs. (`crates/autopilot-gui/src/server/routes.rs:183`)
- Agent status fields are always None (current issue, uptime). (`crates/autopilot/src/parallel/docker.rs:149`)
- Wallet settings set is unimplemented; FROSTR share export is partial and non-serializable. (`crates/wallet/src/cli/settings.rs:30`, `crates/wallet/src/cli/frostr.rs:88`)
- Marketplace provider/earnings are stubbed at the unified CLI layer. (`src/cli/marketplace.rs:45`)

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

### GitAfter
- LNURL payment flow not implemented for bounty claims. (`crates/gitafter/src/server.rs:3724`)
- Restack base-layer rebase is not implemented. (`crates/gitafter/src/stacks/restack.rs:194`)

### GPT-OSS Tools
- Browser search tool is placeholder. (`crates/gpt-oss-agent/src/tools/browser.rs:134`)
- Python tool writes temp file but executes inline `python -c`. (`crates/gpt-oss-agent/src/tools/python.rs:92`)

## Testing / Verification Gaps
- Marketplace E2E tests are conceptual and avoid real relay publish/subscribe flows. (`crates/marketplace/tests/compute_e2e.rs:58`, `crates/marketplace/tests/skill_e2e.rs:1`)
- NIP-SA integration tests are missing from `crates/nostr/tests/` despite directive expectations.
- Autopilot GUI session tests skip on missing DB and do not assert content. (`crates/autopilot-gui/src/sessions.rs:146`)
- Top-level tests cover only CLI/GUI smoke tests. (`tests/cli_integration.rs`, `tests/gui_server.rs`)

## Operational / Portability Risks
- Docker Compose resource limits may be ignored in non-Swarm usage. (`docker/autopilot/docker-compose.yml:29`)
- `docker-compose` binary hard dependency in Rust orchestration. (`crates/autopilot/src/parallel/docker.rs:73`)
- `breez-sdk-spark` is a relative path dependency outside the repo; builds fail without `~/code/spark-sdk`. (`crates/spark/Cargo.toml:38`)

## Recommendations (More Comprehensive)
1. Fix parallel autopilot issue tracking: bundle a known-good `issues-mcp` binary in the image and wire `--issues-db`/`ISSUES_DB` into autopilot.
2. Remove or implement d-012 stubs across wallet/marketplace/compute/ACP; prioritize relay integration to unlock multiple directives.
3. Add SQLite WAL + busy_timeout pragmas to issues/marketplace/metrics DBs.
4. Make autopilot daemon portable or explicitly gate Unix-only code with cfgs and document platform limits.
5. Align agent options (remove `gpt-oss` or implement it) and fix doc mismatches (autopilot vs openagents usage).
6. Convert GUI DB access to non-blocking and make DB paths configurable.
