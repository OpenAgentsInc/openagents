# Connect to our MCP servers

We run two MCP servers on openagents.com, both over Streamable HTTP, and
the `openagents` command can serve a third on your own computer.

| Server | Address | Log in | Tools |
| --- | --- | --- | --- |
| Docs | `https://openagents.com/mcp/docs` | No | `list_docs`, `search_docs`, `read_doc`, `list_models`, `list_payment_methods` |
| Your account | `https://openagents.com/mcp` | Yes, with OAuth or a login key | The five above, plus `whoami` |
| Your computer | `openagents mcp serve` (standard input and output) | Your saved login | The command's read-only commands |

## The docs server

No key and no session. Send JSON-RPC with `POST`; each answer is
`application/json`. Protocol versions: `2025-06-18` (newest),
`2025-03-26`, and `2024-11-05`.

### Step 1: initialize

```sh
curl -s https://openagents.com/mcp/docs \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"my-agent","version":"1"}}}'
```

```json
{"jsonrpc":"2.0","id":1,"result":{
  "protocolVersion":"2025-06-18",
  "capabilities":{"tools":{"listChanged":false},"resources":{"listChanged":false}},
  "serverInfo":{"name":"openagents-docs","title":"OpenAgents docs","version":"...","websiteUrl":"https://openagents.com/"},
  "instructions":"Read-only tools for the OpenAgents docs and the OpenAgents API's model prices. ..."}}
```

A notification (no `id`) gets `202` with no body. `GET` and `DELETE`
get `405`.

### Step 2: list the tools

```sh
curl -s https://openagents.com/mcp/docs \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
```

| Tool | Arguments | Returns |
| --- | --- | --- |
| `list_docs` | none | A Markdown table of every page: name, title, address, summary |
| `search_docs` | `query` (string, required), `limit` (1 to 20, default 5) | The best matching pages, each with its name and a matching line |
| `read_doc` | `name` (string, required), such as `chat`, `api/quickstart`, or `agents/start-here` | The page as Markdown |
| `list_models` | none | The models and their prices, per million tokens |
| `list_payment_methods` | none | JSON: the ways to pay per request the API takes now |

Every tool only reads.

### Step 3: call one

```sh
curl -s https://openagents.com/mcp/docs \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"read_doc","arguments":{"name":"agents/start-here"}}}'
```

```json
{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"# For agents: start here\n..."}],"isError":false}}
```

A tool that fails returns `"isError": true` with the reason as text. An
unknown tool or a bad argument shape is a JSON-RPC error,
`{"code": -32602, "message": "No tool named foo."}`.

The server also answers `resources/list` and `resources/read` (one
resource per page, `text/markdown`) and `ping`.

Its card is at
[/.well-known/mcp/server-card.json](https://openagents.com/.well-known/mcp/server-card.json).

### Add it to an MCP client

```json
{
  "mcpServers": {
    "openagents-docs": {"type": "http", "url": "https://openagents.com/mcp/docs"}
  }
}
```

## The account server

`https://openagents.com/mcp` acts for one account. It takes a bearer
token: an OAuth token from logging in, or a login key (`oa_sk_...`). An
`oak_` API key is refused there.

With no token, the answer is `401` with this header, which MCP clients
follow to log in:

```http
WWW-Authenticate: Bearer resource_metadata="https://openagents.com/.well-known/oauth-protected-resource/mcp", scope="account"
```

Most MCP clients do the OAuth steps for you: add the address and Log in
when the browser opens.

```json
{
  "mcpServers": {
    "openagents": {"type": "http", "url": "https://openagents.com/mcp"}
  }
}
```

The OAuth steps (dynamic registration, PKCE with `S256`, a 30-day
token) are in [auth.md](https://openagents.com/auth.md#log-in-with-oauth).

With a login key instead:

```sh
curl -s https://openagents.com/mcp \
  -H "Authorization: Bearer $OPENAGENTS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"whoami","arguments":{}}}'
```

`whoami` answers with the account it is logged in as.

## A server on your computer

```sh
openagents mcp serve
```

This serves the `openagents` command's read-only commands as MCP tools
over standard input and output, one tool for each group of commands.
Commands that spend, sign, or show a secret are never served. Add it to
a client:

```json
{
  "mcpServers": {
    "openagents-cli": {"command": "openagents", "args": ["mcp", "serve"]}
  }
}
```

## See also

- [Use WebMCP tools in a browser agent](/docs/agents/webmcp)
- [The `openagents` command for agents](/docs/agents/cli)
