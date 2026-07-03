# ROADMAP_BIZ — The Business Fulfillment Engine

Date: 2026-07-02
Status: consolidated build roadmap for the end-to-end business funnel and
fulfillment engine. Source meditation:
[`2026-07-02-business-fulfillment-engine-meditations.md`](./2026-07-02-business-fulfillment-engine-meditations.md).
Governing frame: [`ROADMAP_AFTER.md`](./ROADMAP_AFTER.md) — this roadmap is
the infrastructure build-out underneath **AW-0 (the services engine, primary
revenue)**, sharing tasks with AW-2 (first-dollar spine), AW-3 (redaction),
AW-6 (demand engine), and AW-8 (pricing). Delivery mechanics:
[`EXECUTION.md`](./EXECUTION.md). No client-identifying information —
vertical descriptors only (this file doubles as public staging material).
No promise state flips; claim-bearing surfaces route through `docs/promises/`
and the copy gates.

**The one-sentence goal:** a buyer travels from simple intake at
`openagents.com/business` to a receipted, review-gated, paid deliverable —
and then to a retained subscription — with every seam automated, instrumented,
and honest, and with operator minutes per engagement falling monotonically.

## 0. The funnel contract (what "very solid" means)

```
ACQUIRE ─► INTAKE ─► QUALIFY ─► CONVERT ─► PROVISION ─► FULFILL ─► PROVE ─► RETAIN ─► MULTIPLY
```

Each stage has a typed object, a receipt, and an instrument:

| Stage | Object | Receipt | Instrument |
| --- | --- | --- | --- |
| Acquire | attributed visit | source attribution row | channel dashboard |
| Intake | `business_signup_request` → enriched lead | intake receipt + follow-up send | conversion by source |
| Qualify | typed intake spec (structured "Sucky-25"/goals) | scope summary the buyer confirms | lead→scope rate |
| Convert | checkout → credits kickoff | payment receipt (`business.*`) | close rate, AOV |
| Provision | prefilled workspace + service promise | provisioning receipt (sources listed) | time-to-workspace |
| Fulfill | accepted-outcome contracts per deliverable | per-outcome receipts + evidence bundles | cycle time, review-minutes |
| Prove | KPI dashboard + evidence/handoff bundle | fulfillment receipts against the promise | promise verifier state |
| Retain | subscription/retainer + daily motion | recurring payment + activity receipts | repeat/retainer rate, churn |
| Multiply | referral/white-label/case study | attributed next engagement | referred-engagement count |

Non-negotiable: **no stage may silently dead-end.** Today INTAKE→PROVISION is
a dead-end (a signup row with no follow-up, no workspace); that is BF-1.1.

## 1. Workstreams

### BF-1 — Funnel core: intake to provisioned workspace (the seam that gates everything)

| Task | Description | Receipt/gate |
| --- | --- | --- |
| BF-1.1 | **Close the `/business` dead-end**: signup → enrichment hook → prefilled-workspace creation (existing primitives) → personal invite send (existing Resend ledger) → engagement tracking. Semi-automated allowed at first; zero silent drops | Every signup row reaches `invited` or an explicit operator-parked state |
| BF-1.2 | **Conversational intake, production**: wire the shipped Khala typed-component onboarding into `/business` qualification — model-chosen component streaming over the gateway (Batch-2 item), structured intake spec output (goals, pains, systems of record, vertical), never letting required fields skip | A typed intake spec object per completed conversation |
| BF-1.3 | **Vertical landing pages, real**: wire the shipped legal vertical funnel (application POST, booking, follow-up email + qualification worksheet) to the Worker; templatize page + apply + confirmed + follow-up for the next verticals; remove all demo-only placeholders | A real application lands end-to-end with a real follow-up send |
| BF-1.4 | **Funnel instrumentation**: per-stage counters (visit→signup→spec→pay→provisioned→first-outcome→retained) with source attribution, same exact-only discipline as tokens-served; weekly-readable dashboard | The owner reads one funnel dashboard weekly |
| BF-1.5 | **Landing-page performance discipline**: `/business` + vertical pages ride the optimized landing stack; site-speed-lane budgets apply (no monolithic bundle regressions on funnel pages) | Budgets green on funnel routes |

