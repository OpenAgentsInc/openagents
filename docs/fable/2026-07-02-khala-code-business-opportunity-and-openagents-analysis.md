# Khala Code and the OpenAgents Business — A Deep Analysis

Date: 2026-07-02
Status: analysis/opinion doc in the Fable lane. **Nothing here is a product
promise, served capability, or public claim copy.** The product-promise
registry (`docs/promises/`, live at
`GET https://openagents.com/api/public/product-promises`) governs every
public claim; where this doc says something is unbuilt, inert, red, or
planned, that is the registry state as of `2026-07-01.3` on `main`
(the deployed Worker still served `2026-06-29.5` at the time of writing —
a deploy-lag note, not a discrepancy). This doc flips no promise state and
broadens no copy. It is the zoomed-out business picture the Khala Code
launch deserves: what was actually built, what was actually promised, what
the money actually looks like, and what has to become true — in what
order — for this to be a company rather than a thesis.

Companion docs: `2026-07-01-khala-code-summary-and-analysis.md` (product
deep-dive), `2026-07-01-product-promises-khala-code-launch-alignment.md`
(claim audit), `ROADMAP.md`/`EXECUTION.md` (the 17-workstream build),
`docs/ABOUT.md` and
`docs/collective-intelligence/2026-06-24-collective-intelligence-as-an-economy.md`
(the company thesis), and the money map in
`docs/launch/2026-06-20-revenue-loop-promise-audit-and-tightening.md`.

---

## 0. The one-paragraph read

Khala Code is OpenAgents' bid to enter the largest, most contested surface
in applied AI — the coding agent — **without competing on the harness**,
and to use that surface as the front door to everything else the company
has built: the free OpenAI-compatible Khala API, the Pylon fleet/delegation
network, Bitcoin-settled markets for compute/data/labor/verification, and
a receipts-first accounting substrate no competitor bothers to keep. The
launch headline — *"What if your coding agent pays you?"* — names a real
structural gap in the incumbent model (labs monetize your coding traces
and pay you nothing), and the honest current state is that the entire
pays-you loop is `planned`: nothing is captured by default, nothing is
distilled into plugins, nothing is metered, pooled, or paid. What is
*real* is stronger than it sounds and weaker than the video plays: live
payment rails that have settled actual Bitcoin, an orchestration layer
that verifiably ran billions of exact-counted tokens of coding work
through user-owned capacity at near-zero marginal cost, and an honesty
architecture that marks the company's own boldest claims red. The business
opportunity is not "another coding agent." It is that Khala Code sits at
the exact intersection of (a) the only unbounded demand curve in AI
(machine-assigned work), (b) the cheapest supply wedge in the industry
(capacity people already pay for), and (c) a data economics the incumbents
cannot copy without cannibalizing themselves — **if** the company can
convert an owner-operated proof-of-rails into a two-sided network before
the window closes.

---

## 1. What actually shipped (and what didn't)

