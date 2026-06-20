# Fireworks AI — inference provider integration

Date: 2026-06-19. Status: **provider verified live; integration notes.** Fireworks is our
passthrough supply lane for open-weight models Vertex doesn't carry (DeepSeek, Kimi, GLM,
Qwen, MiniMax, gpt-oss, Nemotron, embeddings, vision, image). Companion to the gateway +
pricing docs in this folder.

## Verified (2026-06-19)
- Key stored in `~/work/.secrets/fireworks.env` as `FIREWORKS_API_KEY` (gitignored, chmod 600,
  never printed/committed).
- OpenAI-compatible: `POST https://api.fireworks.ai/inference/v1/chat/completions`,
  `Authorization: Bearer $FIREWORKS_API_KEY`. Models list: `GET .../inference/v1/models`.
- Real calls succeeded on `accounts/fireworks/models/deepseek-v4-pro` and `glm-5p2` with a
  proper `usage` object returned. Auth + inference confirmed.

## Live on our account now (7)
`glm-5p2`, `glm-5p1`, `kimi-k2p6`, `kimi-k2p5`, `deepseek-v4-pro`, `gpt-oss-120b`,
`flux-1-schnell-fp8` (image). The broader serverless catalog (selectable per model id) also
lists Kimi K2.7 Code, DeepSeek V4 Flash, Qwen 3.7/3.6 Plus, MiniMax M3/2.7/2.5, gpt-oss-20b,
Nemotron 3 Ultra. Model ids use `accounts/fireworks/models/<id>`.

## What it is
Multi-tenant **serverless** inference on Fireworks-managed infra — pay per token, no GPUs to
size, no cold starts. Eligible models carry the **Serverless** tag. (On-demand dedicated GPUs
exist for custom/LoRA models + version stability; not needed for our passthrough lane.)

## Serverless pricing (per 1M tokens, USD — Standard: input / cached-input / output)
Real cost basis for our open/cheap tier:

| Model | input / cached / output |
| --- | --- |
| gpt-oss-20b | 0.07 / 0.035 / 0.30 |
| OpenAI gpt-oss-120b | **0.15 / 0.015 / 0.60** |
| DeepSeek V4 Flash | 0.14 / 0.028 / 0.28 |
| MiniMax M3 / 2.7 / 2.5 | 0.30 / 0.06 / 1.20 |
| Qwen 3.7 Plus | 0.40 / 0.08 / 1.60 |
| Nemotron 3 Ultra | 0.60 / 0.12 / 2.40 |
| Kimi K2.5 | 0.60 / 0.10 / 3.00 |
| Kimi K2.6 | 0.95 / 0.16 / 4.00 |
| Kimi K2.7 Code | 0.95 / 0.19 / 4.00 |
| GLM 5.2 / 5.1 | 1.40 / 0.26 / 4.40 |
| DeepSeek V4 Pro | 1.74 / 0.145 / 3.48 |

Size-based fallback for any other model (uniform in/out, no cached rate): <4B $0.10, 4–16B
$0.20, >16B $0.90, MoE ≤56B $0.50, MoE 56–176B $1.20. Embeddings: $0.008–$0.10 / 1M input.
**Batch inference = 50%** of standard (both directions). **Fast** variants and **Priority**
(`service_tier:"priority"`) cost a premium.

> These are our **marginal cost** for the cheap/open tier — drop them straight into the
> pricing-model multiplier/margin math (`2026-06-19-pricing-model.md`). e.g. gpt-oss-120b at
> $0.15/$0.60 in/out is dramatically cheaper than frontier Claude, so the open tier carries
> fat margin even at a low sell multiplier — and that margin is what fans to serving nodes +
> referrers.

