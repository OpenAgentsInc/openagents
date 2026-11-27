# Coax MCP Library Architecture, Gap Analysis & Build Plan

Part of [MCP in Coax](README.md).
Grounds the implementation of `coax/lib/mcp/` in the actual OpenAgents Coax codebase (`coax/lib/`, `coax/platform/`, `apps/`, `coder/`).

---

## 1. Existing OpenAgents Coax building blocks & MCP code

### 1.1 Reusable libraries already in `coax/lib/`

| Capability | Existing OpenAgents files | Notes & limits |
| --- | --- | --- |
| **JSON parse & write** | `coax/lib/json/json.coax` (`jv_*`, `jo_*`), `coax/lib/json/shape.coax` (`#authority`, `js_*`), `coax/lib/encoding/json-f64.coax` | Caller-owned node arrays (`JV_DEPTH = 64`), zero heap. `shape.coax` adds strict closed-object/duplicate-key/UTF-8 checks. Always check `jo.bad` before publishing output. |
| **SSE framing** | `coax/lib/gateway/sse.coax` (`gw_sse_init`, `gw_sse_line`, `gw_sse_frame`) | Zero-allocation WHATWG SSE line parser & frame formatter. Extend/wrap for `id:`, `retry:`, and keep-alive comments (`:\r\n`). |
| **Child process stdio** | `coax/lib/sys/proc.coax`, `coax/platform/process-native.c` | Spawns child processes with piped `stdin`/`stdout`/`stderr`, process-group isolation, timed `proc_line`, `proc_send`, `proc_close_stdin`, `proc_kill`. Ready for MCP stdio **client** transport. |
| **TCP/TLS `poll` server** | `coax/platform/socket-server-native.c` (`cxsrv_*`) | Non-blocking `poll`-based TCP/TLS server up to `CXSRV_MAX = 288` concurrent connections (`cxsrv_listen`, `cxsrv_poll`, `cxsrv_accept`, `cxsrv_read`, `cxsrv_write`, `cxsrv_flush`, `cxsrv_close`). |
| **HTTP/1.1 client & sockets** | `coax/lib/net/http.coax`, `coax/lib/net/transport.coax`, `coax/platform/{socket,tls}-native.c` | Synchronous HTTP/1.1 client with TLS and dechunking, but buffers full body up to 1 MiB (`HTTP_BODY_MAX`) — usable as-is for OAuth discovery/token JSON calls, **not** yet for streaming SSE responses. |
| **Crypto & encoding** | `coax/lib/crypto/` (`sha256`, `hmac`, `hkdf`, `aes_gcm`, `rsa_verify`, `p256_ecdsa`, `entropy`, `zeroize`), `coax/lib/encoding/` (`base64`, `fi-base64url`, `hex`) | Complete primitives for PKCE `S256`, `=?base64?...?=` header encoding, MRTR HMAC/AEAD `requestState` sealing, RS256/ES256 JWT verification, and DPoP proofs. |
| **OAuth helpers** | `coax/lib/workos/` (`pkce.coax`, `jwt.coax`, `url.coax`, `retry.coax`, `seal.coax`) | Pure `#authority` RFC 7636 PKCE (`S256` + unpadded `b64url_encode` + constant-time state compare), RS256 JWKS/JWT verifier, RFC 3986 percent-encoder, retry policy, and authenticated sealing. Extract/share or mirror into shared helpers. |
| **Bounded storage** | `coax/lib/bounded.coax`, `coax/lib/sequence.coax` | Fixed-capacity caller-owned collections with explicit refusal; ideal for pending request maps, subscription tables, and bounded `EventStore` ring buffers. |

### 1.2 Existing ad-hoc MCP code in this repo (to migrate)

There is currently **no** shared `coax/lib/mcp/` library, but **5** ad-hoc stdio/in-process MCP implementations exist — all tools-only and missing resources, prompts, MRTR, Streamable HTTP, and OAuth:

| File | Current role | Protocol version | Migration target |
| --- | --- | --- | --- |
| `apps/openagents-cli/mcp.coax` (719 LOC) | `openagents mcp` (Realm CLI + device/lean dispatch) | `2025-06-18` | Migrate onto `coax/lib/mcp/` stdio server + `McpToolRegistry` |
| `apps/openagents-cli/executor-mcp.coax` (286 LOC) | `openagents mcp --domains executor` (5 tools) | `2025-06-18` | Migrate onto `coax/lib/mcp/` tool provider |
| `coder/tui/oasim-mcp.coax` (267 LOC) | `oasim mcp` (21 iOS Simulator tools) | `2025-06-18` | Migrate onto `coax/lib/mcp/` tool provider |
| `coder/tui/lean-run-mcp.coax` | Lean verification tools over stdio | `2025-06-18` | Migrate onto `coax/lib/mcp/` tool provider |
| `coax/lib/agent/claude/mcp.coax` (165 LOC) | In-process SDK MCP shim for Claude Code CLI | `2025-11-25` | Migrate onto `coax/lib/mcp/` `InMemoryTransport` / dispatcher |

---

## 2. Explicit gap analysis for 100% compliance

Six concrete gaps separate today's `coax/lib/` from passing 100% of `@modelcontextprotocol/conformance` across both `2025-11-25` and `2026-07-28`:

### Gap 1 — Streaming HTTP/1.1 Server + Streamable HTTP / SSE server transport
- **Current state:** `socket-server-native.c` gives raw TCP/TLS `poll` I/O and `gateway/sse.coax` formats SSE frames, but there is no reusable Coax HTTP/1.1 server capable of header parsing, `Accept`/`Content-Type` negotiation, `Origin` validation (`403 Forbidden`), `Content-Length` + chunked request decoding, and long-lived SSE responses with disconnect detection.
- **Design:**
  - Pure `#authority` HTTP/1.1 parser & response/header serializer (`coax/lib/net/http_server_wire.coax` or `coax/lib/mcp/http_wire.coax`): strict RFC 9110 request-line/header parsing, case-insensitive header lookup, `Origin`/`Host` allowlist check (DNS rebinding protection), chunked framing, body size caps.
  - Non-blocking `poll`-loop HTTP server driver over `cxsrv_*` supporting concurrent long-lived SSE connections (`subscriptions/listen` in `2026-07-28`, POST/GET SSE streams in `2025-11-25`), `X-Accel-Buffering: no`, periodic `:\r\n` keep-alives, and immediate disconnect detection (triggering request cancellation in `2026-07-28`).
  - Bounded `EventStore` ring buffer (byte-capped, deduplicated via `replayedEventIds` set) for `2025-11-25` `Last-Event-ID` & SEP-1699 `retry:` polling.

### Gap 2 — Incremental Streaming HTTP/1.1 + SSE Client
- **Current state:** `coax/lib/net/http.coax` blocks until the full HTTP response arrives (capped at `HTTP_BODY_MAX = 1 MiB`) and cannot yield SSE events incrementally while a request is in flight.
- **Design:** Build a stateful, caller-buffer-owned streaming HTTP/1.1 client (`coax/lib/net/http_stream.coax`) over `coax/lib/net/transport.coax` that parses status + headers first, inspects `Content-Type`, and either reads a single `application/json` body OR pumps chunked/identity bytes incrementally through `gw_sse_line` to deliver JSON-RPC progress notifications, subscription events, or `2025-11-25` server-initiated requests on the fly — and exposes explicit `close()` so `2026-07-28` client cancellation closes the TCP/TLS stream cleanly.

### Gap 3 — Bounded JSON Schema Draft 2020-12 validator & `x-mcp-header` analyzer
- **Current state:** `coax/lib/json/shape.coax` only checks flat closed objects and scalars.
- **Design:** Pure `#authority` module (`coax/lib/mcp/json_schema.coax`) operating directly on borrowed `Jv` trees:
  - Preserves `$schema`, `$defs`, `$ref` verbatim without dereferencing (`json-schema-2020-12-preservation`, `json-schema-ref-no-deref`).
  - Validates Draft 2020-12 core keywords needed for MCP tool `inputSchema`/`outputSchema` and elicitation form schemas (`type`, `properties`, `required`, `additionalProperties`, `items`, `enum`, `const`, `oneOf`/`anyOf`/`allOf`/`not`, `minimum`/`maximum`/`exclusive*`/`multipleOf`, `minLength`/`maxLength`/`pattern` bounded subset, `minItems`/`maxItems`/`uniqueItems`, `default`, `format` metadata, `$defs`/`$ref` with cycle & depth cap).
  - Implements the exact `2026-07-28` `x-mcp-header` static analyzer: validates `1*tchar` header name, case-insensitive uniqueness, primitive-only type (`string`/`integer` safe range/`boolean`; rejects `number`), and strict `properties`-only static reachability (rejects `items`, combinators, `$ref`) so clients automatically exclude invalid tools (`http-invalid-tool-headers`).