The product decision that defines Khala Code was made on July 1 (epic
#7780): stop building "a Khala-native coding harness inspired by Codex"
and become **a direct wrapper around `codex app-server`**. Codex owns
sessions, tools, approvals, sandboxing, MCP, slash commands, rollouts;
Khala Code owns everything *around* the harness — the Electrobun
desktop/web shell, the unified Inbox, the Fleet layer (isolated per-account
Codex homes, capacity advertising, spawn, exact token proof), Gym/proof
surfaces, redaction at the chat boundary, and the deterministic delegation
program that turns one message into a sustained multi-worker run. A Claude
worker lane already runs at ~80% parity in Pylon; the desktop harness pill
(Codex | Claude | Khala) is shipped, with `auto` routing on the roadmap
(WS-8/9).

Test discipline is unusually high for a three-day-old pivot: ~57 desktop
test files, a pinned upstream Codex reference commit forcing
contract/gap-matrix/doc to move together, skip-safe live smokes, and
honest-state reporting baked into the UI (`pending`/`not_measured` instead
of fabricated numbers).

What did **not** ship matters just as much:

- **No public release artifact.** No installer, no outside user has ever
  run it. `khala_code.desktop_codex_wrapper.v1` is deliberately yellow for
  exactly this reason. The only build in-tree is a local `-dev` bundle.
- **The one-message fleet spinout is code-complete but live-unproven**
  (the `khala_fleet` MCP bridge landed hours before the launch docs).
- **The security posture is "trusted local operator"** — full-access
  sandbox bypass on the owner's machine. Fine for the current user base
  (n≈1); a blocker for any less-trusted or remote deployment.
- **The launch video's economics** (free plan pays you with data → scrubbed
  traces → agent plugins → backend revenue share; paid plan buys privacy)
  exist as five `planned` promise records with the on-camera "possibility"
  hedge preserved, not as running code. The plan *surface* and a
  fail-closed, default-off paid-plan purchase seam landed on July 1–2
  (registry `2026-07-01.3`, #7966/#7970/#7974), reusing the live
  privacy-entitlement machinery — but purchasing collects no money and the
  capture pipeline behind the free plan does not exist (the redaction
  dependency is literally not in the tree;
  `docs/traces/2026-06-25-default-on-trace-capture-audit.md`).

This is the correct honest summary: **Khala Code today is a well-tested
orchestration shell with world-class accounting and no distribution.**

---

## 2. The market Khala Code enters

The coding agent is where AI's revenue actually is in 2026. Every frontier
lab ships one (Claude Code, Codex); the best-funded application company of
the cycle (Cursor) is a coding surface; a long tail of open-source
harnesses (OpenCode, Aider, Cline) compete on ergonomics. Three structural
facts about this market shape the opportunity:

**1. The harness is being commoditized from above.** The labs give the
harness away to sell tokens and subscriptions. Any startup competing on
harness quality is in a knife fight with opponents who set their own input
prices. Khala Code's wrapper pivot is the strategically sane response:
consume the commoditized layer, differentiate orthogonally. The July 1
summary doc says it plainly — everything Khala Code adds "sits around the
harness rather than competing with it," which survives upstream velocity.

**2. The subscription model creates stranded capacity.** Millions of
developers pay $20–200/month for Claude/ChatGPT plans and exhaust a
fraction of the entitlement. That unused, prepaid inference is
forward-purchased capacity that expires economically unused every cycle —
the cheapest supply class in the industry, because onboarding it requires
no hardware, no power contract, and no capex, only a login the user
already has. `khala fleet connect` is precisely the harvesting tool for
this class, and the live token mix proves the mechanism works at scale:
of the **7.70B tokens** on the public counter (30-day window,
`/api/public/khala-tokens-served`, read 2026-07-02), **~90% is
Pylon-Codex own-capacity delegation** — coding work routed through
accounts the operator already pays for, at zero marginal provider cost to
OpenAgents, counted exactly, settled as no-spend.

**3. The data asymmetry is the unpriced input.** Every incumbent coding
agent captures user traces and monetizes them (training, evals, product)
while paying the user nothing; the redundancy is the margin — ten thousand
customers pay ten thousand times for their agents to re-derive the same
fix. Nobody in the market shares that value back. This is the gap the
Episode 245 headline names, and it is a *real* gap: the first credible
"your coding agent pays you" product would be differentiated on a
dimension the labs cannot follow without repricing their own data
economics. (The idea has small-scale precedent — the docs' own competitor
teardown of Kickbacks.ai records ~19k installs for a rev-share coding
extension — but no one has attached it to an open market and neutral
settlement.)

The market context also names the risk plainly: this is the most
competitive surface in software, the wrapper couples product availability
to OpenAI's Codex install base and app-server API stability, and
distribution — not technology — is what kills products here.

---

## 3. The business model, leg by leg

The whiteboard model is two plans plus a loop. Here is each leg with its
honest state and its economics.

### 3.1 Free plan ("pay with data")

- **State:** free usage is real (the Khala API free tier is live and
  Sybil-gated: 2.5M tokens/day, 2,000 req/day per key); the *pay-with-data*
  leg is `planned`. Desktop wrapper traffic today is owner-private
  delegation observability, never capture. Default-on capture, consent
  surface, and the redaction service are unbuilt
  (`khala_code.free_plan_trace_capture.v1`,
  `data.free_tier_capture_disclosure.v1` yellow).
- **Cost side:** serving free inference is close to free at current scale.
  The measured provider burn on the paid-provider lanes was ~$0.27 total
  as of June 25 (`docs/inference/2026-06-25-khala-cost-model-and-analytics.md`);
  worst-case fully-maxed free users cost ~$0.60–0.70/day each; and the
  dominant volume rides own-capacity delegation that costs OpenAgents
  nothing. A hundred heavy free users ≈ $1.8k/month worst case. The free
  tier is cheap enough to be a pure acquisition instrument.
- **What the leg is *for*:** three things at once — distribution (the front
  door), supply (fleet connect converts each user's prepaid subscription
  into orchestrated capacity), and eventually raw material (consented,
  scrubbed traces). Even with the data leg unbuilt, the free plan already
  pays for itself strategically.

### 3.2 Paid plan ("private data")

- **State:** the plan structure and purchase seam now exist in source with
  tests (public catalog at `GET /api/public/khala-code/plans`, server-side
  plan status, purchase route flag-gated `KHALA_CODE_PAID_PLANS_ENABLED`
  default-OFF, granting the live capture-opt-out entitlement with a
  dereferenceable receipt). **Not purchasable**: the seam deliberately
  collects no payment; the payment-collection leg, pricing, and copy
  sign-off are owner decisions.
- **The substance is privacy, and privacy is cheap to sell.** The paid
  plan's deliverable — fail-closed capture exclusion — is already-live
  machinery (`inference_privacy_entitlements`, confidential-compute mode).
  Marginal cost of the entitlement is ~zero; the plan is nearly pure
  margin once collectable. The buyer logic is sound: businesses that would
  never accept trace capture will pay a flat fee to be provably excluded,
  and "provably" is the differentiator — the exclusion is fail-closed in
  code, disclosed at a public endpoint, and receipt-backed, which is more
  than any incumbent offers.
- **Pricing is undecided.** The credits architecture (usage-based,
  1 credit = $0.01, card or Bitcoin top-up, ~5% Bitcoin discount funded by
  avoided card fees, cost-proportional model multipliers targeting ~40%
  margin) exists on paper and in staging; the Khala Code plan price point
  does not. Nothing in the registry commits to seat vs. flat vs. usage.

### 3.3 The loop (traces → plugins → revenue share → free-user payouts)

- **State:** entirely `planned` across four records. No distillation
  pipeline, no plugin registry, no invocation metering, no attribution
  ledger, no payout path. Settlement seams are deliberately INERT. The
  nearest real machinery is the offline GEPA/Mutalisk candidate loop with
  evidence-gated Gym admission — governance rails for a flywheel whose
  value-bearing legs don't exist yet.
- **Why the loop matters anyway:** it is the *amortization* argument in
  product form. The incumbents' margin is redundancy — everyone re-solves
  everything. A market that pays once for a verified solution and reuses
  it indefinitely has a cost curve that bends down with cumulative volume
  where the labs' curve is flat. If even a crude version works ("5 cents
  for the Rust-error fix × thousands of businesses" is the launch video's
  own illustration), the free plan becomes self-funding and the data moat
  compounds. If it never works, Khala Code degrades gracefully to "a
  free, open-source fleet console with optional paid privacy" — still a
  viable product, minus the story.
