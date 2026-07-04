# ROADMAP_AFTER — What Comes When The Machine Is Built

Date: 2026-07-02
Status: **speculative successor roadmap.** This doc assumes
[`ROADMAP.md`](./ROADMAP.md) is fully implemented — M1 through M6 landed,
the clean 2B day (T17.2) achieved, cockpit + multi-harness + mobile +
Artanis-on-the-spine live. It asks the only question that matters after
that: what does it take for the company to **survive contact with the
market and generate revenue**. It flips no promise state and broadens no
public copy; every claim-bearing item routes through `docs/promises/` and
the registry gates exactly as before. Delivery mechanics remain
[`EXECUTION.md`](./EXECUTION.md). Sources: the launch claim audit
(`2026-07-01-product-promises-khala-code-launch-alignment.md`), the
business analysis
(`2026-07-02-khala-code-business-opportunity-and-openagents-analysis.md`),
the tool/network strategy analysis
(`2026-07-02-come-for-the-tool-stay-for-the-network.md`), the business
services analysis
(`2026-07-02-agents-that-work-business-services-analysis.md`), Episode
239 (`docs/transcripts/239.md`), the blitz factory lane
(`docs/blitz/forge/`), the Forge synthesis (`docs/forge/`), and the
registry state as of `2026-07-01.3`.

Two strategic frames govern the doc, one per motion:

- **The buyer motion — "Agents that work." (primary revenue).** Businesses
  hire agents from the OpenAgents network to get work done — above all,
  software built very fast — through `openagents.com/business`, delivered
  by the fleet through Forge. Engagements at four-figure price points have
  already been sold (owner-reported; on-platform receipts pending). This
  is the near-term cash engine, and AW-0 below is its workstream. The
  services doc carries the full model, packages, and scaling levers.
- **The user motion — come for the tool, stay for the network.** The tool
  phase (AW-1/2/5/7) must be complete, excellent, and network-silent; the
  network phase (AW-3/4/6/9) is entered by *graduation*, never by force;
  the tool→network graduation funnel is that strategy's only honest test.
  The tool/network doc carries the full argument, including the conscious
  fallback if the network never forms.

The two motions share one substrate (fleet, Forge, receipts, rails) and
feed each other: services engagements are delivered on the machinery the
tool orchestrates, and every engagement produces the external demand
evidence, casework, and solved-problem inventory the user motion needs.
**Prior revisions of this doc over-indexed on squeezing paid usage out of
Khala Code; this revision corrects that** — tool revenue (paid privacy,
credits) is real but secondary; the primary near-term revenue model is
the services engine.

## 0. The Premise Shift

The first roadmap builds the machine: contracts, store, fan-out engine,
cockpit, QA tiers, second harness, status spine, mobile, Artanis. Its
final gate — the clean 2B day — is a **capability proof executed by the
owner, for the owner**. Every success criterion in it can be satisfied
with zero customers.

The moment M6 lands, the binding constraint flips. It stops being
engineering throughput (the fleet has demonstrated it can outbuild any
backlog we write) and becomes three things the fleet cannot manufacture:

1. **Other people.** Users who install, connect fleets, come back.
2. **Owner arming.** Nearly every revenue path is one deliberate owner
   action from live (Stripe secrets, capture flip, settlement arming,
   plan pricing). These do not parallelize across workers; they serialize
   through one person's attention and risk tolerance.
