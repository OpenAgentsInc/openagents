# MASTER ROADMAP — Khala Code MVP (Fully Tested) → Codex → AI Employees → the Suite

Date: 2026-07-07
Status: **the single consolidated execution roadmap.** This document owns
top-level sequencing across everything designed in the 2026-07-07 strategy
set and its predecessors. The source docs remain authoritative for their
*content* (specs, evidence, arguments); when sequencing here and sequencing
there disagree, **this document wins**, and new issues are filed against the
phase lanes named here.

Consolidates:

- Mobile MVP + Agent Computers substrate — epic #8467, AC-1 #8503
  (`2026-07-05-khala-code-mobile-only-mvp-launch-audit.md`,
  `../khala-code/2026-07-06-agent-computers-strategy.md`)
- The mobile testing system (Blueprint-modeled) — QAM-1..7
  (`../khala-code/2026-07-07-mobile-testing-audit-and-plan.md`)
- Codex on Agent Computers + post-MVP directions — CX-1..5
  (`2026-07-07-beyond-mvp-codex-agent-computers-and-ai-employees.md`)
- The horizon ladder H0–H6 and lane reconciliation
  (`2026-07-07-overarching-roadmap-khala-code-agent-computers-ai-employees.md`)
- AI employees / company brain phases — AE-x / CB-x
  (`../agenticsociety/2026-07-03-integration.md`)
- The product suite (mobile/desktop/web/Reactor)
  (`2026-07-07-product-suite-khala-code-openagents-com-reactor.md`)
- The sovereignty analysis deltas (Blueprint-lite brain, assurance-level
  vocabulary, corpus canaries, the 15-step assessment)
  (`2026-07-07-palantir-institutional-sovereignty-smb-analysis.md`)
- The standing lane families: ROADMAP.md (desktop/harness),
  ROADMAP_QA.md (QA engine), ROADMAP_BIZ.md (BF-*), ROADMAP_AFTER.md
  (AW-*), ROADMAP_BACKGROUND_AGENTS.md (BA-*), Reactor RX-*.

Owner directive shaping the order: **first** get the Khala Code mobile app
MVP fully tested through the Blueprint-modeled QA system; **then** Codex
support; then onward through the designed roadmap.

## 0. The one-page shape

```
P0  MVP, fully tested ──────► P1  Your Codex ──► P2  Standing employees
    (QAM gate + suites +           (CX-1..5)          (AE-1: definitions on
     #8503/#8477 proofs +                              agent computers)
     launch readiness)                                     │
                                                           ▼
P6  Scale / GTM / suite ◄── P5  Trust layer ◄── P4  Templates & ◄── P3  Employees &
    (assessment, IAP,           (skills registry,    integrations        the brain
     pairing, Reactor            canaries, input     (AE-3, BI-1..5)     (AE-2..4, CB-1)
     tiers, network)             ceiling)
```

Testing is not a phase that ends: the P0 gate and feature-ladder
discipline apply to **every** phase after it — a lane's exit receipts
include its Eval Suite green at the target ladder rungs.

## P0 — Khala Code mobile MVP, fully tested (the Blueprint-modeled QA system)

Goal: the MVP straight line (GitHub sign-in → repo → cloud turn → live
updates → push → writeback → credits) **proven end-to-end and held green
by the typed testing system** — Eval Suites whose expected-* fixtures are
the oracles, an ordered mobile Release Gate emitting receipts, and the
per-feature test ladder capping claims. Spec:
`../khala-code/2026-07-07-mobile-testing-audit-and-plan.md`.

Order inside P0 (testing starts immediately; nothing waits on infra):

- **P0.1 = QAM-1** The gate. `qa:mobile:gate` (static → units → mounts →
  contracts → generator conformance → fixture tier), typecheck/depcruise
  promoted to blocking, pre-push wiring.
- **P0.2 = QAM-2** Mount debt + fixture suites. Thread-list,
  thread-messages, credits-history, settings mounted (or typed waivers);
  the **agent-computer streaming fixture suite** (runtime events →
  thread UI, typed refusals, writeback card) — authored against fixtures
  now, so #8503's live proof lands into waiting oracles.