- **The hard parts, named:** consent + redaction that actually holds
  (a privacy prefilter is not a security boundary, and the docs say so);
  attribution that survives adversarial gaming (who "owns" a solution
  pattern?); valuation (most traces are worth ~nothing; the tail is fat);
  and the cold-start chicken-and-egg (plugins need paid routing volume;
  paid routing needs plugins worth routing through). None of these is
  solved anywhere in the industry. That is both the risk and the prize.

### 3.4 The rails underneath (the part that is real)

The payments layer is the inverse of the loop: built and proven, waiting
for volume. Per the MPP runbook and the revenue-loop audit:

- **Lightning per-call payments (402 → pay → completion) are live on prod**
  — a real 1-sat mainnet MPP payment settled 2026-06-24, Spark-issued.
- **Money-out is proven**: recipient-confirmed 50,000-sat Spark payouts,
  BOLT12 direct tips with a never-fail sweep/refund ladder (green),
  Bitcoin-settled training rewards (5 sats/window, small by design) and a
  first escrowed labor job.
- **USDC and card rails are live-armed**; the *credit-package* Stripe flow
  (the fiat on-ramp for the credits business) is code-complete and
  deliberately red pending owner-armed prod secrets and one real purchase.

The pattern across every stream is identical: **the machinery is staged to
one owner action and one real receipt.** That is not an accident; it is
the operating doctrine ("no settlement receipt, no payout claim"), and it
means the distance from zero revenue to first revenue is short everywhere
— and the distance from first revenue to *material* revenue is the actual
business problem.

