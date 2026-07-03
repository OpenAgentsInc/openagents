# Apollo Outbound Sales — The $25k / 48-Hour Agent-Readiness Pipeline Plan

Date: 2026-07-03 (v2 — supersedes the v1 plan in this file; warm-partner
expansion removed as already-in-motion, target raised to **$25,000 in
qualified pipeline within 48 hours**, and the whole motion rebuilt around
**audit-first outbound**: deliver each prospect a real agent-readiness
report on their own website inside the first email).

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
**is a leadgen engine we then sell to businesses** — §10.

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

## 10. The second product: sell the leadgen engine itself (dogfood → product)

The QA Swarm arc is the template: the owner needed a QA fleet for his own
say/ship gap, built it, and it became the second product ("built first for
Khala Code, then added to the Autopilot suite for businesses"). This window
runs the same arc on sales. The stack we are assembling to hit our own $25k
target — targeted company discovery, an automated value-add analyzer
(agent-readiness audits today; any vertical-specific analyzer tomorrow),
personalized report-led sequences, dial lists, pipeline queue with receipts
— **is a leadgen engine**, and the buyer for it walks up to us at every
event: the agency operator who says "I need leadgen." She doesn't want an
audit; she wants *this machine pointed at her ICP*.

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

## 11. Owner actions (staged in NEEDS_OWNER.md; nothing here stalls the loop)

1. **Approve the email template family + call script** — one sitting.
2. **Confirm the $100-credit boundaries** in §8 (or adjust the cap).
3. **Take the walkthrough calls** and quote inside the modeled bands —
   the scope call remains the owner's irreducible touchpoint.
4. Optional accelerant: post the public-safe version of our own
   before/after agent-readiness story (the `55297c5deb` wave) — it is the
   case study every email points at, and it's already true.
