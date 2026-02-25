# OpenAgents Comprehensive Codebase Audit

Status: comprehensive snapshot
Date: 2026-02-25
Owner: repo audit (Codex)

## What this audit covers

This document provides a full codebase audit focused on:

1. Current architecture reality vs declared architecture direction.
2. Recent directional shifts (local-first Autopilot, NIP-90 marketplace loop, Hydra/Aegis posture).
3. Code health (complexity concentration, compatibility debt, stubs, lint posture).
4. Priority-ordered cleanup and implementation recommendations.

This audit is code-first: if docs and code disagree, code is treated as ground truth and docs are marked for correction.

## Method

Evidence was collected from active repository surfaces on 2026-02-25 using direct code and repo scans.

Primary authority references:

1. `docs/adr/INDEX.md`
2. `docs/plans/rust-migration-invariant-gates.md`
3. `docs/core/ARCHITECTURE.md`
4. `docs/core/ROADMAP.md`
5. `docs/core/PROJECT_OVERVIEW.md`

Primary code surfaces sampled:

1. `apps/openagents.com/service/src/lib.rs`
2. `apps/openagents.com/service/src/runtime_routing.rs`
3. `apps/openagents.com/service/src/route_split.rs`
4. `apps/runtime/src/server.rs`
5. `apps/autopilot-desktop/src/main.rs`
6. `crates/autopilot-core/src/*`
7. `crates/nostr/client/src/dvm.rs`
8. `crates/spark/src/*`

## Executive summary

OpenAgents has real momentum in the Rust runtime/control direction, local desktop capability, NIP-90 compute pathways, and Hydra implementation. But the current repository still contains significant architectural contradiction and cleanup debt that blocks a clean, defensible "single topology" story.

Top conclusions:

1. Rust service foundations are strong, but repository state is not truly Rust-only.
2. Control and runtime boundaries are still blurred in API ownership and compatibility lanes.
3. `apps/openagents.com/service/src/lib.rs`, `apps/runtime/src/server.rs`, and `apps/autopilot-desktop/src/main.rs` are monolith hotspots.
4. Hydra has substantial implementation presence; Aegis is still architecture/plan-only (no active code namespace).
5. NIP-90 and Spark integrations are substantial and product-relevant, but desktop orchestration is too centralized in one file.
6. Policy/gate posture and actual code posture are misaligned in a few places (especially legacy lanes and workflow artifacts).

## Snapshot metrics (2026-02-25)

### Workspace and code volume

- Workspace members: `51`
- Rust files under `apps/`: `96`
- Rust files under `crates/`: `1183`
- Rust LOC under `apps/`: `112,857`
- Rust LOC under `crates/`: `432,569`
- Total Rust LOC tracked: `545,426`

### App Rust LOC concentration

- `apps/openagents.com`: `51,443` (45.6% of app Rust LOC)
- `apps/runtime`: `45,810` (40.6%)
- `apps/autopilot-desktop`: `8,217` (7.3%)
- `apps/lightning-wallet-executor`: `5,700` (5.1%)
- `apps/lightning-ops`: `1,687` (1.5%)

### Largest single Rust files

1. `apps/openagents.com/service/src/lib.rs` (`16,741` LOC)
2. `apps/openagents.com/service/src/tests.rs` (`11,381` LOC)
3. `apps/runtime/src/server/tests.rs` (`7,961` LOC)
4. `apps/autopilot-desktop/src/main.rs` (`6,662` LOC)
5. `apps/runtime/src/server.rs` (`6,507` LOC)

### API and compatibility pressure

- Route registrations in control service + runtime: `190`
- Runtime service routes in `apps/runtime/src/server.rs`: `57`
- `route_split` references in control service source: `180`
- Legacy chat alias references in control service source: `63`

### Quality markers

- `TODO|FIXME|HACK|XXX|TBD` in `apps/*/src` + `crates/*/src`: `42`
- `todo!()` / `unimplemented!()` in `apps/*/src` + `crates/*/src`: `0`
- `#[allow(...)]` usage in `apps` + `crates`: `194`
- `#[test]` count in `apps` + `crates`: `4357`
- `#[tokio::test]` count in `apps` + `crates`: `618`

