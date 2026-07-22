# OpenAgents as Meta-Agent — The Product Is One Agent

**Date:** 2026-07-22
**Lane:** Fable strategy analysis
**Status:** Strategic evidence, not dispatch authority. This document flips no
promise state, changes no runtime authority, mints no issue, and dispatches no
work. The factual authorities remain current code, `docs/sol/MASTER_ROADMAP.md`,
live issue state, contracts, and receipts. Any packet derived from this
document requires normal Sol admission and owner acceptance.
**Sources:** `docs/transcripts/` episodes 241–246 (the Khala arc),
`~/work/ai/packages/agent-harness-contract` (the `AgentHarness` contract and
the seven live adapters), `apps/openagents-desktop/src/provider-lane*.ts` and
`full-auto-*.ts`,
`docs/analysis/2026-07-22-full-auto-autonomy-decision-quality-rubric.md`,
`docs/ai-sdk/2026-07-22-rlm-full-auto-transcript-roadmap-audit.md`,
`docs/nostr/2026-07-22-full-auto-cross-app-agent-delegation-over-nostr.md`,
epic #9171 (children #9172–#9178), `packages/nip90`, `FASTFOLLOW.md`,
`specs/openagents/sarah-owner-orchestrator.product-spec.md`,
`docs/fable/2026-07-21-ai-sdk-division-of-labor-status-audit.md`,
`docs/fable/2026-07-21-nostr-native-pivot-analysis.md`, and `~/work/ai/docs/dse`.
**Labels:** Claims below carry `[EXISTS]` (in the repositories today),
`[NEEDS BUILD]` (a concrete gap with a known shape), or `[SPECULATION]`
(informed design, no code).

---

## I. The thesis

OpenAgents should present itself as **one agent**. Not a chat app with lanes.
Not a dashboard of providers and runs. One persistent, named, accountable
agent identity that the user talks to in every OpenAgents surface — and that
agent IS the product. Internally it orchestrates everything else: the seven
harness adapters (Codex, Claude Code, OpenCode, Pi, Cursor, Goose, Grok), the
Full Auto loop, cross-app delegation over Nostr, and — eventually — hired
marketplace agents it does not own. Externally it is itself just another
agent: it conforms to the `AgentHarness` contract and it speaks ACP as a
**server**, so Zed or any ACP host can attach to the OpenAgents meta-agent
exactly the way our desktop attaches to Cursor or Goose today.

Three properties make this more than branding:

1. **Recursive composability.** The meta-agent is an `AgentHarness` from the
   outside. Anything that can drive a harness can drive it — including
   another meta-agent instance. The abstraction that lets us consume seven
   external agents is the same abstraction that lets others consume us.
2. **Hill-climbability.** Its routing and decomposition policy is not vibes.
   Every delegation produces receipts (verification pass or fail, coherence
   grade, cost, latency). Those receipts are a training set. The policy is a
   typed program that gets measurably better at being a meta-agent — which is
   exactly the property episode 242 claimed for Khala: "This does not
   depreciate; this actually improves."
3. **Market elasticity.** When its own fleet lacks capability or capacity, it
   hires from the NIP-90/NIP-LBR labor market. When it has spare capability,
   it sells itself as a hireable agent on the same rails. The meta-agent is
   both the buyer and a seller in the machine-work economy.

This is not a new idea for OpenAgents. It is the original Khala idea with the
front door corrected: **the unit of product is an agent, not a model
endpoint.** The rest of this document establishes that lineage, sketches the
architecture from what already exists, designs the improvement loop, and
teases out the implications honestly.

---

## II. What the transcripts actually said — the Khala arc (episodes 241–246)

The Khala / Collective Intelligence concept is documented across six episodes.
Reading them in 2026-07 hindsight, the striking thing is how much of the
current substrate they predicted — and the one framing decision they got
wrong.

### Episode 242 — "Khala: Collective Intelligence" (the concept)

Episode 242 introduces Khala as "the new flagship product of OpenAgents":
collective intelligence behind a free OpenAI-compatible API at
`openagents.com/api`, model slug `khala`. The StarCraft reference is explicit
and deliberate: "Khala is a term from StarCraft — it's the link, the
telepathic connection that all Protoss share. So it's like one collective
mind, built up in our case of a bunch of plugins, a bunch of little programs
that can compose into a response." The response could be "text, maybe code,
maybe full software, a website deployment, a legal brief, a research paper."

