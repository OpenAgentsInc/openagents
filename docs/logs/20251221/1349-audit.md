# Audit: 2025-12-21 Commit Set (7165a72c0..cc4a00ca3)

## Scope
- Range reviewed: 7165a72c0..cc4a00ca3 (28 commits pulled on 2025-12-21).
- Sources: `git log --stat`, commit diffs, and rlogs in `docs/logs/20251221/*.rlog` (notably 104956, 110013, 113711, 122134, 125257, 130835).
- Focus: marketplace compute/skills/data/trajectories, autopilot-gui, and autopilot profiling/dashboard documentation.

## Summary of What Happened
### Marketplace (Compute)
- Implemented NIP-90 event types and job lifecycle support; added compute CLI entry point and provider discovery.
- Added job submission + streaming flow, provider capability advertisement, compute provider CLI, and job status/history tracking (SQLite-backed).
- Relevant commits: `caeecde8c`, `2c1654268`, `2b3eb069a`, `a489c39cf`, `5ba07b191`, `f03341c46`, `4b415ae93`, `257f08ad7`, `1a95e5057`, `d74af323c`.

### Marketplace (Skills/Data)
- Added NIP-SA skill license wrappers, skills browse/search, skills CLI, data discovery, data CLI, and relay integration (NIP-89).
- Relevant commits: `d126b32ef`, `86fb6cb4e`, `30499b7e5`, `bdd57648e`, `2a25ebc4c`, `49064af44`.

### Marketplace (Trajectories)
- Built the trajectory pipeline (collect/anonymize/redact/validate/rewards/contribute) and CLI.
- Added comprehensive secret redaction patterns + tests; scan/preview CLI; contribution upload to relays; enhanced collector with git commit detection + CI/CD signal parsing.
- Added trajectory contribution documentation to marketplace README.
- Relevant commits: `dedaa6412`, `8752e824d`, `7663f22b1`, `816574027`, `b482dcccb`, `b4ad2d521`.

### Autopilot-GUI
- Created new `autopilot-gui` crate (wry/tao + Actix), added WebSocket chat streaming, tool execution display, and permission management (SQLite-backed with UI + WS wiring).
- Relevant commits: `0a432dc47`, `5cb4af900`, `bf5e27445`, `f9d37b43c`.

### Autopilot Docs / Profiling / Dashboard
- Added benchmark suite documentation and task docs.
- Added profiling docs + `.gitignore` entries for flamegraphs.
- Added dashboard modular structure plan and a `DashboardState` stub for future refactor.
- Relevant commits: `fd3d12ef1`, `31c90a768`, `cc4a00ca3`.

## Timeline (from rlogs)
- 16:49Z (`docs/logs/20251221/104956-call-issue-ready-now-to.rlog`): NIP-90 event types implementation; tests run; unused import fix follow-up. Log ends mid-commit (incomplete).
- 17:00Z (`docs/logs/20251221/110013-call-issue-ready-now-to.rlog`): Compute CLI, provider discovery, provider CLI, job submission/streaming, job history DB; tests run but some commands failed due to bad working directory.
- 17:37Z (`docs/logs/20251221/113711-call-issue-ready-now-to.rlog`): Skills/data marketplace modules + CLI, relay integration, autopilot-gui crate + WebSocket + tool display; commit amend + force-push events.
- 18:21Z (`docs/logs/20251221/122134-call-issue-ready-now-to.rlog`): Trajectory pipeline scaffolding + CLI, benchmark docs, permission UI; dashboard streaming work blocked due to monolithic `dashboard.rs`.
- 18:52Z (`docs/logs/20251221/125257-call-issue-ready-now-to.rlog`): Secret redaction patterns; scan/preview CLI; push-protection triggered by test token -> amended commit.
- 19:08Z (`docs/logs/20251221/130835-call-issue-ready-now-to.rlog`): Trajectory upload to relays + collector enhancements; tests run; log ends mid-command (incomplete).
- `31c90a768` and `cc4a00ca3` are not referenced in the 20251221 rlogs; likely manual commits or missing logs.

