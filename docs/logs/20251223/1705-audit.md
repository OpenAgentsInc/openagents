# Audit: 2025-12-23 Codebase (cc4a00ca3..061ebac31)

## Scope
- Range reviewed: cc4a00ca3..061ebac31 (623 commits since the 2025-12-21 audit) plus current working tree at HEAD.
- Sources: `git log --stat`, targeted code inspection, and rlogs in `docs/logs/20251223/*.rlog` (notably 004836, 014205, 073739, 082302, 135621, 153640, 154822, 160758).
- Focus: ACP adapter and agent wrappers, parallel autopilot containers, local inference/GPT-OSS, autopilot metrics/APM, GUI updates, and directive compliance (d-001..d-019).

## Summary of What Happened
### Autopilot & Metrics
- Added APM tracking, baselines, and weekly report generation; expanded metrics filters and anomaly flags.
- Introduced a lightweight `issue-tool` binary and continued improvements to metrics/reporting pipeline.

### ACP + Agent Wrappers
- Added `acp-adapter` crate with Codex/Claude wrappers and rlog/JSONL conversion support.
- Added GUI routes and views for ACP session management plus log streaming in the parallel agents UI.

### Parallel Autopilot Containers
- Added `docker/autopilot/` image + compose; orchestration library and GUI page for parallel agents.
- Added HTMX/WS log streaming and toggles for rlog/jsonl/formatted views.

### Local Inference / GPT-OSS
- Added `local-inference` trait crate and `gpt-oss` client; implemented `LocalModelBackend` for fm-bridge.
- Added `gpt-oss-agent` tool support and local inference docs/benchmarks.

### Desktop/GUI & Infra
- UI refinements (parallel agents page, chat pane changes) and Docker base updates for edition 2024 builds.

## Timeline (from rlogs)
- 00:48Z (`docs/logs/20251223/004836-call-issue-ready-now-to.rlog`): repeated builds/tests for gitafter/autopilot/marketplace/compute with multiple pushes and an amend.
- 01:42Z (`docs/logs/20251223/014205-call-issue-ready-now-to.rlog`): workspace build + clippy/test sweeps and CLI integration tests.
- 07:37Z (`docs/logs/20251223/073739-call-issue-ready-now-to.rlog`): workspace clippy fixes, build/test loops; tests were terminated mid-run.
- 08:23Z (`docs/logs/20251223/082302-call-issue-ready-now-to.rlog`): frostr and gitafter test/build attempts.
- 13:56Z (`docs/logs/20251223/135621-call-issue-ready-now-to.rlog`): `cargo clean` + openagents build.
- 15:36Z (`docs/logs/20251223/153640-call-issue-ready-now-to.rlog`): local-inference build.
- 15:48Z (`docs/logs/20251223/154822-process-issues-from-database.rlog`): issue processing and pushes.
- 16:07Z (`docs/logs/20251223/160758-call-issue-ready-now-to.rlog`): issue-tool build.

## Verification Observed (from rlogs)
- Builds: `cargo build -p openagents`, `cargo build -p autopilot`, `cargo build -p gitafter`, `cargo build -p marketplace`, `cargo build -p compute`, `cargo build -p local-inference`, `cargo build -p issue-tool`.
- Tests: targeted tests for gitafter/autopilot/marketplace; workspace clippy and test invocations.
- Reliability caveat: some long test commands were killed (e.g., `cargo test --workspace --lib` and `cargo test -p gitafter --lib`), so overall test coverage is inconclusive.

## Findings / Areas for Improvement
### Process & Policy Compliance
- **Force pushes to `main`**: `git push origin main --force-with-lease` appears in `docs/logs/20251223/033937-call-issue-ready-now-to.rlog`, violating repo guidance.
- **Amend commits without explicit request**: `git commit --amend --no-edit` appears in multiple rlogs (e.g., `docs/logs/20251223/004836-call-issue-ready-now-to.rlog`, `docs/logs/20251223/033937-call-issue-ready-now-to.rlog`, `docs/logs/20251223/065318-call-issue-ready-now-to.rlog`, `docs/logs/20251223/073739-call-issue-ready-now-to.rlog`).

### d-012 No-Stubs Violations (Selected Examples)
- ACP prompt/cancel endpoints do not invoke live agent connections (`src/gui/routes/acp.rs:224`, `src/gui/routes/acp.rs:268`).
- ACP terminal creation returns an ID without actually spawning a terminal (`crates/acp-adapter/src/client.rs:227`).
- GUI routes for wallet/marketplace/gitafter return `NotImplemented` (`src/gui/routes/wallet.rs:14`, `src/gui/routes/marketplace.rs:15`, `src/gui/routes/gitafter.rs:13`).
- Wallet CLI and GUI payment flows are hard-bailed (`crates/wallet/src/cli/bitcoin.rs:5`, `crates/wallet/src/gui/server.rs:111`).
- Marketplace browse/compute relay operations return explicit "not implemented" errors (`crates/marketplace/src/skills/browse.rs:248`, `crates/marketplace/src/data/discover.rs:289`, `crates/marketplace/src/compute/consumer.rs:260`).
- Compute relay integration is stubbed (`crates/compute/src/services/relay_service.rs:82`) and Ollama service is explicitly disabled (`crates/compute/src/services/ollama_service.rs:1`).
- Autopilot context loss analysis and TaskComplexity deserialization are unimplemented (`crates/autopilot/src/context_analysis.rs:94`, `crates/autopilot/src/model_selection.rs:484`).
- Bifrost threshold ECDH aggregation is not implemented (`crates/frostr/src/bifrost/aggregator.rs:190`, `crates/frostr/src/bifrost/node.rs:556`).
- GPT-OSS browser search tool returns a placeholder response (`crates/gpt-oss-agent/src/tools/browser.rs:134`).
- GitAfter LNURL flow and restack base layer rebase are not implemented (`crates/gitafter/src/server.rs:3724`, `crates/gitafter/src/stacks/restack.rs:215`).