### Multi-language reality

Tracked LOC by extension:

- Rust (`.rs`): `545,426`
- PHP (`.php`): `36,790`
- TS/TSX: `34,649`
- Shell (`.sh`): `7,161`

`apps/openagents.com` legacy lane (`app/`, `resources/`, `routes/`, `database/`, `tests/`) totals `57,128` LOC, which is larger than the Rust control service (`51,443` LOC).

## Directional shifts: what changed and where reality stands

### 1) Product identity shift

Direction has shifted to:

1. Autopilot as a personal agent.
2. Extensibility through a marketplace.
3. Two-sided compute loop (consume compute + provide compute).

Current fit:

1. Desktop has concrete NIP-90 and provider hooks.
2. Nostr client/DVM client implementation is substantial.
3. Runtime marketplace and credit/liquidity APIs exist.

Gap:

1. The distributed story exists across code, but it is operationally fragmented across monolith files and mixed ownership routes.

### 2) Local-first with optional shared runtime/nexus/swarm

Direction has shifted to local-first execution with optional shared runtime support.

Current fit:

1. Desktop contains local identity, Codex orchestration, NIP-90 submission paths, and Spark wallet interactions.
2. Runtime and control remain deeply integrated for auth, sync, and worker/event lanes.

Gap:

1. Clear local authoritative loop boundaries are still not explicit enough in module boundaries or route ownership taxonomy.

### 3) Hydra and Aegis inclusion

Current fit:

1. Hydra has broad runtime/control integration (FX, routing, credit/liquidity observability).
2. Hydra endpoints and telemetry are implemented under runtime internal APIs.

Gap:

1. Aegis is present in architecture/plans but has no active source references in `apps/*/src` or `crates/*/src`.
2. This creates a planning-to-implementation asymmetry that needs explicit phase ownership.

## Architecture fitness review

### Strengths

1. Rust control and runtime services are real and substantial.
2. Proto-first contracts are in place with dedicated workspace support.
3. No production-path `todo!`/`unimplemented!` stubs were found in Rust source.
4. Test volume is large and includes strong service-level coverage.
5. Lightning and wallet execution boundaries exist as separate app surfaces.

### Major architectural tensions

1. Declared topology vs repository reality diverges:
- Docs describe legacy web lanes as archival/non-retained.
- Repo still tracks large active legacy PHP/TS surfaces under `apps/openagents.com`.

2. Invariant conflict risk (`INV-12`):
- Tracked workflow automation exists at:
  - `apps/openagents.com/.github/workflows/lint.yml`
  - `apps/openagents.com/.github/workflows/tests.yml`

3. Boundary ambiguity in control/runtime lanes:
- Control service still owns many runtime-flavored endpoints and compatibility aliases.
- Runtime ownership is partially explicit, but route organization remains mixed.

## Surface-by-surface audit

### `apps/openagents.com`

Current state:

1. Rust control service is active and central.
2. Large retained legacy lane is still tracked (`app/`, `resources/`, `routes/`, `database/`, `tests/`).
3. Service router is very broad and includes many non-minimal surfaces (chat aliases, feed/shouts/whispers, route split controls, compatibility routes).

Risks:

1. Ownership confusion (what is legacy compatibility vs canonical authority).
2. Operational overhead from maintaining dual paradigms in one app root.
3. Increased accidental regression surface for every control service change.

### `apps/runtime`

Current state:

1. Runtime service carries execution authority, marketplace, liquidity/credit, Hydra FX/routing/risk, verification and treasury lanes.
2. Router and handler concentration in `server.rs` is high.

Risks:

1. Large-file concentration makes it hard to enforce strict domain boundaries.
2. Cross-domain changes are easier to introduce accidentally.

### `apps/autopilot-desktop`

Current state:

