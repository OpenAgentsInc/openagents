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
| `stdio_server.coax` | Newline-delimited stdin/stdout MCP server transport (strict LF framing, embedded CR/LF rejection, oversized-line drain + `-32600`, parse-error `-32700`, `stderr`-only `mcp_stdio_log`, host reply & progress/log helpers) |
| `stdio_client.coax` | Child-process MCP stdio client over `coax/lib/sys/proc.coax` (owned handshake string stabilization, `server/discover` → `initialize` backward-compat probe, `notifications/cancelled`, graceful stdin-close → `SIGTERM` → `SIGKILL` shutdown) |

Phase 2 interop harnesses live in [`tools/mcp-conformance/`](../../tools/mcp-conformance/) (`stdio-server.coax`, `stdio-client.coax`, `run.sh`).

## Verification

Run unit/property tests and mutation checks on every module:

```bash
for f in version jsonrpc uri_template headers json_schema auth_policy \
         types event_store mrtr subscriptions primitives server_core client_core memory_transport; do
  ./coax/bin/coax test coax/lib/mcp/$f.coax --library
  ./coax/bin/coax mutate coax/lib/mcp/$f.coax --root-only --native --jobs 8
done
```
