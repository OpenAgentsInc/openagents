# OpenAgents Open-Agent Economy Execution Plan (Liquidity-First)

Status: Active draft (timeline-free)
Date: 2026-02-23
Author: Codex
Primary context sources:
- `docs/SYNTHESIS.md`
- `docs/local/hehehe.md`
- `docs/transcripts/194-the-trillion-dollar-question.md`
- `docs/transcripts/199-introducing-autopilot.md`
- `docs/transcripts/200-the-agent-network.md`
- `docs/transcripts/201-fracking-apple-silicon.md`
- `docs/transcripts/202-recursive-language-models.md`
- `docs/transcripts/203-pylon-and-nexus.md`
- `docs/transcripts/207-your-keys.md`

## 1) Purpose

This plan converts the full system model in `docs/SYNTHESIS.md` into a single execution roadmap where each major concept has a concrete GitHub issue definition.

This plan is intentionally timeline-free.

- No day/week/month estimates.
- Sequencing is dependency-based, not date-based.
- Release gates are capability and verification based.

## 2) Scope

In scope:
- End-to-end system architecture: identity, transport, treasury, exchange, sovereign protocol, wallet, marketplace, autopilot, orchestration, UI, QA, threat model, decentralization migration, and go-to-market rails.
- Full issue catalog with names and implementation scope for each backlog item.
- Synthesis concept coverage traceability.

Out of scope:
- Calendar commitments.
- Non-actionable narrative content without implementation artifact linkage.

## 3) Non-Negotiable Constraints

- Proto-first contracts (`INV-01`, `ADR-0002`).
- Authenticated HTTP mutation authority (`INV-02`).
- Khala WS-only live authority lanes (`INV-03`, `ADR-0003`).
- Control/runtime authority isolation (`INV-04`, `INV-05`, `INV-06`).
- Replay/idempotency guarantees (`INV-07`).
- Service deploy isolation and migration discipline (`INV-08`, `INV-09`, `INV-10`).
- Rust-first architecture (`ADR-0001`) and iOS WGPUI UI ownership (`INV-11`).
- No `.github` workflow automation in repo (`INV-12`).
- Wallet executor auth, custody, and receipt canonicalization (`ADR-0006`).

### 3.1) Authority And Drift Notes

- `docs/SYNTHESIS.md` is a synthesis document, not an execution authority. If it conflicts with code or Rust-era canonical docs, the Rust-era authorities win (ADRs + invariant gates + code).
- Part Fourteen of `docs/SYNTHESIS.md` (“Directive System”) is stale for the Rust-era repo: `.openagents/DIRECTIVES.md` is archived and Rust-era governance is via `docs/adr/` and `docs/plans/active/rust-migration-invariant-gates.md`.

## 4) Priority Ladder (Liquidity-First)

We are prioritizing marketplace liquidity. Autopilot is the guaranteed buyer. OpenAgents Compute is the fastest supply path.

Execution priority (highest first):

1. Autopilot coding agent + OpenAgents Compute liquidity bootstrap.
2. Compute marketplace minimal viable commerce: catalog -> discovery -> routing -> objective verification -> pay-after-verify.
3. Budget + receipts + idempotency baseline (only what (1) and (2) require).
4. Then: treasury/exchange depth, sovereign protocol breadth, skills/data/coalitions expansion, decentralization hardening, ecosystem scale.

## 5) Synthesis Coverage Map

