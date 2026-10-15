# For agents: start here

What OpenAgents offers an agent, how an agent signs itself up, and the
rules every call follows: keys, base addresses, limits, and errors.
Every step below is one request or one command.

## What we offer an agent

| Offer | How to reach it | Key |
| --- | --- | --- |
| Many models through one API, Google's Gemini first | `POST https://api.openagents.com/v1/responses` | API key, or pay per request |
| Typed answers in a fraction of a second | `POST https://openagents.com/api/v1/decisions` | None, free |
| An account of its own, with a free amount each day | `POST https://openagents.com/v1/agents/register` | None |
| Projects, files, and chats for the person it works for | `https://openagents.com/v1/projects`, `/v1/threads` | Login key |
| Coder, our coding agent, run from a script | `openagents coder chat --json` | Your computer's login |
| The docs as MCP tools | `https://openagents.com/mcp/docs` | None |
| Bitcoin payments over Lightning | `pay` objects, x402, and `openagents x402` | None |

## Step 1: sign yourself up

No person, email, or card. Solve a proof of work, register, and keep the
key you get back:

```sh
curl -s https://openagents.com/v1/agents/challenge
curl -s https://openagents.com/v1/agents/register \
  -H "Content-Type: application/json" \
  -d '{"name": "my-agent", "challenge": "<challenge>", "solution": "<solution>"}'
```

The answer has `api_key` (`oak_...`, shown once), a `claim` code for the
person you work for, your `free_tier`, and how to `pay`. Every step,
with a solver: [Sign up as an agent](/docs/agents/sign-up).

The free tier, each day (UTC): 20 requests on
`google/gemini-2.5-flash-lite` (up to 8,000 input characters and 1,024
output tokens each, $0.005 in all), 20 decision calls, and 500 other
calls.

## Step 2: make a call

```sh
export OPENAGENTS_API_KEY=oak_...
curl -s https://api.openagents.com/v1/responses \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model": "google/gemini-2.5-flash-lite", "input": "Say hello."}'
```

OpenAI SDKs work with `base_url` set to `https://api.openagents.com/v1`.

## Say who you are

| Credential | Header | Get it |
| --- | --- | --- |
| Your own agent key | `Authorization: Bearer oak_...` | [Sign up](/docs/agents/sign-up) |
| A person's API key | `Authorization: Bearer oak_...` | [Settings, API keys](https://openagents.com/settings/api-keys) |
| A person's login key (acting as them) | `Authorization: Bearer oa_sk_...` | [Settings](https://openagents.com/settings), Login keys for agents |
| An OAuth token, for MCP clients | `Authorization: Bearer ...` | [Log in with OAuth](https://openagents.com/auth.md#log-in-with-oauth) |
| Nothing: pay for each request | `PAYMENT-SIGNATURE: ...` | [Pay per call with x402](/docs/agents/x402) |

Which key each route takes, and what each scope allows:
[Keys and scopes](/docs/agents/scopes).

Keep keys in environment variables (`OPENAGENTS_API_KEY`,
`OPENAGENTS_TOKEN`), never in code, chat, or a URL.

## Base addresses

| Address | For |
| --- | --- |
| `https://api.openagents.com/v1` | The API: models, keys, usage, workspaces |
| `https://openagents.com/api/v1` | The same public API on the website's address, and decisions |
| `https://openagents.com/v1` | Agent sign-up, projects, files, chats, login keys |
| `https://openagents.com/mcp/docs` and `/mcp` | The MCP servers |

Every route: [Endpoint catalog](/docs/agents/endpoints).

## Limits

We don't cap how much you use the API. The limits are:

| Limit | Answer when hit |
| --- | --- |
| An agent's free tier and daily calls | `402` with a `pay` object, or `429 agent_daily_limit` with `Retry-After` |
| Sign-ups: 5 a day per address, 20 per network, 500 in all | `429 signup_limit` with `Retry-After` |
| Limits set on a key: spend, price, models, rate, expiry | `403 limit_reached`, with `param` naming the limit |
| Your balance | `402 insufficient_balance` |

There are no `x-ratelimit-*` headers. When a `429` has `Retry-After`,
wait that many seconds. Read what is left with `GET /v1/agents/me` or
`GET /v1/key`: [Read your plan and usage](/docs/agents/plan-and-usage).

## Errors

Every error is JSON in one shape:

```json
{"error": {"type": "insufficient_balance", "code": "agent_free_requests_used",
  "message": "...", "param": null, "request_id": "req_..."}}
```

Branch on `type`, then `code`. Every answer has an `x-request-id`
header; quote it when you ask us for help. Every code:
[Error codes](/docs/agents/errors).

## Pay

Payments are Bitcoin only, over Lightning.

- With an agent account: buy credit, then each request is charged what
  it used. [Buy credits over Lightning](/docs/agents/buy-credits).
- With no account: pay each request's quoted price with x402.
  [Pay per call with x402](/docs/agents/x402).

## Next

| Task | Page |
| --- | --- |
| Stream an answer, or start an OpenAgents chat | [Start a chat and stream the answer](/docs/agents/chat-and-stream) |
| Classify and route with typed answers | [Use the Decisions API](/docs/agents/decisions) |
| Make a project, add files, invite a person | [Create a project](/docs/agents/projects) |
| Run Coder from a script | [Run Coder headless](/docs/agents/coder-headless) |
| Use our MCP servers | [Connect to our MCP servers](/docs/agents/mcp) |
| Work in a browser | [Use WebMCP tools](/docs/agents/webmcp) |
| Use the `openagents` command | [The openagents command for agents](/docs/agents/cli) |

## Machine-readable

| File | What |
| --- | --- |
| [llms.txt](https://openagents.com/llms.txt) | Every page, as links to Markdown |
| [llms-full.txt](https://openagents.com/llms-full.txt) | Every page in one file |
| [/docs/agents/llms.txt](https://openagents.com/docs/agents/llms.txt) | Just these pages |
| [skill.md](https://openagents.com/skill.md) | An agent skill for OpenAgents |
| [OpenAPI](https://api.openagents.com/v1/openapi.json) | Every API route |

Any page here is also Markdown: add `.md` to its address, or send
`Accept: text/markdown`.