### Gap 4 — General URI parser + RFC 6570 URI Templates
- **Current state:** `coax/lib/net/uri.coax` is intentionally restricted to DNS `http`/`https`/`wss` URLs without IP literals or fragments.
- **Design:** Pure `#authority` module (`coax/lib/mcp/uri_template.coax`):
  - General RFC 3986 URI parser supporting arbitrary MCP resource schemes (`file://`, `memo://`, `test://`, `https://`, `http://localhost:<port>`, IPv4/IPv6 loopback for local conformance servers), canonicalization for RFC 8707 `resource` indicators (lowercase scheme/host, strip fragment), and path-segment extraction for RFC 9728/8414 well-known URL derivation.
  - RFC 6570 Level 1–3 URI Template expander and reverse matcher (matching concrete resource URIs against registered `resources/templates/list` patterns and extracting template variables).

### Gap 5 — MCP OAuth 2.1 Client & Resource Server layer
- **Current state:** `coax/lib/workos/` provides PKCE `S256`, RS256 JWT/JWKS, URL percent-encoding, retry policy, and sealing — but lacks RFC 9728, RFC 8414/OIDC discovery, RFC 8707, RFC 9207, CIMD, DCR, and `WWW-Authenticate` step-up parsing.
- **Design:** Pure `#authority` OAuth state machine + parsers (`coax/lib/mcp/auth_*.coax`), driven by `coax/lib/net/http.coax` (with loopback redirect listener on `cxsrv_*` for interactive auth code capture):
  - `WWW-Authenticate` RFC 7235/6750 challenge parser (`error`, `scope`, `resource_metadata`, `error_description`).
  - Exact RFC 9728 PRM discovery sequence + `resource` match guard (`auth/resource-mismatch`).
  - Exact RFC 8414 & OIDC Discovery sequence + exact `issuer` equality guard + mandatory `code_challenge_methods_supported` `S256` gate.
  - Client registration priority chain: pre-registered → CIMD → DCR fallback (`application_type`).
  - RFC 8707 `resource` injection on auth + token requests; RFC 9207 exact `iss` callback verification (`2026-07-28`); `client_secret_basic` / `client_secret_post` / `none` token auth; `offline_access` gating; `403 insufficient_scope` union step-up with retry counter; AS migration handling.
  - Server-side `Bearer` token validator + audience check + PRM `/.well-known/oauth-protected-resource` responder + `401`/`403` `WWW-Authenticate` emitter.

### Gap 6 — Unified `coax/lib/mcp/` Protocol Core, Client/Server APIs & Conformance Harnesses
- Pure `#authority` protocol codecs, state machines, MRTR HMAC/AEAD `requestState` sealer, and high-level `McpServer` / `McpClient` APIs supporting both `2025-11-25` and `2026-07-28`, plus `tools/mcp-conformance/` binaries for CI verification.

---

## 3. Proposed module layout (`coax/lib/mcp/`)

Following OpenAgents Coax architecture (`coax/lib/workos/`, `coax/LIBRARIES.md`), **pure `#authority` protocol modules** are strictly separated from **thin `#effects(...)` I/O adapters**:

```text
coax/lib/mcp/
├── README.md                  # Module contracts, buffer limits, verification commands
│
├── # Pure #authority Sans-I/O Core (zero heap, 0 surviving mutants)
├── version.coax               # MCP_LATEST ("2026-07-28"), MCP_LATEST_WITH_INITIALIZE ("2025-11-25"), negotiation, has_initialize()
├── jsonrpc.coax               # JSON-RPC 2.0 envelope parser/writer, ID handling, batch rejection, all error codes (-32700..-32022/-32042)
├── types.coax                 # Capabilities, ClientInfo/ServerInfo, Content blocks, Annotations, Icons, CacheableResult, Meta keys
├── headers.coax               # 2026-07-28 Mcp-Method, Mcp-Name, Mcp-Param-*, =?base64?...?= codec, Origin/Host DNS-rebinding policy
├── mrtr.coax                  # SEP-2322 InputRequiredResult codec + HMAC-SHA256/AEAD requestState sealing & tamper verification
├── primitives.coax            # Tools, Resources, ResourceTemplates, Prompts, Completions, Elicitation (form+URL), Sampling, Roots, Logging
├── subscriptions.coax         # 2026-07-28 subscriptions/listen + acknowledged + subscriptionId filtering; 2025-11-25 resources/subscribe
├── json_schema.coax           # Bounded JSON Schema Draft 2020-12 validator + SEP-2243 x-mcp-header static analyzer
├── uri_template.coax          # General RFC 3986 URI parser/canonicalizer + RFC 6570 template expander/matcher
├── http_wire.coax             # Pure HTTP/1.1 request/response parser, chunked decoder/encoder, SSE event id/retry/keepalive framing
├── event_store.coax           # Bounded byte-capped SSE EventStore ring buffer + replay deduplication (2025-11-25)
├── auth_policy.coax           # Pure OAuth 2.1 state machine: WWW-Authenticate, RFC 9728/8414/OIDC URLs, issuer/resource/iss checks, step-up scope union, CIMD/DCR
├── server_core.coax           # Sans-I/O McpServer state machine & dispatcher (dual 2025 stateful / 2026 stateless, registries, pagination, caching)
├── client_core.coax           # Sans-I/O McpClient state machine (discover/initialize probing, pending IDs, MRTR retry loop, progress/cancel)
├── memory_transport.coax      # Pure in-memory bidirectional message pipe for compile-time #test client↔server tests
│
├── # Thin I/O Transport Adapters (explicit #effects(...))
├── stdio_server.coax          # Newline-delimited stdin/stdout server loop (never leaks logs to stdout; stderr helper)
├── stdio_client.coax          # Child-process stdio client over coax/lib/sys/proc.coax
├── http_server.coax           # Streamable HTTP server over coax/platform/socket-server-native.c (concurrent SSE, 2025 + 2026 routes, PRM endpoint)
├── http_client.coax           # Streamable HTTP client over streaming HTTP/1.1 + SSE client (+ legacy 2024-11-05 HTTP+SSE fallback)
├── auth_client.coax           # OAuth 2.1 HTTP driver + localhost callback listener wiring auth_policy.coax + coax/lib/workos/pkce.coax
├── auth_server.coax           # Resource-server Bearer/JWT/audience middleware + PRM responder
│
└── ext/                       # Optional official MCP extensions (Phase 6)
    ├── tasks.coax             # io.modelcontextprotocol/tasks (SEP-2663)
    ├── skills.coax            # io.modelcontextprotocol/skills (SEP-2640)
    ├── auth_ext.coax          # ext-auth: client_credentials (Basic/JWT), DPoP, enterprise managed auth
    ├── server_card.coax       # SEP-2127 GET /server-card metadata
    └── apps.coax              # io.modelcontextprotocol/ui (SEP-1865)

tools/mcp-conformance/         # Official @modelcontextprotocol/conformance harnesses
├── everything-server.coax     # Coax Streamable HTTP conformance server (2025-11-25 + 2026-07-28)
├── everything-client.coax     # Coax Streamable HTTP + OAuth conformance client (2025-11-25 + 2026-07-28)
└── run.sh                     # Builds both binaries & executes @modelcontextprotocol/conformance --requirements 2025-11-25 & 2026-07-28
```

### Key Coax storage, safety & API invariants

1. **Zero heap allocation in `#authority` core:** Every parser borrows caller-owned input slices and `JvNode` buffers; every encoder writes into caller-owned `[] u8` output buffers and checks capacity before writing (`jo.bad` checked before publishing).
2. **Explicit bounded limits at transport boundaries:**
   - Default max JSON-RPC message / stdio line / HTTP body size: configurable, defaulting to **16 MiB** (matching `go-sdk`), with strict refusal (HTTP `413` or JSON-RPC `-32600`) rather than truncation or OOM.
   - Fixed upper bounds on `JV_DEPTH` (64), pending in-flight requests per connection, SSE `EventStore` bytes, active subscriptions, and OAuth scope step-up retries (default max 2 retries, preventing infinite `403` loops).
3. **Dual-era version guard:**
   - `MCP_LATEST :: "2026-07-28"`
   - `MCP_LATEST_WITH_INITIALIZE :: "2025-11-25"`
   - `mcp_version_has_initialize(v)` returns `true` iff `v < "2026-07-28"`.
   - `initialize` handlers reject or clamp `2026-07-28` down to `2025-11-25`; `2026-07-28` requests missing `_meta["io.modelcontextprotocol/protocolVersion"]` or mismatched with `MCP-Protocol-Version` fail immediately with `-32020`.
