# Model Context Protocol (MCP) in Coax

Status: **research & architecture plan, 2026-11-27**.
Reference clones of all 49 official `modelcontextprotocol` repositories live outside this repo at `~/work/projects/repos/mcp/`.

This documentation set defines what it takes to build a **100% spec-compliant, native Coax MCP client and server library** (`coax/lib/mcp/`) inside `openagents`, verified directly against the official `@modelcontextprotocol/conformance` suite.

## Documents in this directory

| Document | Contents |
| --- | --- |
| [README.md](README.md) | Executive summary, dual-era protocol reality, core architectural decisions, and quick roadmap. |
| [ecosystem-repos.md](ecosystem-repos.md) | Complete catalog of all **49** `github.com/modelcontextprotocol` repositories, extension ecosystem overview, and architecture comparison across the Tier-1 official SDKs (`rust-sdk`, `go-sdk`, `typescript-sdk`, `python-sdk`, `swift-sdk`). |
| [specification-and-conformance.md](specification-and-conformance.md) | Normative specification breakdown (`2025-11-25` stateful vs `2026-07-28` stateless GA & `draft`), transports, MRTR, OAuth 2.1 authorization, JSON-RPC error codes, and the complete `@modelcontextprotocol/conformance` test matrix (SEP-1730). |
| [coax-architecture-and-plan.md](coax-architecture-and-plan.md) | Audit of existing OpenAgents Coax libraries and ad-hoc MCP code, explicit gap analysis, proposed Sans-I/O `#authority` module layout for `coax/lib/mcp/`, verification strategy, and phased build plan (Phases 0–7). |

---

## Executive summary

### 1. What "100% compliant MCP" actually requires today

MCP has crossed a major architectural boundary between its stateful handshake era and its stateless GA era, and **official SDK conformance (`SEP-1730`) scores both**:

| Revision | Wire model | Conformance scored scenarios (`requirements/*.yaml`) | Status |
| --- | --- | --- | --- |
| **`2025-11-25`** | **Stateful**: `initialize` / `notifications/initialized`, `MCP-Session-Id`, HTTP `GET`/`DELETE`, SSE `Last-Event-ID` & `retry:` polling (SEP-1699), server-initiated requests on SSE, `ping`, `resources/subscribe`, `logging/setLevel`. | **30 server + 18 client** scored (10 `not_scored`) | Latest stateful revision; widely deployed across existing MCP clients/servers. |
| **`2026-07-28`** | **Stateless & sessionless GA**: per-request `_meta["io.modelcontextprotocol/*"]`, `server/discover`, `subscriptions/listen` (POST SSE), **Multi Round-Trip Requests (MRTR)** (`resultType: "complete" \| "input_required"` + signed `requestState`), mandatory `Mcp-Method`/`Mcp-Name`/`x-mcp-header` HTTP headers, `CacheableResult` (`ttlMs`, `cacheScope`), disconnect-cancels-request, OAuth 2.1 + CIMD + RFC 9207 `iss` validation. | **37 server + 32 client** scored (20 `not_scored`) | Current stable GA specification (`draft` tracks post-`2026-07-28` refinements). |
| **`2024-11-05` / `2025-03-26` / `2025-06-18`** | Historical stateful revisions (plus deprecated HTTP+SSE fallback from `2024-11-05`). | Backcompat subset (e.g. `2025-03-26` OAuth metadata fallback) | Supported via version negotiation and optional legacy fallback. |

**Bottom line:** A 100% compliant Coax library MUST implement **both** wire models cleanly from day one (`2026-07-28` as `MCP_LATEST`, `2025-11-25` as `MCP_LATEST_WITH_INITIALIZE`), and prove compliance by running the official `@modelcontextprotocol/conformance` CLI against Coax "everything server" and "everything client" binaries in CI.

### 2. Why Coax is a strong fit — and what is missing

OpenAgents already has a strong foundation in `coax/lib/` that maps directly onto MCP's hardest security and wire requirements:

- **Already built in Coax:**
  - Bounded caller-owned JSON parser & writer (`coax/lib/json/json.coax`) + strict `#authority` closed-object/UTF-8 validator (`coax/lib/json/shape.coax`).
  - Zero-allocation WHATWG SSE parser & formatter (`coax/lib/gateway/sse.coax`).
  - Child-process stdio spawning with line reads, stderr capture, process groups, and timeouts (`coax/lib/sys/proc.coax`, `coax/platform/process-native.c`).
  - Non-blocking `poll`-based TCP/TLS server shim (`coax/platform/socket-server-native.c`, up to 288 concurrent connections) and synchronous HTTP/1.1 client (`coax/lib/net/http.coax`).
  - Crypto & OAuth primitives (`coax/lib/crypto/` SHA-256, HMAC-SHA256, AES-GCM, RSA RS256, P-256, CSPRNG; `coax/lib/workos/` RFC 7636 PKCE `S256`, unpadded base64url, RS256 JWKS/JWT verification, RFC 3986 percent-encoding, AEAD/HMAC sealing, retry policy).
- **Six concrete gaps we must build:**
  1. **Streaming HTTP/1.1 Server + Streamable HTTP / SSE layer** on top of `socket-server-native.c` (request/header parsing, `Accept` negotiation, `Origin` DNS-rebinding `403`, chunked/SSE long-lived streams, `X-Accel-Buffering: no`, keep-alives, disconnect detection, and 2025 `MCP-Session-Id` + `EventStore` vs 2026 stateless/subscription dispatch).
  2. **Incremental Streaming HTTP/1.1 + SSE Client** (`coax/lib/net/http.coax` currently buffers full responses up to 1 MiB before returning, so it cannot drive SSE progress/subscriptions/server-requests incrementally).
  3. **Bounded JSON Schema Draft 2020-12 validator & `x-mcp-header` analyzer** (for tool `inputSchema`/`outputSchema`, elicitation schemas, and rejecting invalid `x-mcp-header` tools per `2026-07-28`).
  4. **General URI parser + RFC 6570 URI Template engine** (`coax/lib/net/uri.coax` only handles DNS `http`/`https`/`wss`; MCP resources use arbitrary schemes like `file://`, `memo://`, `ui://`, and RFC 6570 `{var}` templates).
  5. **MCP OAuth 2.1 client & resource-server layer** (RFC 9728 Protected Resource Metadata discovery order, RFC 8414 & OIDC Discovery order + exact `issuer` check, PKCE `code_challenge_methods_supported` gate, RFC 9207 `iss` check, RFC 8707 `resource` indicator & audience check, CIMD, legacy DCR fallback, `WWW-Authenticate` `401`/`403` step-up scope union + retry cap).
  6. **Unified `coax/lib/mcp/` Sans-I/O protocol library + conformance harnesses** replacing the 5 ad-hoc stdio tools-only MCP servers already scattered across `apps/openagents-cli/`, `coder/tui/`, and `coax/lib/agent/claude/`.

### 3. Locked design principles

1. **Sans-I/O `#authority` core, thin `#effects(...)` adapters:** Following `coax/lib/workos/` and `coax/lib/net/`, every wire codec, protocol state machine, header validator, MRTR `requestState` sealer, OAuth policy engine, URI template matcher, and JSON Schema validator lives in pure, effect-free `#authority` Coax with zero heap allocation, compile-time `#test`/`#forall`, and **0 surviving mutants** under `./coax/bin/coax mutate`.
2. **Dual-era architecture without leaky abstractions:** Separate `MCP_LATEST` (`2026-07-28`) from `MCP_LATEST_WITH_INITIALIZE` (`2025-11-25`) so `initialize` never echoes a stateless version and stateless transports never advertise stateful-only versions (lesson directly from `rust-sdk` and `go-sdk`).
3. **Conformance-first development:** Stand up `tools/mcp-conformance/` (Coax "everything server" + "everything client" binaries driven by `@modelcontextprotocol/conformance`) in Phase 2 and burn the pass rate up to **100% on both `requirements/2025-11-25.yaml` and `requirements/2026-07-28.yaml`** with zero `expected-failures`.
4. **Consolidate existing repo MCP code:** Migrate `apps/openagents-cli/mcp.coax`, `apps/openagents-cli/executor-mcp.coax`, `coder/tui/oasim-mcp.coax`, `coder/tui/lean-run-mcp.coax`, and `coax/lib/agent/claude/mcp.coax` onto `coax/lib/mcp/` once the stdio server API stabilizes.
5. **Extensions as clean opt-in modules:** Implement core spec first, then ship official extensions (`io.modelcontextprotocol/tasks`, `io.modelcontextprotocol/skills`, `ext-auth` client credentials & enterprise auth, `ext-server-card`, `ext-apps`) as layered modules under `coax/lib/mcp/ext/`.