1. Contains runtime auth/sync, Codex runtime orchestration, NIP-90 provider/consumer interactions, NIP-06 identity handling, and Spark wallet flow.
2. Functionality breadth is high but heavily centralized in `main.rs`.

Risks:

1. Main-file coupling reduces maintainability and testability for pane-specific behavior.
2. Local-first ambitions are implemented, but layering is too coarse for long-term velocity.

### `apps/lightning-wallet-executor` and `apps/lightning-ops`

Current state:

1. Boundaries exist and are meaningful.
2. Spark-related execution paths are implemented in wallet executor.

Risks:

1. Policy and ownership clarity must stay explicit as Hydra/Aegis orchestration deepens.

### Crate ecosystem

Current state:

1. Strong reusable substrate exists (`wgpui`, `autopilot`, `autopilot-core`, `nostr`, `compute`, `pylon`, `runtime`).
2. NIP-90 foundation is broad and mature.

Risks:

1. `wgpui` and `autopilot` ecosystem size concentration requires deliberate modular discipline.
2. Lint suppression density suggests quality gates are not yet aligned with declared strictness.

## Code smell and risk register

### Critical

1. Repository architecture contradiction (Rust-only narrative vs large tracked legacy web lane).
2. In-repo workflow automation present despite invariant forbidding workflow files.

### High

1. Control service monolith (`lib.rs` size + broad route table).
2. Runtime server monolith (`server.rs` + broad internal API concentration).
3. Desktop monolith (`main.rs` owns many product domains simultaneously).
4. Compatibility/legacy lane persistence (`route_split`, legacy aliases, old driver names) still deeply embedded.

### Medium

1. Aegis implementation gap (plan-level concept without active source namespace).
2. Lint policy mismatch (`allow_attributes = "deny"` vs large `#[allow(...)]` footprint).
3. Hardcoded endpoint branding/defaults still appear in parts of the code/docs (`openagents.com`, nexus defaults), reducing portability.

### Low

1. Debt-marker counts are moderate but mostly comments/fixtures/strings, not production stubs.
2. Test volume is high, but concentration in giant test files can obscure intent and slow targeted maintenance.

## Priority recommendations

### Phase 0 (immediate, 1-2 weeks): make architecture truth explicit

1. Decide and document one truth for `apps/openagents.com` legacy lane:
- Either archive non-Rust lanes to backroom now.
- Or explicitly reclassify them as an active compatibility product lane with ownership and sunset date.

2. Resolve invariant conflict on workflow files:
- Remove/migrate tracked `.github/workflows` artifacts from the repo scope if `INV-12` remains authoritative.

3. Publish a control/runtime route ownership matrix in a single canonical doc, with per-route owner and deprecation target.

4. Freeze new compatibility aliases and route split expansion pending ownership cleanup.

### Phase 1 (near-term, 2-6 weeks): reduce monolith risk

1. Split `apps/openagents.com/service/src/lib.rs` into domain routers plus composition layer.
2. Split `apps/runtime/src/server.rs` by domain (`runs`, `workers`, `marketplace`, `hydra`, `credit/liquidity`, `treasury`, `verification`).
3. Split `apps/autopilot-desktop/src/main.rs` into pane/domain modules (identity, provider, wallet, Codex runtime sync).

Target: no single production Rust file over ~3k LOC without explicit exception rationale.

### Phase 2 (product-direction hardening, 4-8 weeks)

1. Formalize local-first Autopilot execution contract:
- Local Codex authority path when available.
- Explicit fallback hierarchy (local -> shared runtime -> swarm).

2. Normalize endpoint configuration:
- Remove hardcoded production URL assumptions from runtime-facing client code paths.

3. Continue NIP-90 two-sided loop hardening:
- Provider enrollment UX/state boundaries.
- Consumer dispatch policy boundaries.
- Deterministic receipt and replay links.

### Phase 3 (economy and trust coherence, 6-12 weeks)

1. Define minimal Aegis implementation footprint in code and proto:
- Verification/underwriting receipt schema namespace.
- Runtime endpoints and policy wiring.

