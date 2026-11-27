# 68. One gateway

> **Status: partly implemented.** The multi-provider gateway replay, rates card,
> and failover planner ([`coax/lib/gateway/plan.coax`](../../coax/lib/gateway/plan.coax),
> [`rates.coax`](../../coax/lib/gateway/rates.coax),
> [`card.coax`](../../coax/lib/gateway/card.coax),
> [`gateway.coax`](../../coax/lib/gateway/gateway.coax)) run on `cpu`, grounded by
> [`openagents/examples/gateway-failover-rates.coax`](../examples/gateway-failover-rates.coax)
> and [`openagents/examples/gateway-failover-rates.out`](../examples/gateway-failover-rates.out),
> and are consumed by [`coax/lib/microcoder/run.coax`](../../coax/lib/microcoder/run.coax),
> [`coax/lib/microcoder/loop.coax`](../../coax/lib/microcoder/loop.coax),
> [`coax/lib/rlm/model.coax`](../../coax/lib/rlm/model.coax),
> and [`coax/web/agent/chat-micro.coax`](../../coax/web/agent/chat-micro.coax).
> Remaining: routing every member through one unified job-order policy table and
> checking live provider-reported cost reconciliation against the card end-to-end.

## Premise

Every chapter has called "a model" without saying whose. This chapter builds
the answer: one gateway in front of every provider, so that members name what
they need and policy picks who serves it.

## Draft

The agent uses models for three jobs: decisions, conversation and long work.
No single provider is best at all three, and none is certain to be there next
year under the same terms.

The gateway is one interface over many providers. A member asks for a kind of
model and the gateway picks a route. The Coax tree already has routes to the
large hosted providers, to model routers, to Psionic and to Pylons. Each call
records which route answered, how long it took and what it cost.

The archive argues for this from experience. [Episode
67](../../docs/transcripts/067.md) replaces one provider's chat product in
daily use, and [Episode 150](../../docs/transcripts/150.md) argues that
neutral protocols and a choice of model can support an open market. [Episode
204](../../docs/transcripts/204.md) is the cost of the alternative: a provider
restricts access, and working setups break.

Being neutral does not make providers the same. They differ in tools, price,
limits and how they fail. So the choice is a written policy: which provider is
tried first for which job, and what happens when it is down or over its limit.
On desktop and CLI (`#14069`–`#14072`, `#14087`), **OpenAgents Cloud** is
wired as the primary cloud gateway backend (with Google Vertex / Gemini 4 Neon
first behind the cloud tier, OpenRouter/Anthropic/OpenAI as fallbacks), while
local `psionic-inkling-serve` (`Inkling-2-Mini`) runs at a hard-enforced
`$0.00` local compute rate. Decisions go through the Decisions API
(`coax/lib/agent/router/`, chapter 15).

Chapter 16's lesson applies to every model call. One account running out of
credits should never stop the agent.

The gateway is also where cost becomes visible. Each step in a record carries
what it cost, and the Desktop **Always-On Usage HUD** (`#14064`, `#14072`,
[`apps/openagents-desktop/page/usage.coax`](../../apps/openagents-desktop/page/usage.coax)
and [`apps/openagents-desktop/Window+Usage.swift`](../../apps/openagents-desktop/Window+Usage.swift))
syncs OpenAgents Cloud usage live, enforces `$0.00` on local compute, and
gates external harness quotas transparently. Rates and usage are checked
against what providers and `coax/lib/gateway/card.coax` report, not assumed.

## What we build

A plan for what this chapter ships once the draft is approved:

- The gateway as the only path to any model, for every member.
- A routing policy per job: order of providers and failover, with the route
  recorded on each call.
- Cost per step, checked against what the provider reports.

It builds on what exists today:

