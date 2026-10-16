# Calling the API

2026-10-12. Authentication, the workspace header, request ids, list cursors, and the 1 MiB body ceiling are live. Counts, batches, files, and managed agents are proposed on their own pages.

The HTTP API is `https://api.openagents.com/v1`. The older `https://openagents.com/v1` and `https://openagents.com/api/v1` still answer the same routes but are deprecated ([API address](README.md#api-address)). You send UTF-8 JSON and read JSON back. This page is the contract those calls share: which header authenticates you, which workspace the call runs in, how a list pages, how large a body may be, and which path does which job.

It follows [the API design](design.md): `oak_` keys, cursor lists, one error object, and caps you set yourself. Words for the objects are in the [glossary](../glossary.md).

## Which path to call

| You want | Path | Where it is specified |
| --- | --- | --- |
| One model answer | `POST /v1/responses` | [Inference gateway](../inference/gateway.md). Extra fields are in [Model calls](model-calls.md). |
| The same answer in Chat Completions shape | `POST /v1/chat/completions` | [Inference gateway](../inference/gateway.md) |
| The models and prices you can call | `GET /v1/models`, `GET /v1/rates` | [Inference gateway](../inference/gateway.md) |
| This key's balance, spend, and caps | `GET /v1/key` | [Inference gateway](../inference/gateway.md) |
| What one call cost | `GET /v1/usage/{request_id}` | [Inference gateway](../inference/gateway.md) |
| The OpenAPI document for public routes | `GET /v1/openapi.json` | [API design](design.md) |
| How many input tokens a body will use | `POST /v1/responses/count` | [Model calls](model-calls.md), proposed |
| Many model calls at half the rate-card price | `POST /v1/responses/batches` | [Model calls](model-calls.md), proposed |
| A file a model call or a session reads | `/v1/files` | [Model calls](model-calls.md) and [Managed Agents](managed-agents.md), proposed. One `file_` object. |
| A saved agent, a session, an environment | `/v1/agents`, `/v1/sessions`, `/v1/environments` | [Managed Agents](managed-agents.md), proposed |
| Members, invitations, bearer keys, and caps | `/v1/workspaces/{ws}/...` | [Organization administration](organization.md) |

A chat thread uses `POST /v1/messages`. That route is planned. A model call uses `POST /v1/responses`.

## Authentication

| Header | Value | When you send it |
| --- | --- | --- |
| `Authorization` | `Bearer oak_<id>.<secret>` or `Bearer sess_<hex>` | Every call that requires a credential. |
| `Content-Type` | `application/json` | A JSON body. |
| `X-Workspace-Id` | A workspace id: `ws_` followed by 16 hex digits | [Workspace](#workspace). Send it once. |
| `Idempotency-Key` | A string you choose | A `POST` that creates or spends, when that route's page lists the header. |

A bearer key is `oak_`. An account session is `sess_`. You issue a bearer key under [organization administration](organization.md). The secret is shown once.

A route that takes payment per request, and receives no key, answers `402` with the payment challenge. Retry the same request with the payment. The methods are in [agent payments](../payments/agent-payments.md).

The version is `/v1` in the path. A request omits a version header. Inside `/v1` the service adds endpoints and optional fields. A breaking change ships as a new path version, announced before it replaces `/v1`. Proposed responses on the other pages also carry `openagents-version`, a date.

### Workspace

The call runs in one workspace.

- On `/v1/workspaces/{ws}/...`, `{ws}` is the workspace. If you also send `X-Workspace-Id`, it names that same id. A different id is `409` `workspace_conflict`.
- On a model call, a key bound to one workspace uses that workspace when you omit the header. A key that reaches more than one workspace requires the header: `400`, with `param` `X-Workspace-Id`. A session uses your personal workspace when you have one. A session in more than one workspace, and with no single personal workspace, requires the header.
- On a decision call that checks membership, and on `GET /v1/balance`, send exactly one `X-Workspace-Id`. A missing or repeated header is `400` `workspace_required`. A key that is not an active member of that workspace is `403` `workspace_forbidden`.
- A paid decision call with no workspace answers `400` `workspace_required`.

A key that is not a current member of the workspace you named on a model call is `401`. Another workspace's resource id is `404`.

### Idempotency

Decision jobs, card funding, and relay jobs store `Idempotency-Key`. The same key and the same body within 24 hours returns the first answer. The same key and a different body is `409` `idempotency_conflict`. Model calls and managed agents use that rule on the routes their pages name.

## Responses

Every answer carries `x-request-id` (`req_` and hex). The error object's `request_id` is that same value. Include the id when you ask about a specific call.

A refusal is JSON:

```json
{
  "error": {
    "type": "invalid_request",
    "code": "workspace_required",
    "message": "Send exactly one X-Workspace-Id header that names your workspace.",
    "param": null,
    "request_id": "req_018ee1a2b3c4d5e6f708192a"
  }
}
```

`type` is the class (`invalid_request`, `authentication`, `permission`, `payment_required`, `limit_reached`, `not_found`, `conflict`, `rate_limited`, `server_error`, `unavailable`). `code` is the reason. `param` names the field when there is one. A refusal forwarded from a decision door keeps that door's JSON.

A model answer also carries these headers:

| Header | Meaning |
| --- | --- |
| `x-openagents-model` | The model that answered. |
| `x-openagents-upstream` | The provider that answered. |
| `x-openagents-cost-usd` | The cost in dollars, on a single JSON answer. A stream omits this header. Send `x-openagents-events` to receive the `openagents:cost` event. |
| `x-openagents-receipt` | On a paid call, the receipt id. |

The workspace of the call is the one the credential resolved to, or the one `X-Workspace-Id` named.

## Lists

List routes take `limit` (default 20, maximum 100) and `after`. `after` is the `next` cursor from the previous response. The body is `{"data": [...], "next": "<cursor>" or null}`. `next` is null on the last page. An expired cursor is `410` `cursor_expired`.

Where a list accepts `order` (`asc` or `desc`), the cursor belongs to that order. Reusing it with a different `order` is `400`.

A list may also include `next_cursor` with the same value as `next`. Read `next`.

## Body size

A request body on the API is at most 1 MiB (1,048,576 bytes), unless the service is configured with a different `max_body_bytes`. A larger body receives HTTP 413 before the route runs. A skill document over its ceiling (at most 1 MiB) answers `413` `submission_too_large`. Funding writes and the dashboard accept 32 KiB. The card-payment webhook accepts 512 KiB.

These larger ceilings belong to the proposed routes on their pages. A request to the API today meets the 1 MiB ceiling above.

| Body | Ceiling on that page |
| --- | --- |
| [Batch create](model-calls.md#batches) | 256 MB, then `413` `request_too_large` |
| [File upload](model-calls.md#files-images-and-documents) | 500 MB, then `400` `file_too_large`. An inline body the server will not read is `413` `request_too_large`. |
| [Session create](managed-agents.md#endpoints) | 32 MB, then `413` |

## Caps you set

You set the caps on a key: spend, maximum price, allowed models, requests per minute, and expiry. Read and replace them with [Caps you set](organization.md#caps-you-set). A spend, price, or model cap answers `403` `limit_reached`. A request-rate cap answers `429` with `Retry-After`. `param` names the cap.
