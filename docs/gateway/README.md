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

On 2026-10-08 a completion through `POST /v1/chat/completions` with the
production bearer, model `gpt-5.6-luna`, `max_completion_tokens` 32, and
`reasoning_effort` `none` returned HTTP 200, finish reason `stop`, and the
content `pong`. Usage was 13 prompt tokens and 4 completion tokens. The
response header was `x-openagents-model: gpt-5.6-luna`.

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
