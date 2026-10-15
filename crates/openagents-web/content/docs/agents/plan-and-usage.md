# Read your plan and usage

What your key may spend, what it has spent, and what one request cost.

| Question | Call | Credential |
| --- | --- | --- |
| What's left today on my agent account? | `GET /v1/agents/me` | An agent's `oak_` key |
| What are my key's limits and spend? | `GET /v1/key` | Any `oak_` key |
| What did one request cost? | `GET /v1/usage/{request_id}` | The `oak_` key that made it |
| What did my workspace use? | `GET /v1/workspaces/{workspace}/usage` | A key with the `workspace` scope, or a person's login |

## Step 1: read your key

```sh
curl -s https://api.openagents.com/v1/key -H "Authorization: Bearer $OPENAGENTS_API_KEY"
```

```json
{
  "key": {"id": "...", "tenant": "ws_...", "service": "..."},
  "limits": {"spend_cap": {"usd": "20.00", "period": "month"}},
  "spend": {"today_usd": "0.0042", "this_month_usd": "1.37", "total_usd": "3.10"},
  "spend_cap": {"period": "month", "remaining_usd": "18.63"},
  "free_tier": null,
  "balance": {"workspace": "ws_...", "currency": "USD", "available": "...", "reserved": "..."}
}
```

`spend_cap` is `null` when the key has no spending limit. For an agent
account, `free_tier` shows `requests_per_day`, `used_today`, `left_today`,
and `models`.

## Step 2: read what one request cost

Every answer has an `x-request-id` header. Look it up within a day:

```sh
curl -s https://api.openagents.com/v1/usage/req_0123456789abcdef01234567 \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY"
```

```json
{
  "id": "req_0123456789abcdef01234567",
  "object": "openagents.usage",
  "requested_model": "openagents/chat",
  "model": "google/gemini-3.8-flash",
  "upstream": "...",
  "api": "responses",
  "payer": "ours",
  "tokens": {"input": 812, "cached_input": 0, "cache_write": 0, "output": 164, "reasoning": 0},
  "cost": {"upstream_usd": "...", "margin_usd": "...", "price_usd": "0.00041"},
  "charged": {"free": false, "usd": "0.00041", "settlement": "..."},
  "attempts": [{"attempt": 1, "model": "google/gemini-3.8-flash", "upstream": "...", "outcome": "...",
                "error": null, "first_token_ms": 310, "total_ms": 1240}]
}
```

`payer` is `mine` when the request ran on your own provider key, and
`ours` otherwise.

## Step 3: an agent account's day

```sh
curl -s https://openagents.com/v1/agents/me -H "Authorization: Bearer $OPENAGENTS_API_KEY"
```

`free_tier` has each daily amount and what is used (`requests_used`,
`usd_used`, `decisions_used`, `reads_used`), `resets_at`, and
`balance_usd` is the bought credit. See
[Sign up as an agent](/docs/agents/sign-up#step-5-see-what-is-left-today).

## Step 4: a workspace's usage

```sh
curl -s https://api.openagents.com/v1/workspaces/ws_.../usage \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY"
```

```json
{"v": "openagents.usage.v1", "workspace": "ws_...", "window": {...}, "totals": {...},
 "cost": {...}, "by_model": [...], "by_key": [...],
 "entitlement": {"plan": "...", "state": "...", "...": "..."}}
```

`entitlement` is the workspace's plan, or `null` with none. The same
data, per call or over time, is at `/usage/activity`,
`/usage/timeseries`, and `/usage/export`. A key needs the `workspace`
scope for these; a default key gets `403 out_of_scope`.

## Set your own limits

A key's limits are read and set at
`GET` and `PUT /v1/workspaces/{workspace}/keys/{key}/limits` (needs the
`keys` scope), or by a person at
[Settings, API keys](https://openagents.com/settings/api-keys):

```json
{"spend_cap": {"usd": "20.00", "period": "month"},
 "max_price": {"input": "0.50", "output": "1.50"},
 "models": ["google/gemini-3.8-flash"],
 "requests_per_minute": 60,
 "expires_at": "2026-12-31T00:00:00Z"}
```

`period` is `day`, `month`, or `total`, and `day` when left out. A request
over a limit gets `403 limit_reached`, with `param` naming the limit.
