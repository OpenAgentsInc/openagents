# `coax/lib/mcp/` — Native Coax Model Context Protocol Library

Pure `#authority` Sans-I/O MCP protocol library supporting both **`2025-11-25`** (stateful) and **`2026-07-28`** (stateless GA), designed per [docs/mcp/coax-architecture-and-plan.md](../../docs/mcp/coax-architecture-and-plan.md).

## Phase 0 Modules (Pure `#authority`, Zero Heap)

| File | Responsibility |
| --- | --- |
| `version.coax` | Revision constants (`2024-11-05`..`2026-07-28`), `mcp_version_has_initialize`, legacy/modern negotiation, `_meta` & extension ID grammar |
| `jsonrpc.coax` | JSON-RPC 2.0 envelope classifier & writer, batch refusal, duplicate-key refusal, all standard & MCP error codes (`-32700..-32042`) |
| `uri_template.coax` | General RFC 3986 URI parser (`file://`, `memo://`, loopback, IPv4/IPv6), RFC 8707 canonicalizer & PRM resource match guard, RFC 6570 Level 1–3 template expander & reverse matcher |
| `headers.coax` | `2026-07-28` SEP-2243 `Mcp-Method`/`Mcp-Name`/`Mcp-Param-*` helpers, `=?base64?...?=` sentinel codec, numeric integer validation, and `Origin` DNS-rebinding guard |
| `json_schema.coax` | Bounded JSON Schema Draft 2020-12 validator (preserving `$schema`/`$defs`/`$ref` verbatim) + SEP-2243 `x-mcp-header` static analyzer & argument extractor |
| `auth_policy.coax` | OAuth 2.1 `WWW-Authenticate` parser/emitter, RFC 9728 PRM & RFC 8414/OIDC discovery URL sequences, metadata validation (`issuer`, PKCE `S256`), CIMD/DCR priority, RFC 9207 exact `iss` verification, and `403` scope step-up union |

## Phase 1 Modules — Sans-I/O Protocol Core (Pure `#authority`, Zero Heap)

| File | Responsibility |
| --- | --- |
| `types.coax` | `McpImplInfo`, `McpIcon`, `McpClientCaps`/`McpServerCaps`, `McpCacheHint` (SEP-2549), `McpAnnotations`, `McpContent`/`McpResourceContents`, RFC 5424 `McpLogLevel`, and `2026-07-28` per-request `_meta` codecs |
| `mrtr.coax` | SEP-2322 `InputRequiredResult`/`InputRequests`/`InputResponses` codecs, client capability gating (`-32021`), and AES-256-GCM authenticated `requestState` sealing with AAD binding `(method, req_binding, principal)` & TTL expiry |
| `primitives.coax` | Tools, Resources, ResourceTemplates, Prompts, Completions, Elicitation (form + URL + SEP-1034 client defaults), Roots, `server/discover`, `initialize`, Progress/Cancelled/Logging notifications, and pagination cursor codecs |
| `subscriptions.coax` | `2026-07-28` `subscriptions/listen` filter registry, `notifications/subscriptions/acknowledged`, `subscriptionId` routing, graceful close, and `2025-11-25` `resources/subscribe`/`unsubscribe` table |
| `event_store.coax` | Bounded byte-capped SSE `EventStore` ring buffer, `Last-Event-ID` stream replay, `McpReplayDedup`, and SEP-1699 `retry:` delay clamping |
| `server_core.coax` | Sans-I/O dual-era `McpServerCore` state machine & dispatcher (registries, pagination, SEP-1303 schema validation, RFC 6570 template matching, SEP-2164 `-32602` vs `-32002`, SEP-2243 header validation) |
| `client_core.coax` | Sans-I/O dual-era `McpClientCore` state machine (`server/discover` → `initialize` fallback, `_meta` injection, SEP-2243 `x-mcp-header` tool filtering, MRTR continuation, pending-ID tracking) |
| `memory_transport.coax` | Bounded FIFO in-memory bidirectional message pipe + full end-to-end `McpClientCore` ↔ `McpServerCore` integration tests across `2026-07-28` and `2025-11-25` |

## Phase 2 Modules — Stdio & Child-Process Transports (`#effects(...)`)