| Synthesis Scope | Implementation Coverage | Issue Range | Priority |
|---|---|---|---|
| Part One: Cryptographic Foundation | FROSTR/FROST, key hierarchy, threshold ops, rotation/recovery, cryptographic receipts | `OA-ECON-010` to `OA-ECON-024` | Now (minimal subset) / Next (full) |
| Part Two: Communication Substrate | Nostr/NIP stack, relay strategy, WS live lanes, protocol conformance | `OA-ECON-025` to `OA-ECON-039` | Next |
| Part Three: Economic Layer | Spark/Lightning rails, unified key derivation, payment policy, value flow primitives | `OA-ECON-040` to `OA-ECON-065` | Next (minimal) / Later (depth) |
| Neobank Treasury Layer | TreasuryRouter, multi-currency budgets, mint trust, quote/proof lifecycle, reconciliation | `OA-ECON-040` to `OA-ECON-065` | Later |
| Exchange Layer | RFQ/orderflow, NIP-69/NIP-60/NIP-61 stack, settlement v0/v1/v2, liquidity/reputation | `OA-ECON-066` to `OA-ECON-089` | Later |
| Part Four: Sovereign Agent Protocol | NIP-SA lifecycle, tick/trajectory events, agent capability publication | `OA-ECON-090` to `OA-ECON-099` | Later |
| Part Five: Unified Wallet Application | CLI/WGPUI wallet parity, NIP-47, account management, recovery | `OA-ECON-100` to `OA-ECON-119` | Next (minimal) / Later (breadth) |
| Part Six: Agent Git Platform | Optional bonus surface moved out of this plan | `docs/plans/optional/gitafter-bonus.md` | Optional (moved out) |
| Part Seven: Unified Marketplace | Compute liquidity lane now; skills/data/coalitions later | `OA-ECON-120` to `OA-ECON-169` | Now (compute) / Later (skills/data/coalitions) |
| Part Eight: Autonomous Operation | Autopilot wedge + procurement + proof artifacts; maturity later | `OA-ECON-170` to `OA-ECON-189` | Now (wedge + proof) / Next (maturity) |
| Part Nine: Multi-Agent Orchestration | Orchestrator/sub-agent lifecycle, autonomy graduation, budget controls | `OA-ECON-190` to `OA-ECON-199` | Next |
| Part Ten: Tools to Entities | Entity continuity, economic alignment controls, accountability and liability rails | `OA-ECON-210` to `OA-ECON-214` | Later |
| Part Eleven: UI Architecture | WGPUI command surfaces, HUD, fleet control, mobile remote ops | `OA-ECON-171` to `OA-ECON-173`, `OA-ECON-201` to `OA-ECON-205`, `OA-ECON-214` | Next |
| Part Twelve: Quality Assurance | No-stubs enforcement, layered tests, e2e scenarios, coverage gates | `OA-ECON-220` to `OA-ECON-227`, `OA-ECON-257` to `OA-ECON-259` | Next |
| Part Thirteen: Implementation Architecture | Crate boundary governance, protocol boundary checks, architecture conformance | `OA-ECON-240` to `OA-ECON-243`, `OA-ECON-263` to `OA-ECON-265` | Later |
| Part Fourteen: Directive System (stale) | Superseded by ADRs + invariant gates; no directive system work is planned | `docs/adr/*`, `docs/plans/active/rust-migration-invariant-gates.md` | N/A (superseded) |
| Part Fifteen: Emergent Whole | Integration-level acceptance and replayable proof of coherent operation | `OA-ECON-257` to `OA-ECON-262` | Next |
| Part Sixteen: Company and Mission | Revenue rails, packaging, procurement/audit requirements | `OA-ECON-249` to `OA-ECON-256`, `OA-ECON-266` to `OA-ECON-279` | Later |
| Part Seventeen: Wedge to Platform | Demand-floor progression, platformization sequencing and metrics | `OA-ECON-001` to `OA-ECON-009`, `OA-ECON-206` to `OA-ECON-209` | Now |
| Part Eighteen: Intentional Centralization | Signer optionality, multi-signer migration, relay diversification, decentralization scorecards | `OA-ECON-233` to `OA-ECON-239` | Later |
| Part Nineteen: When Things Break | Liability, bonds, disputes, guardian market, recovery drills | `OA-ECON-228` to `OA-ECON-232`, `OA-ECON-270` to `OA-ECON-276` | Later |
| Part Twenty: Threat Model | Threat-to-control mapping, trust-boundary tests, incident drills | `OA-ECON-227` to `OA-ECON-232`, `OA-ECON-261`, `OA-ECON-272`, `OA-ECON-275` | Later |
| Part Twenty-One: End-to-End Vignettes | Wedge flow, marketplace flow, treasury+FX flow as executable acceptance suites | `OA-ECON-257` to `OA-ECON-259` | Next |

Notes:

- The "Neobank Treasury Layer" is a programmable treasury router for agents (not a regulated bank). It is called out separately because multi-rail routing, quotes, and reconciliation become the dominant failure mode once the system expands beyond a single settlement rail. It is sequenced "Later" because liquidity bootstrap only needs minimal, reliable settlement + budget enforcement for the Autopilot -> OpenAgents Compute wedge.

## 6) Sequencing Model (No Duration Estimates)

0. Liquidity Bootstrap (Autopilot coding agent + OpenAgents Compute).
1. Compute Marketplace MVP (discovery + routing + verification + pay-after-verify).
2. Authority baseline (receipts, budgets, idempotency, minimal keys) only as required by (0) and (1).
3. Autopilot reliability + orchestration maturity.
4. Treasury + exchange expansion (deeper liquidity, FX, trust).
5. Sovereign protocol / wallet expansion.
6. Coalitions / skills / data markets expansion.
7. QA + threat + decentralization migration.
8. Commercialization / governance / ecosystem scale.

## 7) Comprehensive GitHub Issue Catalog

### Phase 0: Liquidity Bootstrap — Autopilot Coding Agent on OpenAgents Compute

Autopilot is the guaranteed buyer. OpenAgents Compute is the fastest path to supply-side liquidity.

- `OA-ECON-001` - Define parity contract and verified patch artifact schema. - Create authoritative success contract for wedge execution with replayable evidence.
- `OA-ECON-002` - Instrument wedge baseline metrics. - Add telemetry for leverage, quality, cost, and completion across autonomous loops.
- `OA-ECON-003` - Harden autonomous issue intake and execution loop. - Ensure claim, execute, verify, and close cycle is resilient under retries and crashes.
- `OA-ECON-004` - Ship Verified Patch Bundle pipeline. - Emit signed action logs, diffs, tests, and receipts for each autonomous run.
- `OA-ECON-005` - Implement Autopilot demand-floor compute procurement. - Route autopilot workload as guaranteed buyer demand for marketplace providers.
- `OA-ECON-006` - Enforce org/repo/issue budget reservation pipeline. - Add hierarchical reservation and settlement for every compute purchase.
- `OA-ECON-007` - Add routing modes for user packaging. - Implement Cheapest/Balanced/Fastest policy paths without exposing market complexity to end users.
- `OA-ECON-008` - Build first-buyer settlement ledger. - Track autopilot revenue-to-compute spend conversion with idempotent settlement records.
- `OA-ECON-009` - Build wedge PMF scorecard. - Deliver operational dashboards for value proof and conversion-to-platform readiness.

- `OA-ECON-170` - Harden asynchronous autopilot executor. - Improve unattended reliability for long-running issue workflows.
- `OA-ECON-179` - Complete REPLAY.jsonl v1 emission and replay parity. - Standardize replay format and validator behavior.

- `OA-ECON-187` - Integrate budget enforcer with tool execution hooks. - Block over-limit execution in real time.
- `OA-ECON-207` - Implement hierarchical budget enforcement for org/repo/issue. - Maintain hard spend limits across nested scopes.