3. **Evidence of demand.** External dollars and external tokens, labeled
   under `proof.demand_provenance.v1` ("no external dollar, no demand
   claim") — the one promise no amount of dogfood can advance.

So where ROADMAP.md is organized by engineering seams, ROADMAP_AFTER is
organized by **market-facing proof obligations**. Each workstream ends in
a receipt someone outside the company could have caused. The registry is
the scoreboard: this roadmap exists to move `khala_code.*` and the
revenue-loop records from planned/yellow/red toward green *the only
honest way — by making the gated things actually happen*.

What M6 leaves us with: a tested, orchestrated, multi-harness fleet
console with exact accounting, live Bitcoin rails, a promise registry, a
live `/business` intake door — and (unless AW-0/AW-1 changed it) **no
installer in a stranger's hands, no on-platform revenue receipt, no
consented trace, no plugin, and external demand that exists as
owner-reported closed deals rather than instrumented evidence.** That is
the gap this roadmap closes.

## 1. Dependency Spine

```text
AW-0 Services engine ("Agents that work.")  ← primary revenue; needs NO installer
        |                                      (intake live, fleet proven, Forge landing)
        +── feeds AW-6 (demand evidence), AW-8 (pricing data), AW-4 (solved problems)

AW-1 Ship & distribute (artifact, updates, funnel)   ← gates the USER motion
        |
        +-----------------------+---------------------------+
        |                       |                           |
   AW-2 First-dollar        AW-3 Consent & capture      AW-5 Trust posture
   spine (arm the rails;        (redaction, opt-in)         (other people's
   shared with AW-0)            |                            machines)
        |                  AW-4 Plugin economy v0            |
        |                       (distill→meter→pay)          |
        +-----------+-----------+                            |
                    |                                        |
              AW-6 Demand engine (channels, design       AW-7 Ops for a
              partners, provenance-split growth)          real user base
                    |
              AW-8 Pricing & packaging iteration
                    |
              AW-9 Retention & compounding loops
                    |
              AW-10 Governance/enterprise readiness (parked behind gates)
```

**AW-0 starts now and gates on nothing in this roadmap** — the intake is
live, the delivery fleet is the company's most-proven asset, and deals
are already closing; it must not wait for the installer. AW-1 remains the
WS-1 of the *user* motion: deliberately small, unblocks everything on
that side. AW-2/3/5 run in parallel behind it; AW-2's owner sitting also
arms AW-0's on-platform payment path. AW-4 depends on AW-3 producing raw
material. AW-6 is continuous. AW-10 stays parked behind written gates
exactly like OTEC and the gigawatt lanes always have.

## 2. Workstreams

### AW-0 — The services engine: "Agents that work." (primary revenue)

Source: the services analysis
(`2026-07-02-agents-that-work-business-services-analysis.md`), Ep 239,
the blitz factory lane, the `/business` intake spec. Businesses hire
agents from the network to get software (and vertical work) built very
fast; better/faster/cheaper delivery with receipts. Four-figure
engagements are already selling (owner-reported); this workstream turns
a founder-sold motion into a scalable engine. Registry anchors:
`business.intake_quick_win_offering.v1`, `business.coding_quick_win.v1`,
the three vertical-pack records (all yellow), the customer-one cohort
epics.

| Task | Description | Gate/receipt |
| --- | --- | --- |
| A0.1 | **Formalize the existing revenue on-platform**: record already-sold engagements as quick-win payment receipts (`business-quick-win-payment.ts` — Stripe ref or sat-denominated `buyerPaidRef`) and privacy-reviewed cohort rows; opaque refs only, per the cohort source contract | First on-platform paid `business.*` receipt — the cheapest, truest registry advance in the portfolio |
| A0.2 | **Publish the rate card** through the copy gates: three buyable packages (modeled in the services doc: Quick Win fixed-price $1–5k band; Sprint week-of-fleet $5–15k; Retainer/"On Autopilot" monthly) — fixed scopes, receipt plans up front, no self-serve implication while delivery is operator-assisted | A price on `openagents.com/business` |
| A0.3 | **Pipeline ops**: intake → scope call → receipt plan → close, instrumented (leads, close rate, time-to-quick-win); Slack Connect handoff worked as a queue, not ad hoc; "Agents that work." as the positioning line through the copy gates | Weekly pipeline review exists |
| A0.4 | **Delivery on the factory, every engagement**: run all client work through Forge lanes + fleet delegation with the dogfood-strip/spend-routing projections live per engagement — the engagement's own receipts become the case study (public-safe, opaque refs) | Per-engagement factory evidence bundle |
| A0.5 | **Complete the customer-one cohort**: 3–5 independent teams to `loop_completed` with privacy-reviewed completion bundles (target already defined; currently 0 recorded) | Cohort gate `ready`; epics #5098/#5104 closable |
| A0.6 | **Activate the vertical packs as sold products**: e-commerce, marketing (needs the deliverability smoke), legal (workflow-assistance boundary intact) — each needs exactly one first paid delivery receipt | One paid receipt per activated vertical |
| A0.7 | **Drive the ratio down — the scaling levers in order**: productized scopes (A0.2) → vertical templates (A0.6) → verification ladder (Forge SU-5: machine test → replay → model judge → human review) → **Artanis as delivery manager** (WS-12 AaaS supervising engagements; human keeps client relationship + final acceptance) | Operator-minutes per accepted engagement measured monthly and falling |
| A0.8 | **The referral loop, honestly**: satisfied clients + the Ep 239 refer-once mechanics as the growth channel — copy never re-implies the red `referral.refer_once_earn_forever.v1`; referral payouts only when the ledger machinery settles them | First referred engagement with attribution |
| A0.9 | **Case-study engine**: every completed engagement yields a public-safe writeup (opaque client refs, real receipts, real cycle times) — the content that converts the next buyer | Case studies published; intake attributes arrivals to them |

Delegability: A0.4/5/6/9 MED-HIGH (fleet-deliverable with review);
A0.1/2/3/8 owner-adjacent (money, copy, client relationships). The
scaling thesis in one line: **services COGS is operator minutes; every
lever above converts operator minutes into fleet compute; if the ratio
does not fall while engagement count rises, we are building an agency —
see §5.**

### AW-1 — Ship it: the release and distribution engine

The single highest-leverage workstream in the company. `khala_code.
desktop_codex_wrapper.v1` is yellow solely for "no public release
artifact / no outside user." Everything downstream of this roadmap is
throttled by installs.

| Task | Description | Gate/receipt |
| --- | --- | --- |
| A1.1 | Signed/notarized macOS artifact + release channel (RC → stable, per `docs/DEPLOYMENT.md` conventions; ed25519 + notarization runbooks already exist) | A stranger downloads and runs it |
| A1.2 | Auto-update path for the desktop shell (the Electrobun equivalent of the OTA discipline; RCs never take `latest`) | One remote user updates without reinstalling |
| A1.3 | Install-path plurality: `brew install`, npm launcher parity with `@openagentsinc/khala`, Linux build decision (ship or explicitly scope out à la the Windows/WSL precedent) | Documented, tested install matrix |
| A1.4 | First-run experience as a product surface: Codex detection, fleet connect inline, honest empty states, the plan cards (already built) as the moment of disclosure. **Ship single-player complete, network-silent**: first-run never asks for forum identity, wallet, or capture consent — the tool earns trust first; the network is discovered, not imposed | Time-to-first-delegation < 10 min for an outsider |
| A1.5 | Funnel instrumentation, public-safe: installs → first chat → fleet connected → first delegation → D7 return, **extended through the graduation stages: consent given → forum identity → first sat earned → first plugin routed** — the tool→network funnel is the strategy's only honest test. Aggregate counters with the same exact-only discipline as tokens-served (no per-user surveillance; counts, not identities) | A dashboard the owner reads weekly |
| A1.6 | The landing surface: openagents.com routes Khala Code discovery (download, docs, the registry-pinned claims); revive-or-retire decisions for lapsed standing surfaces (`contributors.bounties_surface.v1` red is the named precedent) | Copy passes the gates; no unsafeCopy leakage |

Delegability: A1.2/3/5 HIGH; A1.1/4/6 owner-adjacent (signing identity,
copy gates).

### AW-2 — The first-dollar spine

Everything here is staged; the work is arming, sequencing, and the first
real receipts. One owner sitting arms multiple lanes — batch it. Note the
reframe: the literal first dollar is arriving through AW-0 (services) —
this workstream's job is making that dollar **collectable and receipted
on-platform** (engagement funding, credits, plans) rather than invoiced
around the product.

| Task | Description | Gate/receipt |
| --- | --- | --- |
| A2.1 | Arm the Stripe credit-package secrets (or decide the Khala Code plan payment leg first — same sitting): `payments.autopilot_credits_purchase.v1` red → one real card purchase | First collected fiat dollar |
| A2.2 | Price and arm the Khala Code paid plan: flip `KHALA_CODE_PAID_PLANS_ENABLED` once the payment leg exists; wire purchase → entitlement → receipt end-to-end in prod (`khala_code.free_paid_plans.v1`) | First paid-plan purchase receipt |
| A2.3 | The $5 orange check as the deliberate warm-up transaction (`identity.orange_check_forum_signal.v1` yellow — cheapest card-rail proof in the registry) | One live badge purchase |
| A2.4 | In-app billing surface: balance, receipts, plan state inside Khala Code settings (the plan panel grows a wallet/billing sibling); Bitcoin top-up path surfaced with the ~5% discount already specced | Purchases possible without leaving the app |
| A2.5 | Credits-for-inference collectable loop: card/Lightning → credit → metered gateway spend → public card-credit-spend receipt (`inference.gateway_credits_business.v1` red → candidate) | One external account funds and spends |
| A2.6 | Revenue accounting discipline extension: `internal vs external` dollar split typed into the ledger the way `demand_kind` already types tokens | Every revenue number carries provenance |

### AW-3 — Consent and capture: the data leg becomes real

The free plan's "pay with data" is unbuildable until this lands, and the
launch-alignment doc is explicit that today's wrapper traffic is
owner-private observability, never capture. Consent-first, smallest
honest scope.

| Task | Description | Gate/receipt |
| --- | --- | --- |
| A3.1 | Land the redaction service (the #6219-class dependency the trace audit names as "not in the tree"); redact-before-tripwire parity with the gateway path | Redaction suite green on adversarial fixtures |
| A3.2 | Desktop consent surface: explicit opt-in per the disclosure spine (`data.free_tier_capture_disclosure.v1`), plan-scoped (paid = excluded fail-closed — already wired), per-session override, visible capture indicator | `khala_code.free_plan_trace_capture.v1` planned → yellow candidate |
| A3.3 | First consented corpus at bounded scope: opt-in users only, owner_only visibility, public aggregate counts (traces captured, redaction drop rate) | A corpus > 0 with zero leak incidents |
| A3.4 | Trace value triage: which captured traces are worth anything (solved-problem density, dedup rate) — measured, not asserted, before any payout copy | An honest "most traces are worth ~0" number |
| A3.5 | The reward marker stays INERT until AW-4 can pay; no earning copy ships before a payout path exists (the Ep 239 red record is the standing warning) | Copy gates hold |

### AW-4 — Plugin economy v0: one plugin, one routed call, one payout

The whole pays-you loop, at the smallest scale that produces a real
receipt. This is deliberately a *toy that settles* rather than a platform
that doesn't.

| Task | Description | Gate/receipt |
| --- | --- | --- |
| A4.1 | Distillation pipeline v0: consented traces → one candidate "solved problem" artifact (skill/plugin), through the existing GEPA/Mutalisk candidate + Gym admission machinery (never auto-promote) | One admitted plugin artifact with provenance |
| A4.2 | Plugin registry v0 with attribution: who contributed, from which consented traces, at which digest (`khala_code.trace_derived_plugins.v1`) | Dereferenceable provenance record |
| A4.3 | Invocation metering: exact rows when routing selects the plugin (same ledger discipline as tokens) | Exact attribution rows exist |
| A4.4 | First revenue-share settlement: one contributor paid one small amount (sats-scale is fine — the 1-sat MPP payment is the precedent for how much a first receipt matters) over the live rails, receipt public-safe (`khala_code.plugin_backend_revenue_share.v1`) | **The category-converting receipt** |
| A4.5 | Paid→free pool mechanics deferred until real paid volume exists; the record (`khala_code.paid_to_free_revenue_share.v1`) stays planned with the pool policy written before any pool copy | Policy doc, no state change |

Code-landed note (2026-07-04, #8251): the RL-7 precedent receipt spine now
exists in source. `POST
/api/operator/khala-code/trace-plugin-revenue-share-precedents` is admin-token
gated and records only public-safe refs for the A4.1-A4.4 chain after settlement
has happened elsewhere; `GET
/api/public/khala-code/trace-plugin-revenue-share-precedents/{receiptRef}`
dereferences the receipt with live-at-read staleness. This is the place the
category-converting n=1 receipt lands, but the code itself moves no sats and
does not turn an empty ledger into market proof.

The honest sequencing note: AW-4 can produce its first receipt with n=1
contributor and an internally-routed paid call — *labeled as such*. That
is plumbing proof, not market proof, and the registry note must say so.
The market version needs AW-6.

### AW-5 — Trust posture: other people's machines

Today's security model is "trusted local operator with full access" —
correct for n≈1, disqualifying for teams. This workstream is what makes
installs safe to want.

| Task | Description | Gate/receipt |
| --- | --- | --- |
| A5.1 | The deferred permission-policy wave: non-owner defaults (sandboxed exec, approval prompts on by default, no `danger-full-access` outside owner mode), per-workspace policy | A new install is safe-by-default |
| A5.2 | Secrets hygiene for user machines: keychain-backed storage for tokens, never-print discipline extended to user paths, the existing redaction boundary audited for the multi-user case | Security review doc, issues filed and fixed |
| A5.3 | Team mode decision: is Khala Code single-operator or multi-seat? (Registry copy currently implies personal use; teams change capture consent, billing, and fleet ownership semantics) | A written decision + scoped records |
| A5.4 | Third-party security review of the delegation path (worker isolation, MCP surface, wallet boundaries) before any enterprise conversation | Findings burned down |

### AW-6 — The demand engine

Distribution is the actual product problem. Everything here is measured
under demand provenance — growth that fakes the counters is worse than no
growth.

| Task | Description | Gate/receipt |
| --- | --- | --- |
| A6.1 | Drop-in channel motion, systematized: Khala as a one-line provider inside OpenCode/Aider/Cline/etc. (the Ep 243 playbook), with per-channel onboarding docs and counted arrivals | Channel-attributed signups |
| A6.2 | The content flywheel the episodes already are: launch video → installable product in the same week rhythm; every public claim registry-pinned (the Ep 222/245 header pattern) | Episodes convert to installs measurably |
| A6.3 | Design partners: 3–10 named outside users/teams with a weekly feedback loop; their fleets, their repos, their acceptance judgments — the first external work-quality dataset. **Graduate multiplayer by scope, not by rewrite**: the claim/settlement machinery runs identically for one household, for partners' fleets, and for the open market — own fleet → design partners → open market is the onboarding ladder (the GPUtopia lesson inverted) | Signed (even informal) partners; repeat weekly usage |
| A6.4 | Services↔product compounding (the AW-0 bridge): route AW-0 engagements *through* Khala Code + Forge surfaces so product and services evidence compound; engagement case studies (A0.9) double as product demand-gen; satisfied services clients are the natural first paid-plan and self-serve-Autopilot buyers | Each engagement produces product-motion evidence |
| A6.5 | Public traction surface: external-vs-internal token split published next to tokens-served (the counter's honesty upgrade — % external becomes the number that matters) | External share > 0 and rising |
| A6.6 | Community/agent loop: forum-first support, the hotbar Forum surface (shipped lane) as the in-product community door, agents onboarding via AGENTS.md | Measurable forum-sourced retention |

### AW-7 — Operations for a real user base

| Task | Description | Gate/receipt |
| --- | --- | --- |
| A7.1 | Crash/error reporting from the desktop, public-safe and opt-in-consistent (Cloudflare-primitives-first, no third-party SaaS per standing policy) | Actionable crash rate dashboard |
| A7.2 | Issue intake at scale: strict-bug form + forum triage holds when reporters are strangers; SLAs for the channels we claim | Median-time-to-triage tracked |
| A7.3 | CI/infra unblock: resolve the GitHub Actions billing lock or double down on owned-runner Tier-2 as the permanent answer; release pipeline must not depend on one laptop | Releases reproducible by any maintainer |
| A7.4 | Payment-rail ops hardening: the MPP container outage class (stale image, Docker-must-run) gets monitors + runbook automation; treasury balance alerting | Zero silent payment outages |
| A7.5 | Support cost as a first-class number: minutes per user per week, folded into plan pricing (this is the "human review dominates the cost stack" lesson applied to the company itself) | Support cost per WAU measured |

### AW-8 — Pricing and packaging iteration

| Task | Description | Gate/receipt |
| --- | --- | --- |
| A8.1 | Price the paid plan (flat privacy fee vs usage credits vs both); publish the reasoning the way the pricing-model doc already reasons about multipliers and the Bitcoin discount | A price on a page |
| A8.2 | Upgrade-funnel mechanics: plan cards → purchase → entitlement, measured; the privacy attestation receipt as the deliverable businesses can show their own auditors | Free→paid conversion rate exists |
| A8.3 | Packaging experiments gated on evidence: AaaS ("Artanis, Fleet Commander") as the natural premium tier once WS-12 P5 exists; mobile companion as retention, not a SKU | One packaging change driven by data, not vibes |
| A8.4 | The credits/plan interaction decided: does Khala Code spend ride the same balance as API credits? (Architecture says yes — one balance, RL-3 boundaries; product copy must match) | One coherent money story in-app |

### AW-9 — Retention and compounding loops

| Task | Description | Gate/receipt |
| --- | --- | --- |
| A9.1 | The cockpit as the daily-return surface: fleet value visible at a glance (what ran overnight, what merged, what it cost, what it earned) — the "morning briefing" loop. **Let the game be a game**: invest in cockpit delight (the RTS aesthetic, streaks of green closeouts, the visceral pleasure of commanding a fleet) as retention machinery with a straight face — game-grade retention buys the time the network needs to form | D7/D30 retention measured and improving |
| A9.2 | Cross-harness session catalog + repo memory as switching costs that favor us (the accumulated acceptance/route/scorecard data the moat argument depends on) | Measurable reuse of prior-run context |
| A9.3 | Earnings visibility: when AW-4 pays anyone anything, the wallet/earnings surface makes it legible (sats earned, receipts linked) — earning is the retention loop no competitor has | First user who returns *because they got paid* |
| A9.4 | Agent-economy onboarding from inside the product: go-online, forum identity, labor-market discovery as cockpit affordances (the funnel steps that already stand on green records) | Fleet→contributor conversion > 0 |
| A9.5 | **The graduation bridge sells itself**: a public, honest in-product earnings feed (real settlements only, however small — powered by the same exact-accounting spine) visible *before* any consent ask, so social proof, not friction or default-on capture, pulls users from tool mode into the network. Paired copy discipline: sell the honest earnings distribution ("your best solved problem might earn," never "everyone earns") — the median trace is worth ~0 and the copy must say so before scale makes it a broken promise | Consent rate moves when the feed ships; zero overclaim incidents |

### AW-10 — Governance/enterprise readiness (parked behind written gates)

Explicitly not-yet, kept honest the way OTEC and gigawatt lanes always
were. Opens only when AW-6 produces enterprise pull.

| Task | Description | Gate |
| --- | --- | --- |
| A10.1 | Receipts export: per-outcome cost/audit bundles in a shape procurement tools consume (the registry/receipt substrate is already the differentiator; this is formatting, not new truth) | First enterprise asks for it |
| A10.2 | Compliance decision memo: SOC2-class certification — when, whether, cost; written before any enterprise copy | Owner decision recorded |
| A10.3 | Control-plane channel posture: integrate-with rather than build; watch the category as a demand-readiness indicator | Standing review, no build |

## 3. Milestones (each one is a receipt, not a feature)

- **MA0 — The services engine is legible.** Already-sold engagements
  recorded on-platform (first paid `business.*` receipt), the rate card
  published, the pipeline instrumented. Revenue that already exists
  becomes revenue the registry can see. *(This milestone gates on
  nothing — it can land before MA1.)*
- **MA0b — The services engine repeats.** Customer-one cohort at ≥3
  privacy-reviewed completed loops; ≥1 repeat/retainer client; ≥1
  engagement delivered with Artanis-supervised runs and the
  operator-minutes ratio measured.
- **MA1 — A stranger runs it.** Signed artifact public; ≥1 outside user
  completes install → chat → fleet connect → delegation unassisted.
  (`khala_code.desktop_codex_wrapper.v1` green-candidate.)
- **MA2 — A dollar collected on-platform.** Any lane: services
  engagement funded through the product, paid plan, credits, orange
  check. The "no on-platform revenue" sentence dies.
- **MA3 — A trace consented.** Redaction landed, opt-in live at bounded
  scope, corpus > 0, zero leaks.
- **MA4 — A contributor paid by the loop.** One plugin, one routed call,
  one settled revenue-share receipt, honestly labeled (plumbing proof).
- **MA5 — External demand is a number.** Public external-vs-internal
  split; external tokens ≥ 10% of a week's volume or ≥ N distinct
  external accounts (pick the threshold before measuring, not after).
- **MA6 — Repeatable revenue.** Two engines measurable at once: the
  services book (engagements/mo, repeat rate, operator-minutes ratio
  falling) and a cohort of externally-provenanced paying users with
  month-over-month retention; unit economics per package and per plan
  measured (operator/support minutes, serving cost, payout obligations)
  and positive.
- **MA7 — The market version of MA4.** A plugin payout caused by an
  *external* paid routing event — the pays-you loop closed by strangers
  on both sides. This is the company's bull case becoming evidence.

## 4. Metrics That Matter (and the ones that don't)

Track weekly, publish what's public-safe. Three blocks now: the services
engine (primary revenue), then the tool/network axes — tool health
(kindling), graduation (the bridge), network health (the moat forming).

**Services engine (AW-0 — reviewed weekly, first):**

0. Pipeline: qualified intakes, scope calls, close rate,
   time-to-quick-win. Revenue: engagements sold and collected (fiat and
   sats, provenance-labeled), repeat/retainer rate. Economics:
   **operator-minutes per accepted engagement** (the ratio the whole
   motion lives on), gross margin after operator time, per-engagement
   fleet spend routing (owned vs metered). Repeatability: cohort
   `loop_completed` count. Trust: delivery acceptance rate, revision
   rate, zero overclaim incidents in case studies.

**User motion:**

1. Installs → activation (first delegation) → D7/D30 return.
2. Fleets connected; distinct external accounts delegating.
3. **External token share** (the honesty upgrade to tokens-served).
4. Dollars collected, split internal/external like everything else.
   Free→paid-privacy conversion is **the Suno number** — tool revenue
   from people who simply like the thing is not a lesser form of revenue
   while the network forms; Dixon's kindling can be cash-flow positive.
5. Churn; support minutes per WAU.
6. **The graduation funnel** (A1.5's extension): consent rate among
   active users, forum identities created in-product, first-sat-earned
   rate, time-to-first-earn, share of 30-day actives with any network
   surface touched.
7. Consented traces captured; redaction drop rate; corpus dedup yield.
8. Plugin invocations metered — **specifically from non-author
   accounts**; sats settled to contributors; contributor retention
   without subsidy; share of tool sessions that consume network
   artifacts (a plugin, a shared skill, another agent's output) without
   the user asking — the endgame network-health number.
9. Review-minutes per accepted outcome (the verification curve) once
   external work flows — the metric the whole thesis ultimately bets on.

The single number to watch quarterly: **network-attributable retention**
— do users who touched the network retain better than pure-tool users?
If yes, the Dixon flywheel is turning and every tool investment
compounds into the moat. If no, we are a good game with a paid tier
(see the Suno review in §5), and we should underwrite ourselves
accordingly.

Explicitly demoted: raw tokens-served (own-capacity dogfood can 4× it
any week we choose — it proves capacity, not demand), PR-merge counts,
promise-count aesthetics ("more greens" is an output of receipts, never
a target), and any counter an internal fleet can move.

## 5. Kill / Pivot Criteria (written before we need them)

Falsifiers, so this roadmap can fail honestly instead of decaying into
narrative:

- **The agency trap (services):** if operator-minutes per accepted
  engagement do not fall over two consecutive quarters while engagement
  count rises — despite the levers (productized scopes, templates,
  verification ladder, Artanis supervision) — we are building an agency,
  not a platform. Response: cap the services book at what current
  leverage sustains, stop hiring toward delivery, and redirect
  engineering to the specific lever that failed. Services revenue is the
  means; the compounding delivery system is the end.
- **Distribution failure:** if after a real release + channel motion
  (AW-1, AW-6) sustained for a quarter, activation or D30 retention
  stays negligible — the coding-console wedge is wrong; fall back to the
  API/channel business (Khala inside other harnesses) and **double down
  on the services engine (AW-0), which does not depend on the shell**,
  and stop investing in the installer.
- **Data-leg failure:** if consented capture yields a corpus whose
  distilled value is ~zero after honest triage (A3.4), or consent rates
  are negligible — retire the pays-you copy publicly (the registry makes
  this cheap and credible), keep the privacy plan (it stands alone), and
  let the free plan be plain freemium.
- **The Suno review (time-box the tool-only era):** set an internal
  review date when AW-1 ships. If by that date the graduation funnel
  shows tool-love but zero network pull *despite shipped bridges*
  (earnings feed live, consent one click, first payouts real), invoke
  the game-not-network fallback **consciously**: a beloved, self-funding
  tool/game with paid privacy — a real business, minus the moat — and
  re-underwrite spend, copy, and registry records to match, instead of
  drifting there while the deck still says network.
- **Loop failure:** if plugin routing shows no measurable quality/cost
  lift over baseline routing after real attempts — the amortization
  thesis fails at this layer; redirect the attribution machinery to the
  labor/training markets where receipts already clear.
- **Platform failure:** if Codex terms or app-server churn break the
  wrapper faster than the parity contract can track — accelerate the
  Claude/native harness lanes from hedge to primary (the abstraction
  seam exists precisely for this).
- **Economic failure:** if paying users cost more to serve+support than
  they pay for two consecutive cohorts — reprice or restructure before
  scaling spend.

Each trigger produces a written after-action and registry updates, not a
quiet narrative shift. Red tiles are the mechanism working.

## 6. Invariants (carried forward, plus the market-contact set)

All ROADMAP.md §5 invariants persist unchanged (isolated homes, exact-only
accounting, public-safe projections, approval prompts, skip-safe live
tiers, no gate-weakening, clean worktrees, no GitHub-hosted CI). Added
for market contact:

- **Demand provenance on everything.** Tokens, dollars, users, payouts —
  internal vs external typed at the ledger, never inferred at the deck.
- **Consent precedes capture; payout paths precede earning copy.** The
  Ep 239 red record is the permanent reminder of what happens otherwise.
- **No growth mechanics that move counters without users.** If a metric
  can be moved by our own fleet, it is not a growth metric.
- **Owner-arming stays serialized and deliberate.** Batch the sittings
  (AW-2 is one afternoon), but never script around the owner gate — the
  gates are the trust product.
- **Strangers get the safe defaults.** Owner-mode full access never
  ships as anyone else's default.
- **The registry is the scoreboard.** Every milestone here corresponds
  to promise-state movement earned by receipts; if the roadmap succeeds
  and the registry doesn't move, one of them is lying.

## 7. The One-Paragraph Version

ROADMAP.md builds a machine that can do the work; ROADMAP_AFTER makes
someone other than us pay for it and come back — and the fastest honest
dollar runs through `openagents.com/business`, where "Agents that work."
is already selling four-figure software-built-fast engagements delivered
by the same fleet the tool orchestrates. So: record the deals already
done, publish three prices, complete the cohort, and drive
operator-minutes per engagement relentlessly down (templates →
verification ladder → Artanis) — while in parallel the user motion ships
the artifact, arms the rails, captures the first consented trace, pays
the first contributor, and measures external demand with the same
exactness we already apply to tokens. Services pays now and discovers
demand; the tool distributes and supplies; the network compounds later.
The engineering fleet stays useful (every workstream decomposes into the
same issue/PR/verify discipline as ever), but the critical path now runs
through owner sittings, client relationships, and outside humans — and
the company's own registry will be the first to say, in public, whether
the proof arrived.
