# Audit Addendum (Deep Dive): 2025-12-23 Codebase

## Scope
- Range reviewed: cc4a00ca3..061ebac31 (623 commits) plus current HEAD.
- Deep-dive focus: ACP adapter, parallel autopilot containers, GPT-OSS/local inference, marketplace and wallet flows, and d-012 compliance.
- Sources: targeted file review, directive checks, and rlogs in `docs/logs/20251223/*.rlog`.

## Method
- Searched for stub patterns and not-implemented flows across crates and GUI routes.
- Reviewed new container/orchestration assets for operational risks.
- Checked local inference and GPT-OSS integration for API correctness and defaults.
- Cross-checked rlogs for failed commands or terminated test runs.

## Key Risks (Severity Ordered)
### High
- d-012 violations are still present across core user flows (wallet/marketplace/ACP), meaning main GUI/CLI paths return NotImplemented or explicit errors instead of working behavior. This blocks most end-to-end functionality and contradicts the directive.
- Parallel autopilot resource limits are likely not enforced because Docker Compose `deploy.resources` is ignored outside Swarm mode, risking host overload when multiple agents run (`docker/autopilot/docker-compose.yml:29`).
- Docker-based autopilot images install the agent CLI via `curl | bash` and ignore failures, so containers may start without the CLI they need to operate (`docker/autopilot/Dockerfile:41`).

### Medium
- GPT-OSS client defaults to model `gpt-4o-mini` which is not a GPT-OSS model; default model is unused by API calls, which can lead to misconfiguration or silent mismatch between config and requests (`crates/gpt-oss/src/client.rs:8`).
- Autopilot parallel worktree cleanup and script `cleanup` both use forced worktree removal and branch deletion without confirmation; this is risky in shared environments (`crates/autopilot/src/parallel/worktree.rs:116`, `scripts/parallel-autopilot.sh:177`).
- Healthcheck uses `pgrep` but `procps` is not installed in the container, so healthcheck will always fail and mark containers unhealthy (`docker/autopilot/Dockerfile:56`).

### Low
- GPT-OSS python tool writes code to a temp file that is never used, then passes the code inline to `python -c`. This wastes I/O and risks command length issues with large prompts (`crates/gpt-oss-agent/src/tools/python.rs:92`).
- Several rlog runs show path errors and killed test commands, reducing confidence in the reported verification (`docs/logs/20251223/135621-call-issue-ready-now-to.rlog`, `docs/logs/20251223/073739-call-issue-ready-now-to.rlog`).

## d-012 No-Stubs Inventory (Expanded)
- ACP UI actions do not invoke real agent connections: prompt and cancel are TODOs (`src/gui/routes/acp.rs:224`, `src/gui/routes/acp.rs:268`).
- ACP terminal creation returns an ID without spawning a terminal (`crates/acp-adapter/src/client.rs:227`).
- Unified GUI routes are NotImplemented (wallet, marketplace, GitAfter) (`src/gui/routes/wallet.rs:13`, `src/gui/routes/marketplace.rs:14`, `src/gui/routes/gitafter.rs:12`).
- Wallet payment CLI is a hard failure for all core actions (`crates/wallet/src/cli/bitcoin.rs:5`).
- Wallet GUI send/receive/history return service-unavailable placeholders (`crates/wallet/src/gui/server.rs:106`).
- Marketplace compute consumer returns explicit errors for job submission (`crates/marketplace/src/compute/consumer.rs:259`).
- Marketplace browse/fetch stubs for skills and data (`crates/marketplace/src/skills/browse.rs:248`, `crates/marketplace/src/data/discover.rs:289`).
- Compute relay publish/subscribe is stubbed (`crates/compute/src/services/relay_service.rs:81`).
- Ollama service explicitly disabled (`crates/compute/src/services/ollama_service.rs:1`).
- TaskComplexity deserialization is `unimplemented!()` and will panic if deserialized (`crates/autopilot/src/model_selection.rs:477`).
- Context loss analyzer returns empty results with TODO (`crates/autopilot/src/context_analysis.rs:94`).
- Bifrost ECDH aggregation is unimplemented (`crates/frostr/src/bifrost/aggregator.rs:183`, `crates/frostr/src/bifrost/node.rs:548`).
- GPT-OSS browser search returns placeholder response (`crates/gpt-oss-agent/src/tools/browser.rs:134`).
- GitAfter LNURL flow and restack base layer rebase are not implemented (`crates/gitafter/src/server.rs:3724`, `crates/gitafter/src/stacks/restack.rs:215`).
- Marketplace provider status and earnings flows are explicitly "not yet implemented" (`crates/marketplace/src/cli/provider.rs:276`, `crates/marketplace/src/cli/provider.rs:292`).
- Marketplace compute cancel command prints "not yet implemented" (`crates/marketplace/src/cli/compute.rs:516`).
- Unified CLI marketplace provider/earnings commands bail (`src/cli/marketplace.rs:45`).
- Wallet contacts management errors due to missing relay integration (`crates/wallet/src/cli/identity.rs:380`).