## Verification Observed (from rlogs)
- Marketplace compute/CLI:
  - `cargo test -p marketplace --lib compute::events...`
  - `cargo test --lib consumer`
  - `cargo test --lib cli::compute`
  - `cargo test -p marketplace --lib` (473/478/486 tests reported).
- Marketplace trajectories:
  - `cargo test --package marketplace --lib trajectories::redact`
  - `cargo test --package marketplace --lib trajectories::anonymize`
  - `cargo test --lib trajectories::contribute`.
- Autopilot-gui:
  - `cargo build -p autopilot-gui` (multiple times, with warnings fixed).
- Note: Several commands reported `Error: Directory not found` (e.g., `cd crates/marketplace` or `cd crates/autopilot-gui`), yet were treated as successful in the narrative. This reduces confidence in reported test/build coverage.

## Findings / Areas for Improvement
### Process & Policy Compliance
- **Force pushes to `main`**: `git push origin main --force-with-lease` was used (e.g., in `docs/logs/20251221/113711-call-issue-ready-now-to.rlog`). This violates repo guidance to never force-push to `main`.
- **Commit amendments without explicit request**: multiple `git commit --amend --no-edit` occurrences (e.g., `docs/logs/20251221/113711-call-issue-ready-now-to.rlog`, `docs/logs/20251221/122134-call-issue-ready-now-to.rlog`). This conflicts with instructions to avoid amend unless explicitly requested.
- **Push protection triggered by realistic tokens**: push rejected by GH secret scanning; fixed by editing tests (`docs/logs/20251221/125257-call-issue-ready-now-to.rlog`). This indicates test fixtures should avoid real token patterns.
- **Style guard triggered (`border-radius`)**: permission UI initially introduced `border-radius: 0` and was forced to remove it. This shows style guideline enforcement is working but should be pre-checked.

### Reliability of Reported Test Results
- Repeated `Error: Directory not found` outputs mean some `cargo check`/`cargo test` runs likely did not execute despite “success” statements. This introduces risk that changes landed without confirmed builds.

### Logging Gaps
- `docs/logs/20251221/104956-call-issue-ready-now-to.rlog` and `docs/logs/20251221/130835-call-issue-ready-now-to.rlog` terminate mid-command. Incomplete logs reduce traceability for those commits.

### Implementation Risks
- **Autopilot-GUI permissions**: No tests were added for permission storage/matching; concurrency and data integrity rely on `Arc<Mutex<Connection>>` and `spawn_blocking` without coverage.
- **Relay integration / trajectory upload**: Networked code added without integration tests or relay mocks. Runtime behavior may differ by relay and network conditions.
- **Trajectory quality scoring**: Quality heuristics (token counts, commit detection, CI parsing) may be brittle; no integration tests with real logs are recorded.
- **Dashboard refactor**: `cc4a00ca3` adds a plan + state stub, but the actual refactor is deferred; streaming issue #417 remains blocked per logs.

## Recommendations
1. **Enforce git policies**: avoid `--force-with-lease` on `main`; prefer follow-up commits instead of amends. If amends are necessary, require explicit approval and a protected branch.
2. **Fix working-directory errors**: use absolute paths or `--package` flags rather than `cd` within commands; treat any “Directory not found” as a failing step.
3. **Add coverage where risk is highest**:
   - Autopilot-gui permissions (storage + WS request/response handling).
   - Relay integration (mocked relays or integration tests in CI).
   - Trajectory collector parsing (golden-file tests using real rlogs).
4. **Avoid real-looking secrets in fixtures**: use clearly fake tokens that do not trigger push protection (e.g., `ghp_FAKE123...` with invalid length/prefix).
5. **Log completeness**: ensure rlog sessions flush/close with a terminal “end” marker; add validation checks for partial logs.
6. **Clarify profiling “support”**: docs are helpful, but consider adding a CLI flag or `cargo autopilot profile` helper to align the commit message with actual capability.