- `OA-ECON-120` - Implement marketplace core catalog service. - Unify listing, discovery, and metadata contracts.
- `OA-ECON-121` - Implement provider announcements with NIP-89. - Standardize provider capability publication and discovery.
- `OA-ECON-122` - Implement job-type registry with verification metadata. - Define objective vs subjective verification semantics per job.
- `OA-ECON-123` - Implement SandboxRun objective verification pipeline. - Verify build/test/lint workloads deterministically.
- `OA-ECON-128` - Implement reserve provider pool manager. - Guarantee fill path when market liquidity is insufficient.
- `OA-ECON-129` - Implement provider qualification suite. - Gate new providers with capability and health validation.
- `OA-ECON-131` - Implement supply class taxonomy in routing layer. - Distinguish SingleNode, LocalCluster (OpenAgents Compute multi-device local clusters), BundleRack, InstanceMarket, ReservePool.
- `OA-ECON-132` - Implement OpenAgents Compute local-cluster provider support. - Support multi-device local clusters presented as one market supplier (fastest supply-side liquidity path).
- `OA-ECON-134` - Implement topology-aware routing. - Route by interconnect, throughput, and stability characteristics.
- `OA-ECON-135` - Implement cost/reliability policy optimizer. - Balance spend, latency, and success probability.
- `OA-ECON-136` - Implement pay-after-verify settlement for compute jobs. - Release payment only after verification pass.
- `OA-ECON-167` - Build market telemetry and liquidity dashboards. - Expose fill, latency, churn, and breadth metrics.

### Phase 1: Compute Marketplace MVP

- `OA-ECON-124` - Implement RepoIndex objective verification pipeline. - Verify index artifact correctness and consistency.
- `OA-ECON-125` - Implement subjective inference tiering engine. - Route inference by risk and verification cost.
- `OA-ECON-126` - Implement best-of-N and adjudication pipeline. - Add consensus-based safeguards for subjective workloads.
- `OA-ECON-127` - Implement human QA sampling lane. - Add calibrated manual validation for quality drift detection.
- `OA-ECON-130` - Implement provider tiering and penalty automation. - Apply quota and routing changes from quality signals.
- `OA-ECON-133` - Implement BundleRack and InstanceMarket adapters. - Integrate datacenter/rented capacity as supply classes.
- `OA-ECON-137` - Implement pricing bands and staged bidding controls. - Move from fixed pricing to market bidding safely.

### Phase 2: Authority Baseline (As Required for Phases 0-1)

Minimal baseline required for liquidity bootstrap:

- `OA-ECON-014` - Implement unified seed derivation service. - Derive Nostr and Bitcoin lane keys from BIP39 with domain separation.
- `OA-ECON-022` - Implement canonical receipt hash and signature pipeline. - Ensure receipt signing is deterministic and replay-verifiable.
- `OA-ECON-023` - Add cross-lane verifier hooks. - Verify every contract-critical mutation against active key graph authority.
- `OA-ECON-049` - Add idempotent payment command handling. - Guarantee safe retries without duplicate spending.
- `OA-ECON-054` - Implement cryptographic treasury receipts. - Link settlement evidence to trajectory, policy, and signer attestations.
- `OA-ECON-055` - Implement reconciliation worker loops. - Resolve pending states and recover from process/network faults.

Next (full authority baseline expansion):

- `OA-ECON-010` - Implement FROSTR key ceremony service. - Support distributed key generation and signer enrollment with auditable transcripts.
- `OA-ECON-011` - Implement threshold ECDH lane. - Add threshold decryption support for encrypted protocol payloads.
- `OA-ECON-012` - Implement Bifrost coordination over Nostr relays. - Coordinate threshold rounds with retry, timeout, and membership handling.
- `OA-ECON-013` - Ship threshold policy DSL and signer role model. - Encode guardian, marketplace signer, and agent-share authority rules.
- `OA-ECON-015` - Add identity compartment derivation and proof linking. - Support public/private personas with optional cryptographic linkage.
- `OA-ECON-016` - Implement rotation and delegation chains. - Preserve identity continuity during key rotation and signer changes.
- `OA-ECON-017` - Implement guardian/key-split custody hooks. - Enforce policy-gated cosigning for high-risk operations.
- `OA-ECON-018` - Add signer heartbeat and deadman recovery trigger. - Detect signer unavailability and enable recovery path without state loss.
- `OA-ECON-019` - Implement hot-wallet sweep guardrails. - Enforce operating balance limits and programmable sweep policies.
- `OA-ECON-020` - Build key compromise drill framework. - Run repeatable rotation/revocation/recovery exercises against staging and local stacks.
- `OA-ECON-021` - Expose identity proof and attestation APIs. - Provide verifiable identity/economic-lane linkage endpoints.
- `OA-ECON-024` - Publish cryptographic operations runbook and threat controls. - Document and automate key lifecycle operations.

- `OA-ECON-025` - Build prioritized NIP coverage plan in code. - Implement and gate required NIP set for commerce-grade operation.
- `OA-ECON-026` - Implement resilient multi-relay client strategy. - Support fan-out subscriptions, dedupe, and reconnection behavior.
- `OA-ECON-027` - Ship operator relay package. - Provide hardened relay deployment with persistence and observability.
- `OA-ECON-028` - Add protocol compatibility validator. - Enforce schema and version compatibility at relay/client boundaries.
- `OA-ECON-029` - Enforce WS-only live authority stream lanes. - Ensure Khala and live sync traffic remains WebSocket-authoritative.
- `OA-ECON-030` - Integrate NIP-57 zap payment flows. - Link event-driven payouts to verified work and reputation evidence.
- `OA-ECON-031` - Integrate L402 pay-per-call rails. - Add HTTP payment challenge and settlement support for API lanes.
- `OA-ECON-032` - Implement encrypted direct coordination lanes. - Support secure peer communication for sensitive agent workflows.
- `OA-ECON-033` - Implement NIP-34 git event primitives. - Add decentralized code collaboration protocol events.
- `OA-ECON-034` - Implement NIP-90 job request/result authority flows. - Standardize compute market contracts over Nostr events.
- `OA-ECON-035` - Implement NIP-69 order event integration. - Adopt existing p2p order format for exchange interoperability.
- `OA-ECON-036` - Implement NIP-60 wallet sync and state lanes. - Persist wallet state and transaction history over relay events.
- `OA-ECON-037` - Implement NIP-87 mint discovery trust ingest. - Ingest mint recommendations and announcements into routing policy.
- `OA-ECON-038` - Implement NIP-32 reputation label graph. - Aggregate trust labels for routing and market controls.
- `OA-ECON-039` - Build protocol conformance and replay test suite. - Verify protocol correctness across client, relay, and runtime components.

