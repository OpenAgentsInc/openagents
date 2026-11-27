# MCP GitHub Organization Inventory & SDK Architecture Analysis

Part of [MCP in Coax](README.md).
Source clones audited on 2026-11-27 at `~/work/projects/repos/mcp/` (**49** public repositories from `https://github.com/modelcontextprotocol`).

---

## 1. Complete catalog of all 49 `modelcontextprotocol` repositories

### 1.1 Core specification & documentation (2)

| Repo | Primary lang | Status | Latest version / ref | Purpose & key files |
| --- | --- | --- | --- | --- |
| `modelcontextprotocol` | MDX / TS / JSON Schema | **Active (canonical)** | `2026-07-28` GA (+ `draft`) | Canonical MCP specification (`docs/specification/{2024-11-05,2025-03-26,2025-06-18,2025-11-25,2026-07-28,draft}/`), authoritative TypeScript & JSON Schema 2020-12 (`schema/<ver>/{schema.ts,schema.json,examples/}`), PR-based SEPs (`seps/`), and Hugo blog (`blog/`). |
| `docs` | MDX | Archived / moved | Last `2025-04-08` | Legacy user-facing Mintlify docs; superseded by `modelcontextprotocol/docs`. |

### 1.2 Conformance & testing (1)

| Repo | Primary lang | Status | Latest version / ref | Purpose & key files |
| --- | --- | --- | --- | --- |
| `conformance` | TypeScript | **Active (authoritative)** | `v0.1.16` (`npm: 0.2.0-alpha.12`) | Official `@modelcontextprotocol/conformance` test runner for MCP clients, servers, and authorization servers, plus SEP-1730 SDK tier assessment (`requirements/2025-11-25.yaml`, `requirements/2026-07-28.yaml`, `src/scenarios/`, `action.yml`). |

### 1.3 Official SDKs (13)

| Repo | Primary lang | Status | Latest version / ref | Purpose & notes |
| --- | --- | --- | --- | --- |
| `typescript-sdk` | TypeScript | Active (v2) | `@modelcontextprotocol/client@2.4.0` | Monorepo (`packages/{core,client,server}`). Zod schemas, Web Standards `StreamableHTTP`, supports `2024-10-07`..`2026-07-28`. |
| `python-sdk` | Python | Active (v2) | `v2.3.0` | Split `mcp-types` (Pydantic v2) + AnyIO memory streams + Starlette/ASGI. Supports `2024-11-05`..`2026-07-28`. |
| `go-sdk` | Go | Active | `v1.9.0-pre.1` | Minimal `Transport`/`Connection` interface, `auth/` & `oauthex/`, byte-bounded `MemoryEventStore`, supports `2024-11-05`..`2026-07-28`. |
| `rust-sdk` | Rust | Active (v3) | `rmcp-v3.12.0` | `rmcp` on Tokio + Tower. Hand-written `serde` model, `LATEST` vs `LATEST_WITH_INITIALIZE`, supports `2024-11-05`..`2026-07-28`. |
| `csharp-sdk` | C# (.NET) | Active (v2) | `v2.3.0` | Official .NET SDK (`ModelContextProtocol.slnx`), wired to `@modelcontextprotocol/conformance`. |
| `java-sdk` | Java | Active (v2) | `v2.2.0` | Official Java SDK (`MIGRATION-2.0.md`), conformance suite in `conformance-tests/`. |
| `kotlin-sdk` | Kotlin (KMP) | Active | `0.16.0` | Kotlin Multiplatform SDK (JVM, Native, JS, Wasm), conformance suite in `conformance-test/`. |
| `swift-sdk` | Swift | Active | `0.12.1` | Actor-isolated Swift 6 `Sendable`/`Codable` SDK (`Sources/MCP/Base/Authorization/`), currently `2024-11-05`..`2025-11-25`, conformance in `Sources/MCPConformance/`. |
| `ruby-sdk` | Ruby | Active | `v1.11.0` | Official `mcp` Ruby gem, conformance harness in `conformance/`. |
| `php-sdk` | PHP | Active | `v0.10.0` | Framework-agnostic PHP SDK (`adr/`, PHPUnit `tests/`). |
| `php-sdk-client` | PHP | Split stub | Last `2026-11-06` | Split read-only client package mirror for `php-sdk`. |
| `php-sdk-schema` | PHP | Split stub | Last `2026-11-06` | Split read-only schema package mirror for `php-sdk`. |
| `php-sdk-server` | PHP | Split stub | Last `2026-11-06` | Split read-only server package mirror for `php-sdk`. |

