# OpenAgents auth.md

How an agent or a program gets and uses a key for openagents.com and the
OpenAgents API. An agent can register itself; see
[Register yourself as an agent](#register-yourself-as-an-agent).

## Who this is for

Agents and programs that call the OpenAgents API at
`https://api.openagents.com/v1`, or that read this site. The older
`https://openagents.com/v1` and `https://openagents.com/api/v1` still
answer but are deprecated.

## What needs no key

- Every page and guide, as HTML or Markdown: add `.md` to an address, send
  `Accept: text/markdown`, or start from [llms.txt](https://openagents.com/llms.txt).
- The docs MCP server at `https://openagents.com/mcp/docs` (Streamable
  HTTP): list, search, and read the docs and the model prices.
- The models and their prices: [Models and prices](https://openagents.com/docs/api/models.md),
  or `GET https://api.openagents.com/v1/rates` as JSON.

## Register yourself as an agent

An agent can make its own account here, with no person and no email. It
gets an API key and a small free amount of use each day; past that it pays
with Bitcoin over Lightning.

1. Get a challenge: `GET https://api.openagents.com/v1/agents/challenge`. The
   answer has `challenge` and `difficulty_bits` (22), and lasts ten minutes.
2. Solve it: find a `solution` (1 to 64 printable characters, such as a
   counter) where SHA-256 of `challenge + ":" + solution` starts with
   `difficulty_bits` zero bits. That takes a few seconds.
3. Register: `POST https://api.openagents.com/v1/agents/register` with
   `{"name": "my-agent", "challenge": "...", "solution": "..."}`. The `201`
   answer has `api_key` (`oak_...`, shown only this once), the account
   (`"kind": "agent"`), a `claim` code, `free_tier`, and `pay`.

```sh
challenge=$(curl -s https://api.openagents.com/v1/agents/challenge)
# solve it, then:
curl -s https://api.openagents.com/v1/agents/register \
  -H "Content-Type: application/json" \
  -d '{"name": "my-agent", "challenge": "<challenge>", "solution": "<solution>"}'
```

Each address may register 5 agents a day, each network (an IPv4 /24 or an
IPv6 /48) 20, and everyone together 500. Past a limit the answer is `429`
with `Retry-After`.

### What an agent gets free each day (UTC)

- 20 model requests on `google/gemini-2.5-flash-lite`, each with at most
  8,000 input characters and `max_output_tokens` up to 1,024 (a request that
  names none gets 1,024), no images or files, and at most $0.005 of use a day
  in all.
- 20 decision calls, and 500 other API calls.
- No other models, computers, Solver, Drive, or website login.

`GET https://api.openagents.com/v1/agents/me` with the key shows what is left
today, the credit, and how to pay. Free use for all agents together has a
daily cap too; when it is reached, free requests wait for the next day.

### When the free amount runs out

A model request the free tier can't take answers `402` at once, before
anything runs. The body says why and how to pay:

```json
{"error": {"type": "insufficient_balance", "code": "agent_free_requests_used",
  "message": "...", "free_tier": {"requests_used": 20, "resets_at": "..."},
  "balance_usd": "0", "request_max_cost_usd": "0.0016",
  "pay": {"available": true, "rail": "lightning", "methods": [{"type": "lightning",
    "protocol": "x402", "method": "POST", "url": "https://api.openagents.com/x/...?account=acct_...",
    "price_sats": 2000, "credit_usd": "1"}]}}}
```

Codes: `agent_free_requests_used`, `agent_free_budget_used`,
`agent_free_tier_full` (every agent's free use for today is used up),
`agent_payment_required` (the request is outside the free tier), and
`agent_balance_too_low`. Over the decision or API-call cap the answer is
`429 agent_daily_limit` with `Retry-After`.

### Paying

Paying is Bitcoin only, over Lightning. `POST` the `pay.methods[].url` with
no payment to get a `402` with an invoice, pay it from any Lightning wallet,
and send the same request again with the proof the `402` asks for. The
credit lands on the agent's balance, and any model then runs within it:
each request holds its most possible cost and is charged what it used.
While `pay.available` is `false`, buying credit isn't open yet.

### Claiming and turning off an agent

The registration answer has a `claim.code` (`clm_...`, good for 30 days). A
person who logs in at <https://openagents.com/settings/agent-accounts>
and enters it links the agent to their account (`POST /v1/agents/claim`
with `{"claim": "..."}` under their session does the same). They can then
turn the agent off there; its key stops at once. Site admins can turn off
any agent.

## Get a key as a person

1. Log in at <https://openagents.com/login> (GitHub).
2. Open **Settings → API keys** (<https://openagents.com/settings/api-keys>).
3. Make a key, and set a monthly spending limit if you want one. The key is
   shown once. It looks like `oak_<id>.<secret>`.
4. Give it to the agent in an environment variable, such as
   `OPENAGENTS_API_KEY`. Never put it in source code or a URL.

An app that acts for a person, such as an MCP client, can log in with
OAuth instead of a key: see [Log in with OAuth](#log-in-with-oauth).

Agents can also call the API with no key and pay for each request. Every
way to find, log in to, and pay OpenAgents is in
[For agents](https://openagents.com/docs/api/for-agents.md).

## Log in with OAuth

Apps that act for a person, such as MCP clients (Claude, ChatGPT, Cursor,
VS Code), get a bearer token with OAuth 2.1: the app registers itself, you
approve the login in your browser, and the app gets a token for your
account. OpenAgents is its own authorization server (login with GitHub;
anyone with a GitHub account may log in).

- Resource metadata: <https://openagents.com/.well-known/oauth-protected-resource/mcp>
  (and <https://openagents.com/.well-known/oauth-protected-resource> for the site).
- Authorization server metadata: <https://openagents.com/.well-known/oauth-authorization-server>.
- Register a client: `POST https://openagents.com/oauth/register` with
  `{"client_name": "...", "redirect_uris": ["..."]}` (RFC 7591). Clients
  are public: no client secret. Redirect addresses are `https`, `http` on
  `127.0.0.1` or `localhost`, or the app's own scheme.
- Authorize: `https://openagents.com/oauth/authorize` with
  `response_type=code`, `client_id`, `redirect_uri`, `state`, and PKCE
  (`code_challenge_method=S256`; plain is refused). The person logs in
  and chooses Approve or Deny.
- Token: `POST https://openagents.com/oauth/token` with
  `grant_type=authorization_code`, `code`, `client_id`, `redirect_uri`,
  and `code_verifier`. The answer is a bearer `access_token` that lasts 30
  days; there is no refresh token, so log in again when it ends.

The app shows in **Settings** under Computers
(<https://openagents.com/settings>), where **Remove** signs it out at once.

## Use the key

Send it as a bearer token on every call:

```sh
curl https://api.openagents.com/v1/responses \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model": "openagents/chat", "input": "Say hello."}'
```

OpenAI SDKs work with `base_url` set to `https://api.openagents.com/v1` and
the key as the API key. See the [quickstart](https://openagents.com/docs/api/quickstart.md).

## When a call is refused

- `401`: the key is missing, wrong, or revoked. Check the
  `Authorization` header.
- `402`: the balance can't cover the request. Top up, then retry. With
  no key, `402` means the call can be paid per request: see
  [Pay per request](https://openagents.com/docs/api/pay-per-request.md).
- `403`: a limit the key's owner set was reached.
- `429`: too many requests. Wait for the time in `Retry-After`, then retry.

Every answer has an `x-request-id` header. The full list is in
[Errors](https://openagents.com/docs/api/errors.md), and the limits are in
[Limits](https://openagents.com/docs/api/limits.md).

## Revoke a key

**Settings → API keys** → **Revoke**. A revoked key stops working at once.
