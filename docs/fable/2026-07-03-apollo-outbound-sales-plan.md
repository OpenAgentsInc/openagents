# Apollo Outbound Sales — The $10k / 48-Hour Plan

Date: 2026-07-03
Status: sales-operations plan in the Fable lane. **Nothing here is a product
promise, served capability, public claim copy, or a published price.** The
registry (`docs/promises/`) governs claims; modeled price bands below are
internal guidance only until the owner publishes a rate card through the copy
gates (AW-0 A0.2). No client-identifying information appears in this doc or
may ever enter this repo from this motion — prospect and customer data lives
in Apollo and private ops surfaces; this repo records only vertical
descriptors, opaque refs, and aggregates (per ROADMAP_BIZ invariants and the
BF-9.2 pipeline privacy contract).

Governing frames: [`ROADMAP_AFTER.md`](./ROADMAP_AFTER.md) AW-0 (services
engine, primary revenue), [`ROADMAP_BIZ.md`](./ROADMAP_BIZ.md) (funnel
contract, BF-9 ops discipline),
[`2026-07-02-agents-that-work-business-services-analysis.md`](./2026-07-02-agents-that-work-business-services-analysis.md)
(offer shape and price bands),
[`2026-07-02-qa-swarm-product-plan.md`](./2026-07-02-qa-swarm-product-plan.md)
(the QA Swarm package),
[`2026-07-03-bf-9-2-weekly-pipeline-review.md`](./2026-07-03-bf-9-2-weekly-pipeline-review.md)
(queue states every lead must land in).

## 0. The one-paragraph plan

The goal is **$10,000 in closed sales within 48 hours**, and the honest math
says cold outbound alone cannot do that — cold email reply cycles are longer
than the window. So Apollo is deployed as three lanes ranked by expected
value inside the window: **(1) enrichment and execution support for warm
expansion** (the design partners who have already asked for more, and
existing `/business` intake rows — this lane carries most of the $10k),
**(2) direct-dial conversations** (Apollo's 160 phone credits are the only
cold channel fast enough to produce a same-day scope call), and **(3) a
tightly targeted 95-contact cold email sequence** across five segments,
which realistically contributes zero to one close inside 48 hours and a
scope-call pipeline that pays in week two. The unit sold is the
accepted-outcome engagement — Quick Win, Sprint, or QA Swarm Audit — priced
per-deal on a scope call (the rate card is unpublished and owner-gated), paid
by conventional invoice, card, or Bitcoin/Lightning (all live), and recorded
as an on-platform `business.*` receipt (AW-0 A0.1). Every touch lands in the
BF-9.2 pipeline queue with source attribution, and every claim in every email
stays inside registry state.

## 1. What we are selling (and what we must not say)

The offer set, mapped to registry-safe claims:

| Package | Shape | Modeled band (internal; quote per-deal) | Registry anchor | 48h fit |
| --- | --- | --- | --- | --- |
| **Coding Quick Win** | Fixed-scope, fixed-price software deliverable (a feature, an integration, an automated workflow, a site), days not weeks, human-review gate, receipt plan agreed up front | $1–5k flat | `business.coding_quick_win.v1` (yellow; four-figure deals already sold, owner-reported) | **Best.** Small enough to say yes to on one call |
| **QA Swarm Audit** | Bounded swarm run against the buyer's app: scenario corpus, monkey/explore night, perf baseline, findings as reproducible distilled tests, shareable evidence URL | Quick-win band | `qa.agentic_qa_runner.v1` (yellow); QS packages owner-gated | **Strong** for dev-tool/SaaS buyers; the machinery is built and the dogfood engagement is the proof |
| **Sprint** | A week of fleet capacity against a backlog, daily human checkpoint | $5–15k | services doc §3 | Possible as an upsell on a hot call; usually closes past 48h |
| **Retainer / marketing program** | Standing monthly lane (incl. the BF-8.1 content/GEO/outbound packages) | $2–10k/mo | `business-marketing-program-package.ts` typed contract exists | Sell in the window only to warm partners who already asked |

Claims discipline (non-negotiable, from the promise registry and copy gates):

- **Say**: fixed scope, fixed price agreed on the call, delivered in days,
  human review before anything ships, every deliverable arrives with
  verification receipts and exact accounting, pay by card or Bitcoin.