### 1.4 Official extensions — `ext-*` (6)

| Repo | Primary lang | Status | Latest version / ref | Extension ID & purpose |
| --- | --- | --- | --- | --- |
| `ext-apps` | TS / MDX | Active (Stable) | `v2.1.0` | `io.modelcontextprotocol/ui` (SEP-1865) — interactive inline UIs (forms, charts, dashboards) for MCP tools rendered in chat hosts (`specification/2026-01-26/apps.mdx`). |
| `ext-auth` | MDX | Active (Stable) | `npm: 1.0.0` | Optional OAuth extensions: **Enterprise-Managed Authorization** (SEP-990) and **OAuth Client Credentials** (`specification/stable/*/2026-07-28/`). |
| `ext-tasks` | TS / MDX | Active (Stable) | `v0.2.3` | `io.modelcontextprotocol/tasks` (SEP-2663) — durable/long-running tasks moved out of core in `2026-07-28` (`tasks/get`, `tasks/result`, `tasks/list`, `tasks/cancel`, status notifications, MRTR composition, `schema/2026-07-28/`). |
| `ext-skills` | TS / MDX | Active (Beta) | `v0.1.0` | `io.modelcontextprotocol/skills` (SEP-2640) — serving and verifying agent skills as MCP Resources (directory reads, manifests, digest/size/frontmatter verification, no-prefetch policy). |
| `ext-events` | MDX | Beta / Exp | Last `2026-11-20` | `io.modelcontextprotocol/events` (SEP-3415) — Tasks & Events WG spec for proactive server-initiated event/trigger notifications (`specification/beta/events/events.mdx`). |
| `ext-server-card` | TS / JSON Schema | Experimental | `npm: 0.0.0` | MCP Server Cards (SEP-2127) — static JSON discovery doc (`GET <streamable-http-url>/server-card`) advertising server identity, endpoints, and supported versions (`schema.ts`, `schema.json`, `examples/ServerCard/`). |

### 1.5 Experimental extensions — `experimental-ext-*` (5)

| Repo | Primary lang | Status | Last activity | Purpose |
| --- | --- | --- | --- | --- |
| `experimental-ext-file-uploads` | Markdown | Experimental | `2026-11-09` | Out-of-band client↔tool file upload/download descriptors (`x-mcp-file`), keeping binary bytes out of JSON-RPC and LLM context. |
| `experimental-ext-filesystems` | Python / MD | Experimental | `2026-11-11` | Resource mutation primitives (`stat`, `create`, `update`, `delete`) with revision-based optimistic concurrency (`docs/proposals/resource-operations.md`). |
| `experimental-ext-interceptors` | C# / Go / Py / TS | Experimental | `2026-11-24` | Cross-SDK middleware/interceptor hooks proposal (`csharp/`, `go/`, `python/`, `typescript/`, `conformance/`). |
| `experimental-ext-tool-annotations` | MDX | Experimental | `2026-08-12` | Layered trust/sensitivity annotations (`io.modelcontextprotocol/trust-annotations`) with pluggable out-of-band schemes. |
| `experimental-ext-variants` | Go / Py / TS | Experimental | `2026-05-29` | Per-LLM tool description variants, context-budget verbosity levels, and API versioning (runnable Go examples in `go/sdk/examples/`). |

### 1.6 Reference servers, clients & developer tooling (11)

