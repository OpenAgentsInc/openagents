# The decision API for agents

An OpenAgents deployment's public origin. Everything here describes
what the code implements today — the [route catalog](api-catalog.json)
is authoritative, and its `unimplemented` list names advertised
surfaces that do not exist.

## What this service is

A keyed HTTP decision API. A caller posts a state and a map of typed
questions — `noul` (yes/no), `choice`, and `score` — to
`POST /v1/systemone` and gets one typed answer per question with
probabilities. `POST /v1/classify` runs batch classification;
`POST /v1/jobs` persists a batch as a resumable job. `POST /v1/feedback`
files a structured report and returns a trackable receipt.
`PUT /v1/updates` records the credential's opt-in product-updates
subscription, with separate product and support consent.
`GET /v1/models` lists the doors a credential may name.

## Start here

1. Read [auth.md](auth.md) — you need an `oak_<id>.<secret>` bearer key
   issued by the operator, or a `sess_` session token from a deployment
   that configures `accounts.signup_tenant` (`POST /v1/accounts` is the
   self-serve entry point).
2. List your doors: `GET /v1/models` with the key.
3. Call `POST /v1/systemone` or use the `oak` CLI / `oak-mcp` server —
   [skills.md](skills.md) compares the surfaces and states supported
   versions.
4. Read the bundled documentation over `GET /v1/docs` without a
   credential — list, read, search, and examples operations with
   pagination.

## Discovery map

| Path | What it is |
| --- | --- |
| `/api` | The API index — every surface, its version tag, its auth, and where its contract lives. |
| `/api-catalog.json` | Every implemented route, request and response shapes, refusal codes, and an explicit unimplemented list. |
| `/openapi.yaml` | OpenAPI 3.1 fragment covering exactly the implemented routes. |
| `/llms.txt` | The llms.txt-convention entry point. |
| `/agents.md` | This document. |
| `/auth.md` | The credential contract: bearer keys, where they may live, typed refusals. |
| `/skills.md` | What an agent needs to call the API. |
| `/.well-known/agent-card.json` | The A2A-format agent card — interfaces, capabilities, skills, security. |
| `/.well-known/agent-skills/index.json` | The agent-skills discovery index with digested skill artifacts. |
| `/mcp/server-card.json` | The MCP server's card — transport, protocol versions, session contract, tool list. |
| `/v1/docs` | The versioned docs API — list/read/search/examples over the bundled corpus. |
| `/plugins/` | Declarative Claude- and Codex-compatible plugin packages for manual installation. |
| `/sitemap.xml`, `/robots.txt` | Crawl metadata for the discovery surface. |

## Honest limits

- Discovery is unauthenticated by design; every inference route
  requires a bearer key.
- Inference consumes quota and may cost money — `GET /v1/balance`
  exists only where the operator enabled monetary admission, and the
  `/v1/accounts`, `/v1/sessions`, `/v1/workspaces`, `/v1/invitations`,
  and `/v1/recovery` management family exists only where the deployment
  configures the `accounts` document. The same `accounts` document
  mounts the `/v1/workspaces/{id}/usage*` reads and the `/dashboard`
  pages — both member-scoped, both absent without it. The `/v1/plans`
  catalog and the `/v1/workspaces/{id}/billing/*` family exist only
  where the deployment configures `billing` — which itself requires
  `accounts` and `money` — and a decision call under billing names a
  subscribed workspace whose plan covers the door. The `/v1/skills`
  and `/v1/submissions` directory family exists only where the
  deployment configures `skills` — which itself requires `accounts` —
  and its public reads serve published versions only.
- No OAuth discovery, no streaming responses, no image input, no
  hosted endpoint — the catalog says so explicitly where a reader
  might assume otherwise.
- MCP tools are served by `oak-mcp-http`, a separate listener this
  origin does not run; `/mcp/server-card.json` describes that
  transport's contract.
