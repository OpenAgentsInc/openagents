# How does Khala decide which models to use?

*Date: 2026-06-27. Answers a question we got on Twitter/X about how the Khala
inference API picks a model. Honest, receipt-first: built-today vs being-proven vs
roadmap.*

## The short answer

You call **one** model — `openagents/khala` — and a **router** underneath picks the
backend per request. It decides on three things, in order:

1. **The shape of the work.** Plain chat, a tool-calling/agentic request, and a
   "do this coding task" request are routed to different kinds of backends, because
   they need different things.
2. **Our own capacity first.** Khala prefers lanes we run ourselves — contributors'
   Codex/Claude coding agents (via Pylon), and our self-hosted open models — over
   third-party APIs. Today the served-token mix is dominated by **own-capacity
   Pylon-Codex**, not a paid lab model. You can watch the live split on
   [openagents.com/stats](https://openagents.com/stats).
3. **Verified value (the direction we're heading).** The long-term selector is not a
   hand-tuned table — it's *which lane actually produced a good, verified outcome for
   the lowest cost*. That part is being built; see "What's roadmap" below.

Routing is **semantic / structured, never ad-hoc keyword matching** — that's an
architectural rule in the repo, not a nicety.

## Built and working today

- **One OpenAI-compatible endpoint, one model.** Point any tool at
  `https://openagents.com/api/v1` with model `openagents/khala`. The router does the
  rest; you never pick a sub-model.
- **Route-by-shape across a real mix of lanes.** Depending on the request, Khala
  routes across: contributors' **Codex/Claude** coding agents running locally via
  **Pylon** (own-capacity, for `codex_agent_task`/`claude_agent_task` coding work),
  our self-hosted **GLM-5.2-REAP** and **GPT-OSS** lanes, and external fallbacks
  (**Gemini Flash**, **Fireworks DeepSeek**, **OpenRouter**). The lanes feeding Khala
  are named in the inference docs and the Episode 242–244 build logs.
- **Own-capacity bias is visible.** The `/stats` "Model Family Mix" panel shows the
  live per-family breakdown, currently led by **Pylon-Codex**. We can show this
  because every served completion is metered into `token_usage_events`, and the
  public counter is a projection of those exact rows.
- **A receipt on every response.** Responses carry an OpenAgents disclosure block so
  you can see how a request was served — we'd rather show the routing than hide it.
- **Honest internal-vs-external accounting.** A lot of today's volume is our own
  dogfooding; we keep internal demand distinguishable from external demand so we
  never imply traction we don't have.

## Being proven live

- **Caller-owned coding delegation.** A Khala request like "implement this PR" is
  delegated to **your own** linked Codex/Claude capacity through a Pylon bound to your
  account — own-capacity only (your subscription doing your work). The end-to-end path
  is proven (see the Khala → Pylon → Codex runbook in `AGENTS.md`); broad automatic
  steering from any request to any linked capacity is still being hardened.
- **The self-hosted GLM-5.2-REAP fleet.** Live but still being made durable and fast
  enough to carry production traffic, so it's used selectively rather than as a
  default.

## What's roadmap (not a claim yet)

- **Learned routing from verified outcomes.** The intended selector ranks lanes by
  **cost-per-accepted-outcome** and verified-trace quality, so Khala gets better as it
  serves more work — "improves, does not depreciate" (Episode 242). Today's routing is
  shape- and capacity-based; the *learned* policy is direction.
- **The open marketplace + contributor pay.** The end state is an open pool where
  other people's agents and models can be selected and **paid** (Bitcoin) proportional
  to the verified value they contribute. That economy is being built, not shipped.

## Why it's built this way

The thesis (Episode 242, "Khala: Collective Intelligence"): instead of one giant
pre-trained model graded on its own benchmark, Khala is a **collective** — many small
composable programs and many capacity lanes — **selected by verifiable, Bitcoin-paid
value, bottom-up**. A request can be answered by a fast hosted model, your own coding
agent, a self-hosted open model, or (eventually) someone else's agent — whichever
actually delivers. Each program is an independently optimizable
**Blueprint/DSPy-style signature** (we run GEPA-style prompt optimization on them), so
the routing and the components both improve over time rather than depreciating.

## Sources (check the work)

- `docs/inference/` — the Khala inference GTM strategy and lane list.
- `AGENTS.md` → "Khala → Pylon → Codex Coding Delegation Runbook" — the caller-owned
  coding-delegation path and its invariants (own-capacity-only, no-resale,
  semantic-not-keyword routing).
- [openagents.com/stats](https://openagents.com/stats) — the live Model Family Mix and
  tokens-served counter.
- `docs/transcripts/242.md`, `243.md`, `244.md` — the thesis and the live build of the
  routing across OpenCode and Codex.
