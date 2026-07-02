# Meditations: The Business Fulfillment Engine (2026-07-02)

Status: synthesis + gap meditation. No promise state flips; no public copy
changes; no client-identifying information (design partners are referred to
by vertical only, per the standing staging-surface convention).

Companion: [`ROADMAP_BIZ.md`](./ROADMAP_BIZ.md) — the consolidated, issue-backed
build plan distilled from this doc and harmonized with
[`ROADMAP_AFTER.md`](./ROADMAP_AFTER.md).

## 1. What this doc digests

The private business-outreach lane (workspace `docs/blitz/`, read in full:
strategy docs, per-partner folders, the onboarding/intake capstone audit, the
factory concept, the funnel GTM spec, pricing docs, compliance guardrails)
plus the fable-folder execution corpus. The blitz lane holds seven live
demand signals:

- **Customer #1 — ourselves.** Six validated coding-agent problems
  (multi-account pool + routing, background/queued agents, mobile control,
  session continuity, admin cost controls, isolated cloud VMs). Mostly built
  at the runtime layer; not yet one product surface.
- **A legal design partner — the first paying customer.** A four-figure pilot
  covering: a private pre-drafting workspace on compute they control, trained
  on their own decades-deep document corpus; an automated
  intake→payment→draft→attorney-review→filing funnel for one bounded document
  product; a membership-subscription funnel; a review/admin interface; and a
  KPI dashboard as the scorekeeper for a value-share arrangement. Their
  renegotiation asks: contained scope, priced expansion tiers, and — pointedly
  — **a marketing program as a firm commitment**, because the automation only
  pays if lead volume rises.
- **A public legal vertical brand.** A shipped high-ticket funnel (VSL page →
  application → booking → follow-up email + qualification worksheet) with a
  verified-data spine, currently demo-wired (no real POST, placeholder
  booking/email).
- **A marketing-agency design partner.** Put the operator herself on
  autopilot, then productize her client-delivery method into repeatable
  packages: per-client workrooms, native sites, native email sequences +
  lists/forms, approval ladders, branded per-tenant dashboards on her own
  subdomain, and a handoff portal.
- **An e-commerce design partner (Bitcoin-only retail).** Inventory-aware
  agentic marketing ("$5 test campaign with a receipt"), one-interface
  multi-platform posting, SEO/site performance, ops math, private-AI option.
  **We promised a deliverable and never shipped it.**
- **A settlement-infrastructure partner.** Not a customer — a primitive:
  trustless Bitcoin escrow (release-on-verified-delivery) that belongs under
  our accepted-outcome unit as the settle step. **Also owed an unshipped
  promised deliverable.**
- **A physician-nonprofit intake exemplar (health vertical).** Triage of
  sensitive help requests with a hard PHI-redaction-before-inference
  requirement, 100% human review, coach matching, and Bitcoin-funded credits
  — the worked proof that the intake spec generalizes beyond legal.

Plus the generalization thesis: the "75/25" persona (operator whose judgment
is the 75%, taxed by a repeatable 25%), the underserved-middle ICP, and the
one-product claim — **every vertical is a config (connectors + grounding
corpus + verification rubric), never a fork.**

## 2. The shape that satisfies all of them

Strip the verticals and one machine appears. Every customer above — and every
future one fitting the persona — is served by the same nine-stage engine:

```
ACQUIRE → INTAKE → QUALIFY → CONVERT → PROVISION → FULFILL → PROVE → RETAIN → MULTIPLY
 (content,  (/business, (typed spec, (pay →     (prefilled  (factory:  (receipts, (subscr., (white-label,
  GEO,       vertical    Sucky-25,    credits →  workspace,  outcome    KPI dash,  briefing,  referral,
  verticals) pages,      rate card)   workspace, corpus +    lifecycle, case       channel,   marketplace,
             conv. AI)                promise)   redaction)  agents)    studies)   expansion) settlement)
```

