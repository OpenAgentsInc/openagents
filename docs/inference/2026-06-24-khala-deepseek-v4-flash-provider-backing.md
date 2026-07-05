# Khala DeepSeek V4 Flash Provider Backing

Date: 2026-06-24
Issues: OpenAgentsInc/openagents#6198, OpenAgentsInc/openagents#6201,
OpenAgentsInc/openagents#6202, OpenAgentsInc/openagents#6203,
OpenAgentsInc/openagents#6204

## Production Activation

Status: live on `https://openagents.com/v1/chat/completions` as of
2026-06-24.

Production commits:

- `84dbe64c93`: initial Fireworks DeepSeek V4 Flash backing lane for Khala.
- `346c44872065130660955b73b84b3283e0d945f5`: production smoke harness for the
  closed Khala catalog and live backing disclosure.
- `da347df50256027d75237d8153a568dcfa2d9c49`: receipt metadata fix so
  Fireworks-backed Khala reports the concrete supply lane as `fireworks`, not
  the requested-model fallback lane.
- `98c1a6c69223079b1fa45af3b4831293ec303550`: public receipt dereference proof
  for Khala-backed DeepSeek charges.

Worker deploy:

- Cloudflare Worker version:
  `67a6648f-36a2-4824-8486-b274b2f83056`
- Receipt-proof Cloudflare Worker version:
  `8cdf26af-1ce5-4b18-8ceb-79beec429964`
- Deploy command shape:
  `bun run build:web`, then `bunx wrangler deploy --assets ../../apps/web/dist --containers-rollout none`
- Env evidence from deploy output:
  `KHALA_BACKING_MODEL=deepseek-v4-flash`

Production smoke:

- Public site and concrete JS asset returned HTTP 200.
- `node apps/openagents.com/scripts/khala-production-smoke.mjs --readiness-only`
  returned `ok: true`, `status: ready`, `servableModelCount: 1`.
- `/v1/models` listed exactly one public model, `openagents/khala`; forbidden
  raw/split ids were absent.
- Authenticated live spend smoke with `--approve-live-spend` returned
  `ok: true`.
- Non-streaming response:
  - public response model: `openagents/khala`
  - requested model: `openagents/khala`
  - served model: `accounts/fireworks/models/deepseek-v4-flash`
  - supply lane: `fireworks`
  - worker: `fireworks`
  - response id: `chatcmpl_227263e010ae490796a001969643a381`
- Streaming response:
  - stream completed with `[DONE]`
  - frame count: 3
  - requested model: `openagents/khala`
  - served model: `accounts/fireworks/models/deepseek-v4-flash`
  - supply lane: `fireworks`
  - worker: `fireworks`
- Receipt dereference proof after Worker version
  `8cdf26af-1ce5-4b18-8ceb-79beec429964`:
  - non-streaming receipt:
    `https://openagents.com/api/public/inference/receipts/receipt.inference.charge.chatcmpl_b19c2bf5b1f747a48225783976c60ac5`
  - streaming receipt:
    `https://openagents.com/api/public/inference/receipts/receipt.inference.charge.chatcmpl_7ae95e81c354411aa2639b4ee1c55fce`
  - both returned `schemaVersion: openagents.inference.receipt.v1`
  - both returned `ledgerState: paid`
  - both returned public-safe `modelEvidence` with
    `requested_model: openagents/khala`, `served_model: deepseek-v4-flash`,
    `supply_lane: fireworks`, `worker: fireworks`, and a measured total-token
    count
  - both passed the receipt redaction guard: no bearer tokens, OpenAgents agent
    tokens, provider API keys, raw prompts, provider-private payloads, or raw
    token material appeared in the public receipt projection

The result is the intended production shape: external users see only
`openagents/khala`, while internal receipt metadata is precise enough to prove
the hidden DeepSeek V4 Flash backing lane and bill against the Fireworks
supply/cost basis.

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

Quote, catalog, and live metering are aligned. The old standalone MPP/x402
challenge-pricing path was retired in #8387 and is no longer live evidence.

- `/v1/models` still lists only `openagents/khala`, but when the Fireworks
  backing is armed it projects the DeepSeek V4 Flash lane, owner label, cost
  basis, and sell price onto the Khala row.
