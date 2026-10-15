# Sign up as an agent and make your first call

An agent can make its own OpenAgents account: no person, no email, and no
card. It solves a small proof of work, gets an API key and a free amount
each day, and pays in bitcoin past that.

## Step 1: get a challenge

```sh
curl -s https://openagents.com/v1/agents/challenge
```

```json
{
  "challenge": "v1.22.1792022400.9f2c...e1.4b7a...0c",
  "algorithm": "sha256",
  "difficulty_bits": 22,
  "expires_at": 1792022400,
  "how": "Find a solution (1 to 64 printable characters, such as a counter) where SHA-256 of the challenge, a colon, and the solution starts with 22 zero bits. ...",
  "register": "/v1/agents/register",
  "docs": "https://openagents.com/auth.md"
}
```

The challenge lasts ten minutes and works once.

## Step 2: solve it

Find a `solution` (1 to 64 printable characters) where
`SHA-256(challenge + ":" + solution)` starts with `difficulty_bits` zero
bits. A counter works and takes a few seconds:

```python
import hashlib, json, sys, urllib.request

c = json.load(urllib.request.urlopen("https://openagents.com/v1/agents/challenge"))
need = c["difficulty_bits"]
n = 0
while True:
    digest = hashlib.sha256(f'{c["challenge"]}:{n}'.encode()).digest()
    if int.from_bytes(digest, "big") >> (256 - need) == 0:
        break
    n += 1
print(json.dumps({"challenge": c["challenge"], "solution": str(n)}))
```

## Step 3: register

```sh
curl -s https://openagents.com/v1/agents/register \
  -H "Content-Type: application/json" \
  -d '{"name": "my-agent", "challenge": "v1.22...", "solution": "1834221"}'
```

`201 Created`:

```json
{
  "account": {"id": "acct_...", "name": "my-agent", "kind": "agent"},
  "workspace": {"id": "ws_..."},
  "api_key": "oak_<id>.<secret>",
  "key": {"id": "...", "scopes": ["responses", "models:read", "usage:read"]},
  "use": "Send the API key as `Authorization: Bearer <api_key>` to https://openagents.com/v1 (or https://openagents.com/api/v1). It is shown only now.",
  "claim": {"code": "clm_...", "page": "https://openagents.com/settings/agent-accounts", "expires_at": 1794614400, "how": "..."},
  "free_tier": {"models": ["google/gemini-2.5-flash-lite"], "requests_per_day": 20, "...": "..."},
  "pay": {"available": true, "rail": "lightning", "methods": [...]},
  "docs": "https://openagents.com/auth.md"
}
```

Save `api_key` now: it is shown only once. Keep it in an environment
variable, never in code or a URL.

```sh
export OPENAGENTS_API_KEY=oak_...
```

| Status | `code` | Why |
| --- | --- | --- |
| 400 | `invalid_name` | `name` must be 1 to 64 bytes with no control characters |
| 400 | `invalid_proof` | Wrong, expired, or already used challenge |
| 429 | `signup_limit` | Too many sign-ups today: 5 per address, 20 per network, 500 in all. Wait for `Retry-After` |
| 503 | `signup_unavailable` | Try again later |

## Step 4: make your first call

The free tier covers `google/gemini-2.5-flash-lite`:

```sh
curl -s https://api.openagents.com/v1/responses \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model": "google/gemini-2.5-flash-lite", "input": "Say hello.", "max_output_tokens": 200}'
```

```json
{
  "id": "resp_...",
  "object": "response",
  "status": "completed",
  "model": "google/gemini-2.5-flash-lite",
  "output": [{"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "Hello!"}]}],
  "usage": {"input_tokens": 9, "output_tokens": 3, "total_tokens": 12}
}
```

## Step 5: see what is left today

```sh
curl -s https://openagents.com/v1/agents/me -H "Authorization: Bearer $OPENAGENTS_API_KEY"
```

```json
{
  "account": {"id": "acct_...", "name": "my-agent", "kind": "agent", "claimed": false},
  "free_tier": {"models": ["google/gemini-2.5-flash-lite"], "requests_per_day": 20, "requests_used": 1,
                "usd_per_day": "0.005", "usd_used": "0.000004", "max_input_chars": 8000, "max_output_tokens": 1024,
                "decisions_per_day": 20, "decisions_used": 0, "reads_per_day": 500, "reads_used": 2,
                "resets_at": "2026-10-16T00:00:00Z"},
  "balance_usd": "0",
  "pay": {"available": true, "rail": "lightning", "methods": [...]},
  "docs": "https://openagents.com/auth.md"
}
```

## What the free tier covers each day (UTC)

| Item | Free amount |
| --- | --- |
| Model requests | 20, on `google/gemini-2.5-flash-lite` only |
| Each request | Up to 8,000 input characters and 1,024 output tokens; no images or files |
| Spend | Up to $0.005 a day in all |
| Decision calls | 20 |
| Other API calls | 500 |

Past it, a model request answers `402 insufficient_balance` with a `pay`
object, and other calls `429 agent_daily_limit`. Buy credit to go on:
[Buy credits over Lightning](/docs/agents/buy-credits). An agent with at
least $0.01 of credit has no daily call cap.

## Let a person claim you

Give the `claim.code` (`clm_...`, good for 30 days) to the person you
work for. They Log in at
[Settings, Agent accounts](https://openagents.com/settings/agent-accounts)
and enter it. They can then turn you off there; your key stops at once.

## What an agent account can't do

It can't log in to the website, use computers, or run Solver. For those,
work with a person's login key instead: see
[Start here](/docs/agents/start-here#say-who-you-are).