### BF-2 — Convert: pricing, payment, and the promise kickoff

Shares the AW-2 owner sitting; nothing here invents new money truth.

| Task | Description | Receipt/gate |
| --- | --- | --- |
| BF-2.1 | **Publish the rate card** through copy gates: three buyable packages (fixed-scope quick win; week-of-fleet sprint; monthly retainer) + the QA Swarm package; fixed scopes, receipt plans up front (= AW-0 A0.2) | A price on `/business` |
| BF-2.2 | **Payment→provision→promise wiring**: checkout success (credits kickoff) triggers workspace provisioning (BF-1.1 path) + creates the per-customer service promise; per-deal split of setup fee vs credit grant supported (self-serve = 100% credits) | Paying without an operator in the loop yields a workspace + promise |
| BF-2.3 | **Recurring billing**: subscription/retainer checkout + entitlement + renewal receipts (membership-funnel and retainer packages both need it) | First recurring payment receipt |
| BF-2.4 | **Volume prepay tiers surfaced**: the internal volume-bonus credit schedule becomes a purchasable path (one balance, never expire, Bitcoin discount stacks per pricing model) | A prepay lands with bonus credits granted atomically |
| BF-2.5 | **On-platform receipts for already-sold engagements** (= AW-0 A0.1): record closed deals as quick-win payment receipts with opaque refs | First paid `business.*` receipt |

### BF-3 — Provision: grounding, corpus, and the private tier

| Task | Description | Receipt/gate |
| --- | --- | --- |
| BF-3.1 | **Scoped read-only ingestion connectors**: cloud-drive/docs first (refresh-token scoped, never root creds), then mail/calendar read paths; per-workspace corpus store with provenance | A customer corpus ingested with a sources receipt |
| BF-3.2 | **Redaction service** (shared task with AW-3 A3.1): PII/PHI redaction before external inference, adversarial fixture suite, applied at ingestion AND at capture | Redaction suite green; regulated-vertical ingestion unblocked |
| BF-3.3 | **Context library per workspace**: the grounded-memory layer (corpus + structured business facts from intake) that every fulfillment workflow retrieves against; nothing published unground | Deliverables cite grounding sources |
| BF-3.4 | **Private/sovereign compute tier**: per-customer isolated workroom/VM lane (cloud repo substrate; orbs-style lifecycle hooks) as the paid upgrade for legal/health-grade privacy — also serves the isolated-VM customer problem. Spec: [`2026-07-02-bf-3-4-private-sovereign-compute-tier.md`](./2026-07-02-bf-3-4-private-sovereign-compute-tier.md) | One customer workspace running on the isolated lane with metering receipts |

### BF-4 — Fulfill: the deliverable engine