### Phase 3: Autopilot Reliability + Orchestration Maturity

- `OA-ECON-171` - Build WGPUI fleet command dashboard. - Provide native operator control center for multi-agent execution.
- `OA-ECON-172` - Build mobile companion approval and status surface. - Support remote operator intervention and review.
- `OA-ECON-173` - Implement policy-aware permission UX flows. - Make approval/reject/override actions explicit and auditable.
- `OA-ECON-174` - Implement APM telemetry service. - Measure action velocity as a first-class performance metric.
- `OA-ECON-175` - Pair APM with success and rework metrics. - Prevent speed-only optimization and track quality-adjusted throughput.
- `OA-ECON-176` - Implement canary rollout controller for agent configs. - Gate prompt/tool/model changes through staged rollout.
- `OA-ECON-177` - Implement known-good fallback and semantic rollback. - Recover automatically from degraded agent revisions.
- `OA-ECON-178` - Implement improvement-opportunity miner. - Generate ranked remediation targets from trajectory analytics.
- `OA-ECON-180` - Integrate DSPy compiler pipeline surfaces. - Bind compiled modules into runtime decision paths.
- `OA-ECON-181` - Implement complexity pipeline optimization loop. - Continuously improve task complexity classification.
- `OA-ECON-182` - Implement delegation pipeline optimization loop. - Improve backend/sub-agent delegation decisions.
- `OA-ECON-183` - Implement RLM trigger optimization loop. - Improve recursive fanout trigger quality and cost control.
- `OA-ECON-184` - Implement auto-optimizer orchestration service. - Trigger retraining/recompilation from performance deltas.
- `OA-ECON-185` - Implement model/backend routing policy engine. - Route workloads by risk, capability, and economics.
- `OA-ECON-186` - Implement multi-backend cost arbitrage controls. - Minimize spend while preserving quality/SLO targets.
- `OA-ECON-188` - Implement parallel container runtime management. - Scale isolated autonomous workers safely.
- `OA-ECON-189` - Harden issue claim/expiry contention behavior. - Eliminate deadlocks and duplicate claim races.
- `OA-ECON-190` - Implement orchestrator-scoped assignment enforcement. - Bound agent edits to assigned module scope.
- `OA-ECON-191` - Implement scope lock and conflict detection service. - Reject out-of-scope mutations before merge.
- `OA-ECON-192` - Implement autonomy-level policy engine. - Enforce supervised/semi/full autonomy with explicit controls.
- `OA-ECON-193` - Implement autonomy graduation suggestion engine. - Suggest trust level transitions from observed performance.
- `OA-ECON-194` - Implement per-action autonomy matrix. - Allow risk-specific autonomy policy by action type.
- `OA-ECON-195` - Implement agent-requested escalation API. - Allow self-escalation when confidence or risk thresholds trip.
- `OA-ECON-196` - Implement sub-agent lifecycle manager. - Manage spawned specialist agent runs under one orchestrator session.
- `OA-ECON-197` - Complete ACP adapter parity. - Support editor-to-agent JSON-RPC lanes with replay compatibility.
- `OA-ECON-198` - Harden Codex app-server protocol integration. - Ensure robust tool, event, and persistence semantics.
- `OA-ECON-199` - Complete local backend parity lane. - Support on-device/open-weight backends through common runtime interfaces.
- `OA-ECON-200` - Finalize external provider adapter boundary policy. - Keep adapter lanes bounded and migration-safe.
- `OA-ECON-201` - Implement fleet leaderboard and performance overlays. - Surface operator-relevant comparative performance views.
- `OA-ECON-202` - Implement earnings HUD and minimap visuals. - Show economic and operational state in one command surface.
- `OA-ECON-203` - Implement control groups and hotkey workflows. - Enable rapid fleet operations from native UI.
- `OA-ECON-204` - Implement escalation and notification router. - Route approvals/incidents across desktop/mobile surfaces.
- `OA-ECON-205` - Define and enforce autopilot reliability SLOs. - Set service-level behavior targets for autonomous lanes.
- `OA-ECON-206` - Build autopilot unit-economics reporting pipeline. - Track margin and cost drivers by workload class.
- `OA-ECON-208` - Integrate autopilot-to-marketplace procurement loop. - Convert runtime demand into marketplace transactions by default.
- `OA-ECON-209` - Build wedge-to-platform conversion analytics. - Measure conversion from autopilot usage to marketplace activity.
- `OA-ECON-210` - Implement persistent entity history service. - Track long-lived identity, behavior, and economic state.
- `OA-ECON-211` - Implement entity reputation passport model. - Provide transferable trust summaries for agents/cohorts.
- `OA-ECON-212` - Implement operator liability disclosure surfaces. - Make responsibility and risk posture explicit in product flows.
- `OA-ECON-213` - Implement operator accountability receipt binding. - Link high-risk operations to signed operator approvals.
- `OA-ECON-214` - Implement remote control APIs for desktop/mobile parity. - Ensure consistent control semantics across clients.
- `OA-ECON-215` - Implement user-facing replay explorer. - Provide searchable, inspectable trajectory and receipt timelines.
- `OA-ECON-216` - Implement autonomous maintenance workflows. - Support continuous dependency/CVE/CI maintenance loops.
- `OA-ECON-217` - Implement open-source maintenance bounty router. - Fund upkeep workflows through programmable micro-bounties.
- `OA-ECON-218` - Publish fleet management API docs. - Document orchestration APIs for internal/external integrators.
- `OA-ECON-219` - Publish autopilot deployment and incident runbooks. - Document operational lifecycle and recovery procedures.