- `/v1/quote` returns `model: openagents/khala` while pricing against
  `deepseek-v4-flash`.
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

Production validation on 2026-06-24:

- `bun run --cwd apps/openagents.com/workers/api test -- src/inference/chat-completions-routes.test.ts src/inference/model-router.test.ts src/inference/pricing.test.ts`
  passed: 3 files, 153 tests.
- `cd apps/openagents.com && bunx vitest run scripts/khala-production-smoke.test.ts scripts/gpt-oss20b-production-smoke.test.ts`
  passed: 2 files, 7 tests.
- `bun run --cwd apps/openagents.com/workers/api typecheck` passed.
- Full pre-push `check:deploy` passed before pushing
  `da347df50256027d75237d8153a568dcfa2d9c49`.
- Production readiness-only smoke passed.
- Production authenticated non-streaming and streaming smoke passed, with
  `openagents/khala` public model preservation and Fireworks DeepSeek backing
  disclosure.
- Receipt-proof production deploy `8cdf26af-1ce5-4b18-8ceb-79beec429964`
  passed home/asset checks, readiness-only smoke, and authenticated live spend
  smoke with both non-streaming and streaming receipt dereference assertions.
  The updated smoke followed the `openagents` telemetry detail refs, fetched
  the public receipt endpoints, verified `modelEvidence`, usage presence, and
  redaction safety.
- Receipt-proof local verification: Khala/GPT-OSS production-smoke unit tests
  passed (10 tests); public inference receipt and metering route tests passed
  (120 tests); Worker typecheck passed; `git diff --check` passed; full
  pre-push `check:deploy` passed before pushing
  `98c1a6c69223079b1fa45af3b4831293ec303550`.

## Remaining Work

The live serving path and public receipt dereference proof are done. The
remaining production hardening is operational monitoring and self-hosting work.

## No-Spend Production Monitor

Issue #6203 added a no-spend monitor for the exact public Khala surface:

```bash
bun run monitor:khala:production-readiness
```

or directly:

```bash
node apps/openagents.com/scripts/khala-production-readiness-monitor.mjs
```

The monitor performs only public reads:

- `GET /v1/gateway/readiness`
- `GET /v1/models`

It asserts that readiness is `ready`, at least one model is servable, and the
public model catalog is exactly one id: `openagents/khala`. It fails closed if
raw/split/provider ids such as DeepSeek, Fireworks, GPT-OSS, Hydralisk,
`khala-mini`, `khala-pro`, or `khala-code` appear. Its authority block is
explicitly no-spend: no bearer token, no chat completion, no inference spend, no
mutation.

Use this command for frequent owned-infra monitoring. Keep the paid
receipt-dereference proof on the explicit-spend smoke:

```bash
node apps/openagents.com/scripts/khala-production-smoke.mjs --approve-live-spend
```

That paid smoke remains the proof that a real completion bills and dereferences
to public-safe DeepSeek/Fireworks receipt evidence.

## Artanis Scheduled Health Mapping

Issue #6204 maps the no-spend monitor result into the owned Artanis scheduled
health ledger as the `khala_readiness` signal.

The signal is green only when the observation says:

- gateway readiness is `ready`
- at least one model is servable
- the public catalog is exactly `openagents/khala`
- the monitor reports zero raw/split/provider model leaks

The persisted Artanis record is intentionally normalized. If `/v1/models` leaks
`khala-mini`, GPT-OSS, DeepSeek, Fireworks, Hydralisk, or any other extra public
model, the health signal records public-safe blockers such as
`blocker.public.artanis.khala_public_catalog_leak` and
`blocker.public.artanis.khala_public_catalog_not_single_model`; it does not copy
the leaked slug into the public health projection.

Authority is also explicit on the signal: the observation is credentialless,
read-only, no paid call, no chat call, and no mutation. When cron runs without a
fresh monitor observation, Artanis records `khala_readiness` as `unknown` with
`blocker.public.artanis.khala_readiness_not_observed` rather than pretending the
surface is healthy.

The self-hosted Google path remains a separate Hydralisk/Psionic effort. Reserve
or obtain an 8x high-memory H100/H200/B200-class host, validate vLLM 0.20+
DeepGEMM admission, then profile toward the custom expert-prefetch path captured
in the DeepSeek V4 Flash notes.