| Task | Description | Receipt/gate |
| --- | --- | --- |
| BF-4.1 | **Surface the omni-workroom family**: routes + UI for the already-built schema (lifecycle, tabs per deliverable, evidence bundles, visibility tiers private/customer/team/public) — the client-visible fulfillment surface | A customer logs into their workroom and sees real state |
| BF-4.2 | **Per-customer service promises**: `committed_deliverables`, SLA terms, promise state on the accepted-outcome contract; mirrored fulfillment-receipt verifier; a promise may not commit a deliverable whose backing capability is red | Promise objects with verifier-evaluated receipts |
| BF-4.3 | **Approval ladder + professional review gate**: draft→suggest→execute-with-approval→trusted levels per workspace; a reviewer role (practitioner sign-off) with receipts; nothing external without the gate | Review decisions recorded on every outbound deliverable |
| BF-4.4 | **Document-product pipeline**: intake spec → generation from the customer's own templates/corpus → review gate → delivery receipt (the bounded formation-style document funnel, generalized; replaces all fake-animation demos) | One real document product delivered end-to-end |
| BF-4.5 | **Native sites authoring**: builder UI over the existing sites services; prompt→generate→approve→publish loop; per-customer landing/funnel pages as fulfillment outputs | A customer site published through the approval gate |
| BF-4.6 | **Native email sequences + lists/forms**: authoring UI over the campaign engine; net-new lists/subscribers/forms schema; site→form→list→sequence loop closed | A nurture sequence running for a real customer |
| BF-4.7 | **Inventory-aware campaign workflow**: grounded marketing outcome (real stock only, accurate imagery, spend cap, stats + receipt, conversational edits) — the e-commerce vertical's first deliverable and the owed make-good | Campaign receipt with live stats |
| BF-4.8 | **Vertical config packs**: legal/health/agency/e-commerce as data (stage templates, rubric, starter workflows, compliance profile) — config not fork, enforced by review | A new vertical onboards with zero bespoke screens |

### BF-5 — Fulfillment agents: the promise-servicing loop

Built on `agent_definition.v1` (harness-agnostic background-agent audit):
each service promise gets standing agents with name/goal/enforced-toolset.

| Task | Description | Receipt/gate |
| --- | --- | --- |
| BF-5.1 | **Fulfillment loop v1**: cron-driven per-promise agent — load CRM state, flag stakeholders, produce daily forward motion, draft client comms (approval-gated) | Daily motion receipts on an active promise |
| BF-5.2 | **Agent definitions as the substrate**: fulfillment agents stored as typed definitions (goal, allowed toolset, triggers, escalation), enforced at the tool-authority boundary; ask-policy hits route to the operator | Definition-backed agents running the loop |
| BF-5.3 | **Client comms cadence**: generalized drip (daily/weekly per promise) through the email ledger + customer-visible workroom updates | Customers see forward motion without asking |
| BF-5.4 | **Escalation + paging**: blocked promises page the operator (owner-notification path); no promise silently stalls | Zero silent stalls over a measured month |

### BF-6 — Connectors and outbound rails

| Task | Description | Receipt/gate |
| --- | --- | --- |
| BF-6.1 | **Connector sidecar, source-verified events**: GitHub first (signed webhooks, dedupe, bounded issue/PR-scoped agents, bound writeback tools); the sidecar never owns membership/payment/email authority | GitHub events driving a workspace lane |
| BF-6.2 | **Shared-channel connector second**: opt-in team-chat channel per engagement (invite on request, never auto), bounded reply drafting; supersedes the manual path | An engagement run through a connected channel |
| BF-6.3 | **Social publishing connector**: X first (approval-gated posting for campaign workflows), then additional platforms behind the same bounded-tool contract | Approved post published with receipt |
| BF-6.4 | **Client-owned payment accounts**: deliverable funnels that charge on the *customer's* processor account (their product, their revenue), with our metering separate — the Connect-style seam | A customer's funnel collects on their own account |
| BF-6.5 | **Connector authority/redaction invariant + tests**: no provider credentials or raw webhook bodies in model context; app-owned idempotency; typed per-connector toolsets | Invariant tests green before any connector GA |

### BF-7 — Prove: dashboards, evidence, and the case-study engine

| Task | Description | Receipt/gate |
| --- | --- | --- |
| BF-7.1 | **Customer KPI dashboard**: baseline snapshot + live funnel/revenue metrics per engagement (lead volume, conversion, AOV, revenue, consult attach) — the scorekeeper deliverable value-share terms depend on | A customer-facing KPI page with real data |
| BF-7.2 | **Locked metric definitions**: auditable definitions for factory + engagement metrics (throughput, cycle time, pass rate, review-minutes, operator-minutes per engagement) | Definitions doc + instrumented queries |
| BF-7.3 | **Handoff portal**: public-proof-bundle → client-facing redacted deliverables page (schema exists; surface it) | A customer shares their handoff link |
| BF-7.4 | **Case-study engine** (= AW-0 A0.9): every completed engagement yields a public-safe writeup (opaque refs, real receipts, real cycle times) feeding the acquire stage | Case studies published; intake attributes arrivals |

