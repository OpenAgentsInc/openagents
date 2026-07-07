# One Roadmap — Khala Code, Agent Computers, Company Brain, AI Employees

Date: 2026-07-07
Status: owner-directed overarching strategic roadmap. This document connects
four bodies of work into a single product ladder:

1. The **mobile-only MVP** and its substrate
   (`docs/fable/2026-07-05-khala-code-mobile-only-mvp-launch-audit.md`,
   epic #8467; `docs/khala-code/2026-07-06-agent-computers-strategy.md`,
   AC-1 #8503).
2. The **post-MVP direction doc** written earlier today
   (`docs/fable/2026-07-07-beyond-mvp-codex-agent-computers-and-ai-employees.md`
   — Codex on Agent Computers, multi-agent management from the phone,
   business integrations; lanes CX-*, AE-*, BI-*).
3. The **Agentic Society material**
   (`docs/agenticsociety/2026-07-03.md`, `-analysis.md`, `-integration.md` —
   field evidence of the market, the six-tuple export format, and the
   integration doc's Phase 0–6 / AE-x / CB-x roadmap).
4. The **blitz program** (private workspace docs, `~/work/docs/blitz/` —
   accepted-outcome packages, prefilled workspaces, the four-prong funnel,
   live design-partner deals, pricing, compliance guardrails; referenced
   here public-safe, no client names beyond what is already public).

Where the two prior lane numbering schemes overlap (this morning's CX/AE/BI
vs the integration doc's AE/CB phases), **this document is the
reconciliation and supersedes both numbering schemes** — the underlying
content of each stands, but new issues should be filed against the horizon
lanes named here (§4).

> **Update (later 2026-07-07):** the customer-facing product layer over
> these horizons — Khala Code mobile (App Store front door, IAP
> "minerals"), Khala Code desktop (the inverted agentic IDE), the
> openagents.com business dashboard, and **Reactor** (which the four-prong
> framing omitted; it is the custody dimension and its own offering, with
> **Reactor Zero** as our own internal deployment) — is articulated in
> `docs/fable/2026-07-07-product-suite-khala-code-openagents-com-reactor.md`.
> That doc amends this one's §5: the sovereignty ladder explicitly
> terminates at Reactor-as-product.
>
> **Update (2026-07-07, third):** Palantir's *Institutional Sovereignty in
> the Age of AI* report (15 steps) maps near-1:1 onto this roadmap; the
> analysis is
> `docs/fable/2026-07-07-palantir-institutional-sovereignty-smb-analysis.md`.
> Deltas it adds here: CB-1 (H3) adopts Blueprint's ontology-lite typed
> vocabulary (objects/properties/links, Action-Submission writes, Access
> Explanation); H5 gains corpus canaries; H6's assessment instrument
> upgrades to the 15-step rubric; the §5 ladder adopts "assurance level
> (structural vs contractual)" as its selling vocabulary.

## 1. The single thesis

Khala Code evolves along one line: **from a coding agent you dispatch from
your phone, to a roster of named AI employees running on your own agent
computers, grounded in your company brain, wired into your business systems,
with every action receipted** — and the same substrate that serves a
self-serve user at $10 of starter credits serves a design-partner engagement
at $32k, because verticals are configs (connectors + grounding corpus +
verification rubric), never forks. The tool (Khala Code) is the front door;
the employees are the product; the network (receipts → outcome stories →
forum identity → routed work → settlement) is the moat. Nothing in this
ladder requires abandoning anything shipped: every horizon is a
recombination of seams that already exist, in the order that each unlocks
the next.

## 2. Why we believe it: three independent sources, one product

The reason to commit to this ladder is that three bodies of evidence,
produced independently, describe the same thing:

**The market says it (Agentic Society, 2026-07-03).** Forty mostly
non-technical Austin operators; half the room already runs *named,
cron-scheduled, MCP-connected background agents* — hand-assembled on $8
VPSes, Hermes/OpenClaw harnesses, OpenRouter, Googled skills, and manually
provisioned M365 identities. Demonstrated ROI up to four orders of magnitude
(a ~$30/month outreach agent landing $200–300K projects). Their governing
ideology is literally "AI employees": onboarding, role-scoped permissions,
named personas, earned trust (read-only → write), a shared "company brain."
Their #1 unsolved problem is supply-chain trust ("never install a
third-party skill — regenerate and audit your own"). Their #1 operational
complaint is always-on hosting ("the agent dies when the laptop closes").
Every layer of their duct-tape stack is a product gap we can own.

**The sales motion says it (blitz).** The live design-partner lanes — legal
(solo-practice pilot in active negotiation; the productized firm-scale
funnel), marketing-agency white-label, Bitcoin e-commerce, physician
nonprofit, the sovereignty outbound campaign — independently demand the same
primitives: prefilled workspaces seeded from public data, per-vertical
intake, grounded business memory, per-customer service promises with
fulfillment loops, human-approval gates on regulated output, receipts and
provenance, and (load-bearing in every serious deal) a **private/sovereign
inference option**. The July framing is a four-prong funnel — (1) rapid
software, (2) lead gen, (3) AI employees, (4) company brain — with
OpenAgents as "the front door to agents" and partners filling prongs. The
hottest live deal is gated on prong 2 (the customer asked for a marketing
program before signing); prongs 3 and 4 are named roadmap.

**The codebase says it (openagents).** The substrate is disproportionately
already built: Agent Computers (Firecracker microVMs, admission, isolation
policy, lifecycle receipts, two-meter billing) one owner-gated proof from
live; the harness-agnostic `agent_definition.v1` record (toolsets, triggers,
budgets, escalation, event-ledger inbox) fully landed with a production
consumer; server-side provider-credential custody landed; a native CRM,
prefilled-workspace epics, per-customer service promises, behavior
contracts, the credits ledger, push, and the promise registry all shipped.
The gap analysis in the post-MVP doc holds: what remains is a credential
broker seam, one lane unification, a cockpit UI, and packaging.

When the market's folk practice, the customers' contract demands, and the
shipped architecture converge on one shape, the strategy risk is not
direction — it is sequencing and discipline. This document is about those.

## 3. The product ladder

Seven horizons. Each has a one-line identity, the lanes it absorbs, and the
single exit receipt that proves it. Horizons overlap in execution (lanes
parallelize) but each *exit* gates marketing copy for the next — the
promise-registry discipline applied to the roadmap itself.

| Horizon | Identity | Absorbs / reconciles | Exit receipt |
|---|---|---|---|
| **H0 Substrate** | A real mobile coding turn on our metal, credit-metered | #8503, #8477, Aiur #8500/#8501 (current MVP scope, unchanged) | The #8503 proof bundle |
| **H1 Your harness** | Bring your own Codex/Claude to your agent computer | CX-1..5 (post-MVP doc §6) | One mobile turn on the user's own Codex inside Firecracker, `tokenChargeMetered: false` |
| **H2 Standing employees** | Agents that run when your laptop is closed (the $8-VPS killer) | Post-MVP AE-1 = integration-doc Phase 1 (AE-1.1–1.3): unpark the definition cloud lane onto Agent Computers | One cron-triggered definition running nightly on an agent computer for 7 consecutive days, receipts each night, auto-pause proven |
| **H3 The employee &amp; the brain** | Named, promotable employees + the company brain object | Post-MVP AE-2..4 + BI-1 = integration-doc Phase 2 (employee object) + Phase 3 (CB-1 company brain); mobile Agents panel/inbox | One employee promoted observe→draft→act-with-approval via phone, each transition receipted; one brain serving two employees with disjoint slices |
| **H4 Templates &amp; integrations** | The hireable catalog, wired into business systems | Post-MVP BI-2..5 = integration-doc Phase 4 (AE-3 templates); BF-6 connectors; native CRM/email; the four-prong templates | Three templates listed, each with a receipted external outcome; one customer running two employees off one brain |
| **H5 Trust layer** | Provenance-receipted skills + input-path security | Integration-doc Phase 5 (AE-4/CB-2): skill registry, head-of-security template, untrusted-input authority ceiling | Registry live with our own templates' skills as first audited entries; input ceiling enforced in the behavior-contract sweep |
| **H6 Scale &amp; network** | The campaign, the partner network, the economy | Integration-doc Phase 6; blitz four-prong GTM; mastermind circuit; white-label operators; sovereignty tier arming | Falling operator-minutes per engagement across a growing cohort; first partner-org prong fulfillment receipted |

Lane-prefix reconciliation (for issue filing): **CX-*** keeps its meaning
from the post-MVP doc (BYO-harness). **AE-*** now means the merged
employee track (this doc's H2–H4 employee-side lanes; the integration doc's
AE-1.x/AE-2.x/AE-3.x numbering slots in unchanged). **CB-*** keeps the
integration doc's meaning (company brain). **BI-*** (business integrations)
from the post-MVP doc becomes the connector sub-track inside H4 and
inherits BF-6's specs. The blitz-side funnel/GTM work keeps its existing
homes (LG-*, BF-*, and the private blitz docs) — this roadmap consumes
their outputs, it does not re-home them.

## 4. The horizons in detail

### H0 — Substrate (unchanged; everything gates on it)

Current MVP scope exactly as tracked: #8503's live Firecracker proof with
the receipt bundle, #8477 writeback, Aiur credits console and ops views.
Nothing in this document adds scope to H0, and no lane below starts its
cloud-side work before the proof bundle exists. (The current blocker is
staging inference supply — an infra/owner action, tracked in
NEEDS_OWNER.md.)

### H1 — Your harness (CX: Codex, then Claude, on Agent Computers)

As specced in the post-MVP doc §2/§6: CX-1 provider-credential invariant +
broker contract, CX-2 mobile Codex connect (device-auth into the existing
`provider_account_token_custody` rail), CX-3 injection + first cloud Codex
turn, CX-4 harness/target selection UX with quota-aware `auto`, CX-5 Claude
parity. Two strategic additions from the new source material:

- **The mastermind confirms the wedge is bigger than convenience.** The
  room's standard stack is *multiple* model subscriptions plus OpenRouter
  juggled by hand; quota-aware rotation across a user's own connected
  accounts, with typed `account_exhausted` truth surfaced to the phone, is
  a capability nobody in their stack can duct-tape. BYO-subscription also
  collapses our token COGS to compute time — which prices H2's always-on
  employees at a level the $8-VPS crowd will pay.
- **Harness churn is the cautionary tale.** The room migrated OpenClaw →
  Hermes in under a year. We wrap and orchestrate; we never compete on the
  harness. CX work must keep the harness a swappable field
  (`agent_definition.v1.harness` already models this) and put the product
  identity in trust, hosting, receipts, and the economy — the layers that
  don't churn.

### H2 — Standing employees (the level-2→3 transition as one toggle)

The single highest-conversion feature for the observed market: **"your
agent doesn't sleep when your MacBook does."** Implementation is the
integration doc's Phase 1, verbatim: dispatch `agent_definition.v1` runs
through the same admission gate and metering rail as mobile turns
(AE-1.1); measure trigger latency before building warm pools (AE-1.2);
`maxCreditsPerDay` as payroll, surfaced per-employee (AE-1.3). The employee
is durable *state*, not a durable *process* — per-run microVM +
scratch-wipe receipts is already the right lifecycle for scheduled work.

This horizon retires the last architectural ambiguity between the
background-agents roadmap (`cloud_workroom` lane parked) and the Agent
Computers strategy: **the definition cloud lane IS the Agent Computer.**
One cloud execution substrate, two dispatch sources (composer turns,
definition triggers).

H2 is also where the cron-is-king insight lands as product: the mastermind
architecture (nightly cron + curated data + one human touchpoint) is
already the shape of `agent_definition.v1`'s cron trigger + escalation
policy. We are not inventing their practice; we are hosting it.

### H3 — The employee and the brain (the two product objects)

Two records, layered on the substrate, per the integration doc's Phases
2–3 with the post-MVP doc's mobile cockpit folded in:

- **`ai_employee.v1`** (AE-2.1): references a definition; adds persona
  (name, role title, soul/tone), **authority state**
  (`observe | draft | act_with_approval | act_within_policy`), and
  identity bindings. Promotion is a typed, receipted transition — the
  room's read-only-first folk doctrine as UI. Identity bindings generalize
  the SCM-broker pattern to mailbox/calendar grants scoped to the
  employee's *own* address (the "Ali has her own mailbox, never the CEO's"
  invariant as infrastructure) (AE-2.2).
- **`company_brain.v1`** (CB-1.1–1.3): named, owner-scoped Khala Sync
  collections with per-entry provenance; ingestion in trust-cost order
  (public scrape → drive/mail summaries → systems of record); role-scoped
  brain slices compiled into toolset policy the same way tool grants are
  ("each agent a genius in its own lane"). Khala Sync's cross-device
  property makes the brain the same object on phone, desktop, and web.
- **The phone is the manager's surface** (post-MVP doc §3.3 = AE-2.3): the
  Agents panel (roster, run history, live state from existing sync
  scopes), the event-ledger inbox as a manager queue, one-tap approvals
  via push deep links. The second product loop after coding sessions:
  *your employee drafted 3 emails; approve, edit, or reject — from your
  phone.* The pending behavior contracts
  (`agents_panel.run_status_indicators_truthful.v1`,
  `definitions.harness_swap.v1`) land with the panel, and authority
  scoping (`owner_self | shared_fleet | owner_operator`, the Artanis
  lesson) lands before the cockpit ships.
- **The prefill pipeline** (CB-1.4) is the bridge to GTM: intake →
  public-data research run → seeded brain + one starter employee in
  `observe` → intro receipt listing every source. This is the blitz
  prefilled-workspace play rebuilt on the brain object, and it must run as
  a fleet lane (one prospect = one automated run) because H6's campaign
  depends on it at scale.

### H4 — Templates and integrations (the catalog, the connectors, the CRM)

The six-tuple the mastermind proved — persona + permission stack + skills +
data seed + schedule + proof — becomes the **template** format (integration
doc AE-3): a definition preset + persona + required brain slices + trigger
schedule + verification rubric + the authority floor it ships at (always
`observe` or `draft`). Ship order follows evidence, not ambition:

1. **Outreach Rep / Leadgen Engine** — furthest along
   (`agent_definition.autopilot.lead_gen.v1` is live dogfood with send
   authority denied), independently validated by the room's best-economics
   build, and **the immediate commercial unblock**: the hottest blitz deal
   is gated on exactly this capability. The discipline goes in the
   template as policy defaults (low daily volume, dedicated identity,
   value-before-ask, approval-gated sends) — customers inherit the taste,
   not just the plumbing.
2. **Controller/CFO Agent** — connector-fed reporting; the finance cluster
   was a quarter of the room.
3. **Content Engine**, **Ops Triage**, **Knowledge Concierge** — each
   anchored to a proven archetype and a blitz segment.

The connector side (BI-* absorbing BF-6): connectors are **framework, not
per-customer code** — MCP grants on the same custody rail as CX
credentials, brokered into the microVM, short-TTL, per-connector typed
toolsets, no raw credentials or webhook bodies in model context (BF-6.5
law). GitHub sidecar first (BF-6.1 exists as spec), Slack second, CRM lane
generalizing the Apollo-as-mirror pattern (the customer's CRM stays the
system of record; the employee reads through a bounded toolset, drafts
into it, never sends without an approval receipt). The native CRM +
Resend email ledger already in the tree serve customers who don't bring
their own systems.

**Catalog gate, promise-registry style:** no template lists until it has
at least one receipted *external* outcome, and each template page carries
its live outcome ledger. This is the honest version of the mastermind's
post-it grammar ("my agent did X and got outcome") — same format, plus the
one upgrade nobody in that room can fake: receipts. Self-reported numbers
never enter the catalog.

### H5 — Trust layer (the moat the room asked for by name)

The room's settled security advice — *never install a third-party skill;
regenerate and audit your own* — is a verdict on the entire current skills
ecosystem and an open goal for us (integration doc Phase 5):

- **Provenance-receipted skill registry** (AE-4.1): content hash, source,
  automated injection-audit receipt, human-legible capability manifest
  (reads/writes/spends), one-click regenerate-under-audit. Skills attach
  through the toolset compiler, so an unaudited skill physically cannot
  ride into an `act_within_policy` employee.
- **Head-of-Security as a built-in template** (AE-4.2): reviews every
  skill, connector, and authority promotion; findings are receipts in the
  owner's inbox. The room's `/security` job title, productized.
- **Input-path ceiling** (CB-2.1): any employee whose triggers include
  untrusted input (webhooks, inbox matching, scraped content) is hard-capped
  at `act_with_approval` for outbound/spend tools absent an explicit
  owner waiver receipt. The analysis doc's sharpest observation is that
  the market defends the install path and not the input path — being the
  vendor that *names* prompt injection in employee-metaphor language
  ("your employee can be socially engineered; here's their training") is
  differentiation the harness vendors haven't earned and the competition
  isn't incentivized to build.

### H6 — Scale and the network (GTM, partners, sovereignty, economy)

Where the product ladder meets the blitz machine:

- **The four-prong funnel is the packaging.** Prong 1 (rapid software) =
  Khala Code coding sessions — shipping now. Prong 2 (lead gen) = the
  Outreach Rep template + the LG-* engine — the immediate deal unblock.
  Prong 3 (AI employees) = H2–H4. Prong 4 (company brain) = H3's CB
  track. OpenAgents is the front door; partner orgs fill prongs they own
  better (the July partner-discovery motion), under referral/fold-in
  shapes with receipted attribution (LG-8/LG-9 rails exist).
- **Channels, in order of proof:** (1) the mastermind circuit — attend and
  share outcome stories, never pitch; the "catch up to level 3" assessment
  → prefilled-workspace funnel (integration doc §5.2) as the follow-up;
  (2) the blitz design-partner ladder (Quick Win → Sprint → retainer) for
  owners who want employees *delivered*; (3) white-label operators
  (agencies with client books) as the multiplier segment — the
  marketing-agency design partner is the live test of per-tenant branded
  surfaces; (4) the sovereignty outbound campaign for high-ACV
  private-deployment buyers.
- **Pricing composes from existing decisions:** usage credits (1 credit =
  $0.01, non-expiring, no subscription), volume bonus tiers on prepay,
  Bitcoin discount, the agent-computer compute rail (rate NEEDS_OWNER),
  BYO-subscription turns at compute-only cost, per-employee payroll
  budgets, and for sales-led deals the setup-fee + credit-grant split
  already proven on live checkout links. The mastermind's observed
  willingness to pay (~$30–130/month infrastructure for four-to-six-figure
  outcomes) prices H2 employees comfortably above our compute cost.
- **The network graduation** (come-for-the-tool doc, enacted): employee
  outcome ledgers → consented public outcome stories → forum identity →
  tips → routed work → settlement. The recognition economy the mastermind
  runs offline (trophies, shout-outs, playbook generosity) is the
  behavior our forum + tipping + receipts substrate hosts online — with
  money instead of applause.

## 5. The sovereignty axis (runs across all horizons)

The blitz evidence is unambiguous: every serious commercial lane — legal
(both scales), health, e-commerce, the enterprise outbound campaign —
treats **private/sovereign inference and data custody as the trust gate
and the paid upgrade**, not a nice-to-have. The Agent Computers strategy
already contains the answer, and it should be named as such:

- **Agent computer classes as the sovereignty ladder.** The placement
  contract's lane field (`cloud-gcp` | `cloud-shc`, with the SHC lane
  "already modeled" as a second placement target) and the strategy doc's
  explicit bare-metal upgrade path ("without changing anything above the
  control plane") generalize to: **org-cloud** (default, H0), **BYO
  subscription** (H1 — the model bill is the user's), **sovereign
  placement** (customer-owned GCP project / on-prem host running the same
  `oa-node` control-plane contract — the private-engine and
  private-LLM-workspace demands from the legal lanes), and **Reactor**
  (open-model private deployment, the RX-* lanes) as the far end. One
  control plane, one isolation contract, one billing model; where the
  metal lives becomes a *placement decision and a price tier*.
- **Redaction-before-inference** (BF-3.2; the room's pipeline-redaction
  practice; the physician lane's PHI gate) is the software half of the
  same promise and belongs in the brain's ingestion path (CB-1.2), not
  bolted per-vertical.
- This axis is also competitive positioning: the enterprise FDE
  competitor's pitch (agents on *your* systems, *your* cloud, no
  migration, everything logged) is the sovereign class of this ladder —
  we meet it from below with a self-serve floor they don't have.

## 6. Disciplines that hold at every horizon

These are the standing laws; every lane above inherits them:

1. **Config, not fork.** A vertical is connectors + grounding corpus +
   verification rubric. A template is a preset. A customer is a config
   row. If a cockpit needs bespoke screens per customer, the thesis is
   failing — stop and productize the bottleneck.
2. **Receipt-first, exact-only.** No self-reported numbers in catalogs,
   promises, or marketing. The mastermind's unaudited multiplier claims
   are the anti-pattern; our differentiation is that our stories carry
   proof.
3. **The authority ladder is typed, never prompted.** Personas and names
   are adoption UX; behind every name is a compiled toolset, an authority
   state, and an audit trail — or the employee metaphor is a liability.
4. **Subscription no-resale, forever.** Connected consumer accounts serve
   their owner's work only — never pooled, never routed to another user,
   never serving org demand. API-inference resale on our own commercial
   accounts remains the separately authorized path.
5. **Human-approval gates on regulated output.** Legal, health, finance:
   professional review before anything client-facing; bar/HIPAA/advertising
   compliance encoded in template policy, not left to operator memory.
6. **The agency-trap tripwire.** Operator-minutes per engagement must fall
   as engagement count rises. When the campaign works and operator-minutes
   rise anyway, the system we build to cope is the next product.
7. **Owner-gated green.** No promise flips green without owner sign-off
   and a dereferenceable receipt; templates, employee capabilities, and
   sovereignty claims all ride the same registry.
8. **What we don't copy** (from the room, explicitly): AI-written mass
   outreach, deliverability tricks, mass-application bots, unverified
   multiplier marketing, harness competition, personality-first agents
   without permission substance.

## 7. Near-term order (what this changes this week)

1. **Nothing about H0.** Finish #8503/#8477; the staging-inference and
   arming actions in NEEDS_OWNER.md remain the critical path for
   everything in this document.
2. **File the reconciled lanes.** CX-1/CX-2 (invariant + mobile Codex
   connect) can start against the custody rail now; AE-1.1 (cloud-lane
   unification) is spec-ready and lands the moment H0's proof exists.
3. **Pull the Outreach Rep template forward.** It is simultaneously the
   H4 catalog's first entry, the four-prong funnel's prong 2, and the
   unblock for the hottest live deal — the rare lane where product,
   evidence, and revenue point at the same artifact. Its constraint set
   (draft-only, approval-gated sends, volume caps) is already enforced
   contract.
4. **Name the sovereignty ladder in the Agent Computers language** (§5) in
   the next strategy-doc revision, so the sales-led lanes stop inventing
   per-deal custody stories and start quoting placement classes.
5. **Keep attending the rooms.** The mastermind circuit is doing our
   market education for free, in our vocabulary, with our proof grammar.
   One outcome story per event, one follow-up list, prefilled workspaces
   as the gift.

## 8. Open questions (carried forward, consolidated)

From the post-MVP doc, still open: OpenAI ToS diligence for custodied
credentials (CX-1); per-account concurrency semantics in the cloud;
definition-run rendering on mobile (thread vs activity feed); the naming
glossary before public copy. New from this synthesis: (a) whether the
sovereign placement class ships as managed-on-their-GCP first or
on-prem-node first (the legal lanes pull opposite directions — decide on
the first signed deal's shape); (b) how partner-org prong fulfillment is
receipted and revenue-shared without importing marketplace/settlement scope
early (LG-9's bookkeeping-only posture is the current answer; it will come
under pressure); (c) whether the company brain's ingestion connectors and
H4's action connectors share one grant model from day one (they should —
one custody rail, read/write as authority states — but the trust-cost
ordering differs and the UX may want them separate).

## 9. Summary

The market hand-assembles AI employees on rented VPSes and begs for trust
infrastructure. The sales motion sells accepted outcomes into verticals
that all reduce to the same config surface and all demand sovereignty
options. The codebase already contains the agent computers, the employee
record, the credential custody, the CRM, the receipts, and the promise
discipline. The roadmap is therefore mostly *sequencing*: prove the
substrate (H0), absorb the user's own harnesses (H1), keep employees
running while the laptop sleeps (H2), give them names, brains, and a
manager's phone (H3), bottle the proven patterns into a receipted catalog
wired to business systems (H4), sell the trust layer the market already
knows it needs (H5), and run the campaign through the rooms and partners
that are already teaching our vocabulary (H6) — one substrate, one config
discipline, receipts all the way down.
