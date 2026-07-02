# QA Swarm — The Product Plan

Date: 2026-07-02
Status: product plan + execution roadmap in the Fable lane. **Nothing here is
a product promise, served capability, or public claim copy** — the registry
(`docs/promises/`) governs claims; `qa.agentic_qa_runner.v1` is yellow and the
QA Swarm records proposed below enter at planned/yellow through the normal
gates. This doc flips no promise state and broadens no copy. Issue map in §9.

Sources: the Rhys requirements and sales-motion record
(`docs/feature-requests/2026-06-24-autonomous-qa-e2e-from-computer-use.md`),
the QA-on-every-push doc
(`docs/qa/2026-06-25-qa-agent-khala-dogfood-and-qa-on-every-push.md`),
[`ROADMAP_QA.md`](./ROADMAP_QA.md) (the fully automated QA cycle, epic #8051),
the QA framework design + §15 addendum
(`2026-07-01-khala-code-desktop-qa-framework-design.md`), the Arbiter graphics
audit (`docs/unit/2026-06-30-arbiter-effect-2d-dataflow-graph-audit.md`), the
services engine analysis
(`2026-07-02-agents-that-work-business-services-analysis.md`), and
[`ROADMAP_AFTER.md`](./ROADMAP_AFTER.md) (AW-0).

## 0. The One-Paragraph Product

**QA Swarm: point a swarm of QA agents at your product and get proof it
works.** A coordinated fleet of autonomous QA agents — scripted scenarios,
seeded monkeys, LLM explorers, perf probes — drives your app through a real
browser, a real terminal, and (on macOS) the real native window; every
discovery distills into a committed, re-runnable e2e test; every run produces
an honest CONFIRMED/REFUTED verdict, videos, exact accounting, and a
**shareable web URL** where the whole swarm is visible as a live, cinematic
StarCraft-blue board — nodes for every agent and scenario, edges that light
only when real receipts land. It is the ROADMAP_QA machine (drivers, oracles,
coverage ledger, budgets, explorers, nightly loop) packaged as the thing a
business buys, built almost entirely from parts already shipped
(`@openagentsinc/qa-runner`, the `/trace/{uuid}` surface,
`@openagentsinc/arbiter-effect`, the fleet-run engine). **We are customer
number one: QA Swarm's standing engagement is Khala Code Desktop itself**, and
its first sales artifact is the evidence that engagement produces.

## 1. Everything Rhys Wanted (the requirements contract)

The named first customer's requirements, from the 2026-06-24 feature-request
record, each now mapped to its QA Swarm surface:

| # | He asked for | QA Swarm answer | Substrate state |
| --- | --- | --- | --- |
| Thesis | Verify agent work **without running anything locally** — read the e2e test + its output | The share URL: verdict + distilled test + video + trace, reviewable in a browser | `/trace/{uuid}` live; share page is QS2 |
| 1 | Real dev tools (Chrome, terminal) | qa-runner drivers: Chromium/CDP, PTY terminal, container, native macOS AX | shipped |
| 2 | Develop → **distill into committed tests** | The distiller emits `*.e2e.test.ts`; explore→distill→regress is ROADMAP_QA Q6.2 | shipped (distiller); loop is Q6.2 |
| 3 | Pluggable targets (dev/prod, same test) | Multi-target registry; prod read-only policy | shipped; third-party onboarding is QS9 |
| 4 | Fast + cross-OS | Parallel sharding, container/native backends; swarm parallelism via the FleetRun engine | shipped core; swarm-parallel runs QS5 |
| 5 | **OSS + local** | MIT `@openagentsinc/qa-runner@0.1.0` on npm, BYO model, no login | shipped |
| 6 | Video output | Playwright video + ffmpeg compose (verdict cards); auto-attach to PRs via the gh-attach motion | shipped; PR bot polish in QS7 |
| 7 | **"Chill evals"** — compare agents across MCP/config changes | `/trace/compare/{ids}` live + the comparison runner; productized variant axis is QS8 | partially shipped |
| Sales | *"the best way to make me believe in your product is to show it working"* — a demo PR on his repo with auto-attached webm + terminal video | The QS7 sales motion: demo PR on `RhysSullivan/executor` with video + distilled test + verdict + share URL | not yet run |
| Candid | *"my setup is decent but the last 10% would go so far"* | The reviewed, typed harness quality bar (`apps/qa-runner/docs/harness-quality-bar.md`) + the ROADMAP_QA gates | shipped bar; gates are QA-8 |

Plus everything ROADMAP_QA adds beyond his list — the parts that make it a
*swarm* rather than a runner: the seeded monkey and LLM explorers with a
coverage frontier, the perf/latency budget family, the memory/zombie oracles,
formal TLA+ checks, the nightly loop with auto-filed regressions, and
fleet-scale parallel dispatch with claims (no two agents duplicate work).

## 2. Customer Number One: QA Swarm Pointed At Khala Code

The dogfood engagement is not a demo — it is the standing production run:

- **The target**: Khala Code Desktop (the hardest kind of target: an
  Electrobun native app + web preview + headless JSONL + a 57-method RPC
  bridge). Everything ROADMAP_QA schedules — the nightly matrix (Q1.1), the
  monkey nights (Q6.3), the perf sweep (Q2.3), headed native runs (Q3.1) —
  **is** the QA Swarm engagement, branded and reported as one.
- **The proof already exists**: the 2026-07-02 audit session ran the swarm's
  parts by hand and caught two real regressions on `main` within the hour
  (stale visual smokes after the fleet-run RPC and Foldkit cockpit landings)
  plus a product robustness bug (cockpit blanks on one failed RPC). That
  session is the case-study seed: *this is what happens the first time the
  swarm actually runs*.
- **The deliverable shape**: a weekly public-safe QA Swarm report for Khala
  Code at a shareable URL — verdict wall, coverage %, perf trends vs budgets,
  the distilled-regression count, videos of the worst findings — generated
  from the same Q1.5 status surface, styled like the product (§4).
- Per the services doctrine (`AW-0`, A0.4): the engagement's own receipts
  become the sales collateral, opaque refs only when the client isn't us.

## 3. What A Business Buys (packages, modeled — not published copy)

Aligned with the services-engine bands
(`2026-07-02-agents-that-work-business-services-analysis.md` §3); prices are
modeled recommendations gated on the owner's rate card:

1. **The Swarm Audit (Quick Win, $1–5k fixed).** Point the swarm at your app
   for a bounded window: seed-corpus scenarios written for your surfaces, one
   monkey night, one explore night, a perf baseline against budget defaults.
   Deliverable: the share-URL report, every finding as a reproducible seed or
   distilled test, and a committed regression pack PR. The Rhys-style demo PR
   is this package run as a sales motion.
2. **QA-on-every-push (Retainer, $2–10k/mo).** The Tier-1/Tier-2 pattern
   installed on the customer's repo: bounded pre-push smoke + full async swarm
   on hosted runners per push, PR comments with auto-attached video + verdict
   + share URL, nightly frontier reports, budget regression alerts. This is
   the `docs/qa/2026-06-25` design, productized.
3. **The Swarm Sprint ($5–15k).** A week of swarm + fleet against a backlog
   of QA debt: coverage climbed to an agreed floor, flake quarantine emptied,
   perf offenders burned down to budget — the ROADMAP_QA motion executed on
   someone else's product.
4. **OSS core stays free** (qa-runner, BYO model, no login) — the funnel and
   the trust anchor, per the tool/network strategy.

## 4. The Visual Layer (the part you can see and share)

The differentiating surface: QA evidence rendered as the same
StarCraft-Protoss energy language as the rest of the product (root
`DESIGN.md`; uniform blue, no light/dark modes), built on the two graphics
substrates this repo already owns:

### 4.1 The Swarm Board — `arbiter-effect` (2D, live, evidence-bound)

The Arbiter audit (`docs/unit/2026-06-30`) built exactly the right primitive:
typed-pin MIMO nodes, first-class links, JSON GraphSpec with embedded layout,
force auto-layout, live datum inspection, and the load-bearing rule — **a
link only lights when a real receipt dereferences**. The QA Swarm board is
its second consumer (after the Gym pane):

- **Nodes**: each swarm agent (scenario runner, monkey, explorer, perf probe,
  headed AX driver), each target surface (panel/RPC group), each oracle
  family, the verdict/distiller stage.
- **Edges light on evidence**: a scenario→oracle edge pulses only when a
  phase's oracle actually evaluated (receipt = the run report row); a
  finding→distilled-test edge lights only when the committed test merges. No
  animation without a receipt — the Verse discipline applied to QA.
- **Live datum on nodes**: coverage counts ticking, current seed, p95 vs
  budget, tokens (exact/pending/not_measured honesty).
- Renders in three places from one projection schema: the **share page**
  (§4.3), a **Khala Code Desktop QA panel** (beside Fleet/Gym), and the
  nightly report.

### 4.2 The spectacle option — `three-effect` (3D, demo-grade)

For the share page hero and episode/demo material: the swarm as a Verse-grade
3D scene — agent motes orbiting the target, edges arcing on receipt events,
settlement-burst-class visual effects on verdict landings. Quality bar per
the standing graphics mandate: additive-HDR + bloom spark-burst grade, not
thin flat lines. Deliberately optional and behind the 2D board in priority —
the 2D board is the daily tool; the 3D scene is the demo weapon.

### 4.3 The share URL — public-safe QA evidence at a link

Extend the shipped `/trace/{uuid}` pattern to a run-level surface:
`openagents.com/qa/{runRef}` — a public-safe projection of one swarm run (or
a standing engagement's latest): verdict wall
(CONFIRMED/REFUTED/INCONCLUSIVE per commitment), coverage ledger + frontier,
perf trends vs named budgets, embedded videos, per-finding links to
`/trace/{uuid}` and distilled tests, and the Arbiter swarm board rendered
live. Redaction-checked like every projection; opaque client refs for
non-owner targets. This is the artifact a buyer forwards to their team — the
"review without running anything" thesis as a URL.

## 5. What Already Exists (inventory, honest)

Shipped and live: qa-runner (two brains, typed actions, four backends,
distiller, verify verdicts, honest exit codes, HTTP control API, npm OSS),
ATIF traces + `/trace/{uuid}` + `/trace/compare`, redaction service,
Codex/Claude→ATIF converters, Tier-1 pre-push QA smoke + Tier-2 GCE trigger
(warning-only, owner-gated arming), `packages/khala-qa-harness` (drivers,
scenario DSL, oracles, seed corpus, monkey, coverage ledger + frontier,
explorer brain, model-based tier, GEPA seam), qa-metrics budgets, TLA+ specs,
`arbiter-effect` (core + Foldkit SVG renderer, Gym pane consumer),
`three-effect`, the FleetRun engine + claim registry, the `/business` intake,
and the promise record `qa.agentic_qa_runner.v1` (yellow).

Not yet existing (the gap list that becomes §9's issues): the QA Swarm
product definition/copy/rate-card, the run-level share page, the swarm-board
projection + renderer consumers, the 3D scene, the one-command hosted swarm
run, the standing dogfood engagement report, the Rhys demo PR, the chill-evals
product surface, the third-party target-adapter onboarding, and
metering/receipts for hosted runs.

## 6. Execution Roadmap (workstream QS; issues in §9)

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| QS1 | **Product definition + registry records**: QA Swarm named surface through the copy gates; extend `qa.agentic_qa_runner.v1` and add `qa_swarm.*` records (product yellow; hosted runs, share surface, packages planned) with honest blockers; modeled rate card staged for the owner sitting (publishing prices is owner-gated) | — | MED (owner-adjacent at the copy/price step) |
| QS2 | **The share URL** `openagents.com/qa/{runRef}`: public-safe run-projection schema (`openagents.qa_swarm.run_projection.v1`) fed by the Q1.5 status artifact + trace/coverage/perf/video refs; render page on the khala design tokens; redaction-checked; deep links to `/trace/{uuid}` | Q1.5 | MED |
| QS3 | **The Arbiter swarm board**: swarm-run GraphSpec projection (agents/surfaces/oracles/verdicts as nodes; receipt-lit edges; live datum) rendered via `@openagentsinc/arbiter-effect` on the share page and as a Khala Code Desktop QA panel; evidence-binding rule enforced (no receipt, no light) | QS2 | MED |
| QS4 | **The 3D swarm scene** (`three-effect`, demo-grade): share-page hero + demo/episode asset at the spark-burst quality bar; strictly a projection of the same run data; reduced-motion + static fallback | QS2 | MED |
| QS5 | **One-command hosted swarm run**: `qa swarm run --target <url>` / one API call composes qa-runner control API + FleetRun parallelism + the nightly-matrix recipe into a single product run on owned runners (GCE Tier-2 + CF Browser Rendering), emitting the QS2 projection; skip-safe live tiers; per-run resource caps | QS2, Q1.1 | MED |
| QS6 | **Customer-one standing engagement**: brand the Khala Code nightly loop as the QA Swarm engagement — weekly public-safe report at the share URL, findings→issues→distilled-regression counts tracked, the 2026-07-02 session written up as case-study seed (A0.9 pattern) | Q1.1, QS2 | HIGH |
| QS7 | **The Rhys sales motion**: run the Swarm Audit package against `RhysSullivan/executor` and open the demo PR he asked for — auto-attached webm + terminal video (gh-attach), distilled `*.e2e.test.ts`, verdict, share URL; chill-evals comparison of his MCP variants included; owner reviews before the PR goes out (outward-facing) | QS2, QS8 | MED (owner gate on send) |
| QS8 | **Chill-evals productized**: the variant axis (MCP set / config / model / before-after) as a first-class run mode with a side-by-side comparison view on the share surface (pass-rate/latency/behavior deltas + videos), reusing `/trace/compare` + the benchmark aggregates | QS2 | HIGH |
| QS9 | **Third-party target onboarding**: the minimal Target adapter contract for arbitrary apps (auth, fresh identity, optional restart, prod-read-only policy) documented + one worked example beyond our own surfaces; "bring your app, get a scenario corpus, an explore night, and a coverage ledger" | QS5 | MED |
| QS10 | **Metering + receipts for hosted runs**: run = dereferenceable receipt (traceRef already lands on receipts); hosted-run metering rows (exact-only discipline), engagement receipts through the quick-win payment machinery; settlement seams stay INERT until owner-armed | QS5 | MED |

Sequencing: QS1 (definition) and QS6 (dogfood engagement) start immediately —
QS6 needs only ROADMAP_QA Q1.1/Q1.5 landing. QS2 is the spine everything
visual and outward-facing hangs on; QS3/QS4/QS8 fan out behind it. QS7 is the
first external sales artifact and should ship the week QS2+QS8 are usable.
QS5/QS9/QS10 make it repeatable for strangers.

## 7. How This Relates To The Other Plans

- **ROADMAP_QA is the engine; QA Swarm is the product skin.** Every QS task
  consumes ROADMAP_QA outputs (run reports, coverage, budgets, traces) —
  none duplicates them. The nightly loop (Q1.1) is simultaneously our merge
  gate and customer-one's engagement run.
- **AW-0 services engine**: the Swarm Audit / Sprint / Retainer slot directly
  into the `/business` intake and the quick-win receipt machinery; QA Swarm
  is the second productized service line after coding quick-wins.
- **#6181 "out-ship Factory"**: QS completes that epic's productization arc —
  distill-to-committed-test + honest verdicts + native-desktop driver breadth
  + the visual evidence layer Factory's droid-control lacks, plus the two
  things Rhys couldn't get anywhere: committed tests and chill-evals.
- **The tool/network strategy**: OSS qa-runner stays the free single-player
  tool; hosted swarm runs and the share surface are the network-facing
  product; distilled tests and traces are the solved-problem exhaust.

## 8. Invariants

- Registry governs claims: QA Swarm records enter planned/yellow; no green
  without receipts; published prices are an owner decision; no copy implies
  self-serve while delivery is operator-assisted.
- Public-safe projections everywhere: the share page, the boards, and every
  video/screenshot pass redaction tripwires; client refs opaque.
- Honest evidence only: verdicts from observed runs; no fake green; edges
  light only on dereferenceable receipts (the Arbiter rule).
- Exact-only accounting for hosted runs; settlement INERT until owner-armed.
- OSS core stays BYO-model, no-login; hosted tiers are more/faster, never
  lock-in.
- Outward-facing artifacts (the Rhys PR, customer reports) get owner review
  before sending.

## 9. Issue Map

Filed 2026-07-02 under the QA Swarm epic; see the epic checklist for state.

Epic: [#8071](https://github.com/OpenAgentsInc/openagents/issues/8071)

| Task | Issue | Title |
| --- | --- | --- |
| QS1 | [#8061](https://github.com/OpenAgentsInc/openagents/issues/8061) | QA Swarm product definition + registry records |
| QS2 | [#8062](https://github.com/OpenAgentsInc/openagents/issues/8062) | Shareable QA Swarm run page (openagents.com/qa/{runRef}) |
| QS3 | [#8063](https://github.com/OpenAgentsInc/openagents/issues/8063) | Arbiter swarm board (2D live evidence-bound graph) |
| QS4 | [#8064](https://github.com/OpenAgentsInc/openagents/issues/8064) | 3D swarm scene (three-effect, demo-grade) |
| QS5 | [#8065](https://github.com/OpenAgentsInc/openagents/issues/8065) | One-command hosted swarm run |
| QS6 | [#8066](https://github.com/OpenAgentsInc/openagents/issues/8066) | Customer-one standing engagement: QA Swarm at Khala Code |
| QS7 | [#8067](https://github.com/OpenAgentsInc/openagents/issues/8067) | The Rhys sales motion: demo PR on executor |
| QS8 | [#8068](https://github.com/OpenAgentsInc/openagents/issues/8068) | Chill-evals productized (variant comparison mode) |
| QS9 | [#8069](https://github.com/OpenAgentsInc/openagents/issues/8069) | Third-party target onboarding (the minimal adapter) |
| QS10 | [#8070](https://github.com/OpenAgentsInc/openagents/issues/8070) | Metering + receipts for hosted swarm runs |