---

## Phased build plan at a glance

| Phase | Scope | Exit gate |
| --- | --- | --- |
| **Phase 0** | Shared `#authority` foundations: general URI + RFC 6570 templates, bounded JSON Schema 2020-12 + `x-mcp-header` validator, `WWW-Authenticate` parser, RFC 9728/8414/8707/9207/CIMD OAuth policy core. | `coax test` + `coax mutate` (0 survivors) across all new `#authority` modules. |
| **Phase 1** | Core `coax/lib/mcp/` Sans-I/O protocol layer: JSON-RPC 2.0 envelope, version negotiation (`2024-11-05`..`2026-07-28`), all 2025 + 2026 types/codecs, MRTR `InputRequiredResult` + HMAC/AEAD `requestState`, `Mcp-Method`/`Mcp-Name`/`Mcp-Param-*` + `=?base64?...?=` codec, `InMemoryTransport`. | Round-trip schema tests against `schema/{2025-11-25,2026-07-28}/examples/`, 0 surviving mutants. |
| **Phase 2** | Stdio & child-process transports + high-level Coax `McpServer` & `McpClient` APIs + initial `tools/mcp-conformance/` harness skeleton. | Full stdio client↔server integration tests (both `2025-11-25` and `2026-07-28`) + migration of existing repo stdio servers. |
| **Phase 3** | Streaming HTTP/1.1 Server + Streamable HTTP server transport (`2026-07-28` stateless + `subscriptions/listen`, and `2025-11-25` stateful `MCP-Session-Id` + GET/DELETE + bounded `EventStore` + SEP-1699 `retry:`) + DNS rebinding `Origin` `403`. | **100% pass** on `@modelcontextprotocol/conformance server` for **both** `--requirements 2025-11-25` (30/30) and `--requirements 2026-07-28` (37/37). |
| **Phase 4** | Incremental Streaming HTTP/1.1 + SSE Client + Streamable HTTP client transport (including `x-mcp-header`, MRTR loop, legacy HTTP+SSE fallback, disconnect cancellation). | **100% pass** on all non-auth `@modelcontextprotocol/conformance client` scenarios for `2025-11-25` and `2026-07-28`. |
| **Phase 5** | Full OAuth 2.1 client & resource-server transport integration (PRM, AS/OIDC discovery, PKCE S256, RFC 8707, RFC 9207 `iss`, CIMD, pre-reg, DCR fallback, `401`/`403` step-up scope union, retry cap, token audience validation). | **100% pass** on **all** `@modelcontextprotocol/conformance client` scenarios for `--requirements 2025-11-25` (18/18) and `--requirements 2026-07-28` (32/32) — **zero expected failures**. |
| **Phase 6** | Official extensions (`coax/lib/mcp/ext/`): `ext-tasks` (`io.modelcontextprotocol/tasks`), `ext-skills` (`io.modelcontextprotocol/skills`), `ext-auth` (client credentials, DPoP, enterprise managed auth), `ext-server-card`, `ext-apps`. | Passing conformance extension scenarios (`--suite extensions` / tasks / skills). |
| **Phase 7** | OpenAgents product integration: Desktop/CLI/Coder MCP host & client manager, migrating all 5 existing ad-hoc MCP servers, CI conformance gate (`tier-check` style). | Zero remaining ad-hoc MCP framing in repo; CI runs Coax conformance gate on PRs. |