### Phase 4: Treasury + Exchange Expansion

- `OA-ECON-040` - Implement TreasuryRouter core policy engine. - Route payments by rail, asset, approval policy, and risk profile.
- `OA-ECON-041` - Implement rail/asset canonical model. - Encode AssetId semantics to avoid hidden risk coupling across rails.
- `OA-ECON-042` - Implement multi-currency budget engine. - Support USD-denominated budget intent with BTC settlement mechanics.
- `OA-ECON-043` - Implement exchange-rate service with fallback quorum. - Normalize rates across providers with fault-tolerant sourcing.
- `OA-ECON-044` - Add marketplace signer mint allowlist controls. - Enforce baseline mint trust policy for managed flows.
- `OA-ECON-045` - Implement mint exposure diversification caps. - Prevent concentrated mint risk in default treasury policy.
- `OA-ECON-046` - Implement account partitions for treasury/operating/escrow/payroll. - Separate custody and spend lanes by purpose.
- `OA-ECON-047` - Implement proof lifecycle state machine. - Enforce unspent/reserved/spent transitions with conflict safety.
- `OA-ECON-048` - Implement quote lifecycle state machine. - Enforce created/unpaid/pending/paid/failed/expired progression.
- `OA-ECON-050` - Implement ECIES secret-at-rest protection. - Encrypt proof secrets and sensitive key material in storage.
- `OA-ECON-051` - Implement LUD-16 payment address service. - Add human-readable receiving identities for agents.
- `OA-ECON-052` - Implement receive-quote callback infrastructure. - Generate on-demand receive quotes with policy checks.
- `OA-ECON-053` - Implement cross-currency receiving conversion. - Convert incoming assets into preferred account denomination.
- `OA-ECON-056` - Implement graceful offline treasury mode. - Support cached visibility and deferred settlement without unsafe writes.
- `OA-ECON-057` - Implement co-sign thresholds by account class. - Enforce stronger approval for high-value treasury lanes.
- `OA-ECON-058` - Implement audit export pack. - Provide enterprise-readable trails tied to cryptographic receipts.
- `OA-ECON-059` - Build treasury policy simulation harness. - Validate routing/budget policy before production rollout.
- `OA-ECON-060` - Build mint trust scoring engine. - Aggregate allowlist, operator policy, and community signals.
- `OA-ECON-061` - Implement NIP-60 wallet recovery path. - Recover wallet state from relay event history with integrity checks.
- `OA-ECON-062` - Implement NIP-87 trust weighting logic. - Apply recommendation strength and source trust in mint selection.
- `OA-ECON-063` - Add treasury fault injection drills. - Exercise network outage, mint outage, and partial failure scenarios.
- `OA-ECON-064` - Ship treasury API surfaces. - Expose control-plane and runtime interfaces for treasury operations.
- `OA-ECON-065` - Ship treasury operational documentation and runbooks. - Publish deploy, incident, and recovery guidance.

- `OA-ECON-066` - Implement exchange RFQ service. - Support request/quote/accept lifecycle with provenance.
- `OA-ECON-067` - Implement NIP-69 orderbook integration. - Publish and consume compatible order events.
- `OA-ECON-068` - Implement taker quote selection and routing engine. - Route by price, latency, trust, and policy bounds.
- `OA-ECON-069` - Implement settlement v0 reputation-first flow. - Enable bootstrap settlement with controlled trust assumptions.
- `OA-ECON-070` - Implement settlement v1 atomic eCash swap. - Add hashlocked P2PK and invoice-coupled atomicity where supported.
- `OA-ECON-071` - Implement settlement v2 cross-mint bridge path. - Support Treasury Agent bridge settlement across mints.
- `OA-ECON-072` - Build Treasury Agent bootstrap toolkit. - Provide quoting, inventory, and risk controls for makers.
- `OA-ECON-073` - Build Treasury Agent economics dashboard. - Track spread, fill quality, and capital utilization.
- `OA-ECON-074` - Implement trade attestation via NIP-32 labels. - Publish signed post-trade labels for trust graph updates.
- `OA-ECON-075` - Implement web-of-trust weighted reputation service. - Score counterparties using graph-local trust weighting.
- `OA-ECON-076` - Implement bond collateral policy engine. - Require collateral for selected high-risk/high-value flows.
- `OA-ECON-077` - Implement escrow and timelock primitives for disputes. - Hold and release funds under programmable dispute windows.
- `OA-ECON-078` - Implement arbitration process contract. - Define decentralized adjudication and evidence paths.
- `OA-ECON-079` - Integrate exchange routing into autopilot procurement. - Use exchange liquidity for treasury-to-compute conversion.
- `OA-ECON-080` - Implement hedge and rate-lock policy primitives. - Support volatility-aware execution policies.
- `OA-ECON-081` - Implement FX receipt provenance schema. - Record rate, source, quote, and settlement evidence.
- `OA-ECON-082` - Ship exchange UX surfaces. - Deliver operator-grade views for quotes, trades, and risk.
- `OA-ECON-083` - Implement exchange circuit breakers and risk limits. - Add halt and guardrail controls for abnormal conditions.
- `OA-ECON-084` - Expose maker APIs for programmatic liquidity providers. - Enable automated market participation.
- `OA-ECON-085` - Implement exchange simulation/backtesting suite. - Evaluate routing and spread policy before rollout.
- `OA-ECON-086` - Add exchange resilience drills. - Validate restart, replay, and recovery behavior under failure.
- `OA-ECON-087` - Implement optional compliance policy hooks. - Support policy modes without violating non-custodial boundaries.
- `OA-ECON-088` - Enforce non-custodial trust boundaries. - Prove matching/routing infrastructure never custodies user funds.
- `OA-ECON-089` - Publish exchange operator runbooks. - Document operational controls and incident response.

