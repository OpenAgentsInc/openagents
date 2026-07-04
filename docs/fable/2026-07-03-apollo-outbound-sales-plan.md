# Apollo Outbound Sales — The $25k / 48-Hour Agent-Readiness Pipeline Plan

Date: 2026-07-03 (v2 — supersedes the v1 plan in this file; warm-partner
expansion removed as already-in-motion, target raised to **$25,000 in
qualified pipeline within 48 hours**, and the whole motion rebuilt around
**audit-first outbound**: deliver each prospect a real agent-readiness
report on their own website inside the first email. **v3 addition
2026-07-04:** Campaign B — "Own your AI" (§11), the Reactor-targeted
founder-personal high-ACV track running beside the automated Campaign A.)

Status: sales-operations plan in the Fable lane. **Nothing here is a product
promise, served capability, public claim copy, or a published price.** The
registry (`docs/promises/`) governs claims; modeled price bands are internal
guidance until the owner publishes a rate card through the copy gates (AW-0
A0.2). No client-identifying information may enter this repo from this
motion — prospect data, audit reports, and lead lists live in Apollo and
private ops surfaces; this repo records only vertical descriptors, opaque
refs, and aggregates (ROADMAP_BIZ invariants; BF-9.2 privacy contract).

Governing frames: [`ROADMAP_AFTER.md`](./ROADMAP_AFTER.md) AW-0,
[`ROADMAP_BIZ.md`](./ROADMAP_BIZ.md) (BF-8.1 marketing packages include GEO;
BF-8.2 names GEO/agent-readable surfaces as our own acquisition motion —
this plan is BF-8.2 weaponized as outbound),
[`2026-07-02-agents-that-work-business-services-analysis.md`](./2026-07-02-agents-that-work-business-services-analysis.md),
[`2026-07-03-bf-9-2-weekly-pipeline-review.md`](./2026-07-03-bf-9-2-weekly-pipeline-review.md).

## 0. The one-paragraph plan

Last night we made openagents.com agent-ready in one overnight wave (commit
`55297c5deb`, 2026-07-03: real JSON at `/.well-known/mcp.json` and the
ai-catalog, a public unauthenticated discovery MCP server, robots.txt,
sitemap.xml, JSON-LD — all the things agent-readiness scanners like ora.ai
flag, all of which previously fell through to an SPA HTML shell that
scanners read as broken). Almost every company's website fails the same
checks today. So the outbound motion is: **run that audit on target
companies ourselves, at fleet scale, and lead every email with their own
report** — "here's what AI agents see when they visit your site, here are
the three things breaking you, we fixed the same things on our own site
overnight and here's the receipt; happy to put a $100 credit on the table
to get you started." The free report is the value-add, the $100 credit is
the conversation-opener, the **Agent-Ready Quick Win** (fixed-scope,
four-figure) is the first sale, and QA Swarm / marketing-GEO / retainer are
the upsells. Apollo supplies the targets (95 lead credits — hard budget),
the phones (160 direct-dial credits), and the sequence machinery; our own
fleet supplies the audits, which is the part no other cold email can copy.
Target: **≥ $25,000 in qualified pipeline** (defined in §7 — quoted
opportunities, not vibes) inside 48 hours. And the second product of the
window is the machine itself: per the Episode 246 heuristic ("if you build
some new system to help you do the thing you're trying to do, that system
is probably what your product should be"), this audit-led outbound stack
**is a leadgen engine we then sell to businesses** — §10. A second track
runs beside all of this: **Campaign B, "Own your AI"** (§11) — 15–25
hand-picked Reactor prospects, a personal email from Chris to each,
five-figure-plus deal shapes, with everything above folded in as
sweeteners.

## 1. Why this motion works (and why now)

1. **The pain is brand new and invisible to the buyer.** AI agents are
   becoming a real traffic and purchasing class, and almost no company knows
   their site serves agents a broken page. An audit that shows them —
   concretely, on their own domain — creates the "oh no" moment cold email
   almost never achieves.
2. **We are provably good at fixing it, as of last night.** The dogfood
   receipt is a public commit on our own production site: scanners flagged
   us, the fleet shipped the fix wave overnight, the surfaces are live and
   verifiable (`curl https://openagents.com/.well-known/mcp.json`). "We did
   this to ourselves last night, here's the diff" is registry-true and
   nobody else's cold email can say it.
3. **The fix is exactly fleet-shaped work.** Well-known JSON surfaces,
   robots/sitemap, llms.txt, JSON-LD/structured data, SSR/prerender for
   agent user-agents, MCP discovery endpoints — bounded, verifiable,
   parallelizable. This is the Coding Quick Win with a built-in demand
   generator.
4. **The audit itself is automatable at fleet scale.** The checks are
   deterministic HTTP probes plus a short rubric. One worker per target
   domain; 100 audits is an evening, not a week. The value-add in every
   email costs us approximately nothing — which is the entire asymmetry of
   this plan.

## 2. The offer ladder

| Rung | What they get | What it costs them | Purpose |
| --- | --- | --- | --- |
| 1. **Free report** (in the first email) | Their domain's agent-readiness findings: top 3 failures, one-line impact each | Nothing — no call required, no gate | The value-add that earns the reply |
| 2. **$100 starter credit** | Platform credit applied to any engagement, plus the full report and fix plan on a 15-minute call | A conversation | Owner-authorized in this directive; mechanics in §8 |
| 3. **Agent-Ready Quick Win** | Fixed-scope fix wave on their site: well-known agent surfaces, robots/sitemap/llms.txt, structured data, agent-readable rendering; verified before/after report | Modeled $1.5–5k, quoted on the call | The first sale; days not weeks; human-review gate before anything ships |
| 4. **Upsells** | QA Swarm Audit (prove the site works, continuously), marketing/GEO package (BF-8.1 typed contract exists), monthly retainer | Modeled $2–10k/mo bands | Where pipeline compounds past the window |

Claims discipline for every rung: describe **their** site factually (every
finding is a reproducible probe), describe **our** work with receipts
(commit refs, live URLs, exact token accounting), promise **scope**, never
outcomes — no traffic/ranking/revenue guarantees, no "AI agents will buy
from you" claims, no implication the fix is self-serve (delivery is
operator-assisted), no published rate card (quote live on the call). If a
prospect conversation needs a claim the registry doesn't back, the claim
routes through `docs/promises/` first, not into the email.