## Billing dimensions (meter against these)
Three billed dimensions: **input**, **cached input** (prompt-cache hits, default ~50% of
input), **output**. The response **`usage` object is the source of truth** (`prompt_tokens`,
`completion_tokens`, `total_tokens`) — our metering decrements credits from `usage`, never
an estimate (receipt-first). Response headers also carry `fireworks-prompt-tokens`,
`fireworks-cached-prompt-tokens`, and the live rate-limit ceilings (`X-Ratelimit-Limit-Tokens-*`).

## Prompt caching (free margin)
On by default for every serverless model; cached input billed ~50%. Caching is
**replica-local**, so to maximize hit rate pin repeated/shared-prefix prompts to one replica
via the **`x-session-affinity`** header (or the OpenAI `user` field). For our gateway: pass a
stable per-customer/per-session affinity key so context-heavy coding sessions hit cache —
directly lowering our cost and widening margin.

## Rate limits (must handle in the adapter)
Three adaptive metrics, **per-account + per-model**, that grow/shrink with usage:
**Total-Prompt TPM**, **Uncached-Prompt TPM**, **Generated TPM**. Starting limits:
**3.6M / 900k / 36k TPM** (~60k / 15k / 600 TPS). Fast vs regular variants have **separate**
limits; Priority shares the regular limit. Higher account **spend tier** → higher upper
bounds. Current effective limits are in the `X-Ratelimit-Limit-Tokens-*` response headers.
- `429 Too Many Requests` → **exponential backoff**, and in our gateway **overflow to another
  supply lane** (Vertex / other passthrough / our network) rather than failing the request.
- `503 Service Overloaded` (load-shed even within limits) → retry / use **Priority tier** for
  latency-sensitive traffic.
- **Don't ramp cold** — the adaptive limit punishes sudden spikes; warm up gradually or
  pre-arrange higher limits (inquiries@fireworks.ai) before a launch.

## Serving paths
- **Standard** — default (no param).
- **Priority** — `service_tier:"priority"`, premium price, higher reliability under load.
- **Fast** — switch the model id to the Fast variant (e.g. `…/kimi-k2p6-fast` / a fast router);
  separate rate-limit pool.

## Model lifecycle
Serverless models are Fireworks-managed; **≥2 weeks notice** before removal (longer for
popular ones). For version-pinned production stability, on-demand deployments give full
control. Our gateway should treat serverless model ids as a managed set and degrade
gracefully if one is retired.

## How it slots into the OpenAgents gateway
- **Provider adapter:** OpenAI-compatible, base `https://api.fireworks.ai/inference/v1`, key
  from `.secrets/fireworks.env`. Near drop-in.
- **Routing:** the **open-model / cheap tier** of the gateway — for models Vertex lacks
  (DeepSeek/Kimi/GLM/Qwen/MiniMax/gpt-oss/Nemotron) + embeddings/vision/image. Vertex stays
  the Claude lane; our Pylon fabric is the contributor-served lane; Fireworks is the
  managed-open-model lane.
- **Pricing:** sell = Fireworks cost × multiplier/margin; cache-affinity + batch (50%) cut our
  cost further; 429/503 → backoff + overflow.
- **Meter from `usage`**, pin `x-session-affinity`, handle the adaptive rate-limit ramp.

## Re-verification (2026-06-19, weekend assault DE-2)

Independently re-confirmed the live provider connection against the OpenAgents-held key
(`~/work/.secrets/fireworks.env`, never printed/committed):

- `POST https://api.fireworks.ai/inference/v1/chat/completions` on
  `accounts/fireworks/models/glm-5p2` → **HTTP 200**, real usage object
  `{prompt_tokens: 19, completion_tokens: 12, total_tokens: 31}`, `finish_reason: length`.

This keeps `inference.fireworks_open_model_provider.v1` at **yellow**: the provider connection
is a verified, reproducible, registered live supply lane in the deployed gateway. It does NOT
green — green needs the PAID open-model product (card/Bitcoin → metered open-model spend →
dereferenceable paid receipt), which is the owner-gated paid-credits path shared with
`inference.gateway_credits_business.v1`. No state flip.