| Repo | Primary lang | Status | Latest version / ref | Purpose |
| --- | --- | --- | --- | --- |
| `inspector` | TS (React/Ink) | Active (v2) | `v2.16.0` | `@modelcontextprotocol/inspector` — 4 interfaces in one package: Web UI, scriptable CLI (`--cli`), Ink TUI (`--tui`), and experimental `mcpdo` daemon CLI with OS keychain secret storage. |
| `registry` | Go | Active (API v0.1) | `v1.8.16` | Official MCP Registry (directory API + PostgreSQL + `cmd/publisher` + `cmd/usage-collector`). |
| `mcpb` | TS / JSON Schema | Active | `v2.1.2` | MCP Bundles (formerly DXT) — `.mcpb` zip archive spec (`MANIFEST.md`, `schemas/mcpb-manifest-v0.4.schema.json`) and CLI (`mcpb init`/`pack`) for single-click desktop installation. |
| `servers` | TS / Python | Active | `v2.5.0` | Official reference servers: `everything`, `fetch`, `filesystem`, `git`, `memory`, `sequentialthinking`, `time`. |
| `servers-archived` | TypeScript | Archived | Last `2025-05-28` | Legacy reference servers moved out of `servers` (Brave Search, GitHub, GitLab, Google Drive, Postgres, Puppeteer, etc.). |
| `example-remote-server` | TypeScript | Active | `npm: 1.0.0` | Reference remote MCP server with OAuth 2.0 and mitmproxy debugging scripts. |
| `example-remote-client` | TS (React) | WIP | Last `2025-07-11` | Example React remote MCP client connecting to multiple servers. |
| `quickstart-resources` | Go / Py / Rb / Rs / TS | Active | Last `2026-11-19` | Companion tutorial servers & clients in 5 languages (`mcp-client-*`). |
| `create-typescript-server` | TypeScript | Inactive | `0.3.1` (`2024-11-25`) | Legacy TS server scaffolding CLI (`@modelcontextprotocol/create-server`). |
| `create-python-server` | Python | Inactive | `v1.0.5` (`2024-11-25`) | Legacy Python server scaffolding CLI (`uvx`). |
| `use-mcp` | TS (React) | Early / Active | `v0.0.21` | React hook library (`useMcp`) for browser MCP clients. |

### 1.7 Working groups, governance & infrastructure (11)

| Repo | Primary lang | Status | Purpose |
| --- | --- | --- | --- |
| `.github` | Markdown / YAML | Active | Org-wide community health files (`CODE_OF_CONDUCT.md`, `CONTRIBUTING.md`, `SECURITY.md`), reusable workflows, org profile. |
| `access` | TS (Pulumi IaC) | Active (prod) | Pulumi IaC managing GitHub teams/repos, Google Workspace groups, Discord roles, Cloudflare Zero Trust Access, and npm/PyPI publish-access drift. |
| `actions` | YAML / Shell / JS | Active | Reusable GitHub Actions (`cloudflare-pages-preview`, `hugo-build`, PR `slash-commands` `/lgtm`, `/hold`, `/stageblog`). |
| `agents-ig` | Markdown | Active | MCP Agents Interest/Working Group — weekly forum on agent↔MCP server ergonomics (`docs/`, `meetings/`, `proposals/`). |
| `core-primitives-wg` | Markdown | Minimal stub | Coordination repo for the Core Primitives Working Group. |
| `dns` | TS (Pulumi IaC) | Active (prod) | Pulumi IaC managing Cloudflare DNS records for `modelcontextprotocol.io`. |
| `financial-services-interest-group` | Markdown | Active (IG) | Financial Services Interest Group (`docs/`, `extensions/`). |
| `governance-wg` | Markdown | Active (WG) | Stewards `docs/community/` governance docs, contributor ladder, decision records, and CoC. |
| `progressive-disclosure-wg` | MD / SDK PoCs | Experimental | Primitive Grouping IG exploring organizing flat Tools/Resources/Prompts/Tasks lists (`docs/`, `sdk/`). |
| `static` | HTML / JSON Schema | Active | Static asset hosting for `static.modelcontextprotocol.io` (`schemas/`). |
| `transports-wg` | Markdown | Active (WG) | Transport usability, scalability, reliability, and protocol semantics (`meetings/`, `proposals/`, `roadmaps/`). |

*(Count check: 2 + 1 + 13 + 6 + 5 + 11 + 11 = **49** repositories.)*

---

## 2. How extensions work in MCP