- **P0.3 = QAM-3** Generators as the enforced path (screen gen emits
  mount test + stories + contract stub + flow stub + visual
  registration; conformance policy test).
- **P0.4** MVP proofs (existing scope, unchanged): the #8503 receipt
  bundle (owner/infra: staging inference supply + no-meter secret, per
  NEEDS_OWNER), #8477 writeback E2E, Aiur #8500/#8501.
- **P0.5 = QAM-4** Visual tier: story screenshots + screen checkpoints
  into `openagents.khala_visual_baselines.v1`; blessing workflow.
- **P0.6 = QAM-5** Nightly mobile row on an owned Mac (Maestro flows,
  device monkey, visual capture, perf budgets, seam probes) reporting
  into the QA nightly/status/strict-issue discipline; mobile nodes on
  the QA Swarm board.
- **P0.7 = QAM-6** Android lane (emulator boot proof, flows, capture
  parity).
- **P0.8** Launch readiness: seeded test account (owner-gated, R4) →
  unattended straight-line E2E green
  (`khala_mobile.platform.launched_app_interaction_smoke.v1` enforced);
  store submission pack current; promises/copy pass.

**P0 exit receipts:** the #8503 proof bundle; the release gate refusing
an unbundled screen; 7 consecutive nightly mobile receipts incl. one
auto-filed strict issue; every MVP feature at its target ladder rung;
straight-line E2E green on iOS + Android emulator; QAM-7 fixture-first
suites authored (red/waived) for every P1+ feature named below.

## P1 — Your Codex on your agent computer (CX-1..5)

Spec: `2026-07-07-beyond-mvp-codex-agent-computers-and-ai-employees.md` §2/§6.

- **CX-1** Provider-credential invariant + broker contract (the
  isolation-policy amendment: `provider_credential_policy: broker_only`,
  never-pooled/never-cross-owner law, scanner coverage, fail-closed
  tests). *This is an INVARIANTS change; write the law first.*
- **CX-2** Mobile Codex connect (device-auth → existing
  `provider_account_token_custody` rail; accounts UI with
  readiness/quota; disconnect/revocation). *Eval Suite from QAM-7 goes
  green here, not written here.*
- **CX-3** Injection + first cloud Codex turn (broker redemption in the
  microVM, isolated `CODEX_HOME` on scratch, `codex_app_server` armed in
  the org-cloud lane set, image layer). **Exit:** one real
  mobile-dispatched turn on the user's own Codex inside Firecracker —
  receipt bundle with `tokenChargeMetered: false` model rows +
  compute-time receipts; reclaim wipes the credential.
- **CX-4** Harness/target selection UX (model-preference store →
  execution targets; per-thread harness pill; quota-aware `auto` with
  typed fallback events).
- **CX-5** Claude account parity (same broker, `claude_pylon` lane).

Standing policy throughout: `subscription_capacity_resale` blocked
unconditionally — connected accounts serve their owner's work only.
Open item carried: provider-ToS diligence documented in CX-1.

## P2 — Standing employees (AE-1: the cloud lane unification)

The definition cloud lane IS the Agent Computer (retires the parked
`cloud_workroom` framing). Spec: integration doc Phase 1 = overarching
roadmap H2.

- **AE-1.1** Dispatch `agent_definition.v1` runs through the same
  admission gate + metering rail as mobile turns; definition-run
  work-context kind; compiled toolset policy in the placement payload.
- **AE-1.2** Trigger-latency measurement first; warm pool only if
  receipts justify it.
- **AE-1.3** Budgets as payroll (`maxCreditsPerDay` at admission,
  per-employee rollups in balance UI + Aiur).

**Exit:** one cron-triggered definition running nightly on an agent
computer for 7 consecutive days, zero desktop involvement, exact token +
lifecycle receipts nightly, auto-pause proven on budget exhaustion.

## P3 — The employee and the brain (AE-2..4 + CB-1, Blueprint-lite)

Specs: integration doc Phases 2–3; post-MVP doc §3; sovereignty analysis
§3 (the brain adopts the **Blueprint-lite typed vocabulary** — typed
objects/properties/links with per-fact provenance, Action-Submission
writes, Access Explanation as the permission surface, versioned/forkable
entries; never a doc pile).

