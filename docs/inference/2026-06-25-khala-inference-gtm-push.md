# Khala Inference GTM Push — Tokens Served Is The Metric

> Status: **internal strategy doc, 2026-06-25.** This is direction-setting, not
> public claim copy and not a product promise. Anything here that ever becomes a
> user-facing or agent-facing claim must go through the product-promise registry
> (`docs/promises/`, `apps/openagents.com/workers/api/src/product-promises.ts`,
> `https://openagents.com/api/public/product-promises`) first, receipt-first, with
> the honesty discipline in `apps/openagents.com/AGENTS.md` and
> `docs/promises/2026-06-23-khala-public-copy-promise-gate-review.md`. Where this
> doc cites a live system it says so; where it sets direction it labels it
> direction. It flips **no** promise state — the registry is owner-gated.
>
> Companion / cross-links:
> [`README.md`](README.md) ·
> [`2026-06-19-inference-gateway-business.md`](2026-06-19-inference-gateway-business.md) ·
> [`2026-06-19-pricing-model.md`](2026-06-19-pricing-model.md) ·
> [`2026-06-19-pricing-vs-factory.md`](2026-06-19-pricing-vs-factory.md) ·
> [`2026-06-19-fireworks-provider.md`](2026-06-19-fireworks-provider.md) ·
> [`2026-06-19-decentralized-serving-shard-wan.md`](2026-06-19-decentralized-serving-shard-wan.md) ·
> [`../khala/README.md`](../khala/README.md) ·
> [`../khala/khala-buildout-roadmap.md`](../khala/khala-buildout-roadmap.md) ·
> [`../khala/khala.md`](../khala/khala.md) ·
> [`../khala/2026-06-23-khala-benchmark-harness-book-p1-5.md`](../khala/2026-06-23-khala-benchmark-harness-book-p1-5.md) ·
> [`../collective-intelligence/2026-06-24-collective-intelligence-as-an-economy.md`](../collective-intelligence/2026-06-24-collective-intelligence-as-an-economy.md) ·
> [`../transcripts/242.md`](../transcripts/242.md)
>
> Promise-review companion:
> [`../promises/2026-06-25-khala-inference-push-promise-review.md`](../promises/2026-06-25-khala-inference-push-promise-review.md)

## 0. What just shipped (live, verified today)

Khala is live as an **OpenAI-compatible inference API**:

- **Endpoint:** `POST https://openagents.com/api/v1/chat/completions`, single public
  model id **`openagents/khala`** (the `khala-mini` / `khala-code` ids are not
  public products; see `../khala/README.md` and the §6228 free-tier module
  header in `apps/openagents.com/workers/api/src/inference/inference-free-tier-key.ts`).
- **Free tier, self-serve:** `POST /api/keys/free` mints a normal `oa_agent_`
  bearer key; free inference on `openagents/khala` is metered against a per-key
  per-UTC-day quota of **200 requests / 200,000 tokens** (`FREE_TIER_MAX_REQUESTS_PER_DAY`,
  `FREE_TIER_MAX_TOKENS_PER_DAY` in `inference-free-tier-key.ts`). Over-quota or
  premium lanes fall through to the normal balance + `402` gate — free within the
  quota, paid beyond it.
- **A live public "Khala Tokens Served" counter + per-day history:**
  `GET /api/public/khala-tokens-served` and
  `GET /api/public/khala-tokens-served/history` (registered in `index.ts`, backed
  by the token-usage ledger). The recorder increments on every completion;
  verified to the token under 24-wide concurrent stress. Shown on `/stats` and
  `/khala`.

This is the buy-side OpenAgents has historically never closed (the Ep 138 "lots
of sell-side, no paying demand" lesson, restated in
`2026-06-19-inference-gateway-business.md`). The free OpenAI-compatible API plus a
public counter is the first time we have a real, measurable demand surface that an
outsider can point an existing tool at in one config line.

## 1. The thesis

**We are now in the inference business. The one metric that matters is tokens
served per day, and we want it exponential.**

