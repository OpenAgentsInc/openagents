# Khala DeepSeek V4 Flash Provider Backing

Date: 2026-06-24
Issue: OpenAgentsInc/openagents#6198

## Decision

Use Fireworks DeepSeek V4 Flash as an operator-selected backing lane for the
single public Khala model:

- internal OpenAgents ecosystem slug: `khala`
- external OpenAI-compatible model id: `openagents/khala`
- hidden provider model: `accounts/fireworks/models/deepseek-v4-flash`
- non-secret Worker routing knob: `KHALA_BACKING_MODEL=deepseek-v4-flash`
- required Worker secret: `FIREWORKS_API_KEY`

This does not reopen public model selection. Customers and MPP sellers still see
and request one thing: `openagents/khala`. The raw DeepSeek model id is an
operator/backing concern only.

## Why This Path

The provider inventory attachment for `deepseek-ai/DeepSeek-V4-Flash` is useful
because it confirms the stock self-host path is still a high-memory multi-GPU
path, not a Google G4/L4 quick win:

- model shape: 284B total parameters, about 13B active
- context: 1,048,576 tokens
- runtime: vLLM 0.20+ with DeepGEMM
- verified target: NVIDIA H200
- listed hardware class: 8x H100 80G, 8x H200 141G, 8x B200 180G, and adjacent
  high-memory 8-GPU systems
- checkpoint storage note: FP8/NVFP4 around 170 GB
- topology note: single-node TEP and TP equal to GPU count to avoid dense
  replication OOM

That keeps the owned-Google path honest: our L4/G4 lanes are not the immediate
stock-vLLM DeepSeek V4 Flash path. The immediate MVP is to route Khala through a
provider lane that already serves the model, while preserving receipt-first
metering and the closed public model surface.

## Implemented Shape

The Worker now resolves lane arming from the live env and binds dispatch through
`makeKhalaBackedAdapterPlan(arming.khalaBacking)`.

When `KHALA_BACKING_MODEL` is absent or unrecognized, Khala keeps the prior
Hydralisk GPT-OSS plan:

1. Hydralisk GPT-OSS 120B
2. Hydralisk GPT-OSS 20B
3. Vertex Gemini graceful degradation

When `KHALA_BACKING_MODEL=deepseek-v4-flash`, Khala routes:

1. Fireworks
2. Hydralisk GPT-OSS 120B
3. Hydralisk GPT-OSS 20B
4. Vertex Gemini graceful degradation

At the Fireworks adapter boundary, `openagents/khala` maps to
`accounts/fireworks/models/deepseek-v4-flash`. The public request and response
model remains `openagents/khala`.

## Pricing And Receipts

Quote, catalog, MPP challenge pricing, and live metering are aligned:

- `/v1/models` still lists only `openagents/khala`, but when the Fireworks
  backing is armed it projects the DeepSeek V4 Flash lane, owner label, cost
  basis, and sell price onto the Khala row.
- `/v1/quote` returns `model: openagents/khala` while pricing against
  `deepseek-v4-flash`.
- `/mpp/v1/chat/completions` derives crypto/card/Lightning challenge amounts
  against the same hidden backing price model.
- `priceRequest` normalizes Fireworks provider-native receipt ids such as
  `accounts/fireworks/models/deepseek-v4-flash` back to the canonical
  `deepseek-v4-flash` pricing row, so receipt-first metering does not fall into
  unknown-model pricing.

## Validation Targets

The focused validation set is:

- policy projection: only Khala public, Fireworks lane when explicitly backed
- router: operator-selected Khala plan starts with Fireworks
- Fireworks adapter: Khala maps to DeepSeek V4 Flash provider id
- pricing: Fireworks provider-native receipt ids normalize to canonical rows
- quote and MPP pricing: response product is Khala, cost basis is backing model
- real smoke: `scripts/fireworks-smoke.ts` using local `FIREWORKS_API_KEY`

## Validation Results

Local validation on 2026-06-24:

- Combined focused inference/payment route suite: 15 files, 376 tests passed.
- `bun run --cwd apps/openagents.com/workers/api typecheck` — passed.
- `git diff --check` — passed.
- `bun run apps/openagents.com/workers/api/scripts/fireworks-smoke.ts` — passed
  against the real Fireworks API. The request model was `openagents/khala`; the
  provider served model was `accounts/fireworks/models/deepseek-v4-flash`; the
  response content matched the smoke sentinel and the response carried
  provider usage metadata for receipt metering.

## Remaining Work

To make this live in production after merge:

1. Ensure the Cloudflare Worker has the `FIREWORKS_API_KEY` secret set.
2. Deploy `apps/openagents.com/workers/api` with
   `KHALA_BACKING_MODEL=deepseek-v4-flash`.
3. Run an authenticated `POST https://openagents.com/v1/chat/completions` with
   `model: "openagents/khala"`.
4. Verify the response model remains `openagents/khala`, the served receipt
   points at the Fireworks DeepSeek backing, and a real ledger/MPP payment
   receipt resolves.

The self-hosted Google path remains a separate Hydralisk/Psionic effort:
reserve or obtain an 8x high-memory H100/H200/B200-class host, validate vLLM
0.20+ DeepGEMM admission, then profile toward the custom expert-prefetch path
captured in the DeepSeek V4 Flash notes.
