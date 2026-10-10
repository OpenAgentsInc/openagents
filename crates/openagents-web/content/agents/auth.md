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

An app that acts for a person, such as an MCP client, can sign in with
OAuth instead of a key: see [Sign in with OAuth](#sign-in-with-oauth).

Agents can also call the API with no key and pay for each request. Every
way to find, sign in to, and pay OpenAgents is in
[For agents](https://openagents.com/docs/api/for-agents.md).

## Sign in with OAuth

Apps that act for a person, such as MCP clients (Claude, ChatGPT, Cursor,
VS Code), get a bearer token with OAuth 2.1: the app registers itself, you
approve the sign-in in your browser, and the app gets a token for your
account. OpenAgents is its own authorization server (sign-in with GitHub;
sign-in is invite-only for now).

- Resource metadata: <https://openagents.com/.well-known/oauth-protected-resource/mcp>
  (and <https://openagents.com/.well-known/oauth-protected-resource> for the site).
- Authorization server metadata: <https://openagents.com/.well-known/oauth-authorization-server>.
- Register a client: `POST https://openagents.com/oauth/register` with
  `{"client_name": "...", "redirect_uris": ["..."]}` (RFC 7591). Clients
  are public: no client secret. Redirect addresses are `https`, `http` on
  `127.0.0.1` or `localhost`, or the app's own scheme.
- Authorize: `https://openagents.com/oauth/authorize` with
  `response_type=code`, `client_id`, `redirect_uri`, `state`, and PKCE
  (`code_challenge_method=S256`; plain is refused). The person signs in
  and chooses Approve or Deny.
- Token: `POST https://openagents.com/oauth/token` with
  `grant_type=authorization_code`, `code`, `client_id`, `redirect_uri`,
  and `code_verifier`. The answer is a bearer `access_token` that lasts 30
  days; there is no refresh token, so sign in again when it ends.

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