### Integration Risks
- Nostr relay integration is still missing across compute/marketplace/wallet, leaving core CLI and service operations unusable.
- ACP session UI reports "prompt_received" and "cancelled" without actually driving an agent connection.
- Bifrost ECDH/signing are not available; E2E tests explicitly expect failure.

### Testing Gaps
- d-014/d-015 E2E tests are incomplete: no NIP-SA integration tests in nostr, and marketplace tests are largely conceptual (see `crates/marketplace/tests/compute_e2e.rs:58` and `crates/marketplace/tests/skill_e2e.rs:1`).
- Several rlogs show test runs aborted mid-command, reducing confidence in reported verification.

## Recommendations
1. Eliminate d-012 violations by implementing or removing stub endpoints and "not yet implemented" flows, starting with wallet/marketplace relay integration and ACP prompt/cancel wiring.
2. Prioritize Nostr relay client integration and reuse across compute, marketplace, wallet, and GitAfter to unlock multiple directives at once.
3. Complete d-014/d-015 E2E test suites using in-process relays and real event flows; remove tests that only assert failure.
4. Wire ACP terminal creation and session prompts to actual subprocess execution and permission handling.
5. Enforce repo git policies: avoid `--force-with-lease` on `main` and do not amend without approval.
6. Ensure tests run to completion; treat killed test commands as failed verification.

## Overall Assessment
The codebase shows major forward progress on ACP integration, parallel autopilot tooling, and local inference infrastructure, plus meaningful additions to autopilot metrics and APM tracking. However, critical directive gaps remain, especially d-012 (no stubs) and the missing relay integration/tests that block wallet/marketplace readiness. The next tranche of work should focus on turning placeholder flows into real integrations and bringing E2E tests up to directive expectations.

## Directive-Focused Codebase Audit (Broader)
### d-001: Spark SDK Integration
Observed:
- `openagents-spark` uses `breez-sdk-spark` and provides `SparkWallet` operations; `UnifiedIdentity` integrates `SparkSigner` (`crates/spark/src/wallet.rs`, `crates/compute/src/domain/identity.rs`).
Gaps / Risks:
- Wallet CLI/GUI payment flows still hard-fail (`crates/wallet/src/cli/bitcoin.rs:5`, `crates/wallet/src/gui/server.rs:111`).
- Wallet service in compute is still commented out (`crates/compute/src/services/mod.rs:6`).

### d-002: Nostr Protocol (Client + Relay + Core)
Observed:
- Core types and events are extensive; relay/client crates exist.
Gaps / Risks:
- Relay/client integration not wired into compute/marketplace/wallet; NIP-07/NIP-47 remain stub-level error surfaces (`crates/nostr/core/src/nip07.rs:238`, `crates/nostr/core/src/nip47.rs:36`).
- No conformance suite observed.

### d-003: Wallet (Identity + Payments)
Observed:
- Wallet crate and CLI/GUI scaffolding exist, plus identity management and config.
Gaps / Risks:
- Payment and NWC flows are blocked (see `crates/wallet/src/cli/bitcoin.rs:5`).
- Contacts/relay sync is not implemented (`crates/wallet/src/cli/identity.rs:381`).
- Unified GUI routes return NotImplemented (`src/gui/routes/wallet.rs:14`).

### d-004: Autopilot Continual Improvement
Observed:
- Expanded metrics pipeline, APM support, weekly reports, anomaly flags, and dashboard updates.
Gaps / Risks:
- Context-loss analyzer stubbed (`crates/autopilot/src/context_analysis.rs:94`).
- Autopilot GUI WebSocket remains demo-only for real agent control (`crates/autopilot-gui/src/server/ws.rs`).

### d-005: GitAfter
Observed:
- GitAfter core flows and review tooling exist.
Gaps / Risks:
- GUI routes are NotImplemented (`src/gui/routes/gitafter.rs:13`).
- LNURL payment path is missing (`crates/gitafter/src/server.rs:3724`).
- Base layer restack is not implemented (`crates/gitafter/src/stacks/restack.rs:215`).

