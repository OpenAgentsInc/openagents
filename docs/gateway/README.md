# Pro inference door

The Pro inference door is the OpenAI-compatible chat service at
`https://pro.openagents.com`. It admits three GPT-5.6 models and forwards each
completion to Stripe's LLM proxy. Callers see OpenAI catalog ids. The door's
own JSON, headers, and error text do not name Stripe or the upstream host.

This page records the door as it runs on 2026-10-08. The crate is not in this
repository yet. Source is the private repository
[OpenAgentsInc/pro](https://github.com/OpenAgentsInc/pro), commit `ced4d60`
(2026-08-28), checked out at `~/work/pro`. The binary name is `pro`; the
package name is `pro-gateway`.

This door is not the [decision gateway](../decision-models/service/gateway.md)
and not the Vercel AI Gateway. Those admit decisions and other model traffic.
This door admits chat completions for the three models below.

## Where a request goes

```text
caller
  POST /v1/chat/completions          public id, for example gpt-5.6-luna
  or POST /api/inference/proxy
        |
        v
pro-gateway (Cloud Run, pro.openagents.com)
        |
        |  bearer PRO_UPSTREAM_KEY
        |  model rewritten to openai/gpt-5.6-luna
        v
POST https://llm.stripe.com/chat/completions
```

A non-streaming response is parsed and its `model` field is rewritten back to
the public id. The response also carries `x-openagents-model` set to that
public id. A streaming response is copied through as `text/event-stream`. The
door sets the same header, and it does not rewrite model ids inside stream
chunks.

## What the hop is

The hop is Stripe's LLM proxy, from the private-preview Billing for LLM tokens
product. The public client is
[`@stripe/ai-sdk`](https://github.com/stripe/ai/tree/main/llm/ai-sdk/provider).
`createStripe` posts an OpenAI-shaped chat completion to
`https://llm.stripe.com/chat/completions` with a Stripe API key as the bearer.
The model id on that request is `publisher/model`, which is why the door
rewrites `gpt-5.6-luna` to `openai/gpt-5.6-luna`. An optional
`X-Stripe-Customer-ID` attributes the call to a Stripe customer so Stripe can
meter the tokens. The door sends that header only when
`PRO_UPSTREAM_CUSTOMER` is set. Production does not set it, so these calls
are not attached to a customer meter.

`pro-upstream-key` is a Stripe test-mode secret key. Its prefix is `sk_test_`.
That same key answered the model list below and, through this door, the three
completions in [Account catalog](#account-catalog).

Three Stripe documents look related and do not grant model access:

- [Bill for LLM tokens](https://docs.stripe.com/billing/token-billing), read
  on 2026-10-08, tells a new integration to send usage events to Metronome.
  Stripe syncs OpenAI, Anthropic, and Google prices, and Metronome invoices
  through Stripe. That page does not describe this proxy, and this door does
  not send Metronome events.
- [Advanced usage-based billing](https://docs.stripe.com/billing/subscriptions/usage-based/advanced/compare)
  (meters, rate cards, pricing plans, and credit grants) is how Stripe
  invoices usage. Configuring it does not add a model to this key.
- [Tokens and pricing](https://docs.stripe.com/billing/token-billing/ai-prices)
  is the price table Metronome can bill. A row there is not proof this key
  can call the model.

The provider README names example families (GPT-5, GPT-4.1, Gemini 2.5, and
Claude 4). That list is not the account catalog. The same README says the
proxy does not support tool calling, embeddings, or image generation. The
door does not add that limit in its own code. It forwards the body.

## Routes

The process listens on `PRO_BIND`, or on `0.0.0.0:$PORT` when `PORT` is set
and `PRO_BIND` is not. The local default is `127.0.0.1:4100`. Cloud Run sets
`PORT=8080`.

| Method and path | Auth | Response |
| --- | --- | --- |
| `GET /health` | none | `{"ok":true,"service":"pro","upstream":<bool>,"default":"gpt-5.6-sol"}` |
| `GET /api/v1/models` | door bearer, when configured | Coder `CatalogResponse` |
| `GET /v1/models` | door bearer, when configured | OpenAI model list (`owned_by` is `openai`) |
| `POST /v1/chat/completions` | door bearer, when configured | Chat completion, JSON or server-sent events |
| `POST /api/inference/proxy` | door bearer, when configured | Same handler as `/v1/chat/completions` |

`upstream` in `/health` is true when `PRO_UPSTREAM_KEY` is non-empty. It does
not probe Stripe. On 2026-10-08, `GET /health` returned `"upstream": true`.

When `PRO_API_KEY` is set, every route except `/health` requires
`Authorization: Bearer <PRO_API_KEY>`. A missing or wrong bearer is HTTP 401
with `{"code":"unauthorized","message":"bearer token required"}`. The
production service sets this variable. An unset or empty `PRO_API_KEY` leaves
the routes open.

## Models

The door serves these three ids and no others. `gpt-5.6-sol` is the default
when the body omits `model`. An unknown id, including `gpt-4o` and the
`gpt-5.6` alias, is HTTP 422 `model_not_served`. With no upstream key, a known
id is HTTP 422 `model_unavailable` and the catalog marks every model
`unavailable`.

OpenAI describes Sol as the flagship of the GPT-5.6 family, Terra as the
balance of capability and cost, and Luna as the lower-cost model. The door
does not add that ranking. It only marks Sol as the default.

The catalog `provider` is `openai`. The string sent upstream is
`openai/` plus the public id. Context window and max output are the door's
declared catalog values, not a live read of the upstream limit. Prices are the
door's declared integers. In the Coder catalog contract those integers are
micro-USD per million tokens, so `4000000` is $4.00 per million input tokens.
`pricing_basis` is `declared`. The pricing id is `declared.<public-id>.v1`.

| Public id | Upstream model | Default | Context | Max output | Input | Output | Cached input |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `gpt-5.6-sol` | `openai/gpt-5.6-sol` | yes | 400,000 | 128,000 | $4.00 | $20.00 | $0.40 |
| `gpt-5.6-terra` | `openai/gpt-5.6-terra` | no | 400,000 | 128,000 | $2.00 | $12.00 | $0.20 |
| `gpt-5.6-luna` | `openai/gpt-5.6-luna` | no | 400,000 | 128,000 | $0.20 | $1.20 | $0.02 |

Prices are USD per million tokens.

Those three declared prices match the proxy's OpenAI per-token rates, read
on 2026-10-08: Sol `0.000004` / `0.00002` / `0.0000004`, Terra `0.000002` /
`0.000012` / `0.0000002`, and Luna `0.0000002` / `0.0000012` / `0.00000002`
(input, output, and cached input). The proxy also publishes a cache-write
rate the door does not declare: Sol $5.00, Terra $2.50, and Luna $0.25 per
million tokens. Each of the three has an Azure price row with
`stripe_ai_gateway_support` false. The door's upstream id uses the OpenAI
provider, which is the row marked supported.

## Account catalog

`GET /v1/models` on this door is the three-id allowlist above. It does not
ask Stripe which models the key can call.

The account catalog is a GET against the proxy:

```text
GET https://llm.stripe.com/v1/models
Authorization: Bearer <PRO_UPSTREAM_KEY>
```

`GET /models`, without `v1`, is HTTP 404. On 2026-10-08 the `v1` list
returned `{"object":"list"}` with 224 models and no further page. Each object
has `id`, `model`, `author`, `owned_by`, `created`,
`stripe_ai_gateway_support`, and `endpoints`. An endpoint has `provider`,
the same support flag, and per-token USD prices for `input`, `output`,
`cached_input`, and `cached_write`.

`stripe_ai_gateway_support: true` means this key's proxy will route that
model. On that read, 56 models were true and 168 were false. The supported
56 are OpenAI (33), Anthropic (15), and Google (8). Priced models from other
publishers, including xAI, DeepSeek, Meta, and Mistral, were in the list with
the flag false. `openai/gpt-6-sol` and `openai/gpt-6-luna` were in the list
with the flag false. The bare id `openai/gpt-5.6` was absent. The door still
rejects any id outside its three-model allowlist, including ids the proxy
marks supported.

Supported ids on 2026-10-08:

- OpenAI: `openai/gpt-4.1`, `openai/gpt-4.1-mini`, `openai/gpt-4.1-nano`,
  `openai/gpt-4o`, `openai/gpt-4o-2024-05-13`, `openai/gpt-4o-mini`,
  `openai/gpt-5`, `openai/gpt-5-chat-latest`, `openai/gpt-5-mini`,
  `openai/gpt-5-nano`, `openai/gpt-5.1`, `openai/gpt-5.1-chat-latest`,
  `openai/gpt-5.1-codex`, `openai/gpt-5.1-codex-max`,
  `openai/gpt-5.1-codex-mini`, `openai/gpt-5.2`,
  `openai/gpt-5.2-chat-latest`, `openai/gpt-5.2-pro`,
  `openai/gpt-5.3-codex`, `openai/gpt-5.4`, `openai/gpt-5.5`,
  `openai/gpt-5.6-luna`, `openai/gpt-5.6-sol`, `openai/gpt-5.6-terra`,
  `openai/gpt-oss-120b`, `openai/gpt-oss-20b`, `openai/o1`,
  `openai/o1-mini`, `openai/o1-pro`, `openai/o3`, `openai/o3-mini`,
  `openai/o3-pro`, `openai/o4-mini`
- Anthropic: `anthropic/claude-3-haiku`, `anthropic/claude-3.5-haiku`,
  `anthropic/claude-3.7-sonnet`, `anthropic/claude-haiku-4.5`,
  `anthropic/claude-opus-4`, `anthropic/claude-opus-4.1`,
  `anthropic/claude-opus-4.5`, `anthropic/claude-opus-4.6`,
  `anthropic/claude-opus-4.7`, `anthropic/claude-opus-4.8`,
  `anthropic/claude-opus-5`, `anthropic/claude-sonnet-4`,
  `anthropic/claude-sonnet-4.5`, `anthropic/claude-sonnet-4.6`,
  `anthropic/claude-sonnet-5`
- Google: `google/gemini-2.5-flash`, `google/gemini-2.5-flash-image`,
  `google/gemini-2.5-flash-lite`, `google/gemini-2.5-pro`,
  `google/gemini-3-flash`, `google/gemini-3-pro-preview`,
  `google/gemini-3.1-flash-lite-preview`, `google/gemini-3.1-pro-preview`

Repeat the GET to see the current set. A list row is the catalog. A
completion is the proof that one id answers.

On 2026-10-08, `POST /v1/chat/completions` on this door with the production
bearer, `reasoning_effort` `none`, `max_completion_tokens` 16, and the prompt
`Reply with the single word pong.` returned HTTP 200 for `gpt-5.6-luna`,
`gpt-5.6-terra`, and `gpt-5.6-sol`. Each finished with reason `stop`, content
`pong`, 13 prompt tokens, and 4 completion tokens. `x-openagents-model`
matched the requested id.

## What the door changes in the body

Before the upstream POST, the door rewrites the JSON body:

- `model` becomes the upstream id, for example `openai/gpt-5.6-sol`.
- `max_tokens` is moved to `max_completion_tokens` when that field is absent.
- `temperature` is removed when its value is `0`.
- A body with `"stream": true` gains `stream_options.include_usage: true` and
  is sent with `Accept: text/event-stream`.

The upstream request uses `Authorization: Bearer <PRO_UPSTREAM_KEY>`. When
`PRO_UPSTREAM_CUSTOMER` is set, the door also sends `X-Stripe-Customer-ID`.
Production does not set that variable, so the live hop does not send it.

The upstream URL defaults to `https://llm.stripe.com/chat/completions`.
`PRO_UPSTREAM_URL` replaces that default. Production does not set it.

An upstream HTTP error is returned with the upstream status, or 502 when the
status is not a recognized code. The error body uses code `provider_failed`.
The message is the upstream text after replacing `llm.stripe.com`, `Stripe`,
and `stripe` with `upstream`, then truncated to 800 characters. A transport
failure is HTTP 502 with the same scrubbing.

## Configuration

The process reads environment variables. A variable already set in the
process wins over a file. Files, when present, are read in this order:
`$PRO_ENV`, `~/.pro/gateway.env`, then `./.env`.

| Variable | Production | Role |
| --- | --- | --- |
| `PRO_BIND` | unset | Listen address. Default `127.0.0.1:4100`. |
| `PORT` | set by Cloud Run | Used only when `PRO_BIND` is unset. Binds `0.0.0.0:$PORT`. |
| `PRO_ORIGIN` | `https://pro.openagents.com` | Public origin recorded on the process. |
| `PRO_API_KEY` | Secret Manager `pro-api-key` | Door bearer. Required in production. |
| `PRO_UPSTREAM_KEY` | Secret Manager `pro-upstream-key` | Stripe bearer. Required for `upstream: true`. |
| `PRO_UPSTREAM_URL` | unset | Replaces the compiled Stripe URL. |
| `PRO_UPSTREAM_CUSTOMER` | unset | Optional `X-Stripe-Customer-ID` value. |
| `PRO_ENV` | unset | Optional env-file path, read first. |

The local checkout has no `.env`, and `~/.pro/gateway.env` is absent. The
running service is the configured instance.

`--bind` on the `pro` binary overrides `PRO_BIND`. If `PRO_ORIGIN` is unset,
the flag also sets the public origin to `http://<bind>`.

## Deployment

Observed with `gcloud` on 2026-10-08. Project `openagentsgemini`, region
`us-central1`.

| Fact | Value |
| --- | --- |
| Cloud Run service | `pro-gateway` |
| Ready revision | `pro-gateway-00001-nj6`, created 2026-08-28T18:53:30Z |
| Image | `us-central1-docker.pkg.dev/openagentsgemini/openagents/pro-gateway:latest` |
| Public URL | `https://pro.openagents.com` (domain mapping to this service) |
| Run URL | `https://pro-gateway-ezxz4mgdsq-uc.a.run.app` |
| Service account | `oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com` |
| Resources | 1 CPU, 512 MiB, concurrency 80, request timeout 300 seconds |
| Container port | 8080 |
| Secrets | `pro-api-key` and `pro-upstream-key`, each one enabled version, created 2026-08-28T18:49Z |

The private repository's only commit does not contain a Dockerfile or a
deploy script. The image in Artifact Registry is the deployed build. The
crate's `Cargo.toml` depends on `openagents-coder-contract` at
`../openagents/crates/openagents-coder-contract`. This repository removed
that crate in `dabc08102f` (2026-09-18). `cargo check -p pro-gateway` from
`~/work/pro` fails on that missing path.

## What stays off the public door

The private repository's tests refuse a public file, other than the upstream
module and those tests, that contains the word Stripe. `/health`, the model
lists, and error messages follow that rule. This page is the operator record
of the hop. Do not copy the upstream host or the customer header into the
door's JSON, headers, or logs.