Per SEP-2133 and SEP-3392 (`modelcontextprotocol/seps/`):

- **Namespace:** Extensions are keyed by reverse-DNS identifiers under `capabilities.extensions` (e.g., `"io.modelcontextprotocol/tasks": {}`, `"io.modelcontextprotocol/skills": {}`, `"io.modelcontextprotocol/ui": {}`). Vendors can define private extensions under their own reverse-DNS prefix.
- **Negotiation:**
  - In stateful revisions (`<= 2025-11-25`), client and server exchange `capabilities.extensions` during the `initialize` handshake.
  - In stateless `2026-07-28`, clients attach `capabilities.extensions` inside `_meta["io.modelcontextprotocol/clientCapabilities"]` on every request, and servers advertise theirs in `server/discover`.
- **Wire extensibility:** Extensions may introduce new RPC methods, namespaced `_meta` keys, and additional `resultType` variants (validated against the union of core `"complete" | "input_required"` plus negotiated extension `resultType`s).
- **Conformance policy:** Per SEP-1730, extensions are **optional** and marked `not_scored` in `requirements/*.yaml` — a library reaches 100% core compliance without them, and can opt into extension suites (`--suite extensions`, tasks, skills) incrementally.

---

## 3. Tier-1 Official SDK Architecture Comparison

We inspected `rust-sdk`, `go-sdk`, `typescript-sdk`, `python-sdk`, and `swift-sdk` in depth (plus conformance wiring in `csharp-sdk`, `java-sdk`, `kotlin-sdk`, `ruby-sdk`, `php-sdk`).

### 3.1 Multi-version & schema strategy

| SDK | Supported versions | Wire model strategy | Version negotiation pattern |
| --- | --- | --- | --- |
| **`rust-sdk`** (`rmcp`) | `2024-11-05`..`2025-11-25`, `2026-07-28` | Hand-written `serde` structs/enums (`crates/rmcp/src/model/`), unified superset with version-aware serde. | Lexical `YYYY-MM-DD` comparison; separates `LATEST` (`2026-07-28`) from `LATEST_WITH_INITIALIZE` (`2025-11-25`) via `has_initialize()`. |
| **`go-sdk`** | `2024-11-05`..`2025-11-25`, `2026-07-28` | Hand-written structs (`mcp/protocol.go`) + custom `MarshalJSON` + `annotateResultType()` for `2026-07-28`. | Optional `ProtocolVersionSupporter` on `Transport` filters advertised versions; client probes `server/discover` and falls back to `initialize`. |
| **`typescript-sdk`** | `2024-10-07`..`2025-11-25`, `2026-07-28` | Zod schemas in `@modelcontextprotocol/core` + Ajv / CF Worker JSON Schema validator. | Dedicated `versionNegotiation.ts` + `probeClassifier.ts` for modern vs legacy fallback. |
| **`python-sdk`** | `2024-11-05`..`2025-11-25`, `2026-07-28` | Split `mcp-types` Pydantic v2 package + `is_version_at_least()` guards. | Separate 2025 (`server/streamable_http.py`) and 2026 (`server/_streamable_http_modern.py`) transport paths. |
| **`swift-sdk`** | `2024-11-05`..`2025-11-25` | Hand-written `Codable`/`Sendable` structs (`Sources/MCP/Base/`). | `Version.negotiate(clientRequestedVersion:)` echoes supported or falls back to `latest`. |