### Phase 5: Sovereign Protocol + Wallet Expansion

- `OA-ECON-090` - Publish NIP-SA canonical schema package. - Define lifecycle event contracts and compatibility guarantees.
- `OA-ECON-091` - Implement AgentProfile event lifecycle. - Publish identity and signer policy metadata.
- `OA-ECON-092` - Implement encrypted AgentState lifecycle. - Persist goals, memory pointers, and capability state securely.
- `OA-ECON-093` - Implement AgentSchedule semantics. - Support wake, trigger, and recurring schedule contracts.
- `OA-ECON-094` - Implement TickRequest/TickResult event pair. - Bracket autonomous execution in auditable protocol events.
- `OA-ECON-095` - Implement TrajectorySession/TrajectoryEvent protocol flows. - Publish transparent decision/activity trails.
- `OA-ECON-096` - Implement SkillLicense/SkillDelivery protocol events. - Bind market skill transactions to sovereign agent lanes.
- `OA-ECON-097` - Implement agent capability registry surface. - Expose discoverable machine-readable capability declarations.
- `OA-ECON-098` - Implement delegation and revocation lifecycle. - Maintain authority continuity with explicit revocation semantics.
- `OA-ECON-099` - Build NIP-SA compatibility and migration policy. - Support safe protocol evolution without state breakage.

- `OA-ECON-100` - Ship sovereign wallet CLI parity. - Expose identity, treasury, and relay controls via unified CLI.
- `OA-ECON-101` - Ship sovereign wallet WGPUI parity. - Deliver native UI for custody, policy, and payment operations.
- `OA-ECON-102` - Implement NIP-47 wallet-connect flows. - Enable external payment requests with policy-gated approvals.
- `OA-ECON-103` - Implement multi-account identity management. - Support compartmentalized personas and account switching.
- `OA-ECON-104` - Implement wallet recovery and migration tools. - Support secure restore and device migration paths.

- Agent Git Platform is optional/bonus; see `docs/plans/optional/gitafter-bonus.md`.

- `OA-ECON-110` - Integrate wallet/protocol identity lanes across surfaces. - Ensure one sovereign identity can transact across wallet, marketplace, and agent runtime surfaces.
- `OA-ECON-111` - Ship protocol examples and client SDK references. - Provide implementation-grade examples for integrators.
- `OA-ECON-112` - Build NIP-SA conformance suite. - Validate lifecycle and compatibility behavior across implementations.
- `OA-ECON-113` - Build wallet and protocol integration suite. - Validate end-to-end identity-to-payment paths.
- `OA-ECON-115` - Harden wallet security controls. - Enforce keychain, encryption, and policy-based approval boundaries.
- `OA-ECON-116` - Add sovereign wallet observability stack. - Publish state and failure metrics needed for operations.
- `OA-ECON-117` - Implement protocol governance publication flow. - Define how schema changes are proposed and accepted.
- `OA-ECON-118` - Implement compatibility negotiation support windows. - Enforce explicit version support policy at boundaries.
- `OA-ECON-119` - Publish migration guides for sovereign surfaces. - Document upgrade/migration plans for operators and clients.

### Phase 6: Skills, Data, and Coalition Market Expansion

- `OA-ECON-138` - Implement skill packaging format and lifecycle store. - Support draft/review/published/deprecated lifecycle.
- `OA-ECON-139` - Implement skill dependency resolution against MCP capabilities. - Validate required/optional tool dependencies pre-install.
- `OA-ECON-140` - Implement skill licensing and encrypted delivery. - Enforce paid capability distribution with secure delivery.
- `OA-ECON-141` - Implement skill pricing model support. - Support free, per-call, per-token, and hybrid pricing.
- `OA-ECON-142` - Implement configurable split policy engine. - Encode creator/provider/platform/referrer payout logic.
- `OA-ECON-143` - Implement deterministic payout ledger. - Guarantee conservation and replayable split outcomes.
- `OA-ECON-144` - Implement minute-bucket earnings analytics. - Provide real-time earnings visibility by revenue type.
- `OA-ECON-145` - Implement referrer attribution and routing. - Track referral provenance and payout eligibility.

- `OA-ECON-146` - Implement data marketplace catalog and purchase flow. - Enable dataset and artifact commerce lanes.
- `OA-ECON-147` - Implement trajectory ingestion and quality scoring. - Score contributions by completeness, complexity, and outcome signals.
- `OA-ECON-148` - Implement trajectory reward calculator. - Convert quality scores to configurable sat-based rewards.
- `OA-ECON-149` - Implement secret redaction pipeline. - Remove sensitive secrets from contributed trajectories.
- `OA-ECON-150` - Implement anonymization pipeline. - Remove personal and machine-identifying metadata for contributions.
- `OA-ECON-151` - Implement sensitivity scoring and contribution gate. - Block or require explicit confirmation for risky submissions.
- `OA-ECON-152` - Implement enterprise exclusion policy mode. - Preserve closed contribution mode while retaining product capabilities.
- `OA-ECON-153` - Implement differential privacy transformation lane. - Provide stronger anti-reconstruction guarantees for contribution data.

