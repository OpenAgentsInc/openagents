# Error codes

Every error is JSON with an HTTP status, in one shape:

```json
{
  "error": {
    "type": "invalid_request",
    "code": "invalid_request",
    "message": "The request body isn't a valid request: ...",
    "param": "input",
    "request_id": "req_0123456789abcdef01234567"
  }
}
```

| Field | Meaning |
| --- | --- |
| `type` | The kind of error. Branch on this |
| `code` | The exact reason. Branch on this when you need to |
| `message` | A sentence for people; don't parse it |
| `param` | The field or limit at fault, or `null` |
| `request_id` | The same as the `x-request-id` header. Quote it when you ask us for help |

Some errors carry more fields beside these, such as `pay`, `free_tier`,
`balance_usd`, or `resets_at`.

## By status

| Status | `type` | What to do |
| --- | --- | --- |
| 400 | `invalid_request` | Fix the field named in `param` |
| 401 | `unauthorized` (model routes), `authentication` (other routes) | Send a valid key in `Authorization: Bearer ...` |
| 402 | `insufficient_balance` | Buy credit, then retry: [Buy credits](/docs/agents/buy-credits) |
| 402 | `payment_required` | Pay the invoice in `PAYMENT-REQUIRED`, then retry: [x402](/docs/agents/x402) |
| 402 | `payment_refused` | The proof wasn't accepted; `reason` says why. Get a new invoice |
| 403 | `limit_reached` | A limit set on the key was hit; `param` names it |
| 403 | `permission` | The key or login may not do this; often `code` is `out_of_scope` |
| 404 | `not_found` | Check the path, the id, or the model name |
| 409 | `conflict` | The thing changed or already exists; read it again |
| 429 | `rate_limited` (also `too_many_requests`) | Wait for `Retry-After` seconds, then retry |
| 500 | `server_error`, `model_error` | Retry; tell us if it keeps happening |
| 502 | `upstream_failed` | Every provider we tried failed. Retry, or name another model |
| 503 | `no_route` | No provider meets the request's needs or price cap |
| 503 | `unavailable` | Retry later |

## Codes an agent meets often

| Status | `code` | Why | What to do |
| --- | --- | --- | --- |
| 400 | `invalid_proof` | Sign-up challenge wrong, expired, or used | Get a new challenge |
| 400 | `invalid_name` | Agent name not 1 to 64 bytes | Pick another name |
| 401 | `unauthenticated` | No key, or an unknown key | Check `Authorization` |
| 401 | `signed_out` | A route that needs a login got an API key or nothing | Use a login key (`oa_sk_...`) |
| 402 | `agent_free_requests_used` | The 20 free requests for today are used | Buy credit, or wait for `resets_at` |
| 402 | `agent_free_budget_used` | The free $0.005 for today is used | Buy credit, or wait |
| 402 | `agent_free_tier_full` | Free use for all agents is used up today | Buy credit, or wait |
| 402 | `agent_payment_required` | The request is outside the free tier (another model, images, too long) | Buy credit, or change the request |
| 402 | `agent_balance_too_low` | The credit can't cover the request's most possible cost | Buy more credit |
| 403 | `out_of_scope` | The key lacks the scope this route needs | Use a key with that scope: [Scopes](/docs/agents/scopes) |
| 403 | `limit_reached` | `param` is `limits.spend_cap`, `limits.requests_per_minute`, `limits.models`, `limits.expires_at`, `scopes.models`, or `scopes.actions` | Raise the limit, or wait |
| 404 | `model_not_found` | No model by that name | `GET /v1/models` lists them |
| 404 | `not_an_agent` | `/v1/agents/me` with a key that isn't an agent's | Use the agent's own key |
| 409 | `idempotency_conflict` | Same `Idempotency-Key`, different body | Use a new key for a new body |
| 429 | `agent_daily_limit` | An agent's 20 decision or 500 other calls for today are used | Wait for `Retry-After`, or hold $0.01 of credit |
| 429 | `signup_limit` | Too many sign-ups today from this address or network | Wait for `Retry-After` |
| 503 | `signup_unavailable` | Sign-up is paused | Try later |

## Errors after an answer has started

A streamed answer can't change its status once it has started. It ends
with a `response.failed` (or `error`) event instead, and a full answer
comes back with `"status": "failed"`. Either way the `error` object has
`code` and `message`.

```text
event: response.failed
data: {"type":"response.failed","response":{"status":"failed","error":{"code":"upstream_failed","message":"..."}}}
```

## Retrying

| Status | Retry? |
| --- | --- |
| 400, 401, 403, 404, 409 | No: change the request first |
| 402 | After paying |
| 429 | After `Retry-After` |
| 500, 502, 503 | Yes, after a short wait. For decisions, send an `Idempotency-Key` |

## MCP errors

The MCP servers answer JSON-RPC errors: `-32700` (not JSON), `-32600`
(not a request), `-32601` (no such method), `-32602` (no such tool, or
bad arguments), and `-32002` (no such resource). A tool's own failure is
a normal result with `"isError": true`. See
[Connect to our MCP servers](/docs/agents/mcp).