Four differentiators against Sakana Fugu and OpenRouter Fusion, verbatim in
structure: grown "like an ecology" rather than engineered, emergent from
markets "bottom-up instead of top-down," selected by "bitcoin-paid verified
value" rather than graded on its own benchmarks, and an open pool rather than
a closed pool. The marketplace framing was there from day one: "anybody
running one of our nodes to be able to contribute compute, or data, or labor —
themselves, or their agents doing coding tasks or verification tasks — all in
this open marketplace, with any paid value distributed fairly to whoever
contributes pieces of the workflow."

And the hillclimbing claim was there too: each internal program is "its own
DSPy signature, which means it's independently optimizable — you can run GEPA
to optimize at the prompt level." Followed by the depreciation inversion:
massive pretrained models depreciate rapidly, but "this does not depreciate;
this actually improves."

### Episode 241 — the Fugu review (the competitive frame)

Episode 241 reviews Sakana Fugu: "a multi-agent orchestration system that
behaves like a single model. Users call one endpoint; internally, Fugu decides
which agents and models to use, delegates work, verifies outputs, and
synthesizes a final answer." The episode's critique, echoing outside
researchers: a closed orchestrator over closed models is not AI sovereignty.
The positioning answer: Khala as "one endpoint that behaves like a single
model but is an agent network underneath," open, inspectable, and watchable.
The episode also records the suspicion that frontier "models" are already
compound systems — the labs just do not say so. The meta-agent thesis is that
suspicion turned into an honest product shape.

### Episodes 243–244 — dogfooding and the "super model"

Episode 243 pushes real OpenCode coding traffic through Khala, scaling to ten
concurrent sessions ("We're now in the inference business"). Episode 244 is
the pivotal one for this document. The host routes his own day-to-day Codex
work through Khala and names the shape: "OpenAI's got the model, GPT-5.5,
they've got the harness, Codex, and we're combining that into our... let's
call it a 'super model.'" The audit that episode produced (epic #6273) found
that the Pylon coding-assignment executor pipeline already existed end-to-end
and that "the net-new work is the router" — the caller-aware selection layer.

Episode 244 also states the product motivation for the inversion in the
plainest possible terms: "the interfaces right now, where I'm like bouncing
between Claude Code for one thing and Codex for another thing, and wrestling
with mobile apps and desktops and logins... I just want all of that away. Put
everything into one orchestration layer with a single basic API endpoint."
And it closes with the Khala CLI answering "hi" with: "No, we are Khala, a
collective intelligence." The product already spoke in first person as one
agent — in a CLI demo, three weeks before the substrate could back it.

### Episodes 245–246 — economics and the two-product frame

Episode 245 ("Khala Code") adds the pays-you economics: free users pay with
scrubbed traces, paid users pay with money, and trace contributors get a cut
when future paid usage routes through plugins condensed from their traces.
Episode 246 goes full-screen in Khala Code as the only harness and converts
lived UX complaints into the enforced behavior-contract registry — the
receipts-first culture the hillclimbing loop in §VI depends on.

### What the transcripts got right early, and what was wrong

**Right, and now buildable:** one identity you talk to that fans work out to
a pool. Market selection over self-benchmarks. Independently optimizable
typed sub-programs. Verification as the load-bearing wall (episode 237's
"accepted outcome" framing). The Protoss communion metaphor itself — a link
that joins many minds into one — is exactly a meta-agent over a harness
fleet.