- `OA-ECON-154` - Implement coalition identity and membership log. - Track coalition lifecycle as first-class protocol state.
- `OA-ECON-155` - Implement coalition discovery engine. - Support orchestrator-driven, registry-driven, and emergent discovery modes.
- `OA-ECON-156` - Implement coalition preference/history model. - Learn compatible partnerships from prior outcomes.
- `OA-ECON-157` - Implement coalition payment coordinator. - Coordinate multi-party payouts with auditable allocation.
- `OA-ECON-158` - Implement multi-recipient atomic payout primitives. - Prevent partial payout failure across coalition distributions.
- `OA-ECON-159` - Implement coalition treasury policies. - Enforce coalition-specific spend and approval governance.
- `OA-ECON-160` - Implement coalition reputation service. - Score coalition-level reliability and payout behavior.
- `OA-ECON-161` - Implement coalition merge/split governance workflow. - Support structural evolution with policy integrity.
- `OA-ECON-162` - Expose coalition APIs and SDK surfaces. - Provide integrator-grade interfaces for coalition operations.

- `OA-ECON-163` - Implement anti-abuse controls for marketplace lanes. - Detect spam/fraud and enforce policy actions.
- `OA-ECON-164` - Implement fraud response automation. - Trigger containment, evidence capture, and recovery actions.
- `OA-ECON-165` - Implement cross-provider interoperability lanes. - Ensure neutral routing across model/provider ecosystems.
- `OA-ECON-166` - Build marketplace e2e conformance suite. - Validate complete transaction flows with replay evidence.
- `OA-ECON-168` - Publish marketplace operator runbooks. - Document deployment, policy, and incident operations.

### Phase 7: QA + Threat + Decentralization Migration

- `OA-ECON-169` - Publish marketplace decentralization migration guide. - Define path from centralized defaults to plural providers/signers.

- `OA-ECON-220` - Enforce no-stubs policy in CI/local gates. - Prevent placeholder code from entering production paths.
- `OA-ECON-221` - Enforce coverage and public-API verification gates. - Require explicit test standards for release progression.
- `OA-ECON-222` - Add property-based and fuzz testing for protocol layers. - Increase confidence at serialization and state-machine boundaries.
- `OA-ECON-223` - Build marketplace end-to-end commerce test suite. - Validate full market cycles across compute/skills/data lanes.
- `OA-ECON-224` - Build treasury and exchange disaster drill suite. - Test recovery behavior under infrastructure and dependency failures.
- `OA-ECON-225` - Build signer outage and key recovery drills. - Verify deadman and rotation flows in controlled simulations.
- `OA-ECON-226` - Build relay partition and replay recovery drills. - Verify multi-relay resilience and replay correctness.
- `OA-ECON-227` - Map threat model controls to executable tests. - Convert threat matrix into verifiable security test artifacts.
- `OA-ECON-228` - Implement liability bond enforcement. - Require economic stake for selected high-risk autonomous actions.
- `OA-ECON-229` - Implement tiered dispute flow engine. - Encode automated, escrow, and arbitration dispute paths.
- `OA-ECON-230` - Implement guardian service APIs and SLA tracking. - Support managed guardian operations with measurable reliability.
- `OA-ECON-231` - Implement third-party guardian integration contracts. - Enable pluggable guardian providers.
- `OA-ECON-232` - Implement hardware escrow guardian mode. - Support non-human guardian custody via hardware controls.
- `OA-ECON-233` - Implement signer optionality path. - Allow operation without default marketplace signer where policy permits.
- `OA-ECON-234` - Implement multi-signer routing and failover. - Support signer plurality and availability-aware selection.
- `OA-ECON-235` - Implement signer rotation with identity continuity. - Migrate signer sets without losing trust/history.
- `OA-ECON-236` - Implement relay diversification automation. - Encourage and enforce multi-relay operation posture.
- `OA-ECON-237` - Ship operator-managed relay package. - Reduce dependency on platform-operated relay infrastructure.
- `OA-ECON-238` - Build centralization-risk telemetry dashboards. - Monitor signer/relay concentration and migration progress.
- `OA-ECON-239` - Define decentralization progression scorecard. - Publish objective criteria for centralization reduction.

- `OA-ECON-243` - Implement anti-concentration routing guardrails. - Use policy defaults to broaden provider and payout distribution.
- `OA-ECON-244` - Implement payout breadth and concentration metrics. - Track Gini/HHI/top-share dynamics for value distribution health.
- `OA-ECON-245` - Implement deflation index for work units. - Track trend in execution cost for standard workload classes.
- `OA-ECON-246` - Implement dividend stream stability index. - Track continuity and variance of participant payout streams.
- `OA-ECON-247` - Implement value-velocity dashboards. - Measure microtransaction throughput and settlement latency.
- `OA-ECON-248` - Implement transparency report pipeline. - Publish periodic market health and trust metrics.

- `OA-ECON-257` - Implement end-to-end wedge vignette test suite. - Execute and verify complete issue-to-payout flow.
- `OA-ECON-258` - Implement end-to-end marketplace vignette test suite. - Execute and verify skill+compute+split flow.
- `OA-ECON-259` - Implement end-to-end treasury+FX vignette test suite. - Execute and verify budget+quote+settlement flow.
- `OA-ECON-260` - Implement platform launch readiness checklist gate. - Require explicit readiness proof before broad rollout.
- `OA-ECON-261` - Implement incident command and rollback protocol. - Standardize major incident response and service restoration.
- `OA-ECON-262` - Implement docs freshness and accuracy checks. - Detect stale docs against active code and protocol state.