- [`coax/lib/gateway/gateway.coax`](../../coax/lib/gateway/gateway.coax)
- [`coax/lib/gateway/card.coax`](../../coax/lib/gateway/card.coax) (OpenAgents Cloud model catalog & retail pricing cards, `#14069`)
- [`coax/lib/gateway/plan.coax`](../../coax/lib/gateway/plan.coax)
- [`coax/lib/gateway/rates.coax`](../../coax/lib/gateway/rates.coax)
- [`openagents/core/wire.coax`](../../openagents/core/wire.coax) and [`coder/tui/oa-worker.coax`](../../openagents/runtime/oa-worker.coax) (OpenAgents Cloud gateway wiring, `#14070`)
- [`apps/openagents-desktop/page/usage.coax`](../../apps/openagents-desktop/page/usage.coax) and [`apps/openagents-desktop/Window+Usage.swift`](../../apps/openagents-desktop/Window+Usage.swift) (Always-On Usage HUD & `$0.00` local compute enforcement, `#14064`, `#14072`)
- [`coax/lib/gateway/anthropic.coax`](../../coax/lib/gateway/anthropic.coax)
- [`coax/lib/gateway/openai.coax`](../../coax/lib/gateway/openai.coax)
- [`coax/lib/gateway/vertex.coax`](../../coax/lib/gateway/vertex.coax)
- [`coax/lib/gateway/openrouter.coax`](../../coax/lib/gateway/openrouter.coax)
- [`coax/lib/gateway/psionic.coax`](../../coax/lib/gateway/psionic.coax)
- [`coax/lib/gateway/pylon.coax`](../../coax/lib/gateway/pylon.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A model request and its stream | `openagents.psionic.responses.v1alpha1` `CreateRequest`, `Response`, `StreamEvent` ([`responses.proto`](../../proto/openagents/psionic/responses/v1alpha1/responses.proto)); the wider package is `openagents.inference.v1` ([spec 16.2](../../docs/protocol/openagents-protocol-spec.md#162-openagentsinferencev1)) | HTTP, with a server-sent event stream | Schema file for the Psionic profile; the rest is a spec sketch. Each provider's request body is an external format |
| Which route answered and what it cost | `openagents.decision.v1` `DecisionCall` ([spec 16.1](../../docs/protocol/openagents-protocol-spec.md#161-openagentsdecisionv1)); usage is in `openagents.common.v1` ([spec 9.9](../../docs/protocol/openagents-protocol-spec.md#99-usage)) | In the run journal (chapter 18) | Spec sketch; no `.proto` file |

### The chapter's check

- **Source:** [`coax/lib/gateway/plan.coax`](../../coax/lib/gateway/plan.coax),
  [`coax/lib/gateway/rates.coax`](../../coax/lib/gateway/rates.coax),
  [`coax/lib/gateway/card.coax`](../../coax/lib/gateway/card.coax),
  [`coax/lib/gateway/gateway.coax`](../../coax/lib/gateway/gateway.coax),
  [`openagents/examples/gateway-failover-rates.coax`](../examples/gateway-failover-rates.coax),
  and [`openagents/examples/gateway-failover-rates.out`](../examples/gateway-failover-rates.out).
- **Trace:** replays the recorded `GW_FX_VERTEX_GEMINI_4_CARBON` stream
  (`in=61`, `out=13`), prices it on the rates card (`micros=504`, `msats=504` at
  \$100k/BTC; cached-input pricing `3720000` micro-USD), formats a secret-free
  attempt log line, benches `vertex` on HTTP `402` to fail over (`GW_NEXT`) to
  `openrouter`, checks 4× backoff (`2000,8000,30000`), and detects a first-token
  stall (`stall=1`).
- **Refusal:** a failure after the first token (`first=true`) never retries
  (`GW_END_PASS`); zero attempts remaining returns `GW_END_PASS` rather than an
  empty answer; an unpriced model off the card returns `card_usd_micros=-1`;
  HTTP `402` is never retried on the same gateway (`retryable_402=false`).
- **Independent check:** `coax/bin/coax run openagents/examples/gateway-failover-rates.coax`
  and `--release` match [`openagents/examples/gateway-failover-rates.out`](../examples/gateway-failover-rates.out),
  and each cost matches hand-derived arithmetic from token counts (`61 * 4 + 13 * 20 = 504`
  micro-USD) and the `#test` checks in [`rates.coax`](../../coax/lib/gateway/rates.coax).

Planned paths: none. The gateway is [`coax/lib/gateway`](../../coax/lib/gateway).

## Sources

- **Episodes:**
  - [Episode 67, *Replacing ChatGPT*](../../docs/transcripts/067.md): replacing one provider's product
  - [Episode 89, *Goodbye ChatGPT*](../../docs/transcripts/089.md): leaving it for good
  - [Episode 150, *Neutrality Wins*](../../docs/transcripts/150.md)
  - [Episode 204, *DO NOT BREAK USERSPACE*](../../docs/transcripts/204.md)
  - [Episode 278, *Coder Commands Codex & Claude*](../../docs/transcripts/278.md): a provider-neutral agent
  - [Episode 181, *American DeepSeek?*](../../docs/transcripts/181.md): studying an open model
- **Old code at `coder-v1.0.0`, crates:** `model-access`, `openrouter`, `inference`, `gateway`
- **NIPs:** [NIP-POL](../../nostr/nips/openagents/NIP-POL.md), [NIP-CAP](../../nostr/nips/openagents/NIP-CAP.md)
- **Also:**
  - [Coder book, chapter 13: Choose an engine](../../coder/book/ch013-choose-an-engine.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P7, P12

## Open questions for the owner

1. What is the default order of providers for each job: decisions,
   conversation and long work?

## Source listing

Run the Chapter 68 example in [`openagents/examples/gateway-failover-rates.coax`](../examples/gateway-failover-rates.coax) (committed transcript [`openagents/examples/gateway-failover-rates.out`](../examples/gateway-failover-rates.out)):

<!-- source: openagents/examples/gateway-failover-rates.coax gateway-failover-rates -->
```jai
gfr_run :: () -> bool {
  // 1. Replay recorded Vertex Gemini 4 Carbon stream and price usage on the rates card.
  r: [1] GwReplay;
  fx := GW_FX_VERTEX_GEMINI_4_CARBON;
  gw_replay(GW_WIRE_GEMINI, fx[..], r[..]);
  priced := gw_price("vertex", "gemini-4-carbon", r[0].usage, 100000);

  // Cached-input discount: 1M input (600k cached), 100k output on Gemini 4 Carbon.
  cached_u := GwUsage.{ok = true, input_tokens = 1000000, cached_tokens = 600000, output_tokens = 100000};
  cached_p := gw_price("vertex", "gemini-4-carbon", cached_u, 100000);

  log_entry := GwAttemptLog.{
    gateway = "vertex", model = "gemini-4-carbon", outcome = GW_OUT_DONE, status = 200,
    first_ms = 120, latency_ms = 410,
    input_tokens = priced.input_tokens, output_tokens = priced.output_tokens,
    card_usd_micros = priced.card_usd_micros, cost_msats = priced.cost_msats, cost_usd_micros = -1
  };
  lbuf: [256] u8;
  line := gw_attempt_line(log_entry, lbuf[..]);
  print("replay done=% in=% out=% micros=% msats=% cached_micros=%\n",
    r[0].done, priced.input_tokens, priced.output_tokens,
    priced.card_usd_micros, priced.cost_msats, cached_p.card_usd_micros);
  print("log=%\n", line);

  // 2. Failover & session benching: 402 out-of-credits benches Vertex and falls through to OpenRouter;
  // exponential backoff grows 4x capped at 30000 ms; stall detector flags silent calls.
  bench: [1] GwBench;
  ref_402 := GwEvent.{kind = GW_EV_REFUSED, status = 402};
  dec_failover := gw_plan_decide(ref_402, false, 1);
  benched_402 := gw_bench_status(402);
  if benched_402 { gw_bench(bench[..], "vertex"); }
  vertex_benched := gw_benched(bench[..], "vertex");
  openrouter_ready := !gw_benched(bench[..], "openrouter");
  bo0 := gw_backoff(2000, 0);
  bo1 := gw_backoff(2000, 1);
  bo2 := gw_backoff(2000, 2);
  stall_first := gw_stall(5000, 10000, 1000, -1, 6000);
  print("failover next=% benched=(vertex=%,openrouter=%) backoff=%,%,% stall=%\n",
    dec_failover == GW_NEXT, vertex_benched, !openrouter_ready, bo0, bo1, bo2, stall_first);

  // 3. Fail-closed refusals: mid-stream cut after first token never retries;
  // zero attempts left returns GW_END_PASS; unknown model returns -1 micro-USD.
  dec_after_tok := gw_plan_decide(GwEvent.{kind = GW_EV_REFUSED, status = 500}, true, 2);
  dec_exhausted := gw_plan_decide(GwEvent.{kind = GW_EV_REFUSED, status = 503}, false, 0);
  off_card := gw_price("openai", "gpt-5.4-mini", r[0].usage, 100000);
  print("refuse cut=% exhausted=% off_card=% retryable_402=%\n",
    dec_after_tok == GW_END_PASS, dec_exhausted == GW_END_PASS,
    off_card.card_usd_micros, gw_retryable(402));

  return r[0].done == 1 && priced.input_tokens == 61 && priced.output_tokens == 13 &&
         priced.card_usd_micros == 504 && priced.cost_msats == 504 &&
         dec_failover == GW_NEXT && benched_402 && vertex_benched && openrouter_ready &&
         bo0 == 2000 && bo1 == 8000 && bo2 == 30000 && stall_first == GW_STALL_FIRST &&
         dec_after_tok == GW_END_PASS && dec_exhausted == GW_END_PASS &&
         off_card.card_usd_micros == -1 && !gw_retryable(402);
}
```

Output (`coax/bin/coax run openagents/examples/gateway-failover-rates.coax`, identical in `--release`):

```text
replay done=1 in=61 out=13 micros=504 msats=504 cached_micros=3720000
log=gateway=vertex model=gemini-4-carbon outcome=done status=200 first_ms=120 ms=410 in=61 out=13 card_usd_micros=504 msats=504 cost_usd_micros=-1
failover next=true benched=(vertex=true,openrouter=false) backoff=2000,8000,30000 stall=1
refuse cut=true exhausted=true off_card=-1 retryable_402=false
ok=true
```
