# MCP server

`oak` serves the same tool set over two MCP transports: `oak-mcp` over stdio,
and `oak-mcp-http` over the Streamable HTTP transport. Both run the same
dispatch, the same tool schemas, and the same operator configuration —
credentials, endpoint, and workspace come from the environment or the
`0600` config file the [caller guide](caller.md) describes, never from a
tool argument.

The tools are:

| Tool | What it does |
| --- | --- |
| `list_models` | The authorized `GET /v1/models` discovery document. |
| `classify` | A complete `openagents.classify.v1` envelope to `POST /v1/classify`. |
| `classify_texts` | One label per text from a caller's label set — a single-label categorical judgment per input. |
| `classify_dimensions` | Several independent judgments over the same inputs — each dimension names its own mode and label set or rubric. |
| `classify_multi_label` | Independent per-label judgments over each text — every label at or above the caller's threshold is selected. |
| `count_labels` | The single-label judgment reduced to aggregates — per-label counts and the unanswered tallies, never per-input selections. |
| `review_uncertain` | The single-label judgment listing only the units under the caller's uncertainty cut, optionally re-judged by a named review door. |
| `decide` | The caller's state and typed questions to `POST /v1/systemone`, verbatim. |
| `list_docs`, `read_doc`, `search_docs`, `get_examples` | The bundled, versioned public documentation corpus — no credentials, no network. See [MCP documentation tools](mcp-documentation.md). |

Every inference tool bounds its inputs, rejects arguments it does not
declare, and returns the service's typed refusals as `isError: true`
results with the refusal code intact. Inference can consume quota or
money; the tools' annotations say so. Documentation tools are read-only.

## `oak-mcp` — stdio

An MCP client launches `oak-mcp` as a subprocess and speaks newline-delimited
JSON-RPC 2.0 on standard input and output:

```text
oak-mcp [--url URL] [--config PATH] [--workspace ID]
        [--timeout SECS] [--retries N]
```

The lifecycle is `initialize`, `notifications/initialized`, then
`tools/list` and `tools/call`. Protocol versions `2025-11-25` and
`2025-06-18` negotiate; an unsupported request settles on the latest
served version and the client decides whether to stay. Calls run
sequentially; closing stdin ends the server.

A host application's client configuration names the binary and the
protected environment — the shape below is the common `mcpServers`
form; check your client's own field names:

```json
{
  "mcpServers": {
    "openagents": {
      "command": "oak-mcp",
      "env": {
        "OPENAGENTS_API_KEY": "oak_acme.9f…c4",
        "OPENAGENTS_BASE_URL": "https://gateway.example.com"
      }
    }
  }
}
```

Keep the key in the `env` block or in the `0600` config file — a key in
a committed client configuration is a leaked key.

## `oak-mcp-http` — Streamable HTTP

`oak-mcp-http` serves the same tools to network clients:

```text
oak-mcp-http [--listen ADDR] [--url URL] [--config PATH]
             [--workspace ID] [--timeout SECS] [--retries N]
             [--allow-origin ORIGIN]...
```

`--listen` defaults to `127.0.0.1:8765`. The endpoint is `POST /mcp`.

A session starts at `initialize`: the response carries
`Mcp-Session-Id`, which the client sends on every later request.
Requests answer `200` with the JSON-RPC response; notifications and
client responses answer `202` with no body. `DELETE /mcp` ends the
session; an unknown or expired session answers `404`, and the client
re-initializes. Sessions are in-memory and bounded at 256 with a
30-minute idle expiry — a restart forgets them all, which is the
reconnect path by design.

`MCP-Protocol-Version` on a request must be a version this build
serves, and once `initialize` settles a version the header must match
it — a mismatch answers `400`. Batched JSON-RPC requests are refused;
send one message per `POST`. `GET /mcp` answers `405`: the
specification's server-initiated SSE stream has nothing to carry
because this server never initiates a message.

`Origin` headers are checked per the transport's browser protections:
localhost origins pass, a configured `--allow-origin` passes, anything
else answers `403`. A missing `Origin` — curl and native clients —
is fine.

### Credentials over HTTP

Each `POST` may carry `Authorization: Bearer <key>`; the key is
forwarded to the service for that call only and never stored or logged.
When a request carries no bearer credential, the tools resolve the
operator's own configuration — `OPENAGENTS_API_KEY` or the config file
— exactly as stdio does. A non-Bearer `Authorization` header answers
`401` rather than being silently dropped. A client configuration that
presents a key looks like:

```json
{
  "mcpServers": {
    "openagents": {
      "url": "https://mcp.example.com/mcp",
      "headers": {
        "Authorization": "Bearer oak_acme.9f…c4"
      }
    }
  }
}
```

With no `headers` entry the same configuration reaches the server
anonymously and the operator's credential serves the calls — the
client's identity is then the operator's, which an operator should
accept only for clients it already trusts.

### The server card

`GET /mcp/card` answers `openagents.mcp-server.v1`, the server's
self-description: name and version, the endpoint, the served protocol
versions, the session contract (header, issue point, `DELETE`, idle
bound, capacity), the auth model, and the tool list. Read it to
configure a client or to confirm a deployment before sending traffic.
A deployed gateway mirrors the same card — generated from the same
shape and the bundled tool snapshot — at `GET /mcp/server-card.json`
on its public discovery surface, so an agent can read it without
reaching the MCP listener at all.

## Bounds and failure modes

The transport bounds are the shared ones: a 16 MiB envelope, 64 MiB
responses, and a 16 MiB plus 256 KiB framing bound on each incoming
message. Inference calls inherit `oak`'s retry loop — bounded attempts
honoring `Retry-After`, with idempotent retries only under a caller's
`request_id`. The dispatch itself adds bounds: at most 1,000 texts,
100 labels, 20 dimensions, and 1,000 planned judgments per call;
`review_uncertain` caps its review phase at 200 items, 400 attempts,
and 30 seconds.

A malformed JSON body answers `400`; a well-formed request with bad
tool arguments answers a JSON-RPC `invalid params` or an `isError`
tool result carrying `invalid_arguments`. The service's typed refusals
pass through unchanged — `unauthenticated`, `door_not_bound`, the
facade's envelope faults — as `isError` results with the refusal code
preserved. There is no server-push channel, so there is no progress or
cancellation notification; an in-flight HTTP call remains bounded by
its timeout.

## Verification

Subprocess tests run the stdio server through both protocol versions
and every tool shape. HTTP tests run `oak-mcp-http` over real TCP
against an upstream stub: session mint and lifecycle, the version
headers, per-request bearer forwarding, invalid arguments, typed
refusal passthrough, `DELETE` and reconnect, and the origin check.
These establish transport and caller behavior — not model quality or a
deployed service's availability.