4. **MRTR `requestState` integrity:** Sealed using HMAC-SHA256 (or AES-256-GCM via `coax/lib/crypto/aes_gcm.coax` / `workos/seal.coax`) with per-server secret key, binding `(method, request_digest, principal, expiry_ms, state_payload)`. Any tampered or expired `requestState` returns JSON-RPC `-32602` (`InvalidParams`).
5. **Effect auditing:** Every `#authority` file has zero effects; every transport adapter declares tight `#effects(...)` verified via `./coax/bin/coax deps FILE --effects`.

---

## 4. Verification & 100% compliance strategy

We combine **four independent verification layers**:

1. **Compile-time unit & property tests (`#test` / `#forall`):**
   - `./coax/bin/coax test coax/lib/mcp/<file>.coax --library`
   - Includes golden round-trip tests generated/imported from `~/work/projects/repos/mcp/modelcontextprotocol/schema/2026-07-28/examples/` and `schema/2025-11-25/` plus adversarial malformed-input fuzzing (truncated JSON, duplicate keys, invalid UTF-8, header injection `\r\n`, `=?base64?` literal escaping, tampered MRTR `requestState`, DNS rebinding `Origin`, OAuth `iss`/audience/resource mismatch).
2. **Mutation testing (0 surviving mutants on `#authority`):**
   - `./coax/bin/coax mutate coax/lib/mcp/<file>.coax --library`
   - Matches OpenAgents' zero-survivor standard established on `coax/lib/workos/` and `coax/lib/bounded.coax`.
3. **In-memory & stdio integration tests:**
   - Full `McpClient` ↔ `McpServer` end-to-end runs over `memory_transport.coax` and `stdio_client.coax` ↔ `stdio_server.coax` covering both `2025-11-25` and `2026-07-28`, multi-round MRTR, subscriptions, cancellation, and progress streaming.
4. **Authoritative `@modelcontextprotocol/conformance` CI gate (`tools/mcp-conformance/run.sh`):**
   - Runs all four scored conformance legs with **zero `--expected-failures`**:
     - `conformance server --url http://127.0.0.1:<port>/mcp --requirements 2025-11-25` (**30/30**)
     - `conformance server --url http://127.0.0.1:<port>/mcp --requirements 2026-07-28` (**37/37**)
     - `conformance client --command ./bin/mcp-conformance-client --requirements 2025-11-25` (**18/18**)
     - `conformance client --command ./bin/mcp-conformance-client --requirements 2026-07-28` (**32/32**)
   - Every captured wire message is automatically checked by the conformance runner's JSON Schema validator (`wire-schema-valid`).

---

## 5. Detailed phased build plan

### Phase 0 — Pure `#authority` Foundations
- **Deliverables:** `uri_template.coax`, `json_schema.coax` (Draft 2020-12 + `x-mcp-header` analyzer), `headers.coax` (`Mcp-Method`/`Mcp-Name`/`Mcp-Param-*` + `=?base64?...?=` + `Origin`/`Host` validator), `auth_policy.coax` (`WWW-Authenticate`, RFC 9728, RFC 8414/OIDC, RFC 8707, RFC 9207, CIMD, DCR, step-up scope union).
- **Gate:** `coax test` + `coax mutate` with 0 survivors on every module.

### Phase 1 — Sans-I/O Protocol Core (`2025-11-25` + `2026-07-28`)
- **Deliverables:** `version.coax`, `jsonrpc.coax`, `types.coax`, `mrtr.coax` (with authenticated `requestState`), `primitives.coax`, `subscriptions.coax`, `event_store.coax`, `server_core.coax`, `client_core.coax`, `memory_transport.coax`.
- **Gate:** Full in-memory client↔server tests across all 21 `2026-07-28` methods and all 31 `2025-11-25` methods, golden checks against official `schema/2026-07-28/examples/`, 0 surviving mutants.

### Phase 2 — Stdio & Child-Process Transports + Repo Migration (Part 1)
- **Deliverables:** `stdio_server.coax`, `stdio_client.coax` (built on `coax/lib/sys/proc.coax`), `server/discover` → `initialize` backward-compat probe, `notifications/cancelled` over stdio, and `tools/mcp-conformance/` skeleton.
- **Gate:** Coax stdio client interoperates cleanly with official `@modelcontextprotocol/server-everything` (from `~/work/projects/repos/mcp/servers`) and Coax stdio server passes `@modelcontextprotocol/inspector --cli` checks; `coax/lib/agent/claude/mcp.coax` and `apps/openagents-cli/mcp.coax` ready for cutover.

