# Stripe Directory & Machine Payments (MPP) — list Khala for agent discovery + pay-per-call

*2026-06-22. We set up the Stripe Directory CLI and surveyed it. Goal: make the
**OpenAgents Khala API discoverable and payable per-call via MPP** (Machine
Payments Protocol, `mpp.dev`), so any agent searching for inference finds Khala
and can pay programmatically — and possibly list other Agent Cloud primitives the
same way.*

## What we set up

- Stripe CLI (`brew install stripe/stripe-cli/stripe`) + the `directory` plugin
  (`stripe plugin install directory`, v0.1.5), logged in **live** as
  **OpenAgents, Inc.** (`acct_1Ln7jh…`). The earlier "OA Payments Test" account is
  test-only and Directory **refuses test mode**, so it was removed from the CLI config.
- Search: `stripe directory search "<keywords>" [--mpp-supported] --format json`.
- **Prerequisite gap:** `stripe directory me` → *"Your account does not have a
  Stripe profile."* OpenAgents has **no public Stripe profile yet** — that profile
  (opt-in, plain-language description) is required to be listed/found at all.

## Searches run

**Inference-provider scan** (`--format compact`, no MPP filter):
`llm inference api`, `openai-compatible inference`, `gpu inference serving`,
`ai model hosting`. Surfaced: RunPod (@runpod), CUDO Compute (@cudo),
io.net (@ionet), TokenRouter (@palebluedotai), LLM Gateway (@llmgateway),
Nunchux AI (@nunchuxai) — plus thin/unknown listings. **Every inference provider
is Link-only; none expose MPP or Stripe Projects endpoints.**

**MPP-only enumeration** (`--mpp-supported`, 38 broad category terms, deduped):
`api ai data web search mail browser agent compute tool email scrape database llm
inference image voice payment finance weather maps sms document pdf code crawl
knowledge research news translation analytics storage commerce checkout shopping`.

## The whole MPP set today = 7 providers (and ZERO inference)

| Provider | What | MPP endpoint |
|---|---|---|
| Browserbase (@browserbase) | headless browser infra | `https://mpp.dev/services#browserbase` |
| Parallel (@parallel_web_systems) | web intelligence infra | `https://mpp.dev/services#parallel` |
| PostalForm (@postalform) | send postal mail | `https://mpp.dev/services#postalform` |
| Zinc (@zinc) | e-commerce buying API | `https://mpp.dev/services#zinc` |
| You.com (@youdotcom) | real-time web data for AI | listed (no public mpp.dev url) |
| Drip (@trydrip) | writers paid when AI uses their work | listed |
| Paperplane Labs (@paperplanelabs) | product studio | listed |

Keyword search has no "list all", so 7 is a near-complete lower bound. The
load-bearing fact: **no LLM/inference provider accepts MPP yet.** Inference-on-MPP
is open — Khala can be the first.

## Goal: add MPP support for OpenAgents (start with Khala)

Make the live Khala gateway (`https://openagents.com/v1/chat/completions`, the
`openagents/khala-*` models) a first-class MPP service: an agent searches
`llm inference api` → finds **Khala** → `mppx fetch` pays per call and gets the
completion. Follow the existing pattern (cf. PostalForm / Zinc):

1. **Public Stripe profile** for OpenAgents (the prerequisite) — plain-language
   description using the words customers type ("OpenAI-compatible LLM inference
   API, pay-per-call", not "machine-work economy"). Multiple capabilities → list
   them (inference, fine-tuning, sandboxes, agentic compute).
2. **MPP listing for Khala**: an `mpp.dev/services#khala` endpoint, a
   `llms.txt`, and an `/agents` surface (the PostalForm shape:
   `postalform.com/llms.txt`, `/agents`). `mppx` shows price + confirms before
   paying.
3. **Other Agent Cloud primitives** later: fine-tuning, sandboxes/agentic compute,
   tasks, data — each an MPP endpoint, same one-balance model.

## Strategic note — rails

MPP rides **Stripe** (card / agent-payment rails). OpenAgents is **Bitcoin-first**
(Spark/Lightning, per the credits + revenue-loop work). So MPP is best treated as:
- a **discovery channel** (be found in the directory agents search), and
- an **additional agent-payment front door** for buyers paying via Stripe/MPP,
while **Bitcoin/Spark stays the primary settlement + the contributor-payout rail**.
Decide the mapping: an MPP/Stripe charge → Khala credits (then spend/settle on our
existing metering + Bitcoin payout loop), so MPP buyers and Bitcoin buyers land in
one balance.

## Next steps

- [ ] Enable the OpenAgents public Stripe profile (NEEDS-OWNER; currently none).
- [ ] Stand up the Khala MPP listing (`mpp.dev` service + `llms.txt` + `/agents` +
      per-call pricing) against the live gateway.
- [ ] Wire MPP/Stripe charge → Khala credits → existing metering/settlement.
- [ ] Re-run `stripe directory search "llm inference api" --mpp-supported` to
      confirm Khala appears (and that we're the first inference result).

## Reference
- Stripe Directory docs: <https://docs.stripe.com/directory.md>
- MPP: <https://mpp.dev> · Stripe Projects: <https://projects.dev>
- Live Khala surface: `https://openagents.com/v1/chat/completions` (`openagents/khala-mini`, `openagents/khala-code`)