2. Keep Hydra boundary explicit:
- Internal (`/internal/v1/*`) vs externalized API posture decision and documentation.

3. Maintain Hydra/Aegis compatibility without expanding speculative surfaces before core marketplace loop is stable.

### Phase 4 (quality gate alignment)

1. Reconcile lint policy with practice:
- Either reduce `#[allow(...)]` counts to match policy.
- Or narrow policy to realistic enforceable rules.

2. Add measurable debt budgets:
- Route count budget per service.
- `#[allow(...)]` net-growth budget.
- Monolith file size guardrails.

3. Update `scripts/local-ci.sh` trigger matrix to match actual retained surfaces and remove stale legacy references if lanes are archived.

## What to consolidate/remove/archive first

If the goal is maximum cleanup with minimum architecture risk, do this order:

1. Resolve legacy web lane status under `apps/openagents.com` and archive/move inactive paths.
2. Remove in-repo workflow files that violate invariant policy.
3. Collapse compatibility routing layers (`route_split`, legacy aliases) where no retained client contract depends on them.
4. Break up the three monolith files (`service/lib.rs`, `runtime/server.rs`, `autopilot-desktop/main.rs`).

## Residual risks if no action is taken

1. Continued docs-code contradiction will reduce confidence in architecture docs and ADR governance.
2. Compatibility lane sprawl will keep complexity high and slow product-direction execution.
3. Monolith hotspots will continue to absorb delivery time and raise regression risk.
4. Aegis will remain a plan-only concept, weakening the stated Hydra+Aegis architecture narrative.

## Bottom line

OpenAgents has strong foundations in Rust runtime/control, local desktop capability, NIP-90 integration, and Hydra execution. The main blocker is not missing capability; it is architectural coherence and cleanup discipline across retained vs legacy surfaces.

The highest-leverage move is to reconcile declared architecture with repository reality immediately, then decompose the three monolith hotspots while preserving the local-first + NIP-90 two-sided marketplace path.

## GitHub Implementation Issue Sequence (Created 2026-02-25)