### Phase 3 — Streaming HTTP/1.1 Server & Streamable HTTP Server Conformance
- **Deliverables:** `http_wire.coax`, `http_server.coax` (on `socket-server-native.c`), `tools/mcp-conformance/everything-server.coax` implementing all 2025 & 2026 server conformance fixtures (tools, image/audio/embedded/mixed content, errors, progress, resources, binary resources, URI templates, prompts, completions, MRTR 14 scenarios, caching, multi-stream SSE, SEP-1699 polling, DNS rebinding `403`, stateless header validation).
- **Gate:** **100% pass** on `@modelcontextprotocol/conformance server --requirements 2025-11-25` (30/30) and `--requirements 2026-07-28` (37/37).

### Phase 4 — Incremental Streaming HTTP/1.1 + SSE Client & Non-Auth Client Conformance
- **Deliverables:** `coax/lib/net/http_stream.coax` (incremental chunked/SSE HTTP/1.1 client), `http_client.coax` (Streamable HTTP client with `Mcp-Method`/`Mcp-Name`/`x-mcp-header`, MRTR loop, disconnect cancellation, `2025-11-25` session/SSE-retry/server-requests, optional `2024-11-05` HTTP+SSE fallback), `tools/mcp-conformance/everything-client.coax`.
- **Gate:** **100% pass** on all non-auth client conformance scenarios for `2025-11-25` and `2026-07-28` (`initialize`, `tools_call`, `sse-retry`, `elicitation-sep1034-client-defaults`, `request-metadata`, `sep-2322-client-request-state`, `http-standard-headers`, `http-custom-headers`, `http-invalid-tool-headers`, `json-schema-2020-12-preservation`, `json-schema-ref-no-deref`).

### Phase 5 — Full OAuth 2.1 Client & Resource Server Conformance
- **Deliverables:** `auth_client.coax` (HTTP discovery + localhost callback server + PKCE + CIMD + DCR + token storage hooks + step-up retry + AS migration), `auth_server.coax` (Bearer JWT/introspection + RFC 8707 audience check + RFC 9728 PRM endpoint + `401`/`403` `WWW-Authenticate`).
- **Gate:** **100% pass** on **all** `@modelcontextprotocol/conformance client` scenarios for `--requirements 2025-11-25` (18/18) and `--requirements 2026-07-28` (32/32), plus `2025-03-26` OAuth backcompat scenarios — **zero expected failures**.

### Phase 6 — Official Extensions (`coax/lib/mcp/ext/`)
- **Deliverables:** `tasks.coax` (`io.modelcontextprotocol/tasks`), `skills.coax` (`io.modelcontextprotocol/skills`), `auth_ext.coax` (`client_credentials` Basic/JWT, DPoP + nonce, enterprise managed authorization), `server_card.coax`, `apps.coax`.
- **Gate:** Passing conformance extension scenarios (`tasks-*`, `sep-2640-*`, `auth/client-credentials-*`, `auth/dpop*`, `auth/enterprise-managed-authorization`).

### Phase 7 — Product Integration & Repo Cleanup
- **Deliverables:** Migrate all 5 existing ad-hoc MCP implementations (`apps/openagents-cli/{mcp,executor-mcp}.coax`, `coder/tui/{oasim,lean-run}-mcp.coax`, `coax/lib/agent/claude/mcp.coax`) onto `coax/lib/mcp/`; expose MCP client manager + IDE config importer for OpenAgents Desktop / Coder / CLI hosts; register `coax/lib/mcp/` in `coax/LIBRARIES.md`; wire `tools/mcp-conformance/run.sh` into CI.
- **Gate:** Zero ad-hoc JSON-RPC MCP framing left in the repository; green CI conformance gate on both `2025-11-25` and `2026-07-28`.

### Phase 8 — Unified MCP Tool Gateway (`coax/lib/mcp/gateway/`)
- **Deliverables:** Layered cleanly *above* `coax/lib/mcp/` per [unified-tool-gateway-spec.md](unified-tool-gateway-spec.md): `#authority` grant/approval/egress policies, Vault-isolated reusable accounts, SWR tool catalog, 4-mode elicitation & upstream elicitation forwarding (`mrtr`/`native`/`ui`/`model`), deterministic Code-Mode (`execute`) powered by `coax/lib/rlm/script.coax`, and the unified `openagents gateway` MCP endpoint.
- **Gate:** 0 surviving mutants on `#authority` gateway policy modules, full pause/resume E2E across all 4 elicitation modes, green `emulate` OAuth+MCP integration suite, and zero credential exposure in sandbox/wire traces.