- **Never say**: "your coding agent pays you" (planned), self-serve delivery
  (operator-assisted), marketplace/subcontracting (deferred design),
  HIPAA/sovereign-compute availability (BF-3.4 is planned), referral payouts
  (red record), guaranteed outcomes, or legal *advice* (legal vertical is
  workflow-assistance only — the pack's own boundary).
- **No published prices.** The rate card is owner-gated (A0.2). Cold copy
  sells the scope call; the owner quotes on the call, which is how the
  existing four-figure deals were already sold. A price in a 1:1 negotiation
  is a deal term, not public claim copy — but it stays out of templates and
  out of this repo.

## 2. Apollo account state and tool inventory

Connected 2026-07-03 via the Apollo.io MCP connector (OAuth, owner account).
Credit state at connect — this is the hard budget for the window:

| Credit type | Balance | What it buys | 48h allocation |
| --- | --- | --- | --- |
| Lead credits | **95** | Email reveals on net-new prospects | ~95 cold contacts across 5 segments — quality over volume; no spray |
| Direct-dial credits | **160** | Phone numbers | Reveal ~30–50 for A-tier prospects and warm-lane contacts; the calling channel |
| AI credits | **5,000** | Research/personalization | Per-prospect account research to personalize every first line |
| Export credits | 0 | CSV export | None needed; work inside Apollo + our pipeline |

Tool mapping for each step of the motion:

| Step | Apollo MCP tools |
| --- | --- |
| Build target lists | `apollo_mixed_people_api_search`, `apollo_mixed_companies_search` |
| Enrich warm leads / signups | `apollo_people_match` / `apollo_people_bulk_match`, `apollo_organizations_enrich` / `bulk_enrich` |
| Research for personalization | `apollo_organizations_job_postings` (hiring pain = buying signal), org enrichment fields, AI credits |
| CRM state | `apollo_contacts_create/update`, `apollo_accounts_create/update` (mirror of, not replacement for, the BF-9.2 queue) |
| Sequences | `apollo_sequences_create/update`, `apollo_emailer_campaigns_add_contact_ids`, `apollo_emailer_campaigns_approve` |
| One-off sends | `apollo_emailer_messages_create/send_now/email_send_status` |
| Mailbox + schedule | `apollo_email_accounts_index`, `apollo_emailer_schedules_index` |
| Call/task blocks | `apollo_tasks_create/bulk_create/search/complete` |
| Budget watch | `apollo_usage_stats_credit_usage_stats` (check at hour 0, 24, 48) |

## 3. The three lanes, ranked by 48-hour expected value

### Lane 1 — Warm expansion (carries the number)

The blitz-lane demand signals are live buyers who already asked for things
(vertical descriptors only, per the standing convention):

1. **The legal design partner** made a marketing program a *firm
   renegotiation ask* (meditations §1). The BF-8.1 typed package contract
   now exists. This is a requested, scoped, priced-expansion conversation —
   the single fastest honest dollar available. Owner sends the offer (his
   relationship); Apollo's role is enrichment of the stakeholder set and the
   follow-up task cadence.
2. **The e-commerce design partner** is owed a make-good (the inventory-aware
   campaign, BF-4.7). Ship-the-make-good + convert to a paid campaign package
   in the same conversation. Owed work first, expansion second — the
   commitment ledger (BF-9.1) says so.
3. **The marketing-agency design partner**: the white-label/packages
   conversation (BF-8.3), scoped to a first paid package.
4. **Existing `/business` signup rows**: every `intake_received` row that
   never reached scope is a warm lead. Apollo bulk-enriches them
   (title/company/phone), and each gets a personal re-engagement email plus
   a call attempt inside the window.

Lane-1 realistic contribution: **$5–10k** (one to two expansion closes at
the four-figure band). This is where the 48-hour goal is actually won or
lost, and it is mostly owner conversation time, not tooling.

### Lane 2 — Named warm-ish prospects

The QA Swarm named first customer (the QS7 packet) and anyone else who has
publicly described wanting what we sell. The QS7 demo PR is **ready and
owner-gated** — clearing that gate during the window converts a sales
artifact that already exists into an active conversation. Rule: the external
send stays owner-approved per the QS7 packet; nothing in this plan overrides
that gate.

Lane-2 realistic contribution: $0–2.5k in-window; high strategic value.

### Lane 3 — Net-new cold via Apollo (builds the machine, tops up the number)

Five segments, 95 lead credits total. Selection criteria per segment tuned
to the "75/25 operator" ICP (underserved middle: real business, no internal
eng bench, operator whose judgment is taxed by repeatable work):

| Segment | Apollo people-search spec | Offer | Credits |
| --- | --- | --- | --- |
| **A. Dev-tool / SaaS eng leaders** | Titles: founder, CTO, VP/Head of Engineering, Head of QA; company 5–100 employees; software/dev-tools keywords; bonus signal: open QA/eng job postings | QA Swarm Audit ("point a swarm at your app; get a verdict, videos, and committed regression tests at a shareable URL") | 30 |
| **B. Agencies (marketing/dev/design)** | Titles: owner, founder, principal; agency keywords; 2–20 employees | Coding Quick Win ("we build the automation/integration/site your team keeps deferring — fixed scope, days") | 20 |
| **C. Small law firms** | Titles: managing partner, operations/practice manager; 2–20 attorneys | Legal workflow Quick Win (intake/document workflow automation — workflow assistance, never advice) | 15 |
| **D. E-commerce / DTC operators** | Titles: founder, owner, head of growth; retail/e-comm keywords; 2–50 employees | Site/campaign/ops Quick Win | 15 |
| **E. Bitcoin/Lightning companies** | Titles: founder, CTO, eng lead; bitcoin/lightning keywords | Coding Quick Win, pay-in-Bitcoin native (live rails; brand-true) | 15 |

Cold math, stated honestly so nobody grades this lane on fantasy numbers:
95 sends → plausibly ~50–60% open, ~4–8 replies, ~2–4 scope calls booked,
**0–1 closes inside 48 hours**. The direct-dial block is what compresses
that: a reveal-and-call pass on the ~30 best A/B-segment prospects can
produce same-day conversations email cannot.

Lane-3 realistic contribution: $0–2.5k in-window, plus the pipeline that
makes next week's number.

## 4. The 48-hour operating cadence

**Hour 0–2 — Arm the machine.**
Verify the sending mailbox in Apollo (`apollo_email_accounts_index`) and the
send schedule; build the five saved searches; bulk-enrich the Lane-1/Lane-2
warm list first (people match + org enrich); check credit balances.

**Hour 0–4 — Lane 1 out the door.**
Draft the three partner expansion offers and the signup re-engagement emails
(personal, not sequenced). These are owner-relationship sends: stage them as
drafts + a call list in `NEEDS_OWNER.md`, and get the owner sitting early —
this is the highest-leverage half-hour of the whole window.

**Hour 2–6 — Lists and sequence live.**
Run the five segment searches, hand-select to the 95-credit budget (no
auto-add of whole result pages), reveal, personalize first lines from
enrichment + job-postings data (AI credits), and load the two-step sequence
(email T0, bump T+24). Owner approves the five templates once
(`apollo_emailer_campaigns_approve`); then sends run on schedule. Suppress
every existing partner, customer, and active intake row from the sequence.

**Hour 6–24 — Calls and scope calls.**
Direct-dial block on revealed A/B-tier numbers plus warm-lane contacts.
Every reply gets an answer inside one hour during the window. Every scope
call happens same-day where possible, and the receipt plan (deliverable,
exclusions, price, review gate, timeline) is sent within two hours of the
call — `receipt_plan_sent` is the stage that closes deals, per BF-9.2.

**Hour 24–48 — Bump, second call block, close.**
Sequence step 2 fires; second dial pass on non-connects; every accepted
receipt plan converts to payment the same conversation — conventional
invoice, Stripe, or Bitcoin/Lightning (all live; the services doc explicitly
allows conventional invoicing while the on-platform loop arms). Each close
is recorded as a quick-win payment receipt (A0.1 machinery) and its
deliverables land in the BF-9.1 commitment ledger before the window ends.

## 5. Messaging frame (template skeleton, claims-gated)

All five segment templates follow one skeleton; personalization is the first
line, not the pitch:

1. **One observed, specific fact** about their company (from enrichment/job
   postings — e.g. hiring for QA, shipping velocity signals).
2. **The offer in one sentence**: a fixed-scope deliverable, delivered in
   days by our agent fleet, human-reviewed before anything ships, with
   verification receipts — "you accept the work, or you don't pay for more."
3. **One proof point that is registry-true**: billions of exactly-counted
   tokens of real coding work through our own fleet; the fleet merges ~20
   reviewed PRs overnight on our own product; QA findings arrive as
   committed, re-runnable tests at a shareable URL. (All real, all
   dogfood-provenance — say "on our own product," never imply external scale
   we don't have. `proof.demand_provenance.v1` applies to marketing too.)
4. **One CTA**: a 20-minute scope call in the next two days, or "reply with
   the thing you keep deferring and I'll send a fixed quote."
5. Honest identification and a working opt-out. Low volume, hand-picked
   list, one bump maximum — this is a rifle, not a cannon (BF-9.3 posture:
   no scraped-outreach beyond licensed data, consent-channel discipline).

## 6. Pipeline and receipts integration (the part that survives the window)

Apollo is the prospecting surface, not the system of record. Every reply,
call connect, or booked meeting creates a BF-9.2 pipeline row:

- `sourceRef`: `apollo_outbound_<segment>` (A–E), `apollo_warm_enrichment`,
  or `partner_expansion` — so BF-1.4 source attribution can ever answer
  "did Apollo pay for itself."
- Stage transitions exactly per the BF-9.2 contract: `intake_received` →
  `scope_scheduled` → `scope_completed` → `receipt_plan_sent` →
  `closed_won`/`closed_lost` → `quick_win_started`.
- Every promised send or deliverable gets a commitment-ledger row (BF-9.1)
  — the two silently-dropped partner commitments are the standing warning.
- Closes are recorded on-platform as `business.*` quick-win payment receipts
  (A0.1) with opaque buyer refs. The $10k claim is only as real as those
  receipts; owner-reported closes without receipt rows repeat the exact gap
  A0.1 exists to close.
- Metrics stay honest: sends, opens, replies, calls, scope calls, receipt
  plans, closes — reported as counts with `pending`/`not_measured` where the
  instrument doesn't exist yet, never invented rates (BF-7.2 discipline).

## 7. Owner actions (staged in NEEDS_OWNER.md, never blocking the rest)

1. **Send the Lane-1 expansion offers** (partner relationships are the
   owner's; drafts staged). Highest-EV action in the window.
2. **Approve the five cold templates + the call script** — one sitting.
3. **Confirm per-deal pricing authority** for the window (quote inside the
   modeled bands on calls; rate card stays unpublished).
4. **Take the scope calls.** Until Artanis-as-delivery-manager, the scope
   call is the owner's irreducible touchpoint (services doc §4).
5. Optionally **clear the QS7 owner gate** — the demo PR is the best Segment-A
   sales artifact we own and it is sitting finished.

Per the standing rule, owner-gated steps never stall the loop: list-building,
enrichment, drafting, and pipeline plumbing all proceed while these sit.

## 8. Success definition and the honest downside

**Success:** ≥ $10,000 in `closed_won` engagements with payment receipts (or
signed receipt plans with payment scheduled inside 7 days) attributable to
the window, plus ≥ 5 scope calls booked and 100% of touches in the pipeline
queue with source attribution.

**The honest downside case:** warm expansion stalls on partner timing and
cold produces calls but no in-window close. Then the window yields: an
enriched warm list, five reusable segment searches, approved templates, a
running two-step sequence, a dialed-in call motion, and a BF-9.2 queue full
of attributed pipeline — i.e., the repeatable outbound machine AW-0 A0.3
requires, which was the missing acquire-stage instrument in ROADMAP_BIZ
(ACQUIRE was the one funnel stage with no active motion). Report it that
way if that is what happens; do not launder pipeline into revenue. The
registry habit — the company marks its own claims red — applies to sales
reporting exactly as much as to product copy.

## 9. Standing guardrails

- **No client-identifying information in this repo** — Apollo holds the
  lists; this doc and any follow-ups carry vertical descriptors, opaque
  refs, and aggregate counts only.
- **No promise-state flips and no public copy changes** from this motion;
  anything a prospect conversation surfaces as a needed claim routes through
  `docs/promises/` first.
- **Subscription no-resale** stays non-waivable; nothing in outbound implies
  reselling Codex/Claude subscription capacity — we sell delivered outcomes.
- **Credit budget is hard**: 95 lead reveals, then stop; more credits is an
  owner purchase decision with the pipeline data as the case.
- **Draft-first for anything owner-relationship**; sequenced cold sends run
  only on owner-approved templates.
- Operator-minutes discipline applies even here: templates, saved searches,
  and the sequence exist so the *second* 48-hour push costs a fraction of
  this one (BF-9.4 — the falsifier watches sales ops too).
