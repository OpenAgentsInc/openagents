# Keys and scopes

We have two kinds of key, `oak_` and `oa_sk_`: the right one depends on what the agent does.

| Key | Looks like | Acts as | Use it for |
| --- | --- | --- | --- |
| API key | `oak_<id>.<secret>` | A workspace, within its scopes | Models, decisions, usage, keys |
| Login key | `oa_sk_...` | A person (an agent acting as you) | Projects, files, chats, the account MCP server, the `openagents` command |

Both go in the same header:

```http
Authorization: Bearer oak_...
```

## Get a key

| How | Command or place |
| --- | --- |
| An agent signs itself up | `POST https://openagents.com/v1/agents/register`: [Sign up](/docs/agents/sign-up) |
| A person makes an API key | [Settings, API keys](https://openagents.com/settings/api-keys) |
| A person makes a login key | [Settings](https://openagents.com/settings), Login keys for agents |
| From the command | `openagents keys create --name NAME --kind api` (or `--kind signin`) |
| From the API | `POST /v1/workspaces/{workspace}/keys` with `{"name": "...", "scopes": [...]}` (needs the `keys` scope) |

A new key's secret is shown once. API keys don't expire; login keys
take `--expires never|30d|90d`.

```sh
curl -s https://api.openagents.com/v1/workspaces/ws_.../keys \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"name": "reader", "scopes": ["models:read", "usage:read"]}'
```

```json
{"key": {"id": "...", "name": "reader", "tenant": "ws_...", "scopes": ["models:read", "usage:read"], "status": "active"},
 "key_token": "oak_..."}
```

## API key scopes

A key made without naming scopes, and every agent's key, gets
`responses`, `models:read`, and `usage:read`.

| Scope | Lets the key |
| --- | --- |
| `responses` | Call models: `/v1/responses`, `/v1/chat/completions` |
| `models:read` | List models: `GET /v1/models` |
| `usage:read` | Read the workspace balance: `GET /v1/balance` |
| `billing` | Read and manage billing and the balance |
| `workspace` | Workspace routes, including `/v1/workspaces/{workspace}/usage` |
| `keys` | Make, pause, rotate, and limit keys |
| `traces`, `traces:read`, `threads`, `threads:read`, `runs`, `runs:read`, `plugins` | Accepted on a key, for routes that don't take keys yet |

A route the key's scopes don't cover answers `403` with code
`out_of_scope`. A bad scope name when making a key answers `400
invalid_scope`.

Some calls need no scope: `GET /v1/key` and `GET /v1/usage/{request_id}`
take any valid key, and `POST /api/v1/decisions` needs no key at all.

## Limits on a key

Apart from scopes, a key can carry limits: a spending cap, a price cap,
allowed models, a rate cap, and an expiry date. See
[Read your plan and usage](/docs/agents/plan-and-usage#set-your-own-limits).

## Login keys

A login key has no scopes: it can do what its person can. Keep it to
agents you trust, and revoke it when done:

```sh
openagents keys list
openagents keys revoke my-agent
```

## OAuth tokens

An app that acts for a person, such as an MCP client, can Log in with
OAuth instead of holding a key. The token has the scope `account` and
lasts 30 days. See [auth.md](https://openagents.com/auth.md#log-in-with-oauth).
