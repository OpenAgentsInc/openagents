# "Agents That Work." — The Business Services Engine

Date: 2026-07-02
Status: strategy analysis/opinion doc in the Fable lane. **Nothing here is
a product promise, served capability, public claim copy, or a published
price** — the registry (`docs/promises/`, `2026-07-01.3`) governs claims;
prices and packages below are *modeled recommendations*, labeled as such,
until the owner publishes a rate card through the copy gates. No client
or lead is identified; per the customer-one privacy contract
(`docs/blitz/forge/2026-06-17-customer-one-cohort-source-contract.md`),
commercial specifics stay behind opaque refs.

Companions: [`ROADMAP_AFTER.md`](./ROADMAP_AFTER.md) (updated alongside
this doc), the business analysis
(`2026-07-02-khala-code-business-opportunity-and-openagents-analysis.md`),
the tool/network strategy
(`2026-07-02-come-for-the-tool-stay-for-the-network.md`), the Forge
synthesis (`docs/forge/2026-06-28-forge-software-factory-synthesis.md`),
the `/business` intake spec and offering coverage
(`docs/business/2026-06-20-*`), the blitz factory lane
(`docs/blitz/forge/`), and Episode 239 (`docs/transcripts/239.md`).

---

## 0. The correction this doc encodes

The prior strategy docs over-indexed on squeezing paid usage out of Khala
Code — plan conversion, credits, per-token margin. That is real but it is
**not the primary near-term revenue model**. The primary near-term
revenue model is the one Episode 239 named ("Let's Make Money — closing
the revenue loop") and the blitz/Forge lane spent June building the
machinery for: **businesses hiring agents from the OpenAgents network to
get work done — above all, software built very fast.** The front door is
`openagents.com/business`. The tagline is **"Agents that work."** The
unit sold is not a seat, not a token, not a plan: it is a delivered,
verified, accepted outcome.

And this is not a hypothesis waiting for a first customer: engagements at
four-figure price points have already been sold (owner-reported;
commercial specifics private). The honest registry framing is that these
deals exist *ahead of* the on-platform receipt machinery — the
first-paid-receipt gates on `business.coding_quick_win.v1` and
`business.intake_quick_win_offering.v1` are now about **formalizing
revenue that is already arriving**, not conjuring revenue that isn't.
That inverts the burden of proof across the whole plan: the question is
no longer "will anyone pay?" but "how do we scale what people are
already paying for without becoming an agency?"

---

## 1. Why services-first is the right revenue model

**1. Businesses buy outcomes, not tools.** Everything in the OpenAgents
thesis says the unit of machine work is the accepted outcome — scoped in
advance, executed cheaply, verified, receipted, settled. A services
engagement is that unit *in its most natural commercial form*: a business
says what it wants, we deliver it, they accept it, they pay. No product
education, no behavior change, no waiting for a network. The services
motion is the thesis sold at retail.

**2. It monetizes what is already true instead of what is planned.** The
delivery capability is the single most-proven asset in the company: the
fleet demonstrably merges ~20 PRs overnight on its own backlog, the
Khala→Pylon→Codex delegation runs at billions of exact-counted tokens,
Forge's coordination substrate (work records, dispatch leases, verified
merges, receipts) is landing on `main` week by week, and the intake door
at `/business` is live. Meanwhile the tool has no release artifact and
the network economics are `planned`. Selling delivery *now* monetizes
the proven layer while the speculative layers mature.

**3. The margin structure is unusual — in our favor.** A conventional dev
agency's COGS is engineer salaries. Ours is: own-capacity fleet compute
(≈ $0 marginal — the runs settle no-spend on capacity we or the operator
already hold), a thin slice of paid-provider inference (cents), and
**operator/supervision minutes** — scoping, review, client comms. The
entire unit-economics question collapses to one number: *operator minutes
per accepted engagement*, and the entire scaling roadmap is the set of
things that push it down (verification ladder, Artanis supervision,
productized scopes, vertical templates). "Better/faster/cheaper" is not
marketing — it is what near-zero-COGS delivery with receipt-backed
verification structurally enables.

**4. It is the demand-discovery instrument everything else needs.** The
company's deepest open question is external demand. Paid engagements
answer it with the highest-quality signal money can buy: real external
repos, real acceptance judgments, real willingness-to-pay at real price
points — per engagement, per vertical. Every deal produces exactly the
dataset (task types, retry rates, acceptance rates, review minutes,
price tolerance) the accepted-outcome pricing model has been waiting
for. Services revenue is also demand-provenance gold: external dollars,
labeled, from day one.