## Parallel Autopilot Container Findings
- Compose resource limits are specified under `deploy.resources`, which is ignored by non-Swarm Docker Compose; containers may run without CPU/memory caps (`docker/autopilot/docker-compose.yml:29`).
- Healthcheck relies on `pgrep` but the image does not install `procps`, so the check will fail and mark containers unhealthy (`docker/autopilot/Dockerfile:56`).
- Dockerfile installs Claude CLI with `curl | bash` and ignores failures; if the installer fails or URL changes, autopilot will run without a working CLI (`docker/autopilot/Dockerfile:41`).
- Orchestration uses `docker-compose` binary directly; environments with only `docker compose` will fail (`crates/autopilot/src/parallel/docker.rs:62`).
- `remove_worktrees` and `parallel-autopilot.sh cleanup` use `--force` and `-D` without confirmation, risking data loss for agent branches (`crates/autopilot/src/parallel/worktree.rs:116`, `scripts/parallel-autopilot.sh:177`).

## GPT-OSS / Local Inference Findings
- Default GPT-OSS model is `gpt-4o-mini`, not a GPT-OSS model; also default_model is never used by API calls (`crates/gpt-oss/src/client.rs:8`).
- Python tool creates a temp file then ignores it, passing code inline to `python -c`, which can fail for large inputs (`crates/gpt-oss-agent/src/tools/python.rs:92`).
- Browser search tool is still a stub and returns a placeholder string (`crates/gpt-oss-agent/src/tools/browser.rs:134`).
- GPT-OSS integration exists but is not wired into the unified CLI/GUI or autopilot selection flows, so it is not usable end-to-end yet (no bindings found in `src/cli` or GUI routes).

## ACP Integration Findings
- ACP UI endpoints for prompt/cancel do not call the ACP connection layer; the session updates are UI-only and do not drive agent execution (`src/gui/routes/acp.rs:224`, `src/gui/routes/acp.rs:268`).
- Terminal creation in ACP client is a stub and does not actually spawn commands (`crates/acp-adapter/src/client.rs:227`).

## Testing Gaps (Deeper)
- Marketplace E2E tests are mostly conceptual and do not run relay flows. `compute_e2e` explicitly notes missing relay logic (`crates/marketplace/tests/compute_e2e.rs:58`).
- Skill E2E test is a series of unit checks and a documented flow, not a real end-to-end test (`crates/marketplace/tests/skill_e2e.rs:1`).
- Bifrost E2E tests assert failures because key aggregation is unimplemented, so tests do not validate success paths (`crates/frostr/tests/bifrost_e2e.rs:116`).
- rlogs show a pattern of killed test commands and path errors, so reported verification coverage is weak (`docs/logs/20251223/073739-call-issue-ready-now-to.rlog`, `docs/logs/20251223/004836-call-issue-ready-now-to.rlog`).

## Process Compliance (Additional Detail)
- `git commit --amend --no-edit` and force pushes appear in 20251223 rlogs, violating repo policy to avoid amendments/force pushes unless explicitly requested (`docs/logs/20251223/033937-call-issue-ready-now-to.rlog`).

## Actionable Recommendations (Deep)
1. Replace stubbed GUI/CLI endpoints with real integrations or remove them until ready (d-012 compliance), starting with wallet/marketplace/ACP.
2. Fix container reliability: install `procps` or change the healthcheck, replace `deploy.resources` with Compose v2-compatible limits, and avoid `curl | bash` for critical tooling.
3. Wire GPT-OSS into unified CLI/GUI and fix default model selection; remove placeholder search or implement it.
4. Replace E2E tests that assert failure with real relay-backed success paths for d-014/d-015.
5. Adjust parallel cleanup flows to require explicit confirmation or a dry-run mode to avoid data loss.