1. **[#2212 OA-AUDIT Phase 0: Delete all legacy PHP/TS/non-retained code surfaces](https://github.com/OpenAgentsInc/openagents/issues/2212)**
Delete all non-retained legacy code from this repository, explicitly including PHP and TS/TSX legacy web lanes, archive only what must be preserved to backroom, and update canonical docs so retained topology is fully truthful.

2. **[#2213 OA-AUDIT Phase 0: Remove in-repo workflow automation and enforce INV-12](https://github.com/OpenAgentsInc/openagents/issues/2213)**
Remove tracked `.github/workflows` automation from repository scope, migrate required checks to canonical local scripts/runbooks, and close the architecture-vs-invariant conflict flagged in the audit.

3. **[#2214 OA-AUDIT Phase 1: Hard control/runtime route ownership and boundary map](https://github.com/OpenAgentsInc/openagents/issues/2214)**
Publish and enforce one authoritative endpoint ownership matrix for control vs runtime lanes, remove mixed ownership ambiguity, and freeze additional compatibility expansion during boundary hardening.

4. **[#2215 OA-AUDIT Phase 1: Decompose openagents.com/service lib.rs monolith](https://github.com/OpenAgentsInc/openagents/issues/2215)**
Refactor control service routing and handlers into modular domain components while preserving retained API behavior and tests, reducing monolith blast radius and ownership confusion.

5. **[#2216 OA-AUDIT Phase 1: Decompose runtime server monolith by domain](https://github.com/OpenAgentsInc/openagents/issues/2216)**
Split runtime server composition by execution/economy domains (runs, workers, marketplace, treasury, verifications, Hydra/liquidity/credit, ops) with no replay/idempotency regressions.

6. **[#2217 OA-AUDIT Phase 1: Decompose autopilot-desktop main.rs monolith](https://github.com/OpenAgentsInc/openagents/issues/2217)**
Modularize desktop app architecture into explicit identity, provider, wallet, codex-sync, and pane orchestration boundaries while keeping startup and product behavior stable.

7. **[#2218 OA-AUDIT Phase 2: Implement explicit local-first execution contract and endpoint portability](https://github.com/OpenAgentsInc/openagents/issues/2218)**
Make local execution authority explicit (local Codex first, then shared runtime/swarm fallback), and remove hardcoded production endpoint assumptions through config-driven transport resolution.

8. **[#2219 OA-AUDIT Phase 2: Complete EP212 wallet/L402/paywall production parity](https://github.com/OpenAgentsInc/openagents/issues/2219)**
Replace remaining synthetic wallet/L402 behaviors with real executor-backed custody-compliant flows, complete Rust-native L402 tooling, and deliver self-serve paywall earnings with deterministic receipts.

9. **[#2220 OA-AUDIT Phase 3: Implement Aegis runtime MVP namespace and contracts](https://github.com/OpenAgentsInc/openagents/issues/2220)**
Move Aegis from plan-only posture to implemented runtime lanes with proto-governed contracts, deterministic receipts, and minimal verification/underwriting primitives integrated into existing authority boundaries.

10. **[#2221 OA-AUDIT Phase 4: Align lint policy and reduce allow-attribute debt](https://github.com/OpenAgentsInc/openagents/issues/2221)**
Reconcile lint policy with real enforcement, remove unjustified `#[allow(...)]` suppressions in critical crates, and establish durable debt accounting for any remaining justified exceptions.

11. **[#2222 OA-AUDIT Phase 4: Add architecture debt budgets and no-net-growth gates](https://github.com/OpenAgentsInc/openagents/issues/2222)**
Introduce measurable complexity budgets (route counts, production file size, suppression growth) and enforce no-net-growth gates through local CI scripts and documented exception workflow.

12. **[#2223 OA-AUDIT Phase 5: Retire compatibility lanes and finalize cleanup signoff](https://github.com/OpenAgentsInc/openagents/issues/2223)**
Remove legacy compatibility lanes (route split/alias debt) after parity evidence, clean `local-ci` trigger residue, and produce final architecture consistency signoff across docs and code.

## Follow-on Adapter-Decoupling Sequence (Completed 2026-02-25)

Post-audit, the primitive hierarchy and adapter decoupling lane was implemented through the following closed issues:

1. **[#2224 Enforce primitive hierarchy: DB-native sync core, Git/GitHub as non-blocking adapters](https://github.com/OpenAgentsInc/openagents/issues/2224)**  
Parent tracking issue closed after all children completed.

2. **[#2225 Decouple issue execution core from inline GitHub branch/PR workflow](https://github.com/OpenAgentsInc/openagents/issues/2225)**  
Core issue progression now queues adapter exports instead of inline branch/PR mutation.

3. **[#2226 Refactor session fork primitive to be git-agnostic](https://github.com/OpenAgentsInc/openagents/issues/2226)**  
Session fork UX/commands now define a snapshot/timeline primitive independent of git; git export is separate.

4. **[#2227 Make git diagnostics optional adapter telemetry in execution/review lanes](https://github.com/OpenAgentsInc/openagents/issues/2227)**  
Execution/review output now separates `core_session_state` from `integration.git` and remains complete without git availability.

5. **[#2228 Split preflight into core readiness vs non-blocking git/github integration readiness](https://github.com/OpenAgentsInc/openagents/issues/2228)**  
Preflight now models core readiness and integration readiness as separate lanes; git/github checks are warnings/capabilities, not core blockers.

6. **[#2229 Implement replayable, idempotent Git/GitHub export queue](https://github.com/OpenAgentsInc/openagents/issues/2229)**  
Export intents are durably persisted with deterministic idempotency keys, checkpoint watermarks, and outage/restart replay recovery.

7. **[#2230 Make desktop git panel/runtime a pluggable integration capability](https://github.com/OpenAgentsInc/openagents/issues/2230)**  
Git runtime/panel is now explicitly optional (feature + env capability), while core session/chat/editor paths remain operational when disabled.