**Wrong, in hindsight:** the front door. Khala shipped as an OpenAI-compatible
**model endpoint** because in that season "OpenAI-compatible API" was the
go-to-market shape people understood (242 says this explicitly — Fugu's launch
was the lightbulb). But a model endpoint is a stateless completion function.
It has no durable identity, no thread ownership, no memory, no accountability
surface, and no way to show its work. Everything the Khala arc wanted —
long-running work, interruption and steering, receipts, "watchable in the
Verse" — fights the completion-API shape. The agent-protocol era (ACP,
sessions, permission requests, plan updates, artifact refs) is the correct
container for the same idea. **The meta-agent is Khala with an agent-shaped
front door instead of a model-shaped one.** Also wrong, or at least premature:
"one model slug" hid the fleet entirely. The 2026 posture is disclosure —
honest attribution of which lane did what is now a behavior-contract-grade
requirement (#9127 landed the single-delegate honest-attribution rule). The
communion aggregates. It must not conceal.

---

## III. The core inversion

### What the UI says today

Today the desktop exposes the machinery: provider lanes, accounts, Full Auto
runs, delegate children, run reports. The user selects, supervises, and
stitches. The 2026-07-22 autonomy audit states the consequence precisely:
Full Auto "is a robust continuation engine... It does not choose the
objective, sequence a roadmap, or check the result." The owner is still the
planner, the verifier, and the memory. `[EXISTS]`

### What the inversion changes

The meta-agent inverts figure and ground. The ONE thing in the UI is a named
agent. You state outcomes. It selects, decomposes, routes, verifies, and
reports — and the lanes, runs, and children become its **observability
drawer**, not its primary surface.

Three product-level consequences follow from making it "the main thing you
talk with," and this is why the framing is a forcing function rather than a
skin:

1. **One thread forces memory.** If every interaction lands on one persistent
   identity, "the agent forgot what we did yesterday" becomes a visible
   defect instead of an accepted property of stateless lanes. That forces
   the RLM recall path (`full-auto-recall.ts`, `@openagentsinc/rlm`) from
   "reachable but not wired" (the RLM audit's verdict) to load-bearing.
2. **One identity forces accountability.** When the meta-agent says "done,"
   there is exactly one party whose claim it is. It cannot shrug toward a
   sub-agent. That forces host-executed verification (HANDS-2 #9173),
   because an accountable identity that repeats unverified sub-agent claims
   is a liar with a name.
3. **One conversation forces routing quality.** If the user no longer picks
   the lane, the router's mistakes are no longer the user's mistakes. Routing
   moves from a convenience to the product's core competence — which is what
   makes the hillclimbing loop (§VI) existential rather than nice-to-have.

The mental-model change for users is from "a workbench of agents" to "an
agent with a workbench." The mental-model change for us is sharper: **the
router, the planner, the verifier, and the memory are the product.** The
harnesses are commodities we did not build and do not control. Episode 244
already knew this ("the net-new work is the router"). The meta-agent thesis
is that finding, promoted from an implementation note to the company shape.

---

## IV. Architecture sketch — the meta-agent's anatomy

The meta-agent is not a new runtime. It is a **named composition** of parts
that exist or are already specified, plus two genuinely new surfaces. From
the outside it is small: an `AgentHarness` and an ACP server. From the inside
it has organs.

### The body plan

```
                      ┌────────────────────────────────────────────┐
   user (any surface) │            META-AGENT (one identity)       │
   ──────────────────►│                                            │
   ACP clients (Zed…) │  face:   AgentHarness impl + ACP SERVER    │
   ──────────────────►│  brain:  select → plan → route → verify    │
   other meta-agents  │  memory: RLM recall over its own history   │
   ──────────────────►│  hands:  harness fleet (7 adapters)        │
                      │  reach:  cross-app delegation over Nostr   │
                      │  wallet: marketplace hiring (NIP-90/LBR)   │
                      │  spine:  receipts / evidence / ledgers     │
                      └────────────────────────────────────────────┘
```

### Face: the meta-agent as an `AgentHarness` — `[NEEDS BUILD]`, thin

The `AgentHarness` contract (`~/work/ai/packages/agent-harness-contract/src/adapter.ts`)
is deliberately minimal: a tagged spec, descriptive fields, one entry method
`start` returning a `HarnessSession` that streams neutral
`KhalaRuntimeEvent`s. Seven production adapters conform today — Codex,
Claude Code, OpenCode, Pi, Cursor, Goose, Grok — and the live seven-lane test
(`seven-lane.live.test.ts`) already runs all of them in ONE orchestrated
conversation on one shared workspace, with an orchestrator that delegates,
corrects, and synthesizes. `[EXISTS]`

The recursive move: implement `metaAgentHarness: AgentHarness` whose `start`
opens a session against the meta-agent brain instead of an external process.
Everything downstream — `HarnessAgent` facade ergonomics
(`harness-agent.ts`), UI chunk projection, event logs, slice runner — works
unchanged, because the contract was designed provider-blind. This is the
cheapest possible proof that the composition is honest: if the meta-agent
cannot conform to the same contract it imposes on its fleet, the abstraction
is wrong. `[NEEDS BUILD]`

### Face: the ACP server inversion — `[NEEDS BUILD]`, medium

Today OpenAgents speaks ACP only as a **client**. The SDK has a generic ACP
harness adapter factory (`acp-adapter.ts`, HARN-04) that turns "ANY admitted
ACP peer" into an `AgentHarness`, with live transports for Cursor, Goose, and
Grok. The desktop maps the ACP projection vocabulary onto the frozen lane
envelope (`provider-lane-acp.ts`) and manages peer binaries
(`acp-provider-host.ts`). `[EXISTS]`

The inversion is to also implement the other side of the same wire: an ACP
**server** that exposes the meta-agent as a peer. Concretely: implement the
agent-side methods (`initialize`, `session/new`, `session/prompt`), emit
`session/update` notifications (message chunks, thought chunks, tool calls,
plan updates — the exact discriminators our own projector consumes), and
route `session/request_permission` to the host approval model we already
have (`RuntimeInteraction`, kind `tool_approval`). We hold an unusual
advantage here: because we CONSUME four ACP peers in production, we know
precisely which subset of the protocol real hosts exercise, and we have a
conformance oracle for free — point our own ACP client adapter at our own
ACP server and replay the seven-lane scenario through it. `[NEEDS BUILD]`

What this buys: Zed users — and every ACP host that follows — can attach the
OpenAgents meta-agent as "just another agent" and transparently get the whole
fleet, the verification gate, and the receipts. Distribution through other
people's editors, without shipping an editor. FASTFOLLOW already tracks
`acp_peer_profiles`, `acp_edge_adapter`, and `bidirectional_acp_mcp` as
learning intents, so this inversion has standing study coverage. `[EXISTS]`

### Brain: selection, planning, verification — the #9171 autonomy core

The brain is exactly the gap ledger of the autonomy rubric, and epic #9171
is already the ordered build plan for it. Current honest scores
(design-grounded, from the audit): D5 Selectivity 1, D6 Self-verification 1,
D3 Foresight 1. In prose: the host does not choose the work (HANDS-1 #9172),
does not verify the done condition (HANDS-2 #9173), and keeps no persistent
decomposed plan (HANDS-3 #9174). The route decision itself is deterministic
and fail-closed (`full-auto-routing.ts`, FA-RT-01) with advisory-only Apple
FM — strong hygiene, no planner. `[EXISTS]` as continuation engine and gate,
`[NEEDS BUILD]` as brain, with issues already open.

The meta-agent framing adds one requirement on top of #9171: the selection,
plan, and verification state must be **thread-visible**. The plan is not an
internal artifact of a run — it is what the agent says when you ask "what
are you doing and why." `[NEEDS BUILD]`

### Memory: RLM recall — `[EXISTS]`, unwired

The 2026-07-22 RLM audit verdict: "REACHABLE BUT NOT WIRED." A complete host
consumer exists (`full-auto-recall.ts`, run-scope isolated, bounded, cited,
advisory), a model-callable `history_recall` host tool reaches Tier D and
gated Tier S, and the engine runs typed recall programs with validated
citations and honesty records. No live loop calls the Full Auto consumer yet.
HANDS-5 #9176 wires it plus a folder corpus adapter. For the meta-agent this
is the difference between an amnesiac dispatcher and a colleague: memory of
its OWN past routing decisions and their outcomes is also the substrate of
the hillclimbing loop (§VI).

### Hands: the harness fleet — `[EXISTS]`

Seven `AgentHarness` adapters in the SDK, two hand-wired desktop lanes
(codex-local, claude-local) plus ACP lanes behind the `ProviderLane` SPI with
its shared dispatcher (`makeProviderLaneDispatcher`) — content admission,
durable turn journal, host-owned history, usage-ledger attribution, restart
recovery that fails closed to `interrupted_by_restart`. The hosted Khala
fallback lane means the meta-agent always has at least one ready hand. The
fleet is the most finished organ.

### Reach: cross-app delegation over Nostr — `[SPECULATION]`, designed

The 2026-07-22 Nostr delegation doc designs the extension: the same owner's
Full Auto agent in another OpenAgents app, possibly on another machine,
becomes one more worker. Discovery, identity, delegation request, result
refs, and live progress ride signed Nostr events. Settlement authority and
the canonical work record stay off-wire, per the Buzz-teardown posture
("signed projection bus"). For the meta-agent this generalizes cleanly: a
remote OpenAgents instance is just a harness whose transport is Nostr instead
of stdio — and because that remote instance is itself a meta-agent, this is
the first place the recursion becomes real: meta-agents delegating to
meta-agents, each verifying what it accepts.

### Wallet: marketplace hiring — rails `[EXISTS]`, integration `[SPECULATION]`

`packages/nip90` (re-exporting `nostr-effect/nip90`) already defines the
market vocabulary: job request kinds 5000–5999, results 6000–6999, feedback
7000, the OpenAgents labor kinds (5934 code task, 5935 review, 5936 document
work), NIP-LBR quote/acceptance lifecycle with decode-time rejection of raw
prompts, credentials, and payment material, provider-bond feedback, and the
content-addressed `LbrLaborCloseout` receipt composing a full lifecycle into
one verifiable digest. Pylon carries provider capacity advertisement and the
assignment/closeout pipeline. What does not exist: any code path where the
desktop routing layer consults the market as a candidate lane. §VII takes
this up.

### Spine: receipts and evidence — `[EXISTS]`, culture-deep

Exact token-usage rows, ATIF traces, assignment closeouts, behavior
contracts with oracle tests, the promise registry, the coherence ledger with
its no-regression ratchet. The meta-agent does not need a new evidence
system. It needs to ROUTE THROUGH the existing one: every delegation emits
who was hired, what was verified, and what it cost. The spine is what makes
the brain trainable.

---

## V. The Khala fusion, restated

Mapping the 242 concept onto the 2026-07 substrate, organ by organ:

| Khala (episodes 241–245) | Meta-agent (now) | Status |
| --- | --- | --- |
| One endpoint that behaves like a single model | One identity that behaves like a single agent (AgentHarness + ACP server) | front door corrected |
| The router to the mesh | Brain: HANDS-1/2/3 selection, plan, verification over deterministic lane gates | issues open |
| Plugins as DSPy signatures, GEPA-optimized | DSE typed programs over receipt datasets (§VI) | SDK shipped, unapplied |
| Pylon workers, compute/data/labor/verification markets | Harness fleet + Nostr reach + NIP-90/LBR hiring | fleet real, market unwired |
| Bitcoin-paid verified value as the selection pressure | Verification pass-rate and cost receipts as the selection pressure, settlement off-wire | invariant-compatible |
| "Watchable in the Verse" | Observability drawer, thread-visible plan, public trace viewer | partially real |
| "We are Khala, a collective intelligence" (CLI first person) | The named meta-agent identity in every surface | the thesis |

What survives untouched: the communion idea, market selection, composable
optimizable parts, openness as the differentiator against Fugu-shaped closed
orchestrators. What the fusion discards: the model-endpoint front door as
primary (an OpenAI-compatible facade can remain as a compatibility view, but
it is a projection of the agent, not the agent), and opacity of the pool.
What the transcripts could not have known: that by 2026-07 the industry would
converge on an agent protocol (ACP) that gives the "one agent" framing a
standard wire, and that our own SDK would hold a production-grade neutral
harness contract making the recursion nearly free.

---

## VI. The hillclimbing loop — how the meta-agent gets better at being one

Episode 242's claim — "this does not depreciate; this actually improves" —
is only true if improvement is a measured loop, not a hope. The pieces of
that loop now exist separately. Here is the closed circuit.

### What gets scored — `[EXISTS]`

1. **Decision quality.** The autonomy rubric (`full-auto-decision-v1`) scores
   seven dimensions, D1–D7, with observable anchors per score. Its own audit
   already demonstrates the honest-scoring discipline.
2. **Coherence at complexity.** The coherence rubric, the deterministic
   screen (`coherence-screen-v2`, `pnpm run grade:coherence`), the complexity
   ladder C0–C4, and the coherence flywheel's ledger ratchet — "the mean must
   not fall" — give a per-conversation grade that already covers routing and
   handoff failure classes (the scenario matrix stresses route authority,
   mid-run Codex→Claude handoff, and rotation on typed failure).
3. **Verification pass-rate.** Once HANDS-2 lands, every delegated unit gets
   a host-executed pass/fail — the single most valuable label, because it is
   an OUTCOME, not a self-report.
4. **Cost and latency per accepted outcome.** The usage ledger already
   attributes exact tokens per lane per turn. Divide by verified outcomes,
   not by turns.

### What gets improved — `[NEEDS BUILD]`, with the optimizer shipped

The routing/decomposition policy becomes a set of **DSE programs**. DSE
(`@openagentsinc/dse`, "the DSPy of Effect") binds a typed signature to an
immutable content-addressed candidate artifact, keeps dataset revisions and
eval splits immutable, bounds candidate search, and records prediction,
promotion, activation, and rollback receipts. Episode 245 already named the
division of labor exactly: GEPA/DSPy optimizes "the program's parameters, not
its control flow." The control flow stays typed and fail-closed
(`full-auto-routing.ts` stays the admission gate). The optimizable surfaces:

- **RouteSelect** — signature: (task features, lane capability reports, lane
  history stats) → ranked lane choice. Baseline: the current owner-ordered
  policy. Dataset: routing receipts labeled by verification pass, coherence
  grade, cost, latency. This is a classic contextual bandit, and the honest
  v0 is not even learned — it is per-lane empirical pass-rate tables with an
  exploration bonus, computed from receipts. `[NEEDS BUILD]`
- **Decompose** — signature: (objective, done condition, repo context) →
  bounded plan of work units with named verifications. Scored by D3/D1 and
  downstream unit pass-rate. This is HANDS-3's plan, made a candidate
  artifact instead of a hand prompt. `[NEEDS BUILD]`
- **MissionPrompt** — the per-turn mission packet phrasing
  (`full-auto-mission.ts`) as a GEPA-evolvable prompt candidate, scored by
  coherence D2 and churn findings from the run analyzer. `[SPECULATION]`
- **AcceptOrRetry** — signature: (unit, result evidence, verification
  output) → accept, retry same lane, rotate lane, or escalate to owner.
  Scored by false-accept rate (the analyzer's `unverified_completion_risk`
  becomes a labeled negative). `[NEEDS BUILD]`

The loop then is: receipts → dataset revision → bounded offline DSE
compilation → held-out eval → promotion receipt → activation, with rollback
receipts on regression. Note this is exactly the loop the Sarah spec already
mandates for her conversational harness ("bounded candidate production,
held-out evaluation, and an independent Blueprint release gate for next-turn
activation" — with self-promotion in the cut list). The meta-agent adopts the
same law: **the policy never promotes itself.** Optimization is offline,
gated, and receipted. The coherence flywheel's ratchet becomes the
meta-agent's fitness floor: no promoted candidate may lower the ledger mean.

Why this is credible rather than aspirational: every input to the loop except
the labels-from-verification is on `main` today — the rubric, the screen, the
ledger, the exact usage attribution, DSE with its optimizer subpath, and RLM
to mine the meta-agent's own history into cited datasets. The loop's critical
path runs straight through HANDS-2. **Verification is not just a safety
feature. It is the label supply for the entire improvement economy.** That
is episode 237's "verification is the load-bearing wall," rediscovered as a
machine-learning statement.

---

## VII. Marketplace implications — the meta-agent as buyer and seller

### Buyer — elastic capacity and missing capability

Today the routing gate admits only owner-local lanes (own-capacity-only is a
non-overridable guardrail, correctly). The marketplace extension adds a THIRD
lane class behind the same fail-closed gate: a market lane, where dispatch
means publishing a NIP-LBR request (kind 5934/5935/5936, ref-only, budget
tagged), evaluating quotes, accepting one, and treating the returned result
refs exactly like a local turn's self-reported evidence — subject to the SAME
host verification before acceptance. The `LbrLaborCloseout` digest becomes
the market analog of the local turn journal's terminal disposition.
`[SPECULATION]` as integration, `[EXISTS]` as protocol.

The trust asymmetry is the key design fact: hired agents are strictly less
trusted than owned harnesses, so the market lane is only safe AFTER HANDS-2
exists. The buy-side ordering is therefore forced: verification first, hiring
second. A meta-agent that hires before it can verify is a fraud amplifier.

Payment boundaries per existing invariants: quotes and amounts may ride the
wire as integer msat refs, but invoices, preimages, wallet material, and
settlement authority stay off-wire (`packages/nip90` rejects them at decode
time). Settlement remains on the existing payout boundary. The meta-agent
never becomes a custodian, and it never claims a market result as verified
until the host check passes — no unverified public claims.

### Seller — the meta-agent as a hireable agent

The inversion of the inversion: the meta-agent announces ITSELF as a service.
Two rails, both cheap once the face exists: the ACP server (any host can
attach it as an agent) and a NIP-89-style handler announcement plus NIP-90
provider behavior (the market can hire it like any DVM). This is where the
recursion earns money: a customer's meta-agent hires the OpenAgents
meta-agent for a capability it lacks, receives a closeout receipt with
verification evidence, and never needs to know or care that seven harnesses
and two sub-hires did the work. `[SPECULATION]`

### The business geometry

Three revenue surfaces fall out, all Khala-arc-native: **capacity arbitrage**
(route each unit to the cheapest lane that clears verification — the margin
between a subscription-backed local lane and market price is real and
measurable from the usage ledger), **marketplace take** (a settled-percentage
fee on brokered hires, justified not by matchmaking but by the verification
and receipt layer — we sell the closeout, not the introduction), and the
episode-245 **trace economy** (scrubbed receipts and routing datasets as the
appreciating asset — the free tier pays in exactly the labels §VI consumes).
The defensible asset in all three is the same: the receipts corpus and the
policy trained on it. Models depreciate. Harnesses churn. The routing policy
plus its label supply improves — that is the moat episode 242 described
without having the mechanism.

---

## VIII. Sarah and the meta-agent — the honest relationship

The Sarah ProductSpec (`specs/openagents/sarah-owner-orchestrator.product-spec.md`)
defines `principal.sarah`: one authenticated owner's persistent orchestrator
on one stable thread, with cited durable memory, bounded fresh business
context, an Effect authority service, and brokered access to existing
capabilities — explicitly forbidden from growing her own CRM, dispatcher, or
authority model. Her hypothesis sentence is the meta-agent thesis scoped to
one user: "the owner can run the company through one mobile conversation."

Conclusion: **the meta-agent and Sarah are the same pattern at different
scope, and Sarah is the first resident instance.** Sarah = meta-agent pattern
+ owner-private business corpus + a specific named persona + the owner
authority profile. The meta-agent generalizes exactly three things: the
principal (every authenticated user gets one), the corpus (their workspace
and history instead of company projections), and the authority profile
(per-user capability grants instead of `AUTHORITY.md`). What does NOT
generalize: Sarah's company-context projections and her owner-only grants.

This resolution has teeth in both directions. For the meta-agent: do not
build a second persona state machine, a second thread model, or a second
authority broker — the Sarah spec already prohibits parallel infrastructure,
and the prohibition should bind the general case too. For Sarah: every organ
the meta-agent hardens (verification, recall, routing receipts, the DSE
promotion gate she already specifies) is Sarah's substrate. Sarah is not a
competitor to the meta-agent and not its predecessor to be retired. She is
its most demanding customer and its proof that "one principal, one thread,
brokered capabilities, receipts everywhere" is buildable under the current
authority model. If the two ever diverge architecturally, one of them is
wrong.

---

## IX. Full implications

### Product

The three apps stop being three products with lanes and become three windows
onto one agent. Desktop: the meta-agent with the deepest observability drawer
and local hands. Mobile: the meta-agent with Sarah's owner scope (already the
landing surface per the Sarah spec). Web: the meta-agent's public,
shareable, receipt-backed face. "One click" (#9171's posture, HANDS-6
auto-update included) becomes "zero clicks with one veto": the agent
proposes, the owner disposes. The UI collapse is real but not total — the
forcing function is one CONVERSATION, and the machinery remains one
disclosure gesture away, because the trust model (next) requires it.

### Trust

One identity accountable for delegated work is the scaling form of the
host-verifies invariant. Today "provider completion is self-reported evidence
only; the host or owner verifies" is a per-turn literal in the mission
packet. Under the meta-agent it becomes the identity's constitution: the
agent may only assert what its spine can cite. This composes outward — an
ACP host or a market buyer attaching to the meta-agent inherits a
counterparty whose claims are receipt-backed by construction. Openness stays
the differentiator: Fugu-shaped orchestrators ask you to trust the router
because you cannot see it. Ours is inspectable AND carries receipts, which is
the stronger claim.

### Competition

Cursor, Devin, and every single-agent product sell one harness with one
model's ceiling. An agent OF agents has a different ceiling: it improves when
ANY vendor ships a better harness, because a new harness is one adapter away
from being its hands (the seven-lane test proves the marginal cost). The
single-agent vendors also cannot credibly become meta-agents, because
routing work to competitors' agents is against their economics — while it IS
our economics. The nearest true competitors are the closed orchestrators
(Fugu) and the labs' internal compound systems. Against those, the moat is
§VI plus openness: they can match the architecture but not publish receipts,
and not accept outside harnesses without breaking their own bundling.

### Risks, stated plainly

1. **Single point of trust.** Concentrating all delegation behind one
   identity makes that identity the aggregation point for prompt injection,
   over-delegation, and a total-compromise blast radius. Mitigation is
   existing law: deterministic fail-closed gates around every dispatch,
   non-overridable guardrails, authority intersection, and no policy
   self-promotion. The meta-agent must be a composition of gated parts, not
   a superuser.
2. **Latency and cost of indirection.** A selection-planning-verification
   sandwich around every trivial request would make the product worse. The
   brain needs a fast path: route trivially, plan only above a complexity
   threshold (the C0–C4 ladder gives the threshold a measurement).
3. **Identity confusion.** One first-person voice over seven sub-agents
   invites laundering — the meta-agent presenting Codex's claim as its own
   verified knowledge. The attribution rules (#9127) and the citation-only
   assertion law are the countermeasure, and they must survive the collapse
   into one voice.
4. **Hiding fleet failures.** A meta-agent graded on smoothness will learn to
   paper over sub-agent failure. The coherence flywheel's tripwires and the
   analyzer's churn findings must remain adversarial to the policy — the
   grader and the graded stay separate, which is one more reason optimization
   is offline and independently gated.
5. **The brand risk of naming.** Once the product speaks as one agent, every
   sub-agent's worst day is the product's worst day, in first person. That
   is also the point: it is the incentive gradient that funds verification.

### For Full Auto

Full Auto does not get replaced. It gets promoted: today's continuation
engine plus #9171's brain IS the meta-agent's autonomous mode. The epic's
child ordering (selection, verification, planning, churn, recall, one-click)
is unchanged by this document — this document only adds the claim that those
six children are not a feature of one mode but the core of the product
identity.

---

## X. Honest staging

### What exists today, precisely

- `[EXISTS]` The neutral `AgentHarness` contract, seven live adapters, the
  seven-lane orchestrated live test, the `HarnessAgent` facade, event logs,
  slice runner, sandbox providers (SDK, epics #9115/#9128 closed).
- `[EXISTS]` The desktop `ProviderLane` SPI, shared dispatcher, ACP client
  lanes, deterministic Full Auto routing gates, mission packets, run
  registry, run analyzer, delegation runtime card, hosted-Khala fallback.
- `[EXISTS]` RLM engine as `@openagentsinc/rlm` with an unwired Full Auto
  consumer and a live `history_recall` host tool.
- `[EXISTS]` DSE with contract, runtime, and gated optimizer subpaths.
- `[EXISTS]` The measurement stack: autonomy rubric, coherence rubric plus
  deterministic screen plus ledger ratchet, complexity ladder, exact usage
  attribution, behavior contracts, promise registry.
- `[EXISTS]` NIP-90/NIP-LBR protocol helpers with closeout receipts, and
  Pylon capacity/assignment rails.
- `[NEEDS BUILD]` Everything that makes these ONE agent: the identity, the
  meta-agent harness implementation, the ACP server, the #9171 brain, the
  wired memory, the receipt-labeled routing dataset, the market lane.

### Meta-agent v0 — the minimal first slice

One bounded slice, all inside existing authority, no new infrastructure:

1. **Name it.** One persistent agent identity presented as the default
   conversation in the desktop — the existing chat, router, delegation, and
   Full Auto behind one thread and one voice, with the machinery moved to a
   drawer. (Naming is an owner decision. "Khala" is the obvious candidate —
   the CLI already used the first person in episode 244.)
2. **Conform it.** `metaAgentHarness: AgentHarness` in front of the existing
   turn kernel, proven by driving it with the SDK's own facade and recording
   a gradable transcript like any other lane.
3. **Expose it.** A loopback-only ACP server serving that harness, proven by
   our own ACP client adapter attaching to it and replaying a bounded
   scenario. Zed attachment is the visible demo.
4. **Grade it.** The v0 identity enters the coherence flywheel as a graded
   surface from day one, so the hillclimb has a baseline before any
   optimization exists.

This deserves a work packet under normal Sol admission (repository policy:
no feature issue from this document — the packet route per the claim
protocol). It deliberately excludes the brain, the market, and any learned
policy.

### The ordered path after v0

1. **Brain:** #9171 as written — HANDS-1 selection, HANDS-2 verification,
   HANDS-3 planning, HANDS-4 churn, HANDS-5 recall wiring, HANDS-6 posture.
   Verification remains the critical path for everything below.
2. **Receipts-to-dataset:** routing receipts labeled by verification and
   coherence, per-lane empirical stats surfaced in the drawer. First
   non-learned policy improvement: evidence-ranked lane ordering.
3. **DSE candidates:** RouteSelect and AcceptOrRetry as gated offline
   candidates with promotion/rollback receipts, ratchet-bound.
4. **Reach:** cross-app Nostr delegation per the 2026-07-22 design — same
   owner first, meta-to-meta later.
5. **Market buy-side:** the NIP-LBR lane behind the same gates, only after
   HANDS-2 is proven in production.
6. **Sell-side:** NIP-89-style announcement plus ACP server hardening —
   the meta-agent as a hireable, receipt-bearing counterparty.

### What would falsify the thesis

Three honest kill conditions. If one-voice presentation measurably degrades
task success versus direct lane selection for expert users and the drawer
does not recover it, the inversion is wrong as a default (keep it as a
mode). If verification pass-rate labels prove too sparse or too noisy to
rank lanes better than a static owner ordering, the hillclimb reduces to
hygiene and the "improves, not depreciates" claim stays rhetoric. If ACP
server adoption yields no external attachment demand within a season, the
face was premature and the effort belongs in the brain.

---

## XI. Closing

Episode 242 promised a collective intelligence that answers as one mind,
grows like an ecology, is selected by verified value, and improves instead of
depreciating. Episode 244 found that the pipeline existed and the router was
the missing organ, then let the CLI say "we are Khala" before the substrate
could stand behind the pronoun. Two seasons later the substrate is mostly on
`main`: a neutral harness contract with seven live hands, a continuation
engine with fail-closed gates, an unwired memory, a shipped optimizer, a
measurement stack with a ratchet, and market rails with receipts. What is
missing is not a system. It is a sentence the product can finally make true:

**"I am the OpenAgents agent. Tell me the outcome you want. I will choose
who does the work, check it before I claim it, show you the receipts, and be
better at this next week than I am today."**

Everything in this document is the engineering behind that sentence.