---

## 4. What the token curve proves — and what it doesn't

The single most-quoted number in the launch arc is the tokens-served
counter: ~1M at the start of Episode 243, 302M by Episode 244, **7.70B on
the live counter today**, ~90% Pylon-Codex own-capacity across 2,514
requests (≈2.7M tokens per request — sustained fleet coding runs, not
chat), ~7% free-tier provider inference across ~248k requests.

What it proves:

1. **The orchestration works at real scale.** Billions of exact-counted
   tokens of real coding work (the fleet merged ~20 PRs in one overnight
   run; the entire 17-workstream roadmap is being burned down by the
   product supervising itself) through isolated multi-account delegation
   with closeout proofs. This is not synthetic load.
2. **The cost structure is extraordinary.** The dominant lane costs
   OpenAgents nothing (user-owned capacity), and the paid-provider lanes
   cost cents. Very few AI companies can show a usage curve whose COGS
   rounds to zero.
3. **The accounting discipline holds under pressure.** Every token is an
   exact ledger row; the public counter is a projection, never an
   estimate; "counter movement is never proof" is written into the
   runbooks.

What it does not prove — and the company's own demand-provenance promise
(`proof.demand_provenance.v1`: "no external dollar, no demand claim")
forbids pretending otherwise:

1. **Demand.** The curve is overwhelmingly the founder's own fleet
   building the founder's own product. It is the best possible dogfood
   and zero external traction.
2. **A network.** The "fleet" is one operator's handful of Codex accounts.
   External contributor counts are single digits; total real Bitcoin to
   independent parties is on the order of a few thousand sats. The
   registry's own red on `pylon.consumer_compute_earns_bitcoin_self_serve.v1`
   says the quiet part in public.
3. **Revenue.** There is no MRR. The only real production money movements
   are sats: tips, a 1-sat MPP payment, small training/labor settlements,
   and owner-directed recognition payouts.

The honest framing writes itself: OpenAgents is **verification-constrained
and demand-constrained, and emphatically not compute-constrained.** The
supply-side machine is over-built relative to demand — which is precisely
the failure mode the company already lived through once (the GPUtopia
lesson: hundreds of providers, one buyer) and the reason the current
sequencing doctrine is demand-first.

---

## 5. Why Khala Code is the right wedge (the strategic logic)

Given all of the above, why is a desktop coding app the correct next move
rather than a distraction? Five reasons, in ascending order of importance:

**1. It is where the users already are.** Coding agents are the one AI
product category with proven willingness to pay and daily-habit usage.
Every other OpenAgents surface (training, labor markets, Sites, business
packs) requires explaining a new behavior. Khala Code meets developers
inside a behavior they already have and are already paying for.