### d-006: NIP-SA Operationalization
Observed:
- NIP-SA types exist in core and are referenced in marketplace/autopilot.
Gaps / Risks:
- No observed relay publishing, scheduling triggers, or wallet-threshold integration.
- No end-to-end NIP-SA tests yet.

### d-007: FROSTR (Threshold Signatures)
Observed:
- Keygen/signing/ECDH modules and Bifrost node scaffolding exist.
Gaps / Risks:
- Threshold ECDH aggregation is not implemented (`crates/frostr/src/bifrost/aggregator.rs:190`, `crates/frostr/src/bifrost/node.rs:556`).
- E2E tests intentionally expect failure (`crates/frostr/tests/bifrost_e2e.rs:121`).

### d-008: Unified Marketplace (Compute/Skills/Data/Trajectories)
Observed:
- Compute/skills/data/trajectory modules and CLI exist.
Gaps / Risks:
- Relay integration not implemented for compute/skills/data browsing or job submission (`crates/marketplace/src/compute/consumer.rs:260`, `crates/marketplace/src/skills/browse.rs:248`, `crates/marketplace/src/data/discover.rs:289`).
- CLI actions like cancel are placeholders (`crates/marketplace/src/cli/compute.rs:516`).

### d-009: Autopilot GUI
Observed:
- WebSocket and UI scaffolding exist, plus permissions screens.
Gaps / Risks:
- WS still demo-only; permission responses not wired to real agent flows (`crates/autopilot-gui/src/server/ws.rs`).

### d-010: Unified OpenAgents Binary
Observed:
- Unified binary and CLI routing are in place.
Gaps / Risks:
- Some CLI commands still bail (marketplace provider/earnings) (`src/cli/marketplace.rs:45`).
- Unified GUI routes still use NotImplemented responses (wallet/marketplace/gitafter).

### d-011: Storybook Coverage
Observed:
- Storybook crate exists with many stories.
Gaps / Risks:
- No evidence of 100% coverage across wallet/agentgit/autopilot-gui/unified views.

### d-012: No Stubs
Observed:
- Multiple explicit not-implemented paths remain across ACP, wallet, marketplace, compute, GPT-OSS tools, and Bifrost.
Gaps / Risks:
- These violate the directive until implemented or removed.

### d-013: Comprehensive Testing Framework
Observed:
- Unit tests and snapshot tests exist in several crates.
Gaps / Risks:
- Required E2E tests are missing or conceptual; integration coverage remains thin.

### d-014: NIP-SA and Bifrost E2E Tests
Observed:
- `crates/frostr/tests/bifrost_e2e.rs` exists with relay-backed scaffolding.
Gaps / Risks:
- NIP-SA integration tests in `crates/nostr/tests/` are missing.
- Current tests expect failures due to missing aggregation.

### d-015: Marketplace and Agent Commerce E2E Tests
Observed:
- `crates/marketplace/tests/compute_e2e.rs` and `crates/marketplace/tests/skill_e2e.rs` exist.
Gaps / Risks:
- Tests do not actually publish/subscribe over relays; many phases (data, trajectory, commerce, payments) are missing.

### d-016: Actions Per Minute (APM)
Observed:
- APM calculation, storage, backfill, and CLI/dashboards implemented in autopilot.
Gaps / Risks:
- Autopilot-gui/HUD integration and live updates are not clearly wired.

### d-017: Agent Client Protocol (ACP)
Observed:
- ACP adapter crate, Codex/Claude wrappers, GUI session routes, and rlog converters exist.
Gaps / Risks:
- Prompt/cancel endpoints are TODO; terminal spawning is stubbed (`crates/acp-adapter/src/client.rs:227`).
- Permission UI handling remains deferred.

### d-018: Parallel Autopilot Container Isolation
Observed:
- Docker image/compose, orchestration library, CLI script, GUI page, and log streaming exist.
Gaps / Risks:
- Status reporting lacks uptime/current-issue details (`crates/autopilot/src/parallel/docker.rs:153`).

### d-019: GPT-OSS Local Inference Integration
Observed:
- Local inference trait + GPT-OSS client + agent wrapper implemented; fm-bridge uses `LocalModelBackend`.
Gaps / Risks:
- No integration into unified CLI/GUI/autopilot flows; browser search tool still placeholder (`crates/gpt-oss-agent/src/tools/browser.rs:134`).

## Additional Codebase Observations (Non-Directive)
- Ollama integration is explicitly disabled in compute (`crates/compute/src/services/ollama_service.rs:1`).
- Relay publishing/subscription services in compute are placeholders pending Nostr client integration.

## Updated Recommendations (Directive-Oriented)
1. d-001/d-003/d-008: Wire Nostr relay + Spark wallet integrations end-to-end so wallet and marketplace flows stop hard-failing.
2. d-014/d-015: Implement real relay-based E2E tests (nostr integration + marketplace commerce) and remove tests expecting failure.
3. d-017: Complete ACP prompt/cancel and terminal handling, and integrate permission handling in UI.
4. d-019: Integrate GPT-OSS into unified CLI/GUI selection and wire tool support beyond placeholders.
5. d-012: Eliminate remaining "not implemented" paths or remove the routes/commands until ready.
