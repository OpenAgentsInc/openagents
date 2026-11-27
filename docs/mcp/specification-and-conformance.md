# MCP Normative Specification Breakdown & Conformance Matrix

Part of [MCP in Coax](README.md).
Distilled directly from `~/work/projects/repos/mcp/modelcontextprotocol` (`docs/specification/`, `schema/`) and `~/work/projects/repos/mcp/conformance` (`requirements/`, `src/scenarios/`).

---

## 1. Specification revision timeline

| Version | Key normative changes |
| --- | --- |
| **`2024-11-05`** | Initial release: JSON-RPC 2.0, `initialize`/`notifications/initialized`, `stdio` + `HTTP+SSE` transports, Tools, Resources, Prompts, Roots, Sampling, Logging, Completions, Pagination. |
| **`2025-03-26`** | Added OAuth 2.1 framework; replaced HTTP+SSE with **Streamable HTTP**; added JSON-RPC batching; added tool annotations, audio content, `message` on progress notifications, explicit `completions` capability. |
| **`2025-06-18`** | **Removed JSON-RPC batching** (PR #416); added structured tool output (`outputSchema`, `structuredContent`) & `resource_link`; reclassified servers as OAuth Resource Servers with **RFC 9728** PRM + mandatory **RFC 8707** Resource Indicators; added **Elicitation** (form mode); required `MCP-Protocol-Version` header; added `title` display names & completion `context`. |
| **`2025-11-25`** *(latest stateful)* | Added OIDC Discovery 1.0, **CIMD** (SEP-991), `WWW-Authenticate` incremental scope step-up (SEP-835), RFC 9728 `.well-known` fallback (SEP-985); **URL mode elicitation** (SEP-1036, error `-32042`), titled/untitled enum schemas (SEP-1330), primitive defaults (SEP-1034); **tool calling in Sampling** (SEP-1577); **icons** (SEP-973); input validation errors as Tool Execution Errors (SEP-1303); experimental Tasks (SEP-1686); **JSON Schema 2020-12** default dialect (SEP-1613); SSE polling via server disconnect + `retry:` (SEP-1699); `403 Forbidden` for invalid `Origin`. |
| **`2026-07-28`** *(current GA)* | **Major stateless redesign**: removed `Mcp-Session-Id` (SEP-2567) and `initialize`/`notifications/initialized` (SEP-2575); added per-request `_meta["io.modelcontextprotocol/*"]` and mandatory **`server/discover`**; replaced HTTP `GET` + `resources/subscribe` with **`subscriptions/listen`** (POST SSE + `notifications/subscriptions/acknowledged`); removed `ping`, `logging/setLevel`, `notifications/roots/list_changed`, `notifications/elicitation/complete`, and `Last-Event-ID`; introduced **Multi Round-Trip Requests (MRTR)** (`resultType: "complete" \| "input_required"`, `inputRequests`, `requestState`, `inputResponses`, SEP-2322); moved Tasks to extension `io.modelcontextprotocol/tasks` (SEP-2663); required **`Mcp-Method`**, **`Mcp-Name`**, and **`x-mcp-header`** (`Mcp-Param-*`) HTTP headers (SEP-2243); added `CacheableResult` (`ttlMs`, `cacheScope`, SEP-2549); formalized error codes `-32020..-32022` & changed resource-not-found to `-32602` (SEP-2164); loosened `inputSchema`/`outputSchema` to full JSON Schema 2020-12 & `structuredContent` to any JSON value (SEP-2106); **deprecated** Roots, Sampling, Logging (SEP-2577), HTTP+SSE, `includeContext` `"thisServer"`/`"allServers"`, and OAuth DCR (RFC 7591). |
| **`draft`** | Post-`2026-07-28` updates: bumps CIMD ref to `draft-ietf-oauth-client-id-metadata-document-03`; restores optional `_meta` on `CreateMessageRequestParams`, `ElicitRequestFormParams`, `ElicitRequestURLParams`, `ElicitResult`, `ListRootsResult` (#3468). |

---

## 2. Side-by-side wire comparison: `2025-11-25` vs `2026-07-28`

| Dimension | `2025-11-25` (Stateful) | `2026-07-28` (Stateless GA) |
| --- | --- | --- |
| **Handshake & discovery** | Mandatory `initialize` → `InitializeResult` → `notifications/initialized`. | No handshake. Every request carries `_meta["io.modelcontextprotocol/protocolVersion"]`, `_meta["io.modelcontextprotocol/clientCapabilities"]`, and SHOULD carry `_meta["io.modelcontextprotocol/clientInfo"]`. Servers **MUST** implement `server/discover`; results SHOULD carry `_meta["io.modelcontextprotocol/serverInfo"]`. |
| **Session header** | `MCP-Session-Id` issued on `InitializeResult`, required on subsequent HTTP requests (`400` missing, `404` expired → re-initialize). | Removed (`Mcp-Session-Id` ignored if received). |
| **HTTP methods** | `POST` (messages), `GET` (standalone SSE stream, `405` if unsupported), `DELETE` (terminate session, `405` if unsupported). | `POST` only; `GET` and `DELETE` return `405 Method Not Allowed` on a `2026-07-28`-only server. |
| **HTTP request headers** | `Accept: application/json, text/event-stream`, `Content-Type: application/json`, `MCP-Protocol-Version` (missing defaults to `2025-03-26`), `MCP-Session-Id`, `Last-Event-ID`. | `Accept: application/json, text/event-stream`, `Content-Type: application/json`, `MCP-Protocol-Version` (MUST match `_meta`), **`Mcp-Method`** (required on all requests), **`Mcp-Name`** (required on `tools/call`, `resources/read`, `prompts/get`), **`Mcp-Param-<Name>`** (for `x-mcp-header` tool params). Mismatch/missing → `400` + JSON-RPC `-32020`. |
| **Origin / DNS rebinding** | Validate `Origin`; invalid → `403 Forbidden`. Bind local servers to `127.0.0.1`. | Same: invalid `Origin` → `403 Forbidden`; bind local servers to `127.0.0.1`. |
| **SSE resumability & polling** | Server primes stream with event `id` + empty `data`; MAY disconnect with `retry:` ms (SEP-1699); client reconnects via `GET` + `Last-Event-ID`. | Removed (`Last-Event-ID` ignored). Keep-alive SSE comments (`:\r\n`) and `X-Accel-Buffering: no` recommended. |
| **Server → Client requests** | Server MAY send JSON-RPC requests (`roots/list`, `sampling/createMessage`, `elicitation/create`) on POST or GET SSE streams; client POSTs JSON-RPC response (`202 Accepted`). | **Forbidden** on SSE/stdio. Replaced by **MRTR** `InputRequiredResult` on `tools/call`, `resources/read`, `prompts/get`. |
| **Subscriptions** | `resources/subscribe` & `resources/unsubscribe`; notifications delivered on GET SSE stream or stdio. | **`subscriptions/listen`** (POST SSE stream with `params.notifications` filter). First message MUST be `notifications/subscriptions/acknowledged` with `_meta["io.modelcontextprotocol/subscriptionId"]` = request `id`. Graceful close sends `{ resultType: "complete", _meta: { ... } }`. |
| **Cancellation** | Client SHOULD send `notifications/cancelled` (`{ requestId, reason? }`); HTTP disconnect SHOULD NOT cancel. | **Streamable HTTP:** closing the SSE stream **MUST** cancel; client sends no `notifications/cancelled`. **stdio:** client **MUST** send `notifications/cancelled`. |
| **Result discriminator** | None (flat result object). | Every result MUST include `resultType`: `"complete"` or `"input_required"` (or negotiated extension type). |
| **Logging** | `logging/setLevel` + `notifications/message` (RFC 5424 levels). | Per-request `_meta["io.modelcontextprotocol/logLevel"]` + `notifications/message` (**deprecated** per SEP-2577). |
| **Health check** | `ping` request (`{}` result). | Removed. |
| **Caching** | None. | Optional `CacheableResult` (`ttlMs`, `cacheScope: "public" \| "private"`) on `server/discover`, `tools/list`, `prompts/list`, `resources/list`, `resources/read`, `resources/templates/list`. |
| **Resource not found error** | `-32002` (implementation/legacy). | `-32602` (`InvalidParams`, SEP-2164). |

---

## 3. Complete RPC method & error code inventory

### 3.1 JSON-RPC 2.0 wire rules & error codes

- **Framing:** Single JSON-RPC 2.0 request, response, or notification per message (JSON-RPC **batching is prohibited** since `2025-06-18`). Request IDs are non-null `string | integer`, never reused within a session/connection.
- **Standard JSON-RPC 2.0 error codes (both versions):**
  - `-32700` `PARSE_ERROR`
  - `-32600` `INVALID_REQUEST`
  - `-32601` `METHOD_NOT_FOUND` (in `2026-07-28` HTTP: also return HTTP `404 Not Found`)
  - `-32602` `INVALID_PARAMS` (in `2026-07-28`: also used for Resource Not Found, SEP-2164)
  - `-32603` `INTERNAL_ERROR`
- **MCP-specific error codes:**
  - `2025-11-25`: `-32042` `URL_ELICITATION_REQUIRED` (server requires out-of-band URL elicitation before proceeding); `-32002` historically used for resource not found.
  - `2026-07-28` (allocation policy: `-32000..-32019` legacy/impl-defined, `-32020..-32099` MCP spec):
    - `-32020` `HEADER_MISMATCH` (HTTP `400`: missing/mismatched `MCP-Protocol-Version`, `Mcp-Method`, `Mcp-Name`, `Mcp-Param-*`, or invalid header chars)
    - `-32021` `MISSING_REQUIRED_CLIENT_CAPABILITY`
    - `-32022` `UNSUPPORTED_PROTOCOL_VERSION` (HTTP `400`, with `data: { supported: string[], requested: string }`)

### 3.2 RPC methods (`2026-07-28` vs `2025-11-25`)

| Category | `2026-07-28` (21 total) | Additional in `2025-11-25` (31 total) |
| --- | --- | --- |
| **Discovery / Lifecycle** | `server/discover` | `initialize`, `notifications/initialized`, `ping` |
| **Tools** | `tools/list`, `tools/call`, `notifications/tools/list_changed` | *(same)* |
| **Resources** | `resources/list`, `resources/read`, `resources/templates/list`, `notifications/resources/list_changed`, `notifications/resources/updated` | `resources/subscribe`, `resources/unsubscribe` |
| **Prompts** | `prompts/list`, `prompts/get`, `notifications/prompts/list_changed` | *(same)* |
| **Subscriptions** | `subscriptions/listen`, `notifications/subscriptions/acknowledged` | *(none — uses `resources/subscribe` + GET SSE)* |
| **Utilities** | `completion/complete`, `notifications/progress`, `notifications/cancelled` *(stdio only)*, `notifications/message` *(deprecated)* | `logging/setLevel`, `notifications/cancelled` *(stdio + HTTP)* |
| **Client Primitives** | *(MRTR `InputRequests` only)*: `elicitation/create` *(active)*, `sampling/createMessage` *(deprecated)*, `roots/list` *(deprecated)* | *(Server→Client JSON-RPC requests)*: `elicitation/create`, `sampling/createMessage`, `roots/list`, `notifications/roots/list_changed`, `notifications/elicitation/complete` |
| **Tasks** | Moved to extension `io.modelcontextprotocol/tasks` (`ext-tasks`) | Experimental core: `tasks/get`, `tasks/result`, `tasks/list`, `tasks/cancel`, `notifications/tasks/status` |

---

## 4. Deep normative details critical for Coax implementation

### 4.1 `2026-07-28` HTTP header mirroring & `x-mcp-header` (`SEP-2243`)

- **Required headers on POST:** `MCP-Protocol-Version` (must match `_meta["io.modelcontextprotocol/protocolVersion"]`), `Mcp-Method` (mirrors `method`), and `Mcp-Name` (required for `tools/call` → `params.name`, `resources/read` → `params.uri`, `prompts/get` → `params.name`). Header names are case-insensitive; values are case-sensitive.
- **Custom parameter headers (`x-mcp-header`):**
  - Tool `inputSchema` properties may carry `"x-mcp-header": "<Name>"`, causing Streamable HTTP clients (**MUST** support) to mirror the argument into `Mcp-Param-<Name>`.
  - Strict schema validity rules (clients **MUST** exclude invalid tools from `tools/list`!):
    1. `<Name>` is non-empty RFC 9110 `1*tchar` (no CR/LF/control chars), case-insensitively unique within the tool's `inputSchema`.
    2. Property type MUST be a primitive `string`, `integer` (within JS safe integer range `-(2^53-1)..(2^53-1)`), or `boolean` — **`number` is forbidden**.
    3. Property MUST be statically reachable strictly via a chain of `properties` keys (forbidden under `items`, `oneOf`/`anyOf`/`allOf`/`not`, `if`/`then`/`else`, or `$ref`).
  - Absent or `null` arguments omit the header.
- **Value encoding (applies to `Mcp-Name` and `Mcp-Param-<Name>`):**
  - If the value contains only visible ASCII (`0x21..0x7E`), space (`0x20`), or tab (`0x09`), has **no leading or trailing whitespace**, and does **not** literally match the sentinel pattern `=?base64?...?=`, send it raw.
  - Otherwise, encode UTF-8 bytes as Base64 wrapped in the case-sensitive sentinel: `=?base64?{Base64}?=`.

### 4.2 Multi Round-Trip Requests — MRTR (`SEP-2322`, `2026-07-28`)

- Allowed **only** on `tools/call`, `resources/read`, and `prompts/get`.
- When a server needs client input (elicitation, sampling, roots), it returns a normal JSON-RPC response with:
  ```json
  {
    "resultType": "input_required",
    "inputRequests": {
      "req_1": { "method": "elicitation/create", "params": { ... } }
    },
    "requestState": "<opaque-server-token>"
  }
  ```
- At least one of `inputRequests` or `requestState` MUST be present. Servers MUST NOT emit input request types absent from `_meta["io.modelcontextprotocol/clientCapabilities"]` (return `-32021` `MISSING_REQUIRED_CLIENT_CAPABILITY` if required and missing).
- **Security invariant on `requestState`:** If `requestState` affects auth, access control, or business logic, the server **MUST** integrity-protect it (HMAC-SHA256 or AEAD — tampering is explicitly tested by `input-required-result-tampered-state`!) and SHOULD bind principal, short TTL, and originating request digest.
- **Client follow-up:** Sends a **new** JSON-RPC request with a **new `id`**, original params, `inputResponses: { "req_1": <result> }`, and the exact unmodified `requestState` (if provided; MUST NOT inspect, modify, or fabricate it). Supports multiple rounds (`input-required-result-multi-round`).

### 4.3 `stdio` transport rules (both versions)

- Newline-delimited UTF-8 JSON-RPC messages over `stdin`/`stdout` (**no embedded `\n` or `\r`**). `stderr` is for diagnostics only.
- In `2026-07-28`, clients SHOULD probe with `server/discover` first and fall back to `initialize` on non-modern error (`-32601` / error) or timeout.
- Never write non-JSON-RPC output (logs, banners, ANSI escapes) to `stdout`.

### 4.4 OAuth 2.1 Authorization (both `2025-11-25` & `2026-07-28`)

- Applies to HTTP transports (stdio SHOULD use environment credentials). All AS endpoints MUST use HTTPS; redirect URIs MUST be `localhost` or HTTPS.
- **Step 1 — Protected Resource Metadata (RFC 9728) discovery order:**
  1. `resource_metadata` URL from `401 Unauthorized` `WWW-Authenticate` header (if present).
  2. Path-scoped well-known: `https://<host>/.well-known/oauth-protected-resource/<mcp-path>`.
  3. Root well-known: `https://<host>/.well-known/oauth-protected-resource`.
  - Validate PRM `resource` matches the target MCP server URL (`auth/resource-mismatch`) and extract `authorization_servers` (≥1).
- **Step 2 — Authorization Server Metadata (RFC 8414 / OIDC Discovery) order:**
  - Issuer with path (`https://auth.example.com/tenant1`):
    1. `https://auth.example.com/.well-known/oauth-authorization-server/tenant1`
    2. `https://auth.example.com/.well-known/openid-configuration/tenant1`
    3. `https://auth.example.com/tenant1/.well-known/openid-configuration`
  - Issuer without path (`https://auth.example.com`):
    1. `https://auth.example.com/.well-known/oauth-authorization-server`
    2. `https://auth.example.com/.well-known/openid-configuration`
  - **MUST** verify metadata `issuer` exactly matches the issuer URL (`auth/metadata-issuer-mismatch`).
  - **MUST** verify `code_challenge_methods_supported` is present and supports `S256`; refuse to proceed if absent.
- **Step 3 — Client Registration priority order:**
  1. Pre-registered `client_id` / `client_secret` (when configured).
  2. **Client ID Metadata Documents (CIMD)** when AS advertises `client_id_metadata_document_supported: true` (`client_id` is an `https://` URL with a path whose JSON doc has matching `client_id`, `client_name`, `redirect_uris`).
  3. Dynamic Client Registration (RFC 7591) if `registration_endpoint` is present *(deprecated in `2026-07-28`, retained for backcompat; must send `application_type: "native" | "web"`)*.
  4. User prompt.
- **Step 4 — Authorization & Token Exchange:**
  - **PKCE `S256`** mandatory.
  - **RFC 8707 `resource`** parameter (canonical MCP server URI, lowercase scheme/host, no fragment) MUST be sent on **both** authorization and token requests.
  - **RFC 9207 `iss` validation (`2026-07-28`):** If AS advertises `authorization_response_iss_parameter_supported: true`, client MUST verify callback `iss` matches expected issuer by exact string comparison (no normalization) before exchanging the code; reject missing, wrong, or unexpected `iss` (`auth/iss-*` scenarios).
  - Support token endpoint auth methods: `client_secret_basic`, `client_secret_post`, `none`.
  - Support `offline_access` scope only when advertised/supported (`auth/offline-access-*`).
- **Step 5 — Scopes, Step-Up & Token Security:**
  - Scope selection priority on initial `401`: `WWW-Authenticate` `scope` → PRM `scopes_supported` → omit `scope`.
  - On `403 Forbidden` with `WWW-Authenticate: Bearer error="insufficient_scope", scope="..."`: compute **union** of previously requested + challenged scopes, re-authorize, and enforce a strict retry limit (`auth/scope-step-up`, `auth/scope-retry-limit`).
  - Handle authorization server migration cleanly (`auth/authorization-server-migration`).
  - Servers MUST accept `Authorization: Bearer <token>` (never query strings), verify RFC 8707 audience matches the server, return `401` on invalid/expired tokens, and **MUST NOT** pass client tokens through to upstream APIs.

---

## 5. Official `@modelcontextprotocol/conformance` test matrix

Package: `@modelcontextprotocol/conformance` (`v0.2.0-alpha.12`, `~/work/projects/repos/mcp/conformance`).
Per SEP-1730, official conformance freezes exact scored requirements per spec revision in `requirements/2025-11-25.yaml` and `requirements/2026-07-28.yaml`, and automatically validates every captured JSON-RPC wire message against the spec JSON Schema (`wire-schema-valid`).

### 5.1 CLI invocation & harness protocol

```bash
# Server conformance (scored per frozen revision requirements)
npx @modelcontextprotocol/conformance server --url http://127.0.0.1:3000/mcp --requirements 2025-11-25
npx @modelcontextprotocol/conformance server --url http://127.0.0.1:3000/mcp --requirements 2026-07-28

# Client conformance (scored per frozen revision requirements)
npx @modelcontextprotocol/conformance client --command "./bin/mcp-conformance-client" --requirements 2025-11-25
npx @modelcontextprotocol/conformance client --command "./bin/mcp-conformance-client" --requirements 2026-07-28

# Ad-hoc single scenario or extension suites
npx @modelcontextprotocol/conformance server --url http://127.0.0.1:3000/mcp --scenario tools-call-simple-text --spec-version 2026-07-28
npx @modelcontextprotocol/conformance client --command "./bin/mcp-conformance-client" --suite extensions
```

**Harness contract:**
- **Server harness:** Exposes a single Streamable HTTP endpoint (`--url`), handles both `2025-11-25` and `2026-07-28` wire protocols, and implements the standard "everything server" fixtures (tools, resources, prompts, completions, MRTR/elicitation/sampling/roots, caching, DNS rebinding protection).
- **Client harness:** Spawned by the runner with `<server-url>` as its final positional CLI arg and three environment variables:
  - `MCP_CONFORMANCE_SCENARIO` (scenario ID, e.g. `tools_call`, `auth/basic-cimd`)
  - `MCP_CONFORMANCE_PROTOCOL_VERSION` (`2025-11-25` or `2026-07-28`)
  - `MCP_CONFORMANCE_CONTEXT` (optional JSON string supplying scenario inputs such as pre-registered credentials)
- **Exit code:** `0` iff every scored scenario passes (and no stale `--expected-failures` entries exist); `1` otherwise.

### 5.2 Complete server scenarios

| Group | `2025-11-25` | `2026-07-28` | Scenarios |
| --- | :---: | :---: | --- |
| **Lifecycle & Session** | Yes | — | `server-initialize`, `server-session-lifecycle`, `ping`, `logging-set-level` |
| **Stateless & Headers** | — | Yes | `server-stateless`, `sep-2164-resource-not-found`, `caching`, `http-header-validation` *(pending)*, `http-custom-header-server-validation` *(pending)* |
| **Tools** | Yes | Yes | `tools-list`, `tools-call-simple-text`, `tools-call-image`, `tools-call-audio`, `tools-call-embedded-resource`, `tools-call-mixed-content`, `tools-call-error`, `tools-call-with-progress`, `json-schema-2020-12` *(pending)* |
| **Tools (2025 server-init)** | Yes | — | `tools-call-with-logging`, `tools-call-sampling`, `tools-call-elicitation`, `elicitation-sep1034-defaults`, `elicitation-sep1330-enums` |
| **MRTR (SEP-2322, 14)** | — | Yes | `input-required-result-basic-elicitation`, `input-required-result-basic-sampling`, `input-required-result-basic-list-roots`, `input-required-result-request-state`, `input-required-result-multiple-input-requests`, `input-required-result-multi-round`, `input-required-result-missing-input-response`, `input-required-result-non-tool-request`, `input-required-result-result-type`, `input-required-result-unsupported-methods`, `input-required-result-tampered-state`, `input-required-result-capability-check`, `input-required-result-ignore-extra-params`, `input-required-result-validate-input` |
| **Resources & Templates** | Yes | Yes | `resources-list`, `resources-read-text`, `resources-read-binary`, `resources-templates-read` |
| **Resource Subscriptions** | Yes | — | `resources-subscribe`, `resources-unsubscribe` |
| **Prompts & Completions** | Yes | Yes | `prompts-list`, `prompts-get-simple`, `prompts-get-with-args`, `prompts-get-embedded-resource`, `prompts-get-with-image`, `completion-complete` |
| **Transport & Security** | Yes | Yes | `server-sse-multiple-streams`, `dns-rebinding-protection`; `2025-11-25` only: `server-sse-polling` *(pending)* |
| **Extensions (opt-in)** | Opt | Opt | **Tasks (10):** `tasks-lifecycle`, `tasks-capability-negotiation`, `tasks-wire-fields`, `tasks-request-state-removal`, `tasks-mrtr-input`, `tasks-request-headers`, `tasks-dispatch-and-envelope`, `tasks-status-notifications`, `tasks-required-task-error`, `tasks-mrtr-composition`. **Skills (3):** `sep-2640-skills-directory`, `sep-2640-skills-enumeration`, `sep-2640-skills-manifest`. |

### 5.3 Complete client scenarios

| Group | `2025-11-25` | `2026-07-28` | Scenarios |
| --- | :---: | :---: | --- |
| **Core Protocol** | Yes | Yes | `tools_call`, `json-schema-2020-12-preservation`; `2025-11-25` only: `initialize`, `elicitation-sep1034-client-defaults`, `sse-retry`; `2026-07-28` only: `request-metadata`, `sep-2322-client-request-state`, `http-standard-headers`, `http-custom-headers`, `http-invalid-tool-headers`, `json-schema-ref-no-deref` |
| **OAuth Metadata & Registration** | Yes | Yes | `auth/metadata-default`, `auth/metadata-var1`, `auth/metadata-var2`, `auth/metadata-var3`, `auth/basic-cimd`, `auth/pre-registration` |
| **OAuth Scopes & Token Auth** | Yes | Yes | `auth/scope-from-www-authenticate`, `auth/scope-from-scopes-supported`, `auth/scope-omitted-when-undefined`, `auth/scope-step-up`, `auth/scope-retry-limit`, `auth/token-endpoint-auth-basic`, `auth/token-endpoint-auth-post`, `auth/token-endpoint-auth-none` |
| **OAuth `2026-07-28` Security (11)** | — | Yes | `auth/resource-mismatch`, `auth/offline-access-scope`, `auth/offline-access-not-supported`, `auth/authorization-server-migration`, `auth/iss-supported`, `auth/iss-not-advertised`, `auth/iss-supported-missing`, `auth/iss-wrong-issuer`, `auth/iss-unexpected`, `auth/iss-normalized`, `auth/metadata-issuer-mismatch` |
| **Backcompat (`2025-03-26`)** | Backcompat | — | `auth/2025-03-26-oauth-metadata-backcompat`, `auth/2025-03-26-oauth-endpoint-fallback` |
| **Extensions (opt-in)** | Opt | Opt | **Auth (6):** `auth/client-credentials-jwt`, `auth/client-credentials-basic`, `auth/enterprise-managed-authorization`, `auth/dpop`, `auth/dpop-nonce`, `auth/wif-jwt-bearer`. **Skills (4):** `sep-2640-client-no-prefetch` + digest/size/frontmatter verification. |
