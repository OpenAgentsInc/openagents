# Audit (Comprehensive): 2025-12-23 Codebase

## Scope
- Range reviewed: cc4a00ca3..061ebac31 (623 commits) plus current HEAD state.
- Areas deep-dived: ACP adapter, parallel autopilot containers, GPT-OSS/local inference, wallet/marketplace/compute integration, and database/concurrency.
- Sources: code inspection, pattern search, and rlogs in `docs/logs/20251223/*.rlog`.

## Method
- Searched for stubs and TODO paths across `src/` and `crates/`.
- Reviewed Docker + orchestration assets for operational correctness.
- Checked data layer configs for concurrency readiness.
- Cross-checked tests for actual end-to-end coverage.

## Findings (Severity Ordered)
### Critical
- **d-012 violations block core flows**: wallet/marketplace/ACP/compute still return NotImplemented or hard error responses, preventing real functionality in default CLI/GUI paths (`src/gui/routes/wallet.rs:13`, `src/gui/routes/marketplace.rs:14`, `src/gui/routes/gitafter.rs:12`, `crates/wallet/src/cli/bitcoin.rs:5`, `crates/marketplace/src/compute/consumer.rs:259`, `crates/compute/src/services/relay_service.rs:81`, `src/gui/routes/acp.rs:224`).
- **Bifrost ECDH/signing still not implemented**; tests explicitly assert failure, meaning core sovereign agent cryptography is not working end-to-end (`crates/frostr/src/bifrost/aggregator.rs:183`, `crates/frostr/src/bifrost/node.rs:548`, `crates/frostr/tests/bifrost_e2e.rs:116`).

### High
- **Parallel autopilot resource limits likely not enforced**: `deploy.resources` is ignored by non-Swarm Docker Compose, so CPU/memory caps may not apply in normal usage (`docker/autopilot/docker-compose.yml:29`).
- **Healthcheck will fail**: container runs `pgrep` but `procps` is not installed, so healthcheck always fails and marks containers unhealthy (`docker/autopilot/Dockerfile:56`).
- **ACP UI actions are non-functional**: prompt/cancel endpoints only update UI state and do not drive actual agent execution, so ACP sessions are effectively fake from the GUI layer (`src/gui/routes/acp.rs:224`, `src/gui/routes/acp.rs:268`).
- **SQLite concurrency not configured for parallel containers**: issue DBs only enable `foreign_keys`; WAL/busy_timeout not set in code, risking "database is locked" under concurrent agent writes (`crates/issues/src/db.rs:20`, `crates/marketplace/src/db.rs:14`, `crates/autopilot/src/metrics/mod.rs:380`).

### Medium
- **Path traversal risk in GPT-OSS apply_patch tool**: when target file does not exist, canonicalization falls back to a non-normalized path, allowing `../` traversal to pass the workspace prefix check (`crates/gpt-oss-agent/src/tools/apply_patch.rs:83`).
- **Docker orchestration assumes `docker-compose` binary**; environments with only `docker compose` will fail (`crates/autopilot/src/parallel/docker.rs:72`).
- **Autopilot GUI uses fixed `autopilot.db` path** and ignores `ISSUES_DB`/`.openagents` conventions, so UI may show stale or empty issue lists (`crates/autopilot-gui/src/server/parallel.rs:242`).
- **Docker image installs Claude CLI via `curl | bash` and ignores failure**, leaving containers running without an agent binary if the script fails or URL changes (`docker/autopilot/Dockerfile:41`).
- **Local inference default model mismatch**: GPT-OSS client defaults to `gpt-4o-mini`, which is not a GPT-OSS model; default is not used by any request path, so config drift is likely (`crates/gpt-oss/src/client.rs:8`).

### Low
- **Parallel cleanup is destructive without confirmation**: worktree removal uses `--force` and branch deletion uses `-D` in both library and script (`crates/autopilot/src/parallel/worktree.rs:116`, `scripts/parallel-autopilot.sh:177`).
- **GPT-OSS Python tool writes unused temp files**, then executes inline `python -c`, wasting I/O and limiting large payloads (`crates/gpt-oss-agent/src/tools/python.rs:92`).
- **Dashboard/session tests are non-assertive**: tests skip if DB missing and do not validate expected data, reducing coverage value (`crates/autopilot-gui/src/sessions.rs:146`).
- **Compose docs reference AGENT_COUNT** but the compose file does not use it; only the helper script handles agent count (`docker/autopilot/docker-compose.yml:3`).