Episode 242 (`../transcripts/242.md`) named the shape: an OpenAI-compatible API
people already know how to consume, one model `openagents/khala`, free research
preview live today, an open marketplace underneath, and any paid value fanned to
contributors. The collective-intelligence essay
(`../collective-intelligence/2026-06-24-collective-intelligence-as-an-economy.md`)
named the mechanism: an **economy** selected by verifiable value, not a lab
artifact graded on its own benchmark. The inference business docs
(`2026-06-19-inference-gateway-business.md`,
`2026-06-19-pricing-model.md`) named the money: credits, multipliers, BTC
discount, three-way split (margin / serving node / referrer).

Tokens served per day is the metric that subsumes all of them:

- It is the **demand** proxy (the buy-side closing).
- It is the **dogfood** proxy (every internal system we route through Khala adds
  tokens).
- It is the **distribution** proxy (every ecosystem tool we land adds tokens).
- It is the **economy** proxy — once the paid loop is collectable, tokens served
  is what the three-way split is computed over.

Everything below is in service of making that one number go up, fast, **honestly**
(real served tokens, not vanity inflation), and **well** (good enough that people
keep their tools pointed at us).

We do not need to invent demand for inference — it is already paid for elsewhere
(OpenRouter, Fireworks, Together, Factory). We need to (a) be trivially adoptable,
(b) be good, and (c) generate our own demand from everything we already run.

## 2. Pillar 1 — The flywheel: plug Khala into EVERYTHING we build/run

Eat our own dog food. Each internal use is **both** dogfood (it hardens Khala and
surfaces real traffic shapes for benchmarking) **and** demand (it adds tokens to
the counter that we control and can grow on day one, before a single external
user arrives). This is the fastest, most honest lever: it is real served tokens
from real work.

Targets, in rough order of how much traffic they can move:

1. **Autonomous QA (`qa-runner` → Khala).** The autonomous QA process (the
   out-ship-Factory QA epic; the 242 draft calls QA "the first real use case")
   does real browser work and leaves green VERIFIED traces. Route its agent
   inference through `openagents/khala`. QA runs continuously, so it is a steady
   token floor and a continuous correctness signal on Khala itself. **Highest-value
   first move:** it is already running, it is internal, and it stress-tests the
   exact code/verification workload Khala is meant to be good at.
2. **OpenAgents agents (Autopilot / Raynor) use Khala.** Autopilot is the gateway's
   designated anchor buyer (`2026-06-19-inference-gateway-business.md` §2): every
   coding session is captive first-party demand. Raynor's forum/progress posting and
   any in-product agent reasoning should default to `openagents/khala` where the
   model fit allows. Coding is the wedge.