| File | Responsibility |
| --- | --- |
| `stdio_frame.coax` | Pure `#authority` newline-delimited JSON-RPC framing & embedded CR/LF refusal |
| `stdio_server.coax` | Newline-delimited stdin/stdout MCP server transport (strict LF framing, oversized-line drain + `-32600`, parse-error `-32700`, `stderr`-only `mcp_stdio_log`, host reply & progress/log helpers) |
| `stdio_client.coax` | Child-process MCP stdio client over `coax/lib/sys/proc.coax` (owned handshake string stabilization, `server/discover` → `initialize` backward-compat probe, `notifications/cancelled`, graceful stdin-close → `SIGTERM` → `SIGKILL` shutdown) |

## Phase 3 Modules — Streamable HTTP Server (`#authority` wire + `#effects` transport)

| File | Responsibility |
| --- | --- |
| `http_wire.coax` | Pure `#authority` HTTP/1.1 request parser, `Content-Length` & `Transfer-Encoding: chunked` decoder, `Accept`/`Content-Type` negotiation, response & SSE `id`/`event`/`retry`/keep-alive/chunk writers |
| `http_server.coax` | Non-blocking `poll` Streamable HTTP MCP server over `coax/platform/socket-server-native.c` (`cxsrv_*`), concurrent SSE streams, `Origin` + `Host` DNS-rebinding `403` guard, session tracking, `GET`/`POST`/`DELETE /mcp` |

## Phase 4 Modules — Streamable HTTP + SSE Client (`#authority` wire + `#effects` transport)

| File | Responsibility |
| --- | --- |
| `http_client_wire.coax` | Pure `#authority` HTTP/1.1 URL parser, request serializer, response head parser, incremental chunked stream decoder (`McpChunkStream`), and incremental WHATWG SSE parser (`McpSseParser`) |
| `http_client.coax` | Streamable HTTP/1.1 + SSE MCP client over `coax/lib/net/transport.coax` (TCP + TLS): dual-era `server/discover` → `initialize` fallback, direct JSON & streaming SSE POST responses, standalone `GET` SSE stream for bidirectional `2025-11-25` server→client requests (with SEP-1034 elicitation defaults), SEP-1699 `retry:` + `Last-Event-ID` polling reconnect, and `2026-07-28` SEP-2243 `Mcp-Method`/`Mcp-Name` headers |

## Phase 5 Modules — OAuth 2.1 Client & Resource Server (`#authority` wire + `#effects` adapters)

| File | Responsibility |
| --- | --- |
| `auth_wire.coax` | Pure `#authority` OAuth 2.1 wire codecs: percent-decoding, callback query parser (`code`/`state`/`iss`/`error`), PKCE `S256` authorize URL builder, RFC 7591 DCR JSON writer (with `refresh_token`), RFC 6749 §2.3.1 Basic header & token form writers, RFC 9728 PRM parser (with RFC 8707 resource mismatch guard) & JSON writer, token response parser, Bearer extractor, and scope/audience checks |
| `auth_client.coax` | OAuth 2.1 HTTP client driver wiring `auth_policy.coax`, `auth_wire.coax`, `http_client.coax`, and `coax/lib/workos/pkce.coax`: RFC 9728 PRM + RFC 8414/OIDC discovery, `2025-03-26` backcompat fallback, Pre-registered → CIMD → DCR priority, `client_secret_basic`/`client_secret_post`/`none` token auth, RFC 9207 `iss` verification, `offline_access` gating, and `401`/`403` `insufficient_scope` step-up with retry limit |
| `auth_server.coax` | OAuth 2.1 Resource Server middleware & RFC 9728 `/.well-known/oauth-protected-resource` responder over `http_server.coax`: constant-time opaque token & RS256 JWKS verification, RFC 8707 audience check, and RFC 6750 `401`/`403` `WWW-Authenticate` challenges |

Phase 2–5 interop & conformance harnesses live in [`tools/mcp-conformance/`](../../tools/mcp-conformance/) (`stdio-server.coax`, `stdio-client.coax`, `everything-server.coax`, `everything-client.coax`, `run.sh`).

## Verification

Run unit/property tests and mutation checks on every pure `#authority` module:

```bash
for f in version jsonrpc uri_template headers json_schema auth_policy \
         types event_store mrtr subscriptions primitives server_core client_core memory_transport \
         stdio_frame http_wire http_client_wire auth_wire; do
  ./coax/bin/coax test coax/lib/mcp/$f.coax --library
  ./coax/bin/coax mutate coax/lib/mcp/$f.coax --root-only --native --jobs 8
done
```