### BF-8 — Retain and multiply

| Task | Description | Receipt/gate |
| --- | --- | --- |
| BF-8.1 | **Marketing program as a sellable add-on**: campaign packages (content, GEO, outbound assist) delivered by BF-4.5/4.6/4.7 machinery — the explicit expansion ask from the first paying vertical, and our own acquisition motion dogfooded | First marketing-program engagement sold + delivered |
| BF-8.2 | **GEO/content acquisition for ourselves**: question-shaped, citable content + agent-readable surfaces driving `/business`; measured under source attribution (BF-1.4) | AI-search-attributed signups > 0 |
| BF-8.3 | **White-label operator mode**: an agency operator resells the engine to their clients (branded tenant dashboards, per-client workrooms, revenue share on the existing payout ledger) | First white-label client provisioned |
| BF-8.4 | **Referral attribution loop** (honest): satisfied-client referrals attributed end-to-end; payout copy only when ledger machinery settles it | First referred engagement with attribution |
| BF-8.5 | **Overflow/peer marketplace (deferred design)**: qualified intake-complete matters sub-contracted to vetted peers with a cut — design doc + gates only until BF-4 is proven | Design doc, no state change |
| BF-8.6 | **Settlement primitive (deferred)**: trustless escrow (release-on-verified-delivery) as the settle step under accepted outcomes — integration design + the owed partner make-good demo; no custody changes | Working demo receipt; no live custody |

BF-8.1 status note (2026-07-03): the package receipt/gate now has a typed
contract in `business-marketing-program-package.ts` for content, GEO,
outbound-assist, and full-funnel packages. Intake routing for new GEO/outbound
phrases still needs the typed/semantic selector path; do not expand the legacy
quick-win keyword list to claim those intents.

### BF-9 — Ops discipline (cross-cutting)

| Task | Description | Receipt/gate |
| --- | --- | --- |
| BF-9.1 | **Commitment ledger**: every promised deliverable/send is a tracked object (owner, due state, engagement ref) surfaced in weekly pipeline review — two silently-dropped partner commitments is the standing warning | Zero untracked commitments; the two owed make-goods shipped |
| BF-9.2 | **Pipeline ops** (= AW-0 A0.3): intake→scope→receipt-plan→close instrumented; weekly review exists | Weekly pipeline review artifact |
| BF-9.3 | **Compliance profiles per vertical**: encoded guardrails (consent channels, provenance, regulated-data handling, no-scraped-outreach, advertising-rule constraints) attached to vertical config packs and enforced at send/publish gates | Profile checks run on every outbound action |
| BF-9.4 | **Operator-minutes metric**: measured per accepted engagement, reviewed monthly, falling — the agency-trap falsifier wired as instrumentation | Monthly ratio series exists |

## 2. Dependency spine

```
BF-1.1 close the dead-end  ──────────────► gates everything customer-facing
   ├─ BF-1.2 conversational intake ─► BF-2.2 pay→provision→promise
   ├─ BF-1.3 vertical pages (real)      │
   └─ BF-1.4/1.5 instrumentation        ▼
BF-2.1 rate card ──► BF-2.2 ──► BF-2.3 recurring ──► BF-2.4 prepay tiers
        (AW-2 owner sitting arms 2.2/2.3/2.4; BF-2.5 can land first)
BF-3.1 ingestion ─► BF-3.2 redaction (shared w/ AW-3) ─► BF-3.3 context library ─► BF-3.4 private tier
BF-4.1 workroom surfacing ─► BF-4.2 service promises ─► BF-4.3 review gates ─► BF-4.4 document pipeline
   └─► BF-4.5 sites ─► BF-4.6 email/lists ─► BF-4.7 campaigns ─► BF-4.8 vertical packs
BF-5.* fulfillment agents  ← needs BF-4.2 promises + agent_definition.v1
BF-6.* connectors          ← independent lane; 6.5 invariant precedes any GA
BF-7.* prove               ← 7.1 KPI needs BF-1.4 + BF-4.2; 7.4 needs completed engagements
BF-8.* retain/multiply     ← needs BF-4 proven; 8.2 GEO can start now
BF-9.* ops                 ← starts now, permanent
```