3. **All our products route inference through Khala.** Sites generation, forum
   agent flows, onboarding programs, the Artanis loop, Concierge — anywhere a
   product currently calls a model directly, make Khala the default lane (with the
   honest caveat that `claude`/premium lanes still route to the balance+premium
   gate, never the free lane — see the free-tier module's lane policy). One
   internal seam, many internal callers, all counted.
4. **The gym trains Khala AND uses Khala.** The training/gym loop is dual-purpose:
   it improves the model that backs `openagents/khala`, and its own agent/eval
   inference can run *through* Khala. Training that consumes the product it
   improves is the tightest possible flywheel — and it ties directly to the
   "improves, does not depreciate" claim from Ep 242. (Direction: the gym↔Khala
   wiring is not a single shipped seam today; treat as the next dogfood lane to
   build, not a live claim.)
5. **The 3D Verse visualization uses Khala.** `../khala/khala-in-the-world.md`
   renders every request to the endpoint as crackling energy fanned to assigned
   Pylons. Driving Verse NPC / scene / narration inference through Khala makes the
   visualization literally show its own traffic, and adds tokens. (Direction.)

**Why this is the right first pillar:** it is the only lever we fully control. We
can move the counter meaningfully *before* a single external developer adopts us,
and every internal token is a real test that makes the product better for the
external developers we are about to court in Pillar 2. Honesty note: internal
dogfood tokens are real served tokens and may be reported as such, but we should
be able to **distinguish** internal vs external demand in our own analytics so we
never imply external traction we do not have.

## 3. Pillar 2 — The ecosystem-tools playbook

This is exactly how every inference business grew. OpenRouter, Fireworks, and
Together did not win by being a website — they won by being a **drop-in base URL +
key + model id** in the tools developers already use, so adoption costs one config
line and zero rewrites. Our gateway is OpenAI-compatible *specifically* so this
works (`2026-06-19-inference-gateway-business.md` §4: "work by changing only the
base URL + key — zero-rewrite adoption removes friction").

The play: get `openagents/khala` listed/documented as a provider or preset in the
popular coding + agent tools, so anyone can point an existing tool at
`https://openagents.com/api/v1` and run.

### First target: OpenCode (`projects/repos/opencode`)

OpenCode is a provider-agnostic coding agent that reads a JSON config and supports
**custom OpenAI-compatible providers** via the AI SDK. Making it trivial to run
coding agents through OpenCode against our endpoint is the cleanest first landing:
it is a coding tool (our wedge), it is config-driven (no upstream PR needed to
start), and it exercises the same code/tool-calling workload Khala must be good at.

**Exact config** (verified against the OpenCode repo docs/schema —
`packages/web/src/content/docs/providers.mdx`, `config.mdx`,
`packages/core/src/config/provider.ts`). Put in `~/.config/opencode/opencode.json`
(global) or `opencode.json` (project root):

```json
{
  "$schema": "https://opencode.ai/config.json",
  "provider": {
    "openagents": {
      "npm": "@ai-sdk/openai-compatible",
      "name": "OpenAgents",
      "options": {
        "baseURL": "https://openagents.com/api/v1",
        "apiKey": "{env:OPENAGENTS_API_KEY}"
      },
      "models": {
        "openagents/khala": {
          "name": "Khala",
          "limit": { "context": 128000, "output": 65536 }
        }
      }
    }
  }
}
```

- **Base URL:** `https://openagents.com/api/v1`. `@ai-sdk/openai-compatible` POSTs
  to `<baseURL>/chat/completions`, i.e. our live `POST /api/v1/chat/completions`.
- **Key:** mint one free with `POST /api/keys/free`, export it as
  `OPENAGENTS_API_KEY`, or use OpenCode's `/connect → Other → id "openagents"` to
  store it in `~/.local/share/opencode/auth.json`. The provider id typed in
  `/connect` must equal the `provider` key in the config.
- **Model selection:** OpenCode's model reference is `providerId/modelKey`. With
  the model **key** `openagents/khala` (which is what gets sent upstream as the
  model id — correct for our API), the in-TUI selector path reads
  `openagents/openagents/khala`. **TBD to confirm in testing:** whether to keep the
  model key literally `openagents/khala` (sends the right model id; selector has a
  doubled segment) or to add server-side acceptance of a shorter key so the
  selector reads cleanly. Document whichever we choose; do not ship ambiguous
  instructions.

**What to test before we publish the OpenCode recipe:**
- The endpoint serves chat-completions at `/api/v1/chat/completions` (it does) and
  OpenCode's request reaches it with `model: "openagents/khala"`.
- **Tool/function calling** works end to end — OpenCode's edit/run loop depends on
  it. This is the single biggest risk: a model that cannot reliably tool-call is
  unusable as a coding agent regardless of token quality.
- **Streaming (SSE)** works for a good interactive experience.
- The free-tier 200 req / 200k tok daily quota is enough for a real "try it"
  session, and the `402`/quota-exceeded fall-through is a clean, legible error in
  OpenCode (not an opaque crash).
- Token accounting: an OpenCode session's tokens show up on the public
  `khala-tokens-served` counter (proving external traffic is counted).

### Next tools after OpenCode

Prioritized by how directly each is a one-config-line OpenAI-compatible drop-in
and how much coding/agent traffic it represents (reference repos under
`projects/repos/`):

1. **Aider** (`projects/repos/aider`) — CLI coding agent; OpenAI-compatible base
   URL + model env config. Big, well-known coding audience.
2. **Cline / Continue** — VS Code agent extensions with custom OpenAI-compatible
   provider config. Large IDE-native audiences.
3. **Vercel AI SDK** (`projects/repos/ai`) — `@ai-sdk/openai-compatible` provider;
   a documented snippet lets every AI-SDK app point at us. This is the substrate
   under OpenCode and many others, so a clean AI-SDK recipe is high-leverage.
4. **LiteLLM / LangChain provider lists** — getting `openagents/khala` into the
   community provider/config lists (LiteLLM `model_list`, LangChain
   `ChatOpenAI(base_url=...)`) reaches the long tail of agent frameworks.
5. **Codex / Claude Code-compatible clients** — any client that already speaks the
   OpenAI Chat Completions shape and accepts a base URL override (per the gateway
   business doc's named clients).
6. **OpenRouter-style aggregators** — listing Khala as an upstream model in
   aggregators that fan out to many providers (later-stage; needs reliability +
   the paid loop).

For each tool the deliverable is the same: a short, exact, copy-pasteable recipe
(base URL + free key + model id) plus a "what to test" checklist, published where
the tool's users look. Keep the recipes in this repo's docs and on `/khala` once
the copy gate clears.

## 4. Pillar 3 — Benchmarking ("make it good," not just "get there")

Adoption dies if the model is not good. We need a repeatable benchmarking process
that measures Khala's **quality + latency + cost** against other tools/models, so
we can improve it deliberately and so our public comparisons are receipts, not
vibes.

We already have the foundation. The typed, fixture-driven, no-spend benchmark
harness shipped under
`apps/openagents.com/workers/api/src/inference/benchmark/`
(`../khala/2026-06-23-khala-benchmark-harness-book-p1-5.md`): a declarative
matrix (lane × engine × workload × sequence-shape × transport × verification
outcome), a runner with a fixture lane (deterministic, spend-free) and an
**owner-gated** real lane (`makeRealLaneSeam`, default OFF, refuses to spend
unarmed), and a public-safe dereferenceable report (latency percentiles,
**cost-per-accepted-outcome**, verification rate, cache hit rate). The book's
lesson is baked in: "faster" is meaningless until you say faster at *what*, on
*which lane*, under *which traffic*, judged on *which outcome*; read latency in
P50/P90/P99, not the mean; and a benchmark is only decision-grade when an
owner-armed real seam runs over **realistic** traffic.

What the GTM push needs on top of that:

- **An owner-armed real sweep.** Replace synthetic shapes with shapes sourced from
  observed Khala traffic (the dogfood from Pillar 1 *is* that traffic), arm
  `makeRealLaneSeam` with a live executor under a budget cap, and produce the first
  `decisionGrade: true` report comparing Khala vs Fireworks/Vertex on chat /
  khala-code / verifier / long-context. This is the existing Open Question #5
  minimum decision suite, run for real. (Owner-gated; not done in this doc.)
- **External head-to-head comparisons.** Extend beyond our own lanes to compare
  `openagents/khala` against the tools/models developers would otherwise use, on
  the same prompts, reporting tokens, $, wall-clock, **and** our unique axes
  (cost-per-accepted-outcome, verified-rate). This is the buildout roadmap's
  north-star "Fugu-vs-frontier"-style head-to-head
  (`../khala/khala-buildout-roadmap.md`), but generalized into a recurring quality
  bar we can publish.

### Run it through the gym — the benchmark ladder

The **gym** is where this runs: the same gym that trains and uses Khala (Pillar 1)
also **benchmarks** it on a recurring basis, so every training or serving change is
automatically re-scored. The gym drives the harness above over a *ladder of
opponents*, all on identical prompts and our axes (tokens, $, wall-clock,
cost-per-accepted-outcome, verified-rate):

1. **BigPickle vs Khala** — the first named baseline target. **BigPickle is
   OpenCode's default free model** (owner-confirmed, Episode 243), so this rung is
   "Khala vs the free model an OpenCode user gets by default," run on the OpenCode
   coding surface — the cleanest, most directly relevant baseline to beat on
   cost-per-accepted-outcome and verified-rate.
2. **Other (open / free) models vs Khala** — the open models a developer would
   otherwise reach for (the catalog/ecosystem set), so we know where Khala stands
   among the free options people compare us to.
3. **Paid (frontier) models vs Khala** — the paid Claude/GPT/Gemini-class models as
   the upper bar, so we can see the gap to frontier and track it closing over time.

The progression is deliberate: prove Khala against the baseline and the **free**
field first (where we must win on cost-per-accepted-outcome and verified-rate),
then measure the climb toward **paid** frontier. The gym publishes the ladder as a
recurring, dereferenceable leaderboard — and per the honesty bar below, numbers are
only published from the owner-armed real seam over realistic traffic
(`decisionGrade: true`).

Honesty bar for any published benchmark number: it must come from the owner-armed
real seam over realistic traffic (`decisionGrade: true`); fixture/synthetic runs
are explicitly labeled illustrative and never published as measurements.

## 5. Sequence

1. **Internal dogfood demand (Pillar 1), starting now.** Route qa-runner →
   Khala, then Autopilot/Raynor, then the rest of our products. This moves the
   counter immediately and generates the realistic traffic the benchmark needs.
   It is the only step that depends on nobody but us.
2. **OpenCode integration (Pillar 2, first target).** Verify and publish the exact
   one-config recipe + test checklist. First external "point your tool at us" win.
3. **Benchmark harness / gem (Pillar 3).** Confirm the owner's benchmark+competitor
   names; arm a real sweep over the now-real dogfood traffic; produce the first
   decision-grade report and a publishable head-to-head.
4. **Broaden to more tools (Pillar 2 continued).** Aider → Cline/Continue → Vercel
   AI SDK → LiteLLM/LangChain lists → aggregators. One recipe at a time, each with
   a test checklist.
5. **Keep driving internal dogfood demand throughout.** Pillar 1 never stops; new
   internal systems default to Khala as they ship. Constant motion.

These overlap (1 and 2 can run in parallel; 1 feeds 3), but the *gating* order is:
internal demand and OpenCode do not block on the paid loop or the registry, so
they ship first; the paid three-way-split economics are owner-gated and sequence
behind proof.

## 6. Metrics + honesty

**The north-star metric is tokens served per day** — read off the live
`/api/public/khala-tokens-served` counter and its `/history`, the same numbers
shown on `/stats` and `/khala`. We want the per-day history curve to bend upward
and stay up. Supporting metrics:

- **Per-tool adoption** — tokens attributable to OpenCode, Aider, etc. (so we know
  which ecosystem landings actually move traffic).
- **Internal vs external split** — keep them distinguishable in our analytics so
  we never imply external traction we do not have.
- **Quality/latency/cost** — from the benchmark harness (P50/P90/P99 latency,
  cost-per-accepted-outcome, verified-rate), decision-grade only.

**What we may claim publicly today** (per
`docs/promises/2026-06-23-khala-public-copy-promise-gate-review.md` and the
free-tier module): Khala is a **free, live, OpenAI-compatible inference API**
(`openagents/khala`, base `https://openagents.com/api/v1`), with a self-serve free
key and a public tokens-served counter; responses carry OpenAgents receipt
disclosure. **What we may not claim:** that broad **paid** Khala is generally
launched, that any customer can fund inference end-to-end via card/Bitcoin/MPP,
that `khala-code` is "verified" absent an executed acceptance verdict, or that
Pylon contributors are paid from Khala serving without owner-armed settlement.
Everything in Pillars 1–3 that is "direction" (gym↔Khala, Verse↔Khala, the paid
three-way split, decentralized serving) stays labeled as direction until it has a
green promise.

This doc is internal strategy. The product-promise registry governs claims; the
companion promise review
(`../promises/2026-06-25-khala-inference-push-promise-review.md`) maps this push
onto the registry, proposes the one missing promise, and suggests what to
deprioritize.