## 3. The audit engine (build in hours 0–6, reuse forever)

We do **not** depend on ora.ai to generate reports at scale — we replicate
the check class ourselves (we just implemented the receiving end of every
one of these checks, so we hold the rubric). ora.ai stays useful as the
owner's spot-check and as category proof that "agent readiness" is a real
axis buyers can independently verify.

Per-domain probe set (deterministic, read-only, rate-limited, public URLs
only):

| Check | Probe | Failure mode we report |
| --- | --- | --- |
| MCP discovery | `GET /.well-known/mcp.json`, `/.well-known/mcp/manifest.json` | Missing, or SPA HTML shell served as JSON (the exact bug we fixed on ourselves) |
| AI catalog | `GET /.well-known/ai-catalog.json` | Missing/invalid |
| Crawl surfaces | `GET /robots.txt`, `/sitemap.xml` | Missing, unregistered routes, agent-hostile rules |
| LLM guidance | `GET /llms.txt` (and `/llms-full.txt`) | Missing |
| Structured data | Homepage JSON-LD (Organization, sameAs), meta/link tags | Absent or malformed |
| Agent-readable rendering | Fetch homepage with agent user-agents; compare content vs browser render | Empty SPA shell — agents see nothing |
| API discoverability | OpenAPI/manifest links, docs reachability | Undiscoverable to agents |

Execution: fleet fan-out, one bounded worker per domain (the standard
delegation path with pinned verify), each emitting a typed finding list +
a three-bullet human summary. Findings for a prospect's domain are **their**
confidential sales material: they go to Apollo/private ops, never into this
repo. The rubric, prober code, and our own domain's report are public-safe
and should land as normal repo work (this is also BF-8.2 infrastructure —
the same prober keeps *us* honest weekly).