**5. The brand asymmetry is real.** OpenAgents sells with an asset no
agency has: the work arrives with receipts. Exact token accounting, a
public promise registry that marks its own claims red, verification
evidence attached to deliveries, and payment rails that settle
contributor-side in bearer money. For a buyer, "we can show you exactly
what ran, what it cost, what was verified, and what you accepted" is a
trust story no body-shop can match — and it is the same story the
emerging procurement/audit wave increasingly demands. The honesty
architecture, built for the registry, turns out to be a *sales* asset.

**6. It feeds, rather than competes with, the tool and the network.**
Every engagement runs on the same substrate Khala Code orchestrates:
fleet delegation, Forge lanes, exact accounting, receipts. Services
engagements are the network's first customer-facing skin — the
demand-side wedge the GPUtopia lesson demanded. Delivered engagements
seed the solved-problem graph the plugin economy needs; satisfied
business clients are the natural first buyers of the paid-privacy plan
and the future self-serve Autopilot; and their referral relationships
are the Ep 239 refer-once mechanics' first honest substrate (kept
carefully clear of the red `referral.refer_once_earn_forever.v1` copy).

---

## 2. What exists today (honest inventory)

From the blitz/Forge/business corpus, the delivery stack layer by layer:

| Layer | State |
| --- | --- |
| **Front door** — `/business` page + `POST /api/public/business-signup` intake, Slack Connect opt-in, operator handoff | **Live** on prod; intake receipts recorded; 7-offering menu registry-pinned (`business.intake_quick_win_offering.v1` yellow) |
| **Scoping** — AI-run intake interview → Output Spec → receipt-planned budget | Specced (`2026-06-20-openagents-business-intake-spec.md`); operator-assisted today |
| **Workspace** — vertical prefilled workspaces (general, e-commerce, legal, marketing) seeded as drafts, invite-claimed, engagement-tracked | **Shipped** as operator tools; all three vertical packs yellow, authority-gated (no publish/send/spend without approval receipts) |
| **Execution** — Forge eight-stage production line over the fleet (Signal→…→Deploy); Khala→Pylon→Codex/Claude delegation; dispatch leases; Docker-isolated verification | **Real for coding** (the proven layer); factory dashboard live/seeded-tagged; coordination-store re-homing in flight (SU-4..8) |
| **Verification** — verify-green gates today; named ladder (machine test → replay → model judge → human review → owner acceptance) | Boolean today; the ladder is the named SU-5 gap — and the main COGS lever |
| **Delivery/acceptance** — human-review gate, artifact + evidence refs, quick-win receipt schema (`business-quick-win-payment.ts`, public paid-receipt projection) | Built; **zero on-platform paid receipts yet** |
| **Payment** — MPP rails (Lightning/USDC/card) live; card→credit purchase loop red pending owner-armed prod secrets | The known single gate; invoicing for services can also run conventionally while the on-platform loop arms |
| **Repeatability proof** — customer-one cohort machinery: 3–5 teams to `loop_completed` with privacy-reviewed completion bundles | Contract/tooling built; **0 completed bundles recorded** |
| **Demand** | Owner-reported closed four-figure deals (private); intake channel live; no published pricing |

The pattern is now familiar from every other lane: **machinery staged to
one owner action and one real receipt** — except here the buyer side is
not hypothetical.

---

## 3. The offer, productized (modeled — not published copy)

The current menu is honest but shaped like an inventory, not an offer.
"Agents that work." needs three or four buyable packages a business can
say yes to in one meeting. A modeled shape, price points illustrative
and deliberately in the already-validated four-figure band:

1. **The Quick Win** — fixed-price, fixed-scope, days-not-weeks. One
   well-scoped deliverable (a feature, an integration, a site, an
   automated workflow) with a receipt plan agreed up front and a
   human-review gate before anything ships. Modeled at **$1–5k** flat.
   This is the wedge: cheap enough to be a stupid-not-to-try decision
   against any agency quote, scoped enough to protect the margin, and
   it maps 1:1 onto the existing `business.coding_quick_win.v1` +
   quick-win receipt machinery.