## Comprehensive Stub Inventory (Selected)
### ACP / Agent Integration
- GUI prompt/cancel endpoints are TODOs (`src/gui/routes/acp.rs:224`, `src/gui/routes/acp.rs:268`).
- Terminal creation returns ID only; no PTY spawn (`crates/acp-adapter/src/client.rs:227`).

### Wallet
- CLI payment flows hard-fail (balance, send, invoice, pay, history) (`crates/wallet/src/cli/bitcoin.rs:5`).
- GUI send/receive/history return placeholder pages (`crates/wallet/src/gui/server.rs:106`).
- Contacts management errors due to missing relay integration (`crates/wallet/src/cli/identity.rs:380`).

### Marketplace / Compute
- Job submission and relay interactions not implemented (`crates/marketplace/src/compute/consumer.rs:259`, `crates/compute/src/services/relay_service.rs:81`).
- Skills/data browsing return explicit "not implemented" errors (`crates/marketplace/src/skills/browse.rs:248`, `crates/marketplace/src/data/discover.rs:289`).
- Compute cancel command is a placeholder (`crates/marketplace/src/cli/compute.rs:516`).
- Provider earnings/state are placeholder outputs (`crates/marketplace/src/cli/provider.rs:276`).

### Autopilot / Metrics
- TaskComplexity deserialization panics at runtime (`crates/autopilot/src/model_selection.rs:477`).
- Context loss analyzer returns empty results (`crates/autopilot/src/context_analysis.rs:94`).

### Bifrost
- Threshold ECDH aggregation not implemented (`crates/frostr/src/bifrost/aggregator.rs:183`).
- Bifrost node ECDH returns error by design (`crates/frostr/src/bifrost/node.rs:548`).

### GPT-OSS Tools
- Browser search is a placeholder response (`crates/gpt-oss-agent/src/tools/browser.rs:134`).

### GitAfter
- LNURL flow not implemented (`crates/gitafter/src/server.rs:3724`).
- Restack base-layer rebase not implemented (`crates/gitafter/src/stacks/restack.rs:215`).

## Testing Coverage Gaps
- Marketplace E2E tests are conceptual and avoid relay publish/subscribe flows (`crates/marketplace/tests/compute_e2e.rs:58`, `crates/marketplace/tests/skill_e2e.rs:1`).
- NIP-SA integration tests in `crates/nostr/tests/` are missing (no `nip_sa.rs`, no `e2e_agent.rs`).
- Root `tests/` contains only CLI/GUI integration files; no `tests/e2e/` directory exists despite d-013 requirements.

## Operational / Portability Risks
- Docker compose resource limits may be ineffective outside Swarm (`docker/autopilot/docker-compose.yml:29`).
- Healthcheck will fail due to missing `pgrep` binary (`docker/autopilot/Dockerfile:56`).
- `docker-compose` binary hard dependency in Rust orchestration (`crates/autopilot/src/parallel/docker.rs:72`).
- `breez-sdk-spark` is a relative path dependency outside the repo; builds will fail unless `~/code/spark-sdk` exists (`crates/spark/Cargo.toml:38`).

## Process Observations (from rlogs)
- Multiple rlogs show force pushes and commit amendments (`docs/logs/20251223/033937-call-issue-ready-now-to.rlog`).
- Several test runs were killed mid-command or executed with wrong paths, reducing confidence in verification coverage (`docs/logs/20251223/073739-call-issue-ready-now-to.rlog`, `docs/logs/20251223/004836-call-issue-ready-now-to.rlog`).

## Recommendations (Comprehensive)
1. Eliminate d-012 violations by wiring ACP prompt/cancel, wallet payment flows, and marketplace/compute relay integration.
2. Update container orchestration to enforce resource limits (Compose v2 `mem_limit`/`cpus`) and fix healthcheck tooling.
3. Add WAL + busy timeout to SQLite initialization for concurrent agent workflows.
4. Harden GPT-OSS tools: fix apply_patch path validation and align GPT-OSS default model and API semantics.
5. Replace conceptual E2E tests with real relay-backed flows; add missing NIP-SA tests under `crates/nostr/tests/`.
6. Align docs with actual behavior (AGENT_COUNT note, docker-compose availability, known-good binary usage).

