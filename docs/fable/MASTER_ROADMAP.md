# MASTER ROADMAP — Khala Code MVP (Tested, Submitted) → Sarah → Codex → AI Employees → the Suite

Date: 2026-07-07 (rev 2, later same day)
Status: **the single consolidated execution roadmap.** This document owns
top-level sequencing across everything designed in the 2026-07-07 strategy
set and its predecessors. The source docs remain authoritative for their
*content* (specs, evidence, arguments); when sequencing here and sequencing
there disagree, **this document wins**, and new issues are filed against the
phase lanes named here.

**Rev 2 changes (owner direction):** (1) P0's exit is strengthened from
"launch-ready" to **submitted to the app stores**; (2) **Sarah** — the AI
sales agent (`2026-07-07-sarah-sales-agent-spec.md`) — is inserted as
**Phase P1**, immediately after the MVP and ahead of Codex; (3) Sarah's web
surface ships as part of **the new openagents.com app on React/TanStack
Start** (the ONE-UI stack), which begins its route-by-route life in P1;
(4) all later phases renumber (Codex P1→P2, employees P2→P3, brain P3→P4,
templates P4→P5, trust P5→P6, scale P6→P7). Cross-referencing docs are
updated in the same commit.

Consolidates:

- Mobile MVP + Agent Computers substrate — epic #8467, AC-1 #8503
  (`2026-07-05-khala-code-mobile-only-mvp-launch-audit.md`,
  `../khala-code/2026-07-06-agent-computers-strategy.md`)
- The mobile testing system (Blueprint-modeled) — QAM-1..7
  (`../khala-code/2026-07-07-mobile-testing-audit-and-plan.md`)