Implementation status (#8266): the LG-1 report renderer now lives in
`@openagentsinc/agent-readiness`. It renders a report into a private
operator-review markdown view, first-email plain-text/HTML fragments with the
top three findings, and separate bump-step fragments with the held-back fourth
finding. Every rendered finding requires a one-line commercial context from
enrichment, HTML is escaped, and rendered prospect output is marked
`private_runtime_only`; only the `openagents.com` own-domain fixture can be
materialized as a repo-persisted case-study artifact. PDF/attachment export
remains deferred.

Personalization compounding: Apollo AI credits + org enrichment supply the
one-line business context ("you sell X; an agent trying to buy X from you
today sees a blank page") that turns a technical finding into a commercial
one.

## 4. Apollo account state and tool inventory

Connected 2026-07-03 via the Apollo.io MCP connector (OAuth, owner account).
Credit state — the hard budget:

| Credit type | Balance | 48h allocation |
| --- | --- | --- |
| Lead credits | **95** | ~95 hand-picked contacts, one per target company, chosen **after** their domain audit comes back bad (never spend a credit on an already-agent-ready company) |
| Direct-dial credits | **160** | Reveal ~40 for the worst-scoring/highest-value targets; the call channel is what compresses pipeline into the window |
| AI credits | **5,000** | Per-prospect commercial context for report framing |
| Export credits | 0 | Not needed |

Tool mapping:

| Step | Apollo MCP tools |
| --- | --- |
| Target company discovery (domains first!) | `apollo_mixed_companies_search` — pull companies + domains per segment; feed domains to the audit engine **before** revealing any person |
| Contact selection on failing domains | `apollo_mixed_people_api_search` scoped to the failing companies; reveal the one right person |
| Enrichment/context | `apollo_organizations_enrich`, `apollo_people_match`, `apollo_organizations_job_postings` |
| CRM state | `apollo_contacts_create/update`, `apollo_accounts_create/update` (mirror of the BF-9.2 queue, not a replacement) |
| Sequence + sends | `apollo_sequences_create`, `apollo_emailer_campaigns_add_contact_ids`, `apollo_emailer_campaigns_approve`, `apollo_emailer_messages_*` |
| Call/task blocks | `apollo_tasks_bulk_create/search/complete` |
| Budget watch | `apollo_usage_stats_credit_usage_stats` at hours 0/24/48 |

The ordering above is the plan's key efficiency trick: **audit domains
before spending lead credits.** Company search returns domains without
burning contact reveals; only domains that fail the audit earn a reveal.
Every one of the 95 emails then ships with a genuinely bad report —
relevance approaching 100%, zero credits wasted on healthy sites.

## 5. Segments (chosen for "an agent visiting your site matters")

| Segment | Company search spec | Why agent readiness bites them | Credits |
| --- | --- | --- | --- |
| **A. E-commerce / DTC** | Retail/e-comm keywords, 10–200 employees, revenue signal present | Agents are starting to shop; a blank SPA shell = invisible shelf | 30 |
| **B. SaaS / API-first products** | B2B SaaS, dev-tools, API keywords, 5–100 employees | Their buyers' agents evaluate docs/integrations; MCP discovery is becoming table stakes | 25 |
| **C. Agencies (marketing/web/SEO)** | Agency keywords, 2–30 employees | Three-fer: fix their site, white-label the audit for their client book (BF-8.3 shape), AND sell them the leadgen engine itself (§10) — agencies live and die on leadgen | 20 |
| **D. Marketplaces / booking / local-services platforms** | Marketplace, booking, directory keywords | Agent-driven search/booking flows fail hard on unreadable sites | 12 |
| **E. Bitcoin/Lightning companies** | Bitcoin/lightning keywords | Brand-true, pay-in-BTC native, culturally warm to agent commerce | 8 |

## 6. The 48-hour operating cadence

**Hour 0–4 — Build the audit engine + pull domains.**
Prober script + typed finding schema (bounded fleet lane); run the five
Apollo company searches; queue ~250–400 domains for audit (over-pull, since
healthy sites are discarded). Verify sending mailbox + schedule in Apollo.

**Hour 2–8 — Audit wave.**
Fleet fan-out across the domain queue. Triage output into: BAD (3+ failing
checks — outreach tier), MEDIOCRE (hold), HEALTHY (discard, zero spend).
Spot-check a sample against ora.ai so our findings and the tool the owner is
using agree in public.

**Hour 4–10 — Contacts + sends, wave 1.**
Reveal the right person at each BAD company (respecting the 95 budget),
generate per-prospect emails (template §7 + their three findings + one line
of commercial context), owner approves the template family once, sequence
goes live. Front-load segments A and B.

**Hour 8–24 — The phone block.**
Direct-dial pass on the top ~40 by (badness × company value). The pitch is
the report read aloud: "I'm looking at what an AI agent sees on your site
right now — it's a blank page. I emailed you the specifics." Book the
15-minute walkthrough on the call. Every reply answered within the hour;
every walkthrough gets the full report + the $100 credit + a quoted
Agent-Ready Quick Win with a receipt plan **on the same call** where
possible.

**Hour 24–48 — Bump + second dial pass + quote everything.**
Sequence step 2 (one bump, referencing one *new* finding held back for this
purpose); second dial block on non-connects; every live conversation driven
to `receipt_plan_sent` with a quoted amount — that is the pipeline event
the $25k target counts. Closes are welcome but the window's job is quoted,
qualified pipeline.

## 7. Pipeline definition and the $25k math (honest)

**Qualified pipeline** = opportunities at `scope_scheduled` or beyond in
the BF-9.2 queue **with a quoted amount attached** (receipt plan sent, or a
scheduled walkthrough where the package + band was stated and not
declined). Sends, opens, and unanswered replies are not pipeline. Every
row: `sourceRef=apollo_agent_readiness_<segment>`, opaque prospect ref,
quoted band, stage, next action.

Implementation status (#8267): `/business` signup and Khala intake-chat now
accept a bounded `sourceRef` token (`direct`,
`apollo_agent_readiness_<segment>`, `partner_expansion`,
`affiliate_<code>`), reject raw URLs/UTMs/contact data, and write aggregate
funnel events split by that token. BF-9.2 pipeline rows can link to the
originating signup row by `businessSignupRequestId`; the signup row records
the reciprocal `linkedPipelineRef`, so Apollo payback is readable from the
sourceRef slices without joining private prospect data.

The math to $25k, with audit-led relevance doing the lifting:

| Funnel stage | Conservative | Good |
| --- | --- | --- |
| Personalized audit emails | 95 | 95 |
| Replies (value-first, their own data: 8–15%) | 8 | 14 |
| Dial connects producing a real conversation | +4 | +8 |
| Walkthroughs booked/held in-window | 7 | 15 |
| Quoted opportunities (Quick Win $1.5–5k, avg ~$2.5–3k; agencies quoted white-label multi-site at $5–10k) | 6 × ~$2.8k ≈ **$17k** | 12 × ~$3k + 2 agency multi-site ≈ **$45k** |

$25k requires the good-not-heroic case: ~8–10 quoted opportunities with one
or two agency/multi-site quotes among them. The agency segment is the
pipeline multiplier — one agency saying "run this on my twelve clients" is
$10k+ of pipeline in a single conversation, which is why C gets 20 credits
and a guaranteed dial.

## 8. The $100 credit mechanics

Owner-authorized in the 2026-07-03 directive that produced this plan.
Boundaries so it stays a sales instrument and not a leak:

- Granted per **held conversation** (walkthrough attended), not per reply;
  cap the window's exposure at 25 grants ($2,500 max notional).
- It is platform credit on the existing credit ledger, applied against an
  engagement — non-transferable, no cash value, no crypto payout path;
  granting produces a receipt row like every other credit event
  (exact-only discipline; no invented balances).
- Implementation status (#8264): operators grant this through
  `POST /api/operator/business/pipeline/{pipelineRef}/starter-credit-grants`
  or
  `bun apps/openagents.com/scripts/operator-business-pipeline.ts grant-credit`.
  The default grant is exactly `10000` USD cents, the default window cap is
  `25`, and cap exceedance returns a typed refusal without minting a credit.
  Redemption receipt refs are linked with
  `/starter-credit-redemptions` and appear on the same BF-9.2 pipeline row
  receipt list as the grant receipt.
- Copy: "a $100 credit toward your first engagement" — never "free money,"
  never implying self-serve redemption while delivery is operator-assisted.
- If the ledger grant path needs a small piece of plumbing, that is a
  bounded fleet task in hour 0–8, not a blocker: the offer can be honored
  manually against the first invoice while the typed path lands.

## 9. Ops integration and standing guardrails

- Every touch lands in the BF-9.2 queue with the stage contract and source
  attribution; every promised report, credit, or deliverable gets a
  BF-9.1 commitment-ledger row (the two silently-dropped partner
  commitments remain the standing warning).
- Closes record as on-platform `business.*` quick-win payment receipts
  (A0.1); payment by conventional invoice, card, or Bitcoin/Lightning (all
  live).
- Warm-partner expansion is **out of scope here** — already in motion on
  its own track; suppress all existing partners, customers, and active
  intake rows from this sequence.
- Implementation status (#8265): sequence copy is now rendered through
  approval-gated template tooling, not one-off paste work. Operators render a
  draft from a BF-9.2 pipeline row plus LG-1 audit/finding refs through
  `POST /api/operator/business/pipeline/{pipelineRef}/outreach-drafts` or
  `bun apps/openagents.com/scripts/operator-business-pipeline.ts render-outreach`.
  Rendering enforces suppression before text exists: partner-routed rows,
  existing customer/partner suppressions, and active intake rows are refused.
  A send event can only be recorded through `/outreach-sends` after an owner
  approval receipt has been stored for the template version through
  `/api/operator/business/outreach/template-approvals`. Recorded sends append
  `receipt.business.outreach_send.*` to the same pipeline row, carry
  `sourceRef`, and enforce the configured per-mailbox daily send cap (default
  and hard maximum: 95).
- Template claims lint is part of the route/test boundary. The denylist blocks
  gated copy for self-serve delivery, pays-you/revenue-share loops, HIPAA or
  sovereignty posture, published prices, and referral payouts. Copy approval
  remains owner-gated even when a draft renders cleanly.
- Audit probes are read-only, rate-limited, against public URLs — never
  auth-gated surfaces, never load-generating. The report states facts a
  prospect can reproduce with curl; nothing speculative.
- Deliverability: one mailbox, ≤ 95 total cold sends across the window, one
  bump max, honest identification + working opt-out (BF-9.3 posture).
- Metrics reported as counts with `pending`/`not_measured` where
  instruments don't exist (BF-7.2 discipline). Pipeline is reported as
  *quoted pipeline*, never as revenue; the registry habit applies to sales
  reporting exactly as much as to product copy.
- Subscription no-resale stays non-waivable; we sell delivered outcomes.
- Credit budget is hard: 95 reveals, then stop — more credits is an owner
  purchase decision argued from this window's conversion data.

## 10. The second product: Autopilot Lead Gen (dogfood → product)

**Episode 247 named this product on camera** (`docs/transcripts/247.md`,
"Sell in Public"): *Autopilot Lead Gen*, introduced with "don't worry about
it, we're using it ourselves" — which is precisely this plan. The QA Swarm
arc is the template: the owner needed a QA fleet for his own say/ship gap,
built it, and it became the second product ("built first for Khala Code,
then added to the Autopilot suite for businesses"). This window runs the
same arc on sales. The stack we are assembling to hit our own $25k target —
targeted company discovery, an automated value-add analyzer
(agent-readiness audits today; any vertical-specific analyzer tomorrow),
personalized report-led sequences, dial lists, pipeline queue with receipts
— **is Autopilot Lead Gen v0**, and the buyer for it walks up to us at
every event: the agency operator who says "I need leadgen." She doesn't
want an audit; she wants *this machine pointed at her ICP*.

What makes ours sellable rather than "another Apollo wrapper":

1. **The value-add analyzer is the differentiator.** Generic cold email is
   dead; report-led email works because the first touch delivers something
   real. Our fleet can generate a *per-vertical* analyzer as a config —
   agent-readiness for web/SaaS, site-speed reports (the site-speed lane
   rubric already exists), QA findings (QA Swarm lite), inventory/SEO
   checks for e-commerce. "One engine, config per vertical" is already the
   ROADMAP_BIZ thesis; this is that thesis applied to the ACQUIRE stage.
2. **It runs as standing fulfillment agents, not a consultant.** The
   `agent_definition.v1` substrate (BA-A/B landed: definitions, cron
   triggers, budget caps, enforced toolsets) is exactly the shape of a
   per-customer leadgen agent: weekly target discovery → analyzer run →
   drafted sequence → **approval gate before any send** (BF-4.3 — nothing
   external without recorded human approval, always, and doubly so for
   outreach carried out in a customer's name).
3. **Receipts, as always.** Sourced-attribution pipeline rows, per-send
   ledger, quoted-pipeline reporting with `not_measured` honesty — an
   agency buying leadgen has never once seen an honest funnel report. Ours
   is the same instrument we run on ourselves (this very plan's §7 table).
4. **Existing typed contract to hang it on.** The BF-8.1
   marketing-program package family already includes **outbound-assist**;
   the leadgen engine is that package's delivery machinery, and BF-8.3
   white-label mode is its agency skin (their brand, per-client workrooms,
   our engine underneath).

Packaging (modeled, owner-gated like all prices): a **Leadgen Engine
Setup** quick win — ICP definition + analyzer config + first 100-prospect
report-led campaign, $2.5–5k — then a **monthly engine retainer**
($2–7.5k/mo: standing discovery, N reports/sequences per month, pipeline
reporting, approval-gated sends), with agency white-label multi-client
pricing on top. Compliance profile is load-bearing here (BF-9.3): consent
channels, no scraped-outreach beyond licensed data, sends only from
customer-owned mailboxes with their approval receipts — the permission
model is the product, in leadgen more than anywhere.

In-window actions this adds (cheap, because it's the same work):

- Build this window's tooling as **reusable engine parts** from hour zero:
  the analyzer as a config-driven prober, the sequence templates as
  parameterized families, the pipeline queue as the reusable instrument —
  not throwaway scripts.
- Add the leadgen-engine offer as the second talk track for Segment C
  calls and as the natural upsell wherever a prospect says "this email was
  great — how did you do this?" (answer: "this is our engine; we can run
  it for you"). Quoted engine setups count toward the $25k pipeline.
- After the window, the engine's own receipts (this plan's measured funnel)
  become the case study that sells it — A0.9 pattern, opaque refs.

Implementation issue map (filed 2026-07-04 under epic
[#8261](https://github.com/OpenAgentsInc/openagents/issues/8261), shared
with the Reactor RX lane): LG-1 prober
[#8262](https://github.com/OpenAgentsInc/openagents/issues/8262), LG-2
pipeline queue [#8263](https://github.com/OpenAgentsInc/openagents/issues/8263),
LG-3 starter credits [#8264](https://github.com/OpenAgentsInc/openagents/issues/8264),
LG-4 sequence tooling [#8265](https://github.com/OpenAgentsInc/openagents/issues/8265),
LG-5 report renderer [#8266](https://github.com/OpenAgentsInc/openagents/issues/8266),
LG-6 source attribution [#8267](https://github.com/OpenAgentsInc/openagents/issues/8267),
LG-7 Lead Gen agent definition [#8268](https://github.com/OpenAgentsInc/openagents/issues/8268),
LG-8 affiliate attribution [#8269](https://github.com/OpenAgentsInc/openagents/issues/8269),
LG-9 partner routing [#8270](https://github.com/OpenAgentsInc/openagents/issues/8270).

Implementation status (#8268, 2026-07-04): LG-7 now has a v0 standing
background-agent definition at
`apps/openagents.com/workers/api/src/autopilot-lead-gen-agent-definition.ts`.
It is `agent_definition.autopilot.lead_gen.v1`, uses the existing
`own_pylon` run/history substrate, carries weekday + manual triggers and
BA-B4 caps, and keeps per-customer ICP/analyzer/template/cap state in the run
payload rather than forking definitions. OpenAgents is dogfood customer #1 via
`lead_gen_config.openagents.customer_001.v1`; the public-safe receipt is
`docs/fable/2026-07-04-autopilot-lead-gen-agent-definition-receipt.md`.
The toolset is deliberately draft-only: report drafts, sequence-entry drafts,
operator-inbox escalation, receipt writing, and the Forge receive-pack scope
needed for dispatch. Email/Apollo send and activation refs are denied, and the
receipt records `sendAuthority.allowed=false` until a separate LG-4 approval
receipt exists. The useful Ora-style public URL/readiness rubric idea is folded
into the analyzer config ref only; no ora.ai integration or score claim is
created.

### 10.1 Episode 247 reconciliation (2026-07-04)

Episode 247 ("Sell in Public", `docs/transcripts/247.md`) landed after this
plan's v2 and confirms/extends it; the deltas to carry:

- **Product name is settled: Autopilot Lead Gen.** All engine work from this
  window ships under that name (Autopilot suite member, alongside QA Swarm),
  not as an unnamed "leadgen engine."
- **The affiliate lane is announced direction.** 247 attaches an affiliate
  program with referral codes wired into openagents.com surfaces (homepage,
  landing pages, Autopilot sites). Build posture: referral *attribution* is
  BF-8.4 (honest, end-to-end) and can start now; referral *payout copy*
  stays gated by the red `referral.refer_once_earn_forever.v1` record —
  attribution first, payout claims only when the ledger settles them.
- **Partner-org fulfillment joins the funnel.** ~Five design-partner orgs
  (agentic ad running, performance marketing, brand design) share
  fulfillment; OpenAgents is the front door. For this plan: a prospect whose
  need is ad-running/brand work is a *routable* opportunity, not a
  disqualify — record it in the pipeline queue with a partner-route flag,
  and count it toward the $25k only when a quoted arrangement exists. The
  formal machinery is the BF-8.5 overflow/peer design (its entry gates and
  opaque-ref receipts apply); until those gates pass, partner routing is
  manual, operator-approved, and receipt-planned per deal.
- **The Coding Agent Pool is the named delivery asset.** 247's framing —
  community agents pointed at our backlog, paid for coding work through
  mechanisms already built (never subscription-inference pooling; the
  no-resale invariant holds) — is what delivers the Agent-Ready Quick Wins
  this plan quotes. Sales copy may say "our agent pool builds it fast"
  only as far as the registry backs it: own-capacity delegation is proven
  at billions of exact-counted tokens; third-party-paid pool labor claims
  stay within the existing labor-market records.
- **Sell-in-public is the reporting format.** This window's funnel numbers
  (sends, replies, walkthroughs, quoted pipeline — §7 table, honest-state
  discipline intact) are candidate on-channel material. Public versions use
  aggregates and opaque refs only; no prospect-identifying data ever, and
  revenue graphs only from receipt-backed numbers.

## 11. Campaign B — "Own your AI" (Reactor-targeted, founder-personal, high-ACV)

Added 2026-07-04. Runs **beside** the agent-readiness campaign, not instead
of it. Same Apollo account, same LG-2 pipeline queue, opposite shape:
Campaign A is 95 automated report-led emails at four-figure quick wins;
Campaign B is **15–25 hand-picked accounts, a personal email from Chris to
each, and five-figure-plus deal shapes**. Source frames: the Reactor plan
(`2026-07-04-reactor-open-model-private-deployment-plan.md`), the Friedberg
transcript, the Mistral CEO post, and the harness-optimization audit
(`../research/2026-07-04-harness-optimization-evolve-the-harness-audit.md`).

### 11.1 The message: declare independence from the big labs

The narrative every touch carries, all of it registry-safe because it
describes the *engagement*, not certifications:

- **Your data is your moat, and the labs are mining it.** Closed providers
  now force data retention; as your teams connect models to business
  context, the labs see it, learn from it, and have "a track record of
  going after their most successful customers" (Mistral CEO, quoted as
  third-party validation, alongside Friedberg's "everyone is walking this
  path"). Every prompt your company sends a frontier lab is a business-
  intelligence leak you pay for.
- **Owning your AI is now practical.** Open models (Nemotron, Llama,
  GPT-OSS class) on your own hardware, under a signed model-provenance
  policy you control (US-origin-only if you want it), with your corpus
  never leaving your custody.
- **Frontier quality without frontier custody.** The published
  harness-evolution result (frozen open model 63.4→80.1 on a hard legal
  agent benchmark, ~7× cheaper, zero weight changes — attributed, never
  claimed as ours) means the gap buyers assume between "the lab's model"
  and "my model" is mostly harness, and harness is buyable.
- **Independence is a ladder, not a leap.** Assessment → pilot → managed —
  each step receipted, each step exit-friendly ("the switch button in your
  hand"), no rip-and-replace demanded up front.

### 11.2 The motion: personal email from Chris, not a sequence

This buyer (owner/CEO/GC/CTO of a data-rich mid-market company) does not
convert on automated sequences. The mechanics:

- **Target selection**: Apollo company search for data-rich, regulated or
  IP-sensitive verticals — legal, health/biotech, finance/insurance,
  defense-adjacent, data-heavy manufacturing/logistics — 25+ employees,
  buying signals (AI hires in job postings, AI-feature announcements).
  15–25 accounts max; every one hand-approved.
- **Per-target dossier** (Apollo enrichment + the RX-8 model-custody
  analyzer): which labs/SaaS AI features currently plausibly see their
  data (public signals only — subprocessor lists, DPA pages, AI-feature
  disclosures), plus one line of business context. Facts a reader can
  verify, no speculation dressed as finding.
- **The email is 1:1 from Chris's own mailbox** — founder-to-founder, short,
  references their specific custody posture, carries the independence
  narrative in two sentences, one CTA: a 30-minute "own your AI"
  conversation. We draft (one per target, staged in the private ops
  surface + NEEDS_OWNER queue); Chris edits and sends personally. Never an
  Apollo sequence, never a template smell. Follow-ups tracked as Apollo
  tasks; direct-dial reveals reserved for these accounts first.
- Campaign A's automated machinery still serves B: the agent-readiness
  report can ride along as a bonus attachment ("also, here's what AI
  agents see on your public site"), and replies land in the same LG-2
  pipeline with `sourceRef=own_your_ai`.

### 11.3 The offer ladder (modeled, owner-gated; quoted per-deal on the call)

Higher bands than Campaign A — this is infrastructure + independence, not
a quick win. Extends the Reactor §5 bands upward for this buyer class:

| Rung | Shape | Modeled band |
| --- | --- | --- |
| **Own-your-AI Assessment** | Data-custody audit (where your data goes today), model-policy workshop (their provenance constraints signed as `reactor.model_policy.v1`), hardware spec, independence roadmap | $7.5–15k |
| **Reactor Pilot + internal code forge** | Private node on their infrastructure, policy-conforming models, one real workload on their corpus — plus the **internal code forge starter**: their own coding-agent fleet + work-coordination patterns on their infra, so their team ships internal software with agents they own | $25–75k |
| **Sovereignty Retainer** | Managed ops, model refreshes within policy, harness evolution on their tasks (rung zero of the improvement ladder), eval regressions, KPI reporting | $5–20k/mo |
| **Full replatform** (later, gated) | The Mistral-shaped complete engagement: data liberation + private inference + access control + flywheel | quoted, six figures; only after pilots prove the motion |

**Sweeteners, not headliners:** everything Campaign A sells — agent-ready
site fixes, coding quick wins, QA Swarm audits — folds into these deals as
included deliverables ("and while we're in there, we'll fix what AI agents
see on your site"). The custom-software motion becomes the concession that
closes the infrastructure deal, not the deal itself.

### 11.4 Honest state and claims discipline

Reactor is planned (RX-1 records; issues #8271–#8281 open). Campaign B
therefore sells **assessments and scoped pilots** — engagements we can
deliver with today's machinery (audit, policy workshop, Hydralisk-lane
install per RX-3/RX-5 as it lands) — and the pilot pipeline paces the RX
build, exactly the staged-to-one-owner-action pattern. No "sovereign/
HIPAA/certified" words; no capability claims beyond scoped engagements;
published-price rules unchanged; quotes live only in 1:1 deal
conversations. Every quoted rung lands in LG-2 as pipeline with amounts —
one accepted Campaign-B conversation can carry the whole $25k target by
itself.

## 12. Owner actions (staged in NEEDS_OWNER.md; nothing here stalls the loop)

1. **Approve the email template family + call script** (Campaign A) — one
   sitting.
2. **Confirm the $100-credit boundaries** in §8 (or adjust the cap).
3. **Take the walkthrough calls** and quote inside the modeled bands —
   the scope call remains the owner's irreducible touchpoint.
4. **Campaign B**: approve the 15–25 target list and the modeled §11.3
   bands, then **edit and personally send the drafted "Own your AI"
   emails** from your own mailbox — the founder-personal send is the
   campaign; it cannot be delegated.
5. Optional accelerant: post the public-safe version of our own
   before/after agent-readiness story (the `55297c5deb` wave) — it is the
   case study every email points at, and it's already true.