### Phase 8: Commercialization / Governance / Ecosystem Scale

- `OA-ECON-240` - Keep protocol evolution governance ADR-first. - Use `docs/adr/` + `ADR-0002` for contract changes; avoid separate governance systems.
- `OA-ECON-241` - Define the external NIP proposal workflow. - If we upstream Nostr protocol proposals, specify the review + versioning path without inventing new internal processes.
- `OA-ECON-242` - Enforce compatibility/deprecation policy in code and docs. - Apply `ADR-0005` support windows via compatibility negotiation and explicit deprecation surfaces.

- `OA-ECON-249` - Implement autopilot subscription revenue instrumentation. - Track wedge economics and conversion drivers.
- `OA-ECON-250` - Implement marketplace fee revenue instrumentation. - Track transaction-fee economics across market lanes.
- `OA-ECON-251` - Implement treasury/exchange revenue instrumentation. - Track spread and treasury-service economics.
- `OA-ECON-252` - Implement SMB and mid-market packaging controls. - Encode practical onboarding, budgets, and operational defaults.
- `OA-ECON-253` - Implement enterprise gateway procurement surfaces. - Support enterprise spend controls and integration boundaries.
- `OA-ECON-254` - Implement invoice/accounting export surfaces. - Produce finance-compatible statements linked to receipts.
- `OA-ECON-255` - Implement audit evidence bundle generation. - Create exportable compliance packets from operational trails.
- `OA-ECON-256` - Implement customer ROI scorecard surfaces. - Report outcome and leverage metrics directly to operators.

- `OA-ECON-263` - Implement ADR/invariant-to-issue traceability checks. - Keep plans aligned with `docs/adr/` + invariant gates without reviving the archived directive system.
- `OA-ECON-264` - Implement dependency graph and critical path board. - Track blocked/unblocked issue flow programmatically.
- `OA-ECON-265` - Implement release train and cutover governance. - Control rollouts through explicit deploy gates.
- `OA-ECON-266` - Implement support and escalation operating model. - Define owner paths for incidents and customer escalations.
- `OA-ECON-267` - Implement production capacity planning model. - Forecast and provision capacity for demand growth.
- `OA-ECON-268` - Implement pricing governance framework. - Manage pricing policy changes with measurable impact controls.
- `OA-ECON-269` - Implement capture-resilience review cadence. - Audit chokepoints and countermeasures continuously.
- `OA-ECON-270` - Implement legal policy surfaces for operator liability. - Encode legal accountability boundaries in product and docs.
- `OA-ECON-271` - Implement multi-jurisdiction readiness model. - Track deploy and policy constraints across jurisdictions.
- `OA-ECON-272` - Implement abuse-response policy tooling. - Detect and respond to misuse across agent and market lanes.
- `OA-ECON-273` - Implement public security disclosure and bounty policy. - Provide vulnerability intake, triage, and response workflows.
- `OA-ECON-274` - Implement third-party audit program plan. - Define recurring external verification of critical controls.
- `OA-ECON-275` - Implement continuous red-team program. - Exercise adversarial scenarios against production-like environments.
- `OA-ECON-276` - Implement guardian ecosystem growth program. - Expand non-platform guardian options and reliability.
- `OA-ECON-277` - Implement Treasury Agent ecosystem bootstrap. - Grow and qualify initial liquidity provider set.
- `OA-ECON-278` - Implement compute provider ecosystem bootstrap. - Grow diverse supply across provider classes.
- `OA-ECON-279` - Implement skills/data creator ecosystem bootstrap. - Grow creator-side liquidity with payout-first onboarding.

## 8) Cross-Phase Release Gates

- Gate L (Liquidity Bootstrap): Autopilot coding runs generate Verified Patch Bundles; work routes via OpenAgents Compute providers by default (reserve pool fallback); pay-after-verify settlement completes end-to-end; liquidity dashboard shows fill rate, median latency, cost, and provider breadth.
- Gate A: Every authority mutation emits deterministic, signed receipts.
- Gate B: Live authority lanes remain WS-only, replay-safe, and idempotent.
- Gate C: Budget and policy controls are enforced before settlement.
- Gate D: Marketplace payouts are deterministic and conservation-safe.
- Gate E: Threat-model controls have executable tests and drill evidence.
- Gate F: Centralization fallback paths exist and are tested.
- Gate G: Customer-facing ROI and reliability metrics are measurable and published.

## 9) Definition of Completion

The plan is complete only when:

- Each issue in `OA-ECON-001` to `OA-ECON-279` is either shipped, explicitly superseded by a linked replacement, or explicitly marked optional in a pointer plan (e.g., `docs/plans/optional/gitafter-bonus.md`).
- Every synthesis concept is traceable to implemented issue outcomes.
- End-to-end vignette suites (`OA-ECON-257`, `OA-ECON-258`, `OA-ECON-259`) pass with replay artifacts.
- Decentralization migration controls (`OA-ECON-233` to `OA-ECON-239`) are operational and verified.
- Macroeconomic outcomes (deflation index, payout breadth, velocity, concentration controls) are measured and actionable.

This is the execution backbone for moving from wedge product to full open-agent economy infrastructure.

## 10) Reorder Checklist (Liquidity-First)

- [x] Agent Git Platform carved out to `docs/plans/optional/gitafter-bonus.md`.
- [x] Priority Ladder inserted near top.
- [x] Sequencing Model updated with Liquidity Bootstrap first.
- [x] Issue catalog begins with liquidity bootstrap spine.
- [x] OpenAgents Compute terminology explicit.
- [x] Liquidity Bootstrap gate added.
