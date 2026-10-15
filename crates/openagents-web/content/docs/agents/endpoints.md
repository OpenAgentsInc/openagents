# Endpoint catalog

Every endpoint an agent uses, by host. Each row is a route our servers
mount today; a test checks this page against the routes in the code. The
full machine-readable description is the OpenAPI document:

```sh
curl -s https://api.openagents.com/v1/openapi.json
```

## Hosts

| Host | What answers there |
| --- | --- |
| `https://api.openagents.com/v1` | The API: models, keys, usage, workspaces, agent accounts |
| `https://openagents.com/api/v1` | The same public API, on the website's address. Decisions are here |
| `https://openagents.com/v1` | The website's own routes: projects, files, chats, login keys, agent sign-up |
| `https://openagents.com` | Docs, `llms.txt`, MCP servers, and discovery files |

Credentials: **none**; **key** (`oak_`); **agent key** (an agent's
`oak_`); **login** (`oa_sk_` login key, OAuth token, or session);
**session** (a person logged in on the website);
**pay** (no key, paid with x402).

## Agent accounts

| Endpoint | Host | Credential | What |
| --- | --- | --- | --- |
| `GET /v1/agents/challenge` | `openagents.com` | none | A proof-of-work challenge, good for ten minutes |
| `POST /v1/agents/register` | `openagents.com` | none | Register: an account, an API key (shown once), a claim code |
| `GET /v1/agents/me` | `openagents.com` | agent key | The free tier today, the credit, and how to pay |
| `POST /v1/agents/claim` | `openagents.com` | session | A person links an agent with its claim code |
| `GET /v1/agents/claimed` | `openagents.com` | session | A person's agents |
| `POST /v1/agents/{account}/revoke` | `openagents.com` | session | Turn an agent off; its key stops at once |

## Models

| Endpoint | Host | Credential | What |
| --- | --- | --- | --- |
| `POST /v1/responses` | `api.openagents.com` | key or pay | Open Responses: one answer or a stream |
| `POST /v1/chat/completions` | `api.openagents.com` | key or pay | Chat Completions: one answer or a stream |
| `POST /v1/responses/compact` | `api.openagents.com` | key | Shorten a long conversation |
| `GET /v1/responses/{id}` | `api.openagents.com` | key | A stored response |
| `GET /v1/models` | `api.openagents.com` | key | The models |
| `GET /v1/rates` | `api.openagents.com` | none | Models and prices as JSON |
| `GET /v1/usage/tokens-served` | `api.openagents.com` | none | Tokens served, in all |
| `GET /v1/openapi.json` | `api.openagents.com` | none | The OpenAPI 3.1 description |

## Decisions

| Endpoint | Host | Credential | What |
| --- | --- | --- | --- |
| `POST /v1/decisions` | `openagents.com/api` | none | Typed answers: predicate, choice, score. Free |

## Keys, usage, and money

| Endpoint | Host | Credential | What |
| --- | --- | --- | --- |
| `GET /v1/key` | `api.openagents.com` | key | This key's limits, spend, free tier, and balance |
| `GET /v1/usage/{request_id}` | `api.openagents.com` | key | What one request cost, within a day |
| `GET /v1/account` | `api.openagents.com` | key or session | Your account |
| `GET /v1/balance` | `api.openagents.com` | key (`usage:read`) | The workspace's balance, with `X-Workspace-Id` |
| `GET /v1/workspaces/{workspace}/usage` | `api.openagents.com` | key (`workspace`) | The workspace's usage and plan |
| `GET /v1/workspaces/{workspace}/usage/activity` | `api.openagents.com` | key (`workspace`) | Calls, newest first |
| `GET /v1/workspaces/{workspace}/usage/timeseries` | `api.openagents.com` | key (`workspace`) | Usage over time |
| `GET /v1/workspaces/{workspace}/usage/export` | `api.openagents.com` | key (`workspace`) | Usage as a file |
| `POST /v1/workspaces/{workspace}/topups/{door}` | `api.openagents.com` | key | Top up the workspace over Lightning |
| `POST /v1/workspaces/{workspace}/keys` | `api.openagents.com` | key (`keys`) | Make a key with `name` and `scopes` |
| `GET /v1/workspaces/{workspace}/keys/{key}/limits` | `api.openagents.com` | key (`keys`) | A key's limits; `PUT` sets them |
| `POST /v1/workspaces/{workspace}/keys/{key}/rotate` | `api.openagents.com` | key (`keys`) | A new secret, shown once |
| `POST /v1/workspaces/{workspace}/keys/{key}/pause` | `api.openagents.com` | key (`keys`) | Pause a key |
| `POST /v1/workspaces/{workspace}/keys/{key}/resume` | `api.openagents.com` | key (`keys`) | Resume a key |
| `POST /v1/keys` | `openagents.com` | login | Make a login key or API key (`{"kind": "signin" or "api", "name"}`) |

## Projects and files

| Endpoint | Host | Credential | What |
| --- | --- | --- | --- |
| `GET /v1/projects` | `openagents.com` | login | Your projects (`?limit=`, `?after=`) |
| `POST /v1/projects` | `openagents.com` | login | Create a project |
| `GET /v1/projects/{id}` | `openagents.com` | login | One project; `PATCH` edits, `DELETE` removes |
| `POST /v1/files` | `openagents.com` | login | Upload a file's bytes (`?purpose=project&filename=`) |
| `POST /v1/projects/{id}/attachments` | `openagents.com` | login | Attach a file or a text document |
| `GET /v1/projects/{id}/members` | `openagents.com` | login | Members, and pending invitations for the owner |
| `POST /v1/projects/{id}/invitations` | `openagents.com` | login | Invite by GitHub username, or make a link |
| `POST /v1/projects/{id}/leave` | `openagents.com` | login | Leave a project |

## Chats

| Endpoint | Host | Credential | What |
| --- | --- | --- | --- |
| `POST /v1/threads` | `openagents.com` | login | Start a chat (`request_id`, `text`, optional `project`) |
| `GET /v1/threads` | `openagents.com` | login | Your chats |
| `GET /v1/threads/{id}` | `openagents.com` | login | One chat and its messages |
| `POST /v1/threads/{id}/messages` | `openagents.com` | login | Reply in a chat |

## MCP

| Endpoint | Host | Credential | What |
| --- | --- | --- | --- |
| `POST /mcp/docs` | `openagents.com` | none | The docs MCP server |
| `POST /mcp` | `openagents.com` | login | The account MCP server |

## Discovery

| Address | What |
| --- | --- |
| [`/llms.txt`](https://openagents.com/llms.txt) | Every page, as Markdown links |
| [`/llms-full.txt`](https://openagents.com/llms-full.txt) | Every page in one file |
| [`/auth.md`](https://openagents.com/auth.md) | How to get and use a credential |
| [`/skill.md`](https://openagents.com/skill.md) | An agent skill for the API |
| [`/openapi.json`](https://openagents.com/openapi.json) | The OpenAPI description, on the website's address |
| [`/.well-known/api-catalog`](https://openagents.com/.well-known/api-catalog) | Where each API and its description live |
| [`/.well-known/ai-catalog.json`](https://openagents.com/.well-known/ai-catalog.json) | The MCP server, agent card, skills, and API |
| [`/.well-known/mcp/server-card.json`](https://openagents.com/.well-known/mcp/server-card.json) | The docs MCP server's card |
| [`/.well-known/agent-card.json`](https://openagents.com/.well-known/agent-card.json) | A2A agent card |
| [`/.well-known/oauth-authorization-server`](https://openagents.com/.well-known/oauth-authorization-server) | OAuth metadata, and how agents register |