**2. It converts users into supply on contact.** The `khala fleet connect`
onboarding (npm install → device login → fleet status) turns each user's
existing Codex/Claude subscription into orchestrated capacity in minutes.
No other acquisition motion in the company's portfolio adds supply and
demand in the same gesture. This is the fifth supply class — prepaid,
zero-capex, zero-opportunity-cost — and Khala Code is its only harvester.

**3. It is the gateway to the whole registry.** The launch-alignment doc
maps the funnel explicitly: install free (green substrate: free API, open
source) → connect your fleet (green: own-capacity delegation) → wallet
appears when money appears (green rails) → forum identity (green) → go
online/contribute (green training/labor records) → traces earn / hand the
fleet to Artanis / paid plan (planned). Steps 1, 2, 4, 5, 6 stand on green
records *today*. Khala Code doesn't have to implement the other lanes —
it routes to them. A single well-distributed client makes every other
bet in the portfolio cheaper to test.

**4. Its data story attacks the incumbents where they cannot follow.**
A lab whose margin is trace redundancy cannot pay users for traces
without repricing itself. A closed platform cannot let plugin authors
earn from routing without disintermediating its own take rate. The
pays-you loop is unproven, but it is *asymmetric*: only an open,
settlement-neutral network can offer it at all. Bitcoin/Lightning is not
ideology here; it is the only rail that can pay a contributor 5 cents —
or an agent 5 sats — across borders, instantly, with nothing to rug.

**5. It manufactures the buyer class the rest of the thesis needs.** The
deeper OpenAgents argument (docs/ABOUT.md, the collective-intelligence
essay) is that the unit of machine work is migrating to the *accepted
outcome* — work verified, receipted, and settled — and that the buyers of
such work will increasingly be agents themselves, forming subgroups no
platform brokers (the Reed's-law argument; agents have no Dunbar limit).
That future needs a population of agents with identity, wallets, and work
to do. A coding fleet console that every user runs is the hatchery: each
connected fleet is a set of agents one config away from the forum, the
labor market, and the training rails. Meanwhile the enterprise world is
converging on the same unit from the buy side — per-outcome cost and
audit evidence are becoming procurement requirements — and a vendor whose
every outcome already ships with receipts, provenance labels, and exact
accounting is structurally the easiest vendor to govern. Khala Code is
how that evidence stream gets generated at product scale instead of demo
scale.

---

## 6. Competitive position and moat, honestly weighed

| Moat candidate | Reality today | Durability if it works |
|---|---|---|
| Honesty architecture (promise registry, receipts, exact accounting, copy gates) | **Real and enforced** — live public registry, transition receipts, the company marks its own flagship claims red | High and compounding — it is cultural + machine-enforced, and it maps 1:1 onto emerging enterprise audit demand. Hard to copy because it *hurts* every quarter you fake it |
| Bitcoin/Lightning settlement + MPP | **Real, live, tiny volume** — proven in/out rails, three payment methods, agent-native wallets (BOLT12) | High for the agent-economy lanes (no incumbent will pay contributors in bearer money); irrelevant for plain SaaS competition |
| Own-capacity orchestration (fleet, isolated homes, exact proof) | **Real at owner scale** — billions of tokens, ~20 PRs/night, deterministic delegation | Medium — genuinely hard engineering, but labs could ship "use your own seat for background agents" and erase the wedge's uniqueness (they'd cannibalize API revenue to do it, which buys time) |
| Data flywheel (traces → plugins → revenue share) | **Designed, unbuilt** — capture off, redaction missing, settlement inert | Potentially the deepest moat in the design (solve-once economics + paid contributor network) — and currently pure narrative |
| Open source + open protocols (Nostr identity, open registry) | Real | Weak alone (forkable), strong as a *neutrality* guarantee the closed platforms structurally cannot match |
| Multi-harness neutrality (Codex + Claude + auto) | Claude worker lane ~80% parity; desktop harness abstraction shipping | Medium — "one console over all your agent subscriptions" is a real product position no lab will offer for its competitors |