**Start-now set (no dependencies):** BF-1.1, BF-2.5, BF-2.1 (copy prep),
BF-3.2 (redaction), BF-6.1 (connector skeleton), BF-8.2 (GEO), BF-9.1/9.2.

## 3. Milestones (receipts, not features — harmonized with ROADMAP_AFTER)

- **MB1 — The funnel has no dead-ends.** A stranger's `/business` signup
  reaches a provisioned, prefilled workspace with an invite and a follow-up,
  untouched by an operator. (Feeds MA0/MA1.)
- **MB2 — A dollar provisions itself.** Checkout → credits → workspace →
  service promise, fully wired; the rate card is public. (= MA2 lane.)
- **MB3 — A deliverable clears the engine.** One real document product or
  campaign generated from a real customer corpus, through the review gate,
  with a fulfillment receipt against a service promise.
- **MB4 — A customer watches it happen.** Workroom + KPI dashboard live for a
  paying customer; daily motion posted by a fulfillment agent; zero silent
  stalls that month.
- **MB5 — The engine retains.** First recurring/retainer receipt + first
  referred or case-study-attributed engagement. (Feeds MA6.)
- **MB6 — The engine multiplies.** First white-label client or second
  vertical onboarded as pure config; operator-minutes ratio measurably down
  two months running.

## 4. Invariants (inherited + engine-specific)

All ROADMAP.md §5 and ROADMAP_AFTER §6 invariants persist (exact-only
accounting, demand provenance, owner-arming serialized, safe defaults,
registry as scoreboard). Added:

- **Grounded or it doesn't ship.** No deliverable asserts what the corpus
  can't support; redaction precedes external inference for regulated data.
- **Approval before anything external.** Send/publish/file/spend all gate on
  recorded human approval; professional-review gates where licensure applies.
- **Config, not fork.** A vertical that needs bespoke screens is a design
  failure to fix, not a precedent.
- **No demo theater in production surfaces.** Fake progress animations and
  seeded data presented as live are banned; honest empty states instead.
- **Commitments are objects.** A promised deliverable without a ledger row
  is a defect.
- **Client-identifying information never enters this repo** — vertical
  descriptors only; opaque refs in receipts and case studies.

## 5. Issue map

Epic: [#8073](https://github.com/OpenAgentsInc/openagents/issues/8073).
Task issues are sequential — BF-1.1 = #8074 through BF-9.4 = #8118:

| Workstream | Issues |
| --- | --- |
| BF-1 funnel core (1.1–1.5) | #8074 #8075 #8076 #8077 #8078 |
| BF-2 convert (2.1–2.5) | #8079 #8080 #8081 #8082 #8083 |
| BF-3 provision (3.1–3.4) | #8084 #8085 #8086 #8087 |
| BF-4 fulfill (4.1–4.8) | #8088 #8089 #8090 #8091 #8092 #8093 #8094 #8095 |
| BF-5 fulfillment agents (5.1–5.4) | #8096 #8097 #8098 #8099 |
| BF-6 connectors (6.1–6.5) | #8100 #8101 #8102 #8103 #8104 |
| BF-7 prove (7.1–7.4) | #8105 #8106 #8107 #8108 |
| BF-8 retain/multiply (8.1–8.6) | #8109 #8110 #8111 #8112 #8113 #8114 |
| BF-9 ops (9.1–9.4) | #8115 #8116 #8117 #8118 |