**Takeaway for Coax:** **Hand-write the protocol structs/codecs** (matching `rust-sdk`, `go-sdk`, `swift-sdk`, and Coax's own zero-heap `Jv`/`Jo` discipline), and validate them mechanically against `modelcontextprotocol/schema/{2025-11-25,2026-07-28}/examples/*.json` and `@modelcontextprotocol/conformance`'s built-in wire schema validator (`wire-schema-valid`).

### 3.2 Convergent 5-layer stack

Every mature SDK converges on the exact same 5 layers:

1. **Protocol / Wire Types & Codecs:** JSON-RPC 2.0 envelope, version constants, request/response/notification structs, content blocks, annotations, MRTR `InputRequiredResult`, error codes.
2. **Session / Multiplexer / Dispatcher:** Pending request ID table, request-scoped progress tokens, cancellation tracking, stateless per-request dispatch (`2026-07-28`) vs stateful session lifecycle (`2025-11-25`).
3. **Transport Abstraction:** Tiny message-oriented interface (`Read`/`Write`/`Close` in `go-sdk`; `send`/`receive` in `swift-sdk`; `Transport<R>` in `rust-sdk`) implemented by `InMemoryTransport`, `StdioTransport`, `ChildProcessTransport`, `StreamableHttpClient`, `StreamableHttpServer`, and optional `LoggingTransport` / legacy `SseClient`.
4. **Auth / OAuth 2.1 Layer:** Client discovery/PKCE/CIMD/step-up engine + server bearer-token/audience validator, `WWW-Authenticate` emitter, Protected Resource Metadata endpoint, and `Origin`/`Host` security middleware.
5. **Ergonomic Client & Server APIs:** Tool/Resource/Prompt registries (`ToolRouter`/`PromptRouter`), client convenience methods, and optional middleware/interceptors.

### 3.3 Conformance harness pattern across SDKs

Every Tier-1 SDK embeds two conformance binaries/scripts driven by `@modelcontextprotocol/conformance`:

- **`rust-sdk`**: `conformance/src/bin/server.rs` (~1,965 LOC) + `conformance/src/bin/client.rs` + `expected-failures-extensions.yaml`.
- **`go-sdk`**: `conformance/everything-server/main.go` (~1,315 LOC) + `conformance/everything-client/main.go` + `mcp/conformance_test.go`.
- **`typescript-sdk`**: `test/conformance/src/{everythingServer,everythingClient,authTestServer}.ts` + `.github/workflows/conformance.yml`.
- **`python-sdk`**: `.github/workflows/conformance.yml` running three legs (`--spec-version 2025-11-25`, `--spec-version 2026-07-28`, and `--suite all` for extensions).
- **`swift-sdk`**: `Sources/MCPConformance/{Server,Client}/main.swift` + `conformance-baseline.yml`.

### 3.4 Six high-value design lessons & real-world pitfalls to borrow

1. **Separate `MCP_LATEST` from `MCP_LATEST_WITH_INITIALIZE`:** Prevents `initialize` handlers from ever echoing `2026-07-28`, and prevents stateless-only servers from advertising stateful revisions (`rust-sdk` `model.rs`, `go-sdk` `ProtocolVersionSupporter`).
2. **Keep the Transport contract minimal and message-oriented:** `InMemoryTransport` lets 90% of protocol, MRTR, pagination, capability, and error tests run in compile-time `#test` blocks with zero sockets or subprocesses.
3. **Never deadlock cancellation paths:** In `2025-11-25` HTTP mode, sending `notifications/cancelled` over a broken or unauthorized HTTP connection must be best-effort with a bounded timeout (`go-sdk` uses async 5s timeout); in `2026-07-28` HTTP mode, closing the SSE connection *is* cancellation.
4. **Bound SSE `EventStore` memory and deduplicate replay races (`2025-11-25`):** Track `replayedEventIds` (`typescript-sdk` `streamableHttp.ts`) so concurrent `storeEvent` + `Last-Event-ID` reconnect never delivers duplicates, and cap stored bytes (`go-sdk` `MemoryEventStore`) so slow/disconnected clients cannot exhaust RAM.
5. **Enforce strict frame/line/body size limits at transport boundaries:** Cap stdio line length, HTTP body size, SSE event size, and header lengths up front (`go-sdk` `frameLimitReader`, `DefaultMaxEventSize`; `rust-sdk` `max_line_length`) — natural fit for Coax's fixed caller-owned buffers.
6. **Split `2025-11-25` and `2026-07-28` Streamable HTTP dispatch cleanly (`python-sdk` pattern):** Keep shared JSON-RPC/tool/resource/prompt handlers unified, while cleanly routing stateful session/GET/DELETE/SSE-resumability (`2025-11-25`) separately from stateless per-request `_meta`/header-mirroring/`subscriptions/listen` (`2026-07-28`) so neither era pollutes the other's invariants.