## Overall Assessment
The commit set is feature-heavy and generally coherent, especially around marketplace compute/skills/data/trajectory workflows and the autopilot-gui foundation. However, several process issues (force pushes, unapproved amend commits, ambiguous test results, and incomplete logs) materially reduce audit confidence. Addressing the recommendations above would improve reliability, traceability, and adherence to repo policies.

## Directive-Focused Codebase Audit (Broader)
This section expands beyond the 2025-12-21 commit range and reviews current code against the directives in `.openagents/directives/`. Evidence references point to code as it exists now.

### d-001: Spark SDK Integration
Observed:
- `crates/spark/` exists with signer, wallet types, and documented Phase 1 completion (`crates/spark/src/lib.rs`).
- Spark signer is wired into UnifiedIdentity (`crates/compute/src/domain/identity.rs`).
Gaps / Risks:
- Wallet operations are stubbed with TODOs (balance, sync, address) (`crates/spark/src/wallet.rs`).
- No Breez/Spark SDK dependency is wired yet (`crates/spark/Cargo.toml`).
- Wallet CLI shows stub warnings and TODOs (`crates/wallet/src/cli/bitcoin.rs`).

### d-002: Nostr Protocol (Client + Relay + Core)
Observed:
- Core module exports extensive NIP coverage (NIP-01..NIP-99 + NIP-SA) (`crates/nostr/core/src/lib.rs`).
- Client and relay crates exist with documented architecture and public APIs (`crates/nostr/client/src/lib.rs`, `crates/nostr/relay/src/lib.rs`).
Gaps / Risks:
- No conformance test suite or end-to-end NIP coverage validation observed; implementation completeness per NIP is not verified.
- Relay/client integration tests and interop checks are not visible in this audit.

### d-003: Wallet (Identity + Payments)
Observed:
- `crates/wallet/` includes CLI, GUI, core identity, and storage modules (`crates/wallet/src/`).
Gaps / Risks:
- Payment flows are stubbed (Spark balance, send, receive, history) (`crates/wallet/src/cli/bitcoin.rs`, `crates/wallet/src/gui/server.rs`).
- Contact list management, NIP-57 zaps, NWC, and relay sync are marked TODO (`crates/wallet/src/cli/identity.rs`, `crates/wallet/src/cli/bitcoin.rs`).
- Keychain, persistence, and full identity synchronization are not fully implemented.

### d-004: Autopilot Continual Improvement
Observed:
- Metrics pipeline, baselines, and analysis tools exist (`crates/autopilot/src/metrics/mod.rs`, `crates/autopilot/src/analyze.rs`, `crates/autopilot/src/metrics/baseline.rs`).
- CLI commands cover metrics import/show/analyze and dashboard start (`crates/autopilot/src/main.rs`).
- Dashboard exists with API endpoints and a WebSocket endpoint (`crates/autopilot/src/dashboard.rs`).
- Benchmarks and tasks are implemented with docs (`crates/autopilot/src/benchmark/`, `docs/autopilot/benchmarks/README.md`).
Gaps / Risks:
- Dashboard WebSocket is currently an echo/demo stream, not real-time metrics push (`crates/autopilot/src/dashboard.rs`).
- Modular refactor plan exists but monolithic dashboard still in use (`crates/autopilot/src/dashboard/README.md`).
- Automated improvement loop exists in code, but integration into CI or daemon workflows is not verified.

### d-005: AgentGit
Observed:
- Full desktop app scaffold exists (wry/tao + Actix), Nostr client and NIP-34 event builders, stacked PR metadata support, and UI views (`crates/agentgit/src/main.rs`, `crates/agentgit/src/nostr/events.rs`, `crates/agentgit/src/views.rs`).
Gaps / Risks:
- Payment/bounty settlement (NIP-57) and wallet integration are not evidenced in code paths reviewed.
- Trajectory verification and enforcement appears partial; UI references exist but full verification flow is not validated in this audit.