Against the named competitors: **Claude Code/Codex** are upstream
suppliers more than rivals — the wrapper strategy makes their improvement
Khala Code's improvement, at the price of platform dependency (the
sharpest single business risk: a Codex ToS change, app-server breakage,
or account-sharing crackdown hits the core loop). **Cursor** competes on
IDE ergonomics with enormous distribution and no pays-you story; it
validates the category's willingness to pay. **OpenRouter** proves the
drop-in-API GTM that Khala already copied (Episode 243's OpenCode
integration) but routes tokens, not verified work, and pays no one.
**OpenCode/Aider/Cline** are channels, not competitors — Khala is a
one-line base-URL swap inside them. The teardown-worthy precedent is the
small rev-share extension class (Kickbacks et al.), which proves users
will install for money-back mechanics but has no network, no verification,
and no settlement rail.

The moat verdict: **today's defensibility is the honesty architecture and
the rails; the claimed defensibility (data network, Reed's-law agent
economy) is entirely forward-looking.** The gap between those two is the
company's actual to-do list.

---

## 7. Risk register

1. **Platform dependency (highest).** The product is a wrapper on an
   experimental third-party surface (Codex app-server), and the dominant
   usage lane rides ChatGPT-account capacity whose terms OpenAI controls.
   Mitigations exist (Claude lane at ~80% parity, legacy native runtime,
   multi-harness abstraction) but are not yet load-bearing.
2. **No distribution yet.** No release artifact, no outside user. Every
   week the launch video ages without an installable product, the claim
   surface (public promises, public video) carries risk without earning
   users. Shipping the signed artifact is the single highest-leverage
   owner action in the portfolio.
3. **The loop's hard problems are unsolved industry-wide.** Consented
   capture, redaction that holds, attribution, trace valuation, plugin
   distillation. The registry hedges are honest, but the headline is
   public; if the loop stays `planned` for quarters, the story decays
   from differentiator to vaporware-adjacent.
4. **Demand remains hypothetical.** One operator's dogfood, however
   spectacular, is not a market. The demand-provenance discipline
   prevents self-deception but does not create buyers.
5. **Correlated risk-on exposure.** The company's macro posture (Bitcoin
   settlement + AI demand) draws down together in the adverse scenario;
   the treasury is tiny; the runway math lives outside these docs but the
   sat-denominated numbers visible in-repo are small.
6. **Security/trust posture.** Full-access local execution is fine for
   the owner and disqualifying for teams/enterprises until the deferred
   permission-policy work lands.
7. **Operational fragility.** Single-operator ops, a GitHub Actions
   billing lock forcing local CI, container-dependent Lightning deploys —
   small-company frictions that show up as public outages (the MPP
   Lightning gap already did).
8. **Naming/brand debt.** Khala the model vs. Khala Code the product vs.
   deprecated khala-code model lanes vs. an unrelated openagents.org —
   cheap to fix now, expensive later.

---

## 8. What would make this a business: the proof ladder

The registry already encodes the answer; sequencing it as a ladder, each
rung a receipt:

1. **Ship the artifact.** Signed/notarized Khala Code build + one outside
   user running it → `khala_code.desktop_codex_wrapper.v1` green-candidate.
   Everything else is throttled by this.

   2026-07-04 RL-1 release-lane update: the repo now has an owner-run macOS
   release lane for this rung. `clients/khala-code-desktop` exposes
   `release:plan` and `release:macos`; `apps/oa-updates` has a product-specific
   `desktop/khala-code-desktop/<channel>/feed.json` path; RC versions are
   rejected from stable/latest lanes; and `NEEDS_OWNER.md` names the receipt
   set required before this rung can be represented as complete. The physical
   signed/notarized/stapled DMG, feed upload, GitHub release, and clean-Mac
   first-run proof remain owner-gated receipts, not repo claims.

   2026-07-04 RL-2 install-truth update: `/code/download` is now the public
   copy-gated install page for this rung. It shows Codex install/login as a
   prerequisite, exposes the public npm `khala` CLI and source-build paths,
   and labels the desktop DMG as a pending public artifact. The paired
   `GET /api/public/khala-code/download-counts` endpoint reports only exact
   `khala_code_download_events` rows, or `counts: []` with blocker refs. This
   improves funnel clarity but still does not supply the missing signed DMG or
   outside-user evidence.

   2026-07-04 RL-3 evidence-intake update: Khala Code now has an explicit,
   opt-in outside-user run receipt path. Desktop Settings offers a "Run
   evidence" action that posts only app version, platform, architecture,
   distribution channel, and bounded Codex/Pylon readiness to
   `POST /api/public/khala-code/outside-user-runs`; each receipt dereferences
   at `/api/public/khala-code/outside-user-runs/{receiptRef}` with
   `generatedAt` and the public projection staleness contract. This creates a
   citable intake lane and outside-user template, but still does not supply a
   real outside-user receipt row, signed DMG, or owner-signed promise upgrade.
2. **Rehearse the flagship live.** One real message-triggered fleet run
   (the WS-17 "clean 2B day" is the maximal version: ≥2.0B exact
   tokens/day, ≥15 workers, zero duplicate PRs, 100% closeout coverage).
   This converts the demo claim from code-complete to demonstrated.
3. **First collectable dollar.** Arm the Stripe credit-package secrets (or
   the Khala Code paid-plan payment leg — same owner sitting) and let one
   real customer buy something: paid plan, credits, or the $5 orange
   check. One receipt flips the entire "no production revenue" sentence.
4. **Arm consented capture at the smallest honest scope.** Land redaction,
   flip capture for opt-in free users only, publish the disclosure. As of
   #8250, the desktop has the default-off consent UI, persisted consent RPCs,
   and a fail-closed local planner: session events must pass Rampart redaction
   before owner_only ingest, paid-plan opt-out blocks capture, redaction failure
   returns `not_captured`, and payout/settlement markers stay inert. The rung is
   still not complete until the owner arms `KHALA_CODE_DESKTOP_TRACE_CAPTURE_ENABLED`,
   connects the owner_only ingest sink, and records a public-safe live receipt.
   The loop cannot even begin accumulating raw material until that evidence
   exists.
5. **One plugin, one routed request, one paid contributor — however
   small.** A single 5-cent (or 50-sat) revenue-share payout with a
   receipt would do for the data loop what the 1-sat MPP payment did for
   machine payments: convert a category from narrative to precedent.
6. **External demand provenance.** The first N external users and the
   first external dollars, labeled as strictly as modeled-vs-settled
   already is. The KPI set is already named in the docs: external-dollar
   split, acceptance rate, review-minutes per accepted outcome trending
   down, contributor retention without subsidy, free→paid conversion.

Notably, rungs 1–4 are each **one owner sitting plus work that is already
staged.** The company's own operating style (fleet burning the roadmap,
PROMISSORY passes clearing promise blockers) means the ladder's
constraint is owner attention and external distribution, not engineering
throughput.

---

## 9. Scenarios

**Bear.** The loop stays planned; Codex tightens terms; no artifact ships
for months. Khala Code remains an extraordinary personal dev tool and the
company's own build accelerant, with the free API as a modest acquisition
channel. Value accrues to the substrate (rails + registry) awaiting a
different wedge (the business packs, the legal lane). Survivable — the
cost base is tiny and the dominant workload is self-funded — but the
launch story expires.

**Base.** Artifact ships in weeks; a small real user base connects fleets;
the paid plan becomes collectable and privacy-buyers trickle in; capture
goes live opt-in; the first toy plugin payouts happen at demo scale.
OpenAgents becomes the niche-but-real "open, Bitcoin-settled fleet console
over your existing subscriptions" with the industry's most auditable
claims — a defensible small business with an option on the network story,
attractive to exactly the investors who fund rails-and-receipts theses.