- **Sarah, the sales agent — SR-1..6**
  (`2026-07-07-sarah-sales-agent-spec.md`)
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
- The web-stack decision: **one UI ecosystem — React + Tailwind on
  TanStack Start** for web (owner decision 2026-07-04,
  `2026-07-04-tanstack-start-sites-and-web-app-evaluation.md`; ONE-UI
  epic #8339 and TS-6 web app-shell migration #8348 reopen here)
- The standing lane families: ROADMAP.md (desktop/harness),
  ROADMAP_QA.md (QA engine), ROADMAP_BIZ.md (BF-*), ROADMAP_AFTER.md
  (AW-*), ROADMAP_BACKGROUND_AGENTS.md (BA-*), Reactor RX-*.

## 0. The one-page shape

```
P0  MVP: tested + SUBMITTED ──► P1  Sarah + the new ──► P2  Your Codex ──► P3  Standing
    (QAM gate + suites +            openagents.com          (CX-1..5)         employees
     #8503/#8477 proofs +           (SR-1..3 on React/                        (AE-1)
     store submission)              TanStack Start)                              │
                                                                                 ▼
P7  Scale / GTM / suite ◄── P6  Trust layer ◄── P5  Templates & ◄── P4  Employees &
    (assessment, IAP,           (skills registry,    integrations        the brain
     pairing, Reactor            canaries, input     (AE-3, BI-1..5,     (AE-2..4, CB-1;
     tiers, network)             ceiling)            SR-6 template)      generalizes Sarah)
```

Testing is not a phase that ends: the P0 gate and feature-ladder
discipline apply to **every** phase after it — a lane's exit receipts
include its Eval Suite green at the target ladder rungs. Sarah's suite
(SR) is authored fixture-first under the same law.

## P0 — Khala Code mobile MVP: fully tested, submitted to the stores

Goal: the MVP straight line (GitHub sign-in → repo → cloud turn → live
updates → push → writeback → credits) **proven end-to-end, held green by
the typed testing system, and submitted to the App Store and Play
Store.** Testing spec:
`../khala-code/2026-07-07-mobile-testing-audit-and-plan.md`.

Status note (2026-07-07): the #8503 wall is breaking — the exact
in-microVM model-token receipt is reported proven on main
(`dd5aa4e231`, phase-2 no-meter turn-runner re-pinned into the baked
rootfs). Verify the full DoD bundle against the issue before treating
P0.4 as closed.

P0 issue index: QAM-1..7 = #8536 #8537 #8538 #8539 #8540 #8541 #8542;
launch readiness #8543; store submissions #8544; in-flight proofs #8503,
#8477 under epic #8467.

Order inside P0 (testing starts immediately; nothing waits on infra):

- **P0.1 = QAM-1 (#8536)** The gate. `qa:mobile:gate` (static → units → mounts →
  contracts → generator conformance → fixture tier), typecheck/depcruise
  promoted to blocking, pre-push wiring.
- **P0.2 = QAM-2 (#8537)** Mount debt + fixture suites. Thread-list,
  thread-messages, credits-history, settings mounted (or typed waivers);
  the **agent-computer streaming fixture suite** (runtime events →
  thread UI, typed refusals, writeback card).
- **P0.3 = QAM-3 (#8538)** Generators as the enforced path (screen gen emits
  mount test + stories + contract stub + flow stub + visual
  registration; conformance policy test).
- **P0.4** MVP proofs (existing scope, unchanged; in flight with the
  other agent): the full #8503 receipt bundle verified and the issue
  closed; #8477 writeback E2E; Aiur #8500/#8501 (closed 2026-07-07).
- **P0.5 = QAM-4 (#8539)** Visual tier: story screenshots + screen checkpoints
  into `openagents.khala_visual_baselines.v1`; blessing workflow.
- **P0.6 = QAM-5 (#8540)** Nightly mobile row on an owned Mac (Maestro flows,
  device monkey, visual capture, perf budgets, seam probes) reporting
  into the QA nightly/status/strict-issue discipline; mobile nodes on
  the QA Swarm board.
- **P0.7 = QAM-6 (#8541)** Android lane (emulator boot proof, flows, capture
  parity).
- **P0.8 (#8543)** Launch readiness: seeded test account (owner-gated, R4) →
  unattended straight-line E2E green
  (`khala_mobile.platform.launched_app_interaction_smoke.v1` enforced);
  promises/copy pass.
- **P0.9 (#8544) Store submission (the exit).** App Store submission executed
  (build uploaded, metadata/screenshots final, review answers prepared,
  account-deletion + 3.1.1 compliance verified — #8483/#8502/#8491
  packs current) and Play submission executed through the Play lane
  (#8490 owner actions). "Submitted" means in review at both stores —
  approval timing is theirs; ours is the submission receipt.

**P0 exit receipts:** the verified #8503 proof bundle; the release gate
refusing an unbundled screen; 7 consecutive nightly mobile receipts
incl. one auto-filed strict issue; every MVP feature at its target
ladder rung; straight-line E2E green on iOS + Android emulator; **both
store submissions recorded** (submission IDs + review states in the
registry evidence); QAM-7 (#8542) fixture-first suites authored (red/waived) for
every P1+ feature named below.

## P1 — Sarah, on the new openagents.com (React/TanStack Start)

Two tracks that ship together: the sales agent, and the beginning of the
new web app she lives in. Spec: `2026-07-07-sarah-sales-agent-spec.md`.

**Track A — the new openagents.com app (WEB-1).** Sarah's surface is the
first first-class product route on the **new openagents.com app built on
React + TanStack Start** (ONE-UI/shadcn components, one Protoss-blue
theme, no light/dark split) — reopening the parked TS-6/#8348 web
app-shell migration with a concrete product driver instead of a
migration-for-its-own-sake. Scope discipline: the new app starts with
Sarah's route + the business/funnel pages she feeds (business intake,
pricing/packages, checkout return surfaces), absorbing legacy-web
surfaces route-by-route thereafter (dashboard shell lands with P4's web
Agents panel; no new legacy Foldkit surfaces, per the standing ONE-UI
decision). The typed-component renderer (the shipped closed catalog) is
ported to React as part of this track.

**Track B — Sarah lanes (from the spec):**

- **SR-1** Sarah v1 (text, on-site): durable sessions + prospect refs;
  persona program (public-scoped Artanis pattern); qualification on the
  shipped intake spine; registry-bound honesty; component channel on;
  `human_handoff`; pack-priced `credit_kickoff` checkout; behavior
  contracts registered; Eval Suite green at the fixture tier.
- **SR-2** Deal engine + checkout tool: `sarah.deal_rules.v1` (rate
  card owner-signed; volume tiers + Bitcoin stack imported; bundle
  rules; `close_on_call` tactic armed or parked);
  `sales.checkout_link.create` (arbitrary amount ≤ cap, split support,
  Lightning, honest TTLs); `quote_card`/`deal_summary` components.
- **SR-3** Email + CRM continuity: Sarah's mailbox; inbound routing →
  `event_ledger` **email source** → `inbox_match`; prospect↔contact
  binding; approval-gated continuation replies; one relationship
  thread across web and email.

**Deferred within the Sarah family (land in later phases):** SR-4 voice
(after P2 — it competes with nothing and benefits from the deal-engine
receipts; may pull forward if owner priorities say so), SR-5 contracts +
custom bundles (with P5's template work), SR-6 Sarah-as-template (P5,
under the catalog gate).

**P1 exit receipts:** a stranger completes qualification → quote →
settled starter credit purchase entirely with Sarah on the new
TanStack Start surface; a composed multi-module quote with a bundle rule
applied closes via an agent-created link, with property tests proving no
unruled price is reachable; a web conversation resumed by prospect email
and answered through the approval queue; the new app serving Sarah +
funnel routes in production with the legacy routes untouched.

## P2 — Your Codex on your agent computer (CX-1..5)

Spec: `2026-07-07-beyond-mvp-codex-agent-computers-and-ai-employees.md`
§2/§6.

- **CX-1** Provider-credential invariant + broker contract
  (`provider_credential_policy: broker_only`; never-pooled /
  never-cross-owner law; scanner coverage; fail-closed tests). *An
  INVARIANTS change; write the law first.*
- **CX-2** Mobile Codex connect (device-auth → existing
  `provider_account_token_custody` rail; accounts UI with
  readiness/quota; disconnect/revocation).
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
Provider-ToS diligence documented in CX-1.

## P3 — Standing employees (AE-1: the cloud lane unification)

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

## P4 — The employee and the brain (AE-2..4 + CB-1, Blueprint-lite)

Specs: integration doc Phases 2–3; post-MVP doc §3; sovereignty analysis
§3 (the brain adopts the **Blueprint-lite typed vocabulary** — typed
objects/properties/links with per-fact provenance, Action-Submission
writes, Access Explanation as the permission surface, versioned/forkable
entries). **Sarah is the generalization seed:** the P1 persona program,
CRM-mapped memory, and authority posture harden into the formal
`ai_employee.v1` record here; Sarah is migrated onto the formal record
as its flagship instance.

- **AE-2.1** `ai_employee.v1` (persona + authority state
  `observe|draft|act_with_approval|act_within_policy` + identity
  bindings; promotions typed and receipted).
- **AE-2.2** Identity bindings via the broker pattern (mailbox/calendar
  grants scoped to the employee's own address — Sarah's mailbox from
  SR-3 is the precedent).
- **AE-2.3** The phone cockpit: Agents panel + event-ledger inbox +
  one-tap push approvals; pending contracts land with it. The **web
  twin lands on the new TanStack Start app** (the P1 Track A codebase),
  beginning the business-dashboard build-out.
- **AE-4 (scopes)** Authority scoping law before the cockpit ships:
  `owner_self | shared_fleet | owner_operator`.
- **CB-1.1–1.3** `company_brain.v1` (named owner-scoped collections on
  Khala Sync; ingestion in trust-cost order; role-scoped slices
  compiled into toolset policy).
- **CB-1.4** The prefill pipeline as a fleet lane (intake → public-data
  research run → seeded brain + starter employee in `observe` → intro
  receipt naming every source) — fed directly by Sarah's intake specs.

**Exit:** one employee promoted observe→draft→act-with-approval from the
phone, each transition receipted, one push-approved outbound action; one
brain serving two employees with disjoint slices; one prospect workspace
prefilled end-to-end with zero hand-editing; Sarah running on the formal
employee record.

## P5 — Templates and business integrations (AE-3 + BI-1..5 + SR-5/6)

Specs: integration doc Phase 4; post-MVP doc §4; BF-6 connector lanes;
Sarah spec §13.

- **AE-3.1** Template = preset bundle (definition preset + persona +
  brain slices + schedule + verification rubric + authority floor).
  Ship order: **Outreach Rep first** (lead-gen definition generalized),
  then Controller, Content Engine, Ops Triage, Knowledge Concierge.
- **SR-5** Sarah contracts + custom bundles (order-form generation,
  `contract_review`, e-sign handoff, milestone-escalation priced as
  "costs extra").
- **SR-6** **Sarah as product**: the sales-employee template extracted
  into the catalog — customers hire their own Sarah on their brain and
  rate card; her outcome ledger is the template's receipted proof.
- **AE-3.2** Catalog gate, promise-registry style: no template lists
  without a receipted *external* outcome; template pages carry live
  outcome ledgers.
- **AE-3.3** Hiring flow; time-to-first-receipt as activation metric.
- **BI-1..5** Connector grants on the custody rail; GitHub sidecar
  first, Slack second; CRM-as-mirror lane; ingestion + redaction; the
  owner-priced connector/orchestration margin as a third labeled
  receipt kind.

**Exit:** three templates listed with receipted external outcomes (the
sales-employee template among them); one customer running two employees
off one brain against a real business system through a brokered
connector; one signed Sarah-originated order form.

## P6 — The trust layer (AE-4.x/CB-2 + sovereignty deltas)

Specs: integration doc Phase 5; sovereignty analysis §6.

- **AE-4.1** Provenance-receipted skill registry (content hash, source,
  injection-audit receipt, capability manifest, regenerate-under-audit).
- **AE-4.2** Head-of-Security as a built-in template.
- **CB-2.1** Input-path ceiling enforced in the behavior-contract sweep
  (untrusted-input triggers cap outbound/spend at `act_with_approval`
  absent an owner waiver receipt) — Sarah's inbound-email posture from
  SR-3 is the working precedent.
- **P6.4** Corpus canaries (seeded facts + periodic external probes →
  misappropriation-detection receipts).
- **P6.5** Data-posture policy objects per inference lane (typed,
  receipt-backed retention statements). *Cheap; may land earlier
  opportunistically.*
- **SR-4 voice** lands here at the latest (push-to-talk web voice with
  the transport-invariant safety fixtures), unless pulled forward by
  owner priority after P2.

**Exit:** registry live with our own templates' skills as the first
audited entries; input ceiling sweep-enforced; first canary receipt;
Sarah voice v1 receipts if not earlier.

## P7 — Scale, the suite, and the network

Specs: product suite doc; integration doc Phase 6 + §5 campaign;
sovereignty analysis §5–6; suite/pricing owner gates.

- **P7.1** The assessment instrument upgraded to the 15-step
  sovereignty rubric (SMB translation) feeding the prefill lane —
  audit-first outbound at fleet scale, with Sarah as the landing
  conversation for every assessment link.
- **P7.2** Suite arming, owner-timed: IAP reopen (#8481/#8482) with the
  credits-brand decision (*"minerals"* gate); desktop pairing reopen
  (MC-5); the two-register design spec; the **openagents.com business
  dashboard** completed on the P1/P4 TanStack Start codebase (spend,
  receipts, roster, approvals, team) with legacy-web retirement per the
  reopen ledger.
- **P7.3** Sovereignty ladder as quoted **assurance levels**
  (structural vs contractual): hosted → BYO subscription →
  `regulated_private` (BF-3.4) → **Reactor** (RX-* lanes, sales-led).
  Reactor Zero serving share as a tracked internal metric (public claim
  owner-gated).
- **P7.4** Network graduation: employee outcome ledgers → consented
  public outcome stories → forum identity → tips → routed work; partner
  prong fulfillment receipted under LG-8/LG-9 bookkeeping.
- **P7.5** The agency-trap tripwire watched continuously
  (operator-minutes per engagement falling per cohort, in Aiur).

**Exit:** falling operator-minutes across a growing cohort; first
partner-fulfilled prong receipted; first assessment-sourced customer
closed by Sarah running a templated employee.

## Cross-cutting disciplines (bind every phase)

1. **Blueprint-modeled quality**: every phase's lanes ship with Eval
   Suites (expected-* fixtures as oracles), pass the release gate, and
   record receipts incl. the could-not-prove list. Feature ladder rungs
   cap claims.
2. **Receipt-first, exact-only; owner-gated greens; public-safe
   projections.**
3. **Config, not fork** — verticals, templates, customers, deal rules
   are config.
4. **Subscription no-resale, never waivable.** Org-cloud never touches
   user-owned machines.
5. **One web stack forward**: new web surfaces land on the React/
   TanStack Start openagents.com app (ONE-UI/shadcn, one Protoss-blue
   theme); legacy web is absorbed route-by-route and never grows.
6. **Owned vocabulary** — Blueprint (never "ontology"); no third-party
   company names in public copy.
7. **No hosted CI / no third-party build-update-visual SaaS** — owned
   runners, owned OTA, owned engines.
8. **Constant motion**: owner-gated steps go to NEEDS_OWNER and work
   continues on the next non-blocked lane.

## Current owner gates (as of 2026-07-07 rev 2)

- #8503 DoD verification + production arming decisions (P0.4).
- Seeded public-safe test account (P0.8 / R4).
- **Store submission actions** (App Store Connect + Play Console) at
  P0.9.
- **Sarah SR-2 sign-offs**: rate card, bundle rules, tactics registry
  parameters, per-transaction cap; the tactic-vs-no-discounts
  reconciliation confirmation (Sarah spec §14.4).
- Sarah surname/IP check before any public use beyond "Sarah";
  investor-routing posture (qualify-and-route only) confirmation.
- Agent-computer compute rate; IAP arming + credits-brand
  ("minerals"); template/services pricing; any promise green flips.

## Document map (content authorities under this roadmap)

| Area | Doc |
|---|---|
| Mobile MVP audit | `2026-07-05-khala-code-mobile-only-mvp-launch-audit.md` |
| Agent Computers strategy | `../khala-code/2026-07-06-agent-computers-strategy.md` |
| Mobile testing system (P0) | `../khala-code/2026-07-07-mobile-testing-audit-and-plan.md` |
| **Sarah (P1, SR-*)** | `2026-07-07-sarah-sales-agent-spec.md` |
| Web stack decision (P1 Track A) | `2026-07-04-tanstack-start-sites-and-web-app-evaluation.md` |
| Codex/BYO harness (P2) | `2026-07-07-beyond-mvp-codex-agent-computers-and-ai-employees.md` |
| Horizon ladder + lane reconciliation | `2026-07-07-overarching-roadmap-khala-code-agent-computers-ai-employees.md` |
| Employees/brain phases (P3–P6 detail) | `../agenticsociety/2026-07-03-integration.md` |
| Product suite | `2026-07-07-product-suite-khala-code-openagents-com-reactor.md` |
| Sovereignty analysis + deltas | `2026-07-07-palantir-institutional-sovereignty-smb-analysis.md` |
| Narrative / talking points | `2026-07-07-what-openagents-is-essay-and-talking-points.md` |
| Reactor plan | `2026-07-04-reactor-open-model-private-deployment-plan.md` |
| Desktop/harness lanes | `ROADMAP.md` |
| QA engine lanes | `ROADMAP_QA.md` |
| Business fulfillment lanes | `ROADMAP_BIZ.md` |
| Market-contact lanes | `ROADMAP_AFTER.md` |
| Background-agent lanes | `ROADMAP_BACKGROUND_AGENTS.md` |