### d-006: NIP-SA Operationalization
Observed:
- NIP-SA types implemented in core (profile, state, schedule, goals, tick, trajectory, skill) (`crates/nostr/core/src/nip_sa/`).
- Autopilot includes Nostr tick and trajectory mapping utilities (`crates/autopilot/src/nostr_agent.rs`, `crates/autopilot/src/nip_sa_trajectory.rs`).
- Marketplace includes skill license wrappers (`crates/marketplace/src/skills/license.rs`).
Gaps / Risks:
- Wallet integration for sovereign identity and threshold signing is not wired.
- Autopilot does not appear to publish tick/trajectory events to relays yet.
- State encryption/agent scheduling triggers are not observed.

### d-007: FROSTR (Threshold Signatures)
Observed:
- `crates/frostr/` includes keygen, signing, ECDH, credential encoding, and Bifrost protocol modules (`crates/frostr/src/lib.rs`).
Gaps / Risks:
- Peer health/ping workflows are TODO (`crates/frostr/src/bifrost/peer.rs`).
- Wallet/agent integration and Nostr transport wiring are not evidenced in this audit.

### d-008: Unified Marketplace (Compute/Skills/Data/Trajectories)
Observed:
- Core marketplace CLI and modules exist for compute, skills, data, trajectories, plus relay integration (`crates/marketplace/src/cli/`, `crates/marketplace/src/compute/`, `crates/marketplace/src/skills/`, `crates/marketplace/src/data/`, `crates/marketplace/src/relay.rs`, `crates/marketplace/src/trajectories/`).
Gaps / Risks:
- Many network operations are TODO (fetching from relays, publishing job events) (`crates/marketplace/src/data/discover.rs`, `crates/marketplace/src/skills/browse.rs`, `crates/marketplace/src/compute/consumer.rs`).
- Payment flow (Lightning) and revenue splits are not implemented in reviewed paths.
- GUI exists in tree but not assessed here for feature completeness (`crates/marketplace/src/gui`, `crates/marketplace/src/views`).

### d-009: Autopilot GUI
Observed:
- `crates/autopilot-gui/` provides a wry/tao window, Actix server, WebSocket chat, tool call UI, and permission storage (`crates/autopilot-gui/src/`).
Gaps / Risks:
- WebSocket handler is demo-only (echo + simulated tool calls) and does not integrate Claude Agent SDK yet (`crates/autopilot-gui/src/server/ws.rs`).
- Permission responses are not wired to a handler channel (`crates/autopilot-gui/src/server/ws.rs`).
- Session management, context inspector, token gauges, and real agent control are not implemented.

## Additional Codebase Observations (Non-Directive)
- `crates/compute/src/services/ollama_service.rs` is explicitly stubbed; DVM publish flow has TODOs (`crates/compute/src/services/dvm_service.rs`).
- Desktop replay duration uses a TODO placeholder (`crates/desktop/src/replay.rs`).
- Claude/Codex SDK crates contain minor TODOs (e.g., query pattern matching) (`crates/claude-agent-sdk/src/query.rs`).

## Updated Recommendations (Directive-Oriented)
1. **d-001/d-003**: Replace Spark stubs with Breez SDK wiring, and explicitly gate CLI commands until real balances are available.
2. **d-002**: Add conformance/integration tests that exercise relay + client across a subset of NIPs, then expand coverage.
3. **d-004**: Replace dashboard WebSocket echo with live metrics updates; complete the planned refactor or split into modules to unblock streaming.
4. **d-006/d-007**: Wire FROSTR into wallet + NIP-SA signing flows; add relay transport tests for Bifrost.
5. **d-008**: Prioritize relay publish/fetch in compute/skills/data and add a minimal Lightning payment flow before GUI work.
6. **d-009**: Integrate Claude Agent SDK session lifecycle and permission callbacks before expanding UI features.
