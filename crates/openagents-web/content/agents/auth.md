# OpenAgents auth.md

How an agent or a program gets and uses a key for openagents.com and the
OpenAgents API.

## Who this is for

Agents and programs that call the OpenAgents API at
`https://api.openagents.com/v1` (the same API is at
`https://openagents.com/api/v1`), or that read this site.

## What needs no key

- Every page and guide, as HTML or Markdown: add `.md` to an address, send
  `Accept: text/markdown`, or start from [llms.txt](https://openagents.com/llms.txt).
- The docs MCP server at `https://openagents.com/mcp/docs` (Streamable
  HTTP): list, search, and read the docs and the model prices.
- The models and their prices: [Models and prices](https://openagents.com/docs/api/models.md),
  or `GET https://api.openagents.com/v1/rates` as JSON.

## Get a key

A person makes the key; there is no sign-up endpoint for agents.

1. Sign in at <https://openagents.com/login> (GitHub).
2. Open **Settings → API keys** (<https://openagents.com/settings/api-keys>).
3. Make a key, and set a monthly spending limit if you want one. The key is
   shown once. It looks like `oak_<id>.<secret>`.
4. Give it to the agent in an environment variable, such as
   `OPENAGENTS_API_KEY`. Never put it in source code or a URL.

OpenAgents has no OAuth sign-in for agents yet
([#11084](https://github.com/OpenAgentsInc/openagents/issues/11084)).

Agents can also call the API with no key and pay for each request. Every
way to find, sign in to, and pay OpenAgents is in
[For agents](https://openagents.com/docs/api/for-agents.md).

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