2. **The Sprint** — a week of fleet capacity against a backlog, with a
   daily human checkpoint. Modeled **$5–15k**. This is "software built
   very fast" as a product: the buyer watches the factory dashboard,
   accepts work item by item, and the engagement's own receipts are the
   sales collateral for the next one.
3. **The Retainer / "On Autopilot"** — the intake spec's own endgame:
   a standing lane (vertical pack or custom) run continuously, monthly
   fixed + metered overage. Modeled **$2–10k/mo**. This is where
   services revenue becomes recurring and where the account naturally
   grows into self-serve Autopilot as it productizes.
4. **(Later) The Build** — larger fixed-bid builds ("full software" from
   the whiteboard's response box), quoted per engagement, only after the
   cohort proves repeatability. Five figures. Not offered until the
   verification ladder and durable work-order lifecycle land — selling
   this early is the agency trap with extra steps.

Pricing mechanics that fit the existing rails: fixed price up front
(receipt-planned), pay-per-accepted-outcome as the differentiating
option the intake already offers, Bitcoin discount (~5%, already specced
for credits) as brand-consistent garnish, and the no-subscription framing
the live page already uses. The honest-copy rule carries over intact:
publish the rate card through the copy gates, and never let package copy
imply self-serve delivery while every lane is operator-assisted.

---

## 4. The scaling model: engagements per operator-hour

The services business scales or dies on one ratio. The naive path —
more deals → more human review → hire delivery managers — is the agency
trap: linear revenue, linear headcount, no compounding asset. The
OpenAgents path is different because the delivery system is the product
under development:

**The constraint:** operator minutes per engagement (scoping, supervising,
reviewing, client comms). Today that operator is effectively one person
assisted by the fleet.

**The levers, in the order they should land:**

1. **Productized scopes** (§3) — a fixed menu kills the most expensive
   minutes (unbounded scoping and negotiation).
2. **Vertical templates** — the prefilled workspaces exist precisely so
   engagement setup is minutes, not days; each completed engagement
   should improve its template (the factory's amortize move applied to
   delivery ops).
3. **The verification ladder (SU-5)** — every rung moved from "human
   review" toward "machine test / replay / model judge" converts the
   dominant COGS line into near-zero-cost compute. This is the same
   review-minutes-per-accepted-outcome curve the whole thesis bets on,
   now with paying customers on the other side of it.
4. **Artanis as delivery manager** — the fleet-administrator lane
   (WS-12, AaaS) is not just a product demo; it is the services
   business's scaling technology. Target: Artanis runs the engagement
   (dispatch, refill, verification, flags), the human handles the two
   irreducibly human touchpoints — the client relationship and final
   acceptance sign-off.
5. **The agentic sales force** (Ep 239's pursued world-first) — sales
   agents working the top of the funnel. Aspirational, registry-pinned
   as a pursuit (`claims.pursued_world_first_largest_agentic_sales_force.v1`),
   and the right long-run answer to "who does outbound when the operator
   is reviewing deliveries."

**The modeled arc** (illustrative, to be replaced by measured numbers
per the metric definitions in
`docs/blitz/forge/2026-06-16-forge-factory-metric-definitions.md`):

| Stage | Engagements/mo | Operator-hrs per engagement | Modeled revenue |
| --- | --- | --- | --- |
| Now (manual, owner-sold) | 1–2 | 10–20 | low four figures/mo |
| Cohort proven (3–5 teams, templates + rate card) | 4–8 | 5–10 | mid four → five figures/mo |
| Ladder + Artanis supervision | 10–25 | 2–5 | five figures/mo, margin expanding |
| Self-serve Autopilot absorbs the repeatable tail | services becomes the high-touch tier | — | services + product mix |

Every row is *modeled* until the cohort ledger and the factory metrics
produce measured values — the machinery to measure all of it (cycle
time, pass rate, per-engagement spend routing, operator minutes) already
exists or is specced in the blitz lane.

**The endgame is deliberate:** services is not the terminal business —
it is the revenue-now engine, the demand-discovery instrument, and the
forcing function that hardens the factory, on the well-trodden path
where a services motion funds and specs the product that eventually
absorbs it. The guard against the agency trap is written into the
metrics: if operator-minutes per engagement do not fall while engagement
count rises, we are building an agency, and the plan says so out loud.

---

## 5. How this reorders the strategy stack

The three revenue engines, now correctly ranked by time-to-cash:

1. **Services — "Agents that work." (now).** Four-figure engagements,
   already selling, scaling by the §4 levers. Primary revenue focus.
2. **Tool revenue — Khala Code paid privacy + credits (near).** The Suno
   number; real but secondary; its machinery (plan seam, MPP, credits)
   doubles as the services payment rail.
3. **Network economics — traces → plugins → routed revenue share
   (later).** The compounding moat; fed by both of the above; stays
   `planned` until receipts.

And the flywheel that connects them — this is the actual company-level
loop, with the services engine in the position previous docs gave to
plan-conversion:

> **Episodes and brand → `/business` intake → paid engagements delivered
> by the fleet through Forge → receipts, casework, and measured
> economics → (a) referrals and repeat retainers, (b) hardened factory
> and falling review costs, (c) the solved-problem graph and demand
> data the network economy needs → better delivery → more engagements.**

Khala Code's role in this loop is infrastructure and eventual
distribution: it is the console the delivery fleet already runs on, the
surface a business's own team eventually adopts, and the network's front
door — but it is not the near-term cash register. The tool/network
strategy doc stands unchanged for the *user* motion; this doc adds the
*buyer* motion beside it, and the buyer motion pays for the runway.

---

## 6. What has to be true (the services proof ladder)

Mirroring ROADMAP_AFTER's receipt discipline:

1. **Formalize the existing revenue.** Record the already-sold
   engagements on-platform: quick-win payment receipts (Stripe ref or
   sat-denominated), cohort rows with privacy-reviewed completion
   bundles. This flips `business.coding_quick_win.v1`'s gap from
   "no paid customer exists" to "receipt recorded" — the cheapest,
   truest registry advance available anywhere in the portfolio.
2. **Publish the rate card.** Three packages, fixed prices, through the
   copy gates. A repeatable four-figure motion cannot run on per-deal
   improvisation.
3. **Complete the cohort.** 3–5 teams to `loop_completed` with bundles —
   the repeatability proof the machinery was built for, currently at 0.
4. **Arm the on-platform payment loop** (shared with AW-2): card/credit
   collectable so engagements can be funded without invoicing
   workarounds; Bitcoin path for the buyers who prefer it.
5. **Land the verification ladder + durable work-order lifecycle**
   (SU-4/5/7): the COGS curve and the "clean ≠ correct" trust gap,
   answered before the Build tier is offered.
6. **Measure the ratio.** Operator-minutes per accepted engagement,
   published internally every month, trending down while engagement
   count trends up. The agency-trap tripwire.

---

## 7. Risks specific to this motion

1. **The agency trap** (the big one): revenue grows, operator minutes
   grow with it, the product roadmap starves. Mitigation is structural —
   the §4 levers are roadmap items with owners, and the §6.6 tripwire
   is a standing metric, not a vibe.
2. **Delivery variance:** agent-built software fails in ways that burn
   trust fast. The human-review gate and honest verification labels are
   non-negotiable; the Build tier waits for the ladder.
3. **Scope creep at fixed prices:** the quick-win must be genuinely
   fixed-scope; the receipt plan agreed up front is the contract.
4. **Concentration:** a couple of deals is not a market. The cohort
   target (3–5 independent teams) exists precisely to de-risk this;
   until then, revenue claims stay modest and provenance-labeled.
5. **Regulated verticals:** legal stays workflow-assistance-only (the
   pack's own boundary); never let a services pitch imply advice
   authority.
6. **Brand risk from overclaim:** the trusted brand being sold is the
   honesty architecture; one engagement sold on copy the registry
   wouldn't sign is worth less than zero. All packages, availability
   labels, and case studies route through the gates like everything
   else.

---

## 8. Verdict

The fastest honest dollar in the company runs through
`openagents.com/business`, and it is already running — ahead of the
receipts, ahead of the rate card, ahead of the cohort proof. "Agents
that work." is the correct commercial identity for a company whose
proven asset is a fleet that ships software and whose differentiator is
that every deliverable arrives with receipts. The strategy is not to
choose services *over* the tool and the network but to sequence them:
services pays now and discovers demand; the tool distributes and
supplies; the network compounds later. The near-term work is almost
embarrassingly concrete: record the deals we've already done, publish
three prices, complete five loops, arm one payment flag, and drive one
ratio — operator-minutes per accepted engagement — relentlessly down.