**Bull.** The pays-you loop works even crudely, at the moment enterprise
procurement starts demanding per-outcome receipts. Free users join for
money, bring their fleets (supply), and generate the solved-problem graph
(data); paid privacy and routed-plugin fees fund the pool; agent-to-agent
markets start clearing without the platform brokering each match. The
cost curve bends down with cumulative volume while incumbents' stays
flat, and Khala Code becomes what the whiteboard drew: one client box
among many on an economy — the front door to the first coding network
whose users are also its shareholders-in-sats. That outcome remains a
long shot, but it is a long shot with named gates, live rails, honest
instrumentation, and — unusually for this industry — a public registry
that will say so, in red, if it isn't happening.

---

## 10. Verdict

Khala Code is the right product at the right layer with the right
sequencing discipline and, as of today, zero market evidence — and the
company knows it, in writing, at a public URL, which is itself the most
differentiated thing about it. The opportunity is real and asymmetric:
the downside is a cheap, self-funding dev tool; the upside is the first
coding-agent network with user-shared data economics on neutral money.
The distance between them is five receipts: an installer in a stranger's
hands, a dollar collected, a trace consented, a plugin paid, and an
external buyer who came back. Everything in the repo says the machinery
will be ready before the demand is. The business now depends on the only
thing the fleet cannot build by itself: other people.

## 11. Episode 246 addendum (2026-07-03)

Episode 246 (`docs/transcripts/246.md`, "Dogfooding Khala Code") converts
several of this document's conclusions into on-camera owner commitments,
and they should be read as the operating posture for the proof ladder:

1. **The win condition is named, in the owner's words.** "The thing that I
   need as a company is to close the gap between what I've been saying and
   what we're shipping. That's it. If I close that gap, I have a successful
   company." That is §8's proof ladder restated as identity: the say/ship
   gap is the business, and the registry + the new UX Behavior Contract
   layer (`khala_code.ux_behavior_contracts.v1`) are its instruments — the
   same promise-grade rigor now applied down to micro-interactions.
2. **Customer-number-one pressure on rung 1 is now real.** The owner
   switched to Khala Code full-screen as his only harness ("stop using
   Codex, just force yourself to fix the bullshit") and fixes Khala Code
   with Khala Code. This does not substitute for the outside-user receipt
   rung 1 requires, but it is the forcing function that makes the artifact
   worth shipping — and it generates the contract backlog (36 hours of
   session history mined into pending contracts) that defines "smooth
   enough that I love using it."
3. **Rung 3 (first collectable dollar) is re-affirmed as the loop that
   matters.** The episode's revenue framing — ideas and evidence living in
   "a product that a user will connect a payment method to," with revenue
   flowing into the network and the network strengthening the product —
   is §3's flywheel said plainly, with Episode 239 cited as the design and
   this product as the vehicle.
   - **Code-landed note (2026-07-04, #8249):** Khala Code desktop now hands
     paid-plan payment-required responses to the real server-returned checkout
     URL and exposes the existing openagents.com billing checkout from the same
     plans surface for credit packages. The desktop still renders plan state
     from server status and leaves credit package/balance truth on the billing
     authority rather than fabricating it locally
     (`khala_code.plans.checkout_handoff_server_truth.v1`).
4. **The portfolio is now officially two products.** "OpenAgents has two
   products: Khala Code, our equivalent of Claude Code, and Autopilot, our
   business system for running stuff on Autopilot" — with the promise/
   contract rigor "applied all up and down the business" as Autopilot's
   substance. QA Swarm (the §5 wedge logic applied to QA) is the first
   service productized out of the dogfood loop: built first for Khala
   Code, then added to the Autopilot suite for businesses
   (`qa_swarm.product_surface.v1` records the sequencing).
5. **A product-discovery heuristic worth keeping:** "if you build some new
   system to help you do the thing you're trying to do, that system is
   probably what your product should be." QA Swarm is the first instance;
   the fleet substrate itself was arguably the zeroth.