- **AE-2.1** `ai_employee.v1` (persona + authority state
  `observe|draft|act_with_approval|act_within_policy` + identity
  bindings; promotions typed and receipted).
- **AE-2.2** Identity bindings via the broker pattern (mailbox/calendar
  grants scoped to the employee's own address; read precedes send).
- **AE-2.3** The phone cockpit: Agents panel + event-ledger inbox +
  one-tap push approvals; land the pending
  `agents_panel.run_status_indicators_truthful.v1` and
  `definitions.harness_swap.v1` contracts with it. Web twin on
  openagents.com follows the mobile panel.
- **AE-4 (scopes)** Authority scoping law before the cockpit ships:
  `owner_self | shared_fleet | owner_operator` on definitions, contexts,
  grants.
- **CB-1.1–1.3** `company_brain.v1` (named owner-scoped collections on
  Khala Sync; ingestion in trust-cost order; role-scoped slices compiled
  into toolset policy).
- **CB-1.4** The prefill pipeline as a fleet lane (intake → public-data
  research run → seeded brain + starter employee in `observe` → intro
  receipt naming every source).

**Exit:** one employee promoted observe→draft→act-with-approval from the
phone, each transition receipted, one push-approved outbound action; one
brain serving two employees with disjoint slices; one prospect workspace
prefilled end-to-end with zero hand-editing.

## P4 — Templates and business integrations (AE-3 + BI-1..5)

Specs: integration doc Phase 4; post-MVP doc §4; BF-6 connector lanes.

- **AE-3.1** Template = preset bundle (definition preset + persona +
  brain slices + schedule + verification rubric + authority floor).
  Ship order: **Outreach Rep first** (the shipped lead-gen definition
  generalized; discipline as policy defaults), then Controller, Content
  Engine, Ops Triage, Knowledge Concierge.
- **AE-3.2** Catalog gate, promise-registry style: no template lists
  without a receipted *external* outcome; template pages carry live
  outcome ledgers. No self-reported numbers, ever.
- **AE-3.3** Hiring flow (instantiate → bind slices → confirm identities
  → set payroll → starts in observe/draft); time-to-first-receipt is the
  activation metric.
- **BI-1** Connector grants on the custody rail (owner-scoped MCP
  connector credentials brokered into the microVM; BF-6.5 law: no raw
  creds/webhook bodies in model context).
- **BI-2** First-party GitHub connector sidecar (BF-6.1), Slack second.
- **BI-3** CRM lane (customer's CRM as system of record, mirror pattern,
  drafting-only defaults, approval-receipt send gates).
- **BI-4** Ingestion + grounding (BF-3.1/3.2 redaction-before-inference
  in the brain's ingestion path).
- **BI-5** Employee pricing rail (owner-priced connector/orchestration
  margin as a third labeled receipt kind on the same ledger).

**Exit:** three templates listed with receipted external outcomes; one
customer running two employees off one brain against a real business
system through a brokered connector.

## P5 — The trust layer (AE-4.x/CB-2 + sovereignty deltas)

Specs: integration doc Phase 5; sovereignty analysis §6.

- **AE-4.1** Provenance-receipted skill registry (content hash, source,
  injection-audit receipt, capability manifest,
  regenerate-under-audit); unaudited skills physically excluded from
  `act_within_policy` employees via the toolset compiler.
- **AE-4.2** Head-of-Security as a built-in template (reviews skills,
  connectors, promotions; findings are inbox receipts).
- **CB-2.1** Input-path ceiling: untrusted-input triggers hard-cap
  outbound/spend tools at `act_with_approval` absent an owner waiver
  receipt; enforced in the behavior-contract sweep.
- **P5.4** Corpus canaries (seeded canary facts + periodic external
  probes → misappropriation-detection receipts).
- **P5.5** Data-posture policy objects per inference lane (typed,
  receipt-backed retention statements; per-model upstream retention
  metadata in the catalog). *Cheap; may land earlier opportunistically.*

**Exit:** registry live with our own templates' skills as the first
audited entries; input ceiling enforced with a sweep test; first canary
receipt produced.

## P6 — Scale, the suite, and the network

Specs: product suite doc; integration doc Phase 6 + §5 campaign;
sovereignty analysis §5–6; suite/pricing owner gates.

- **P6.1** The assessment instrument upgraded to the 15-step
  sovereignty rubric (SMB translation) feeding the prefill lane —
  audit-first outbound at fleet scale.
- **P6.2** Suite arming, owner-timed: IAP reopen (#8481/#8482) with the
  credits-brand decision (*"minerals"* gate); desktop pairing reopen
  (MC-5) as the mobile→desktop power lane; the two-register design spec
  for openagents.com.
- **P6.3** Sovereignty ladder as quoted **assurance levels**
  (structural vs contractual): hosted → BYO subscription →
  `regulated_private` placement (BF-3.4) → **Reactor** (RX-* lanes;
  customer-side deployment, sales-led). Reactor Zero serving share as a
  tracked internal metric (public claim owner-gated).
- **P6.4** Network graduation: employee outcome ledgers → consented
  public outcome stories → forum identity → tips → routed work; partner
  prong fulfillment receipted under LG-8/LG-9 bookkeeping.
- **P6.5** The agency-trap tripwire watched continuously:
  operator-minutes per engagement falling as engagement count rises, per
  cohort, in Aiur.

**Exit:** falling operator-minutes across a growing cohort; first
partner-fulfilled prong receipted; first assessment-sourced customer
running a templated employee.

## Cross-cutting disciplines (bind every phase)

1. **Blueprint-modeled quality**: every phase's lanes ship with Eval
   Suites (expected-* fixtures as oracles), pass the release gate, and
   record receipts incl. the could-not-prove list. Feature ladder rungs
   cap claims.
2. **Receipt-first, exact-only; owner-gated greens; public-safe
   projections.**
3. **Config, not fork** — verticals, templates, customers are config.
4. **Subscription no-resale, never waivable.** Org-cloud never touches
   user-owned machines.
5. **Owned vocabulary** — Blueprint (never "ontology"), our named system
   family; no third-party company names in public copy.
6. **No hosted CI / no third-party build-update-visual SaaS** — owned
   runners, owned OTA, owned engines.
7. **Constant motion**: owner-gated steps go to NEEDS_OWNER and work
   continues on the next non-blocked lane.

## Current owner gates (as of 2026-07-07)

- Staging inference supply + `OA_CLOUD_RUNTIME_NO_METER_SECRET` (P0.4 /
  #8503 — in NEEDS_OWNER).
- Seeded public-safe test account (P0.8 / R4).
- Agent-computer compute rate; IAP arming + credits-brand ("minerals");
  template/services pricing; any promise green flips.
- Production arming decisions for the provisioner beyond staging.

## Document map (content authorities under this roadmap)

| Area | Doc |
|---|---|
| Mobile MVP audit | `2026-07-05-khala-code-mobile-only-mvp-launch-audit.md` |
| Agent Computers strategy | `../khala-code/2026-07-06-agent-computers-strategy.md` |
| Mobile testing system (P0) | `../khala-code/2026-07-07-mobile-testing-audit-and-plan.md` |
| Codex/BYO harness (P1) | `2026-07-07-beyond-mvp-codex-agent-computers-and-ai-employees.md` |
| Horizon ladder + lane reconciliation | `2026-07-07-overarching-roadmap-khala-code-agent-computers-ai-employees.md` |
| Employees/brain phases (P2–P5 detail) | `../agenticsociety/2026-07-03-integration.md` |
| Product suite | `2026-07-07-product-suite-khala-code-openagents-com-reactor.md` |
| Sovereignty analysis + deltas | `2026-07-07-palantir-institutional-sovereignty-smb-analysis.md` |
| Narrative / talking points | `2026-07-07-what-openagents-is-essay-and-talking-points.md` |
| Reactor plan | `2026-07-04-reactor-open-model-private-deployment-plan.md` |
| Desktop/harness lanes | `ROADMAP.md` |
| QA engine lanes | `ROADMAP_QA.md` |
| Business fulfillment lanes | `ROADMAP_BIZ.md` |
| Market-contact lanes | `ROADMAP_AFTER.md` |
| Background-agent lanes | `ROADMAP_BACKGROUND_AGENTS.md` |