The meditation in one line: **we have built most of the FULFILL substrate and
almost none of the connective tissue on either side of it.** The fleet can
outbuild any backlog (ROADMAP.md's machine, proven), but a buyer today cannot
travel from `/business` to a receipted deliverable without an operator
hand-carrying them across at least five manual seams.

## 3. What exists (do not rebuild)

- **Intake front door**: `/business` landing + signup form (phone field,
  opt-in shared channel) is live; the dynamic `/autopilot` onboarding Batch 1
  shipped (Khala-driven intake conversation, typed-component SSE channel,
  closed component catalog, credit-kickoff card, one vertical overlay).
- **Money**: Stripe Checkout + webhook + credit ledger (secrets-gated),
  cost-preview, referral/revshare payout ledger, volume-bonus prepay schedule
  (internal offer doc), Bitcoin/Lightning rails live elsewhere in the product.
- **Workspace machinery**: private team/project workspaces, invite fanout API,
  deny-by-default member gating, prefilled-workspace + engagement-tracking
  primitives, operator runbooks.
- **Fulfillment substrate**: the fleet + Forge lanes + EXECUTION.md
  discipline; accepted-outcome contract schema and the full omni-workroom
  family (lifecycle, evidence bundles, public proof bundles, kind templates)
  — **built at the schema/service layer, unsurfaced (no routes, no UI)**;
  Blueprint programs + context packs; native email-campaign engine; sites
  builder services (no UI); agent-runs; decisions/approvals.
- **Proof**: promise registry + verifier, evidence bundle schemas, exact
  token accounting, public counters.
- **Verticalized QA**: the QA Swarm product plan (first sellable
  service package) and ROADMAP_QA's verification machinery.

## 4. What is still missing — the honest gap list

Ordered by where it bites in the funnel:

1. **The funnel dead-end.** `/business` signup creates a request row and
   stops — no enrichment, no workspace, no invite, no follow-up sequence. The
   single highest-leverage seam in the company: everything downstream is
   unreachable without it.
2. **Payment→provision→promise wiring.** Checkout success does not yet create
   the prefilled workspace + per-customer **service promise**. The decided
   design (credits kickoff as process trigger; service promises homed on
   accepted-outcome contracts with `committed_deliverables`, SLA terms, and a
   mirrored fulfillment-receipt verifier) is specced, undone.
3. **A published rate card.** Three buyable packages with fixed scopes and
   receipt plans (`ROADMAP_AFTER` A0.2). Nothing self-serve is priced today;
   volume-bonus credit tiers exist as an internal offer only.
4. **Corpus ingestion + redaction.** No scoped read-only connectors (cloud
   drive, mail, calendar), no PII/PHI redaction service in the tree — yet
   redaction-before-inference is the hard trust gate for legal, health, and
   any regulated vertical, and it is the same service AW-3 (consent/capture)
   needs. One build, two workstreams served.
5. **The workroom surfacing gap.** The entire omni-workroom family is
   type-complete and route-absent. Client-visible fulfillment — deliverables,
   approvals, evidence, handoff — has no surface a customer can log into.
6. **Deliverable generation pipelines.** Drafting from the customer's own
   templates/corpus (document products, brand/marketing packages,
   inventory-aware campaigns) exists as demos with hardcoded animations, not
   as workflows with real generation, real review gates, real receipts.
7. **Native sites + email + lists as first-class fulfillment outputs.**
   Builder services and campaign engine exist; authoring UI, forms/lists
   schema, and the site→list→sequence loop do not.
8. **Professional review gates.** The approval ladder (draft → suggest →
   execute-with-approval → trusted) and a professional-reviewer role
   (attorney/practitioner sign-off with receipts) — schema exists in
   decisions/workroom lifecycle; the reviewer-facing surface does not.
9. **Fulfillment agents.** The cron loop that services each promise (CRM
   load, stakeholder flagging, daily forward motion, client comms) is
   designed, not built. It should be built **on the harness-agnostic
   agent-definition record** (name/goal/allowed-toolset/triggers) from the
   background-agents audit, so fulfillment agents are the first production
   consumers of `agent_definition.v1`.
10. **Customer KPI dashboards.** The scorekeeper deliverable (baseline
    snapshot + live funnel/revenue metrics) that the paying partner's
    value-share literally depends on. Also: locked, auditable metric
    definitions for the factory view.
11. **Connector lane.** The connector sidecar (source-verified events, GitHub
    first, then the shared-channel platform), social publishing (X first),
    e-sign, and client-owned payment accounts (the legal funnel charges on
    the *customer's* processor account, not ours — a Connect-style seam we do
    not have).
12. **The marketing program as a sellable add-on.** The paying partner's
    renegotiation ask, and the agency partner's entire product: campaign
    packages (content, GEO, outbound assist) delivered by the same engine.
    GEO for ourselves is also the acquisition motion (`/business` traffic).
13. **A commitment ledger.** Two design partners were promised deliverables
    that silently never shipped. That is an ops-discipline hole, not a
    platform hole: promised sends must be tracked objects with owners and
    due states, surfaced in the weekly pipeline review.
14. **Case-study + demand engine.** Every engagement should yield a
    public-safe writeup with real receipts (A0.9); vertical landing pages
    (legal brand live first) wired to real application POSTs, booking, and
    follow-up email — on the newly optimized landing-page stack (site-speed
    lane budgets apply to these pages too).

## 5. Which fable docs bear on this (the harmonization map)

- **[`ROADMAP_AFTER.md`](./ROADMAP_AFTER.md)** — the governing frame.
  ROADMAP_BIZ **is the infrastructure build-out underneath AW-0** (the
  services engine, primary revenue) and shares single tasks with AW-2 (the
  first-dollar spine: same Stripe arming sitting), AW-3 (A3.1 redaction is
  the same service as our ingestion redaction), AW-6 (A6.4
  services↔product compounding; A0.9 case studies), and AW-8 (pricing).
  Where ROADMAP_AFTER states the *proof obligation* ("a price on a page",
  "first on-platform `business.*` receipt"), ROADMAP_BIZ states the *plumbing
  that makes the proof repeatable*. Kill-criteria alignment: the agency-trap
  falsifier (operator-minutes per engagement must fall) is the design
  constraint on every BF workstream — each one must convert operator minutes
  into fleet compute.
- **[`ROADMAP.md`](./ROADMAP.md) + [`EXECUTION.md`](./EXECUTION.md)** — the
  delivery machine and its operating procedure. Every BF issue is executed
  per EXECUTION.md (fleet delegation, clean worktrees, reviewed PRs,
  token-verified runs). WS-dependencies: the cockpit and status spine are
  where fulfillment agents and engagement lanes become visible.
- **[`ROADMAP_QA.md`](./ROADMAP_QA.md) +
  [`2026-07-02-qa-swarm-product-plan.md`](./2026-07-02-qa-swarm-product-plan.md)**
  — the verification ladder (machine test → replay → judge → human) that
  A0.7 applies to client work, and the first productized service package
  (QA Swarm) that should appear on the rate card.
- **`2026-07-02-agents-that-work-business-services-analysis.md`** — the
  services business model (packages, price bands, scaling levers) that the
  rate card and pipeline ops implement.
- **`2026-07-02-come-for-the-tool-stay-for-the-network.md`** — the user
  motion; services engagements feed its demand evidence and its first paid
  users.
- **`2026-07-02-harness-agnostic-background-agent-definitions-audit.md`** —
  `agent_definition.v1` is the substrate for fulfillment agents (BF-5): a
  service promise's cron loop is exactly a stored agent definition with an
  enforced toolset and an escalation path.
- **`2026-07-02-amp-orbs-adaptation-audit.md`** — the isolated per-customer
  compute story (workrooms, repo lifecycle hooks, snapshots) that backs the
  sovereign/private tier sold to legal/health customers and customer #1's
  "isolated cloud VMs" problem.
- **`2026-07-01-artanis-fleet-administrator-audit.md`** — Artanis as delivery
  manager (A0.7): the supervisor role over engagement lanes once BF-4/BF-5
  give it typed objects to supervise.
- **`2026-07-02-site-speed-lane-spec.md` + the theme-reset audits** — the
  optimized landing-page stack the funnel pages must ride (perf budgets apply
  to `/business` and vertical pages; one uniform theme).
- **`2026-07-01-product-promises-khala-code-launch-alignment.md`** — copy/
  promise gates: no BF surface ships copy implying capability whose backing
  record is red/planned; per-customer service promises may not commit
  deliverables backed by red capabilities.
- **`2026-07-02-khala-code-install-path-audit.md` /
  `2026-07-01-khala-code-summary-and-analysis.md`** — the tool motion's
  funnel discipline (install → activation instrumentation) that BF-1 copies
  for the business funnel.

## 6. Design principles carried into ROADMAP_BIZ

1. **One engine, config per vertical.** Connectors + grounding corpus +
   verification rubric are the only per-vertical slots. If a vertical needs
   bespoke screens, the thesis is failing.
2. **Grounded or it doesn't ship.** Nothing asserted or published without the
   customer's own corpus/data behind it; redaction before external inference
   for regulated data, always.
3. **Approval before anything external.** Human sign-off gates every send,
   publish, filing, and spend; professional-reviewer gates where a license is
   on the line. The permission model is the product.
4. **Receipts everywhere.** Every stage emits a receipt someone outside the
   company could have caused; the registry is the scoreboard; demand
   provenance (internal vs external) typed at the ledger.
5. **Operator minutes → fleet compute.** Every workstream must reduce the
   marginal operator cost of an engagement, or it is agency-building.
6. **Honest surfaces.** No fake animations, no seeded data presented as live,
   no scarcity/projection theater outside clearly-labeled vertical funnels.
7. **Commitments are objects.** A promised deliverable that isn't tracked is
   a future apology.
