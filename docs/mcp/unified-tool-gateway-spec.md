# OpenAgents Unified MCP Tool Gateway — Specification & Harvesting Strategy

Part of [MCP in Coax](README.md).
Status: **specification & harvesting plan, 2026-11-27**.
Reference inspiration repos (read-only research clones, **not** runtime dependencies): `~/work/projects/repos/UsefulSoftwareCo/` (`executor`, `integrations`, `executor-apps`, `mcporter`, `emulate`, `sunpeak`).

---

## 1. Goal & architectural boundary

OpenAgents will ship a **native Coax Unified MCP Tool Gateway** (`coax/lib/mcp/gateway/`) so a user connects accounts, local stdio MCP servers, remote OAuth MCP servers, HTTP/OpenAPI services, and built-in OpenAgents tools **once** — and every agent harness (OpenAgents Desktop, Coder, CLI, Claude Code, Codex, Cursor, Antigravity) reaches them through **one MCP endpoint** with:

1. **Zero credential exposure** to models or sandboxed scripts.
2. **Progressive disclosure & Code-Mode execution** (a compact meta-tool surface instead of dumping hundreds of tool schemas into context).
3. **Deterministic, policy-checked authorization & human-in-the-loop approvals** across every MCP client capability level.
4. **Full `2025-11-25` + `2026-07-28` MCP compliance** inherited cleanly from `coax/lib/mcp/`.

### Strict layering rule

```text
┌─────────────────────────────────────────────────────────────────────────┐
│  Layer 3 — Product Hosts (OpenAgents Desktop, CLI, Coder, IDE Plugins)  │
│  Vault UI, Approval Sheets, IDE Config Importer, Connector Directory    │
├─────────────────────────────────────────────────────────────────────────┤
│  Layer 2 — Unified MCP Tool Gateway (coax/lib/mcp/gateway/)   <-- THIS  │
│  Meta-tools (skills/search/describe/call/execute/resume),               │
│  SWR Catalog, Account/Grant/Approval #authority Policy,                 │
│  4-Mode Elicitation Router, Sandboxed Code-Mode (rlm/script.coax)       │
├─────────────────────────────────────────────────────────────────────────┤
│  Layer 1 — 100% Spec-Compliant MCP Protocol SDK (coax/lib/mcp/)         │
│  2025-11-25 + 2026-07-28 Server & Client, Streamable HTTP/SSE, Stdio,   │
│  MRTR, OAuth 2.1, JSON Schema 2020-12, RFC 6570, Official Extensions    │
└─────────────────────────────────────────────────────────────────────────┘
```

**Hard invariant:** Layer 1 (`coax/lib/mcp/`) remains a pure, general-purpose, 100% `@modelcontextprotocol/conformance`-passing SDK with zero gateway/product coupling. Layer 2 (`coax/lib/mcp/gateway/`) is an ordinary consumer of Layer 1's `McpServer` (downstream) and `McpClient` (upstream).

### What we deliberately leave behind from `UsefulSoftwareCo/*`

We are **harvesting architectural ideas, schemas, and test patterns — not adopting the codebase or stack**:

| UsefulSoftwareCo choice | Why OpenAgents rejects it | OpenAgents native replacement |
| --- | --- | --- |
| TypeScript + Effect v4 monorepo | Wrong language/runtime; heavy runtime footprint; unit tests banned by repo convention. | Pure `#authority` Coax cores (`#test`/`#forall`, 0 surviving mutants) + thin `#effects(...)` adapters. |
| Embedded Cloudflare `workerd` (V8 isolates) | Massive C++/V8 dependency, bundling pipeline, and IPC bridge just to sandbox tool glue scripts. | Native Coax deterministic interpreter adapted from `coax/lib/rlm/script.coax` (zero heap/syscalls/network/clock, suspend-memo-replay). |
| Stateful `2025-11-25`-era `effect/ai` + `@modelcontextprotocol/sdk` | Misses `2026-07-28` stateless GA (`server/discover`, MRTR `InputRequiredResult`, `subscriptions/listen`, `x-mcp-header`, CIMD). | Native `coax/lib/mcp/` dual-era server & client verified against both conformance suites. |
| Hosted multi-tenant Postgres / Hyperdrive SaaS | OpenAgents is local-first (Desktop/CLI/Coder) with optional Nostr/WorkOS/Vault sync. | Local-first OpenAgents Vault + bounded local SQLite/file stores, product-owned org policy hooks. |

---

## 2. Harvest matrix (source → pattern → OpenAgents module)

| Source repo | Specific files / design | Harvested pattern | Target OpenAgents module |
| --- | --- | --- | --- |
| `executor` | `packages/mcp/src/contracts/tools.ts`, `execute.ts`, `skills.ts` | Compact **6-tool progressive gateway surface** (`skills`, `search`, `describe`, `call`, `execute`, `resume`) with explicit runtime budgets & structured recovery errors. | `coax/lib/mcp/gateway/surface.coax` |
| `executor` | `packages/mcp/src/contracts/elicitation.ts`, `interactions.ts`, `implementation/native-elicitation.ts`, `packages/apps/src/implementation/mcp-call.ts` | **Multi-mode elicitation router** (upgraded to `mrtr`, `native`, `ui`, `model`, `automatic`) + **upstream MCP elicitation forwarding** through the gateway. | `coax/lib/mcp/gateway/elicitation.coax` |
| `executor` | `packages/authorization/src/contracts/policy.ts`, `packages/mcp-auth/src/contracts/grant*.ts` | Pure **capability/grant policy** (`actions`, `tools: all\|readOnly\|selected`, `targets: account_ids`, `events`), non-downgradeable approval channel enforcement, typed `GrantRefusal`. | `coax/lib/mcp/gateway/policy.coax` (`#authority`) |
| `executor` | `packages/apps/src/contracts/approval.ts` | **Tool-owned approval predicate** evaluated *after* strict schema decoding (`approved \| denied \| user_approval`), fail-closed on predicate error. | `coax/lib/mcp/gateway/approval.coax` (`#authority`) |
| `executor` | `packages/sdk/src/contracts/account.ts`, `CONTEXT.md` | **Provider → Account → Connector slot** separation; agent-readable account `description` for disambiguation; typed `AccountCheckStatus` health states; zero secret exposure in metadata. | `coax/lib/mcp/gateway/accounts.coax` + OpenAgents Vault |
| `executor` | `packages/sdk/src/contracts/declarations.ts` | **Stale-While-Revalidate (SWR) tool catalog** (`fresh_ms`, `max_stale_ms`, `refresh_ms`, `load_ms`), keyed by `(connector, version, account_set_hash)`. | `coax/lib/mcp/gateway/catalog.coax` |
| `integrations` | `DEFINITIONS.md`, domain/surface schema | **Execution-level domain allowlists** mechanically derived from `depends_on` service edges; vendor-official connector registry schema (MCP + OpenAPI + auth prose). | `coax/lib/mcp/gateway/egress.coax` (`#authority`) + connector catalog JSON |
| `mcporter` | `src/config-imports.ts`, `config-normalize.ts`, `record` | **Multi-IDE MCP config discovery & import** (Claude Desktop/Code, Cursor, Codex, Windsurf, VS Code, OpenCode) with `${ENV}` resolution + **wire record/replay fixtures**. | `coax/lib/mcp/gateway/config_import.coax` + `tools/mcp-conformance/` |
| `emulate` | `packages/@emulators/*` (GitHub, PlanetScale, Miro, Zapier, generic MCP) | **Local stateful OAuth + DCR + MCP integration test targets** for real-world client testing beyond synthetic conformance fixtures. | CI integration suite (`tools/mcp-conformance/emulate-e2e.sh`) |
| `sunpeak` | Host runtime simulator + tool-state fixture pinning | **Deterministic MCP Apps (`ext-apps` / `ui://`) testing harness** for OpenAgents Desktop/Web interactive tool views. | Phase 6 `coax/lib/mcp/ext/apps.coax` test harness |

---

## 3. Unified gateway meta-tool surface

Dumping 200–1,000 upstream tools into an LLM's `tools/list` wastes tens of thousands of tokens and degrades tool selection. By default, the gateway exposes a **bounded, constant-size meta-tool surface** over a single MCP endpoint (while optionally allowing a per-grant **direct pass-through mode** for small/pinned tool sets):

### 3.1 Core meta-tools

| Tool | Purpose | Input summary | Output summary |
| --- | --- | --- | --- |
| `skills` | Progressive discovery of connector workflow guides (aligned with `io.modelcontextprotocol/skills` / SEP-2640). | `{}` (list all), `{ connector }` (list connector skills), `{ connector, name, file? }` (read doc). | Bounded skill summaries or markdown document; invalid combos return actionable `dependentRequired` guidance. |
| `search` | Bounded full-text / tag search over cached upstream tools. | `{ query, connector?, limit?, cursor? }` | Compact matches (`connector`, `tool`, 1-line summary, `readOnly` hint, account requirement status), capped by `max_search_page_bytes` + cursor + `unavailable[]` diagnostics for timed-out connectors. |
| `describe` | Load full JSON Schema 2020-12 `inputSchema` / `outputSchema`, annotations, and linked account choices for 1–N specific tools. | `{ tools: [{ connector, name }] }` | Exact schemas (preserving `$defs`/`$ref`), approval hint (`never \| conditional \| always`), linked accounts (`id`, `label`, `description` — never secrets). |
| `call` | Execute a single upstream tool directly with JSON arguments. | `{ connector, name, arguments, accounts? }` | Structured tool result (`content`, `structuredContent`, `isError`), or `PendingInteraction` / `InputRequiredResult` if approval/input is needed. |
| `execute` | **Code-Mode:** run a short deterministic script across multiple connectors in one turn (filtering, joining, paging, batching without flooding LLM context). | `{ code, connectors?, accounts? }` | Final script output (bounded by `max_output_bytes`), structured `call_log` trace (connector, tool, duration, outcome), or suspended `PendingInteraction` / `InputRequiredResult`. |
| `resume` | Exposed **only** in `model` / `ui` fallback modes (omitted when client uses `2026-07-28` MRTR or `2025-11-25` native elicitation). | `{ requestId, action: "accept" \| "decline" \| "cancel", content? }` | Resumed `call` / `execute` result (or next pending interaction). |

### 3.2 Actionable, recovery-oriented errors

Harvested from `executor/packages/mcp/src/contracts/execute.ts`: gateway errors returned to models MUST distinguish retryable infrastructure faults from missing human actions and **forbid silent identity substitution**:

- `ConnectorAccountRequired`: names the connector and slot, instructs the agent to tell the user to link an account in OpenAgents Vault, and explicitly states *"Never substitute another account automatically."*
- `ConnectorUnavailable` / `DiscoveryTimedOut`: partial results still succeed; failed connectors appear in `unavailable[]` with bounded diagnostics rather than failing the entire `search`.
- `GrantRefused`: names the exact refusal class (`app`, `tool`, `target`, `approval`, `events`) without leaking hidden connector names the grant cannot see.
- `BudgetExceeded`: names the exact limit hit (`max_code_bytes`, `max_steps`, `max_tool_calls`, `max_output_bytes`, `timeout_ms`) so the agent can narrow its query.

---

## 4. Domain model: Providers, Reusable Accounts & Credential Isolation

Harvested from `executor/CONTEXT.md` & `packages/sdk/src/contracts/account.ts`, adapted to OpenAgents' local-first Vault:

1. **Provider**: Normalized service auth definition (`provider_id`, named auth methods: `oauth2_mcp`, `oauth2_custom`, `bearer`, `api_key_headers`, `basic`, `none`). Matching provider definitions share saved accounts across connectors.
2. **Account**: An independently owned, reusable saved credential instance stored encrypted in OpenAgents Vault (`coax/lib/crypto/aes_gcm.coax` / platform keychain).
   - **Public metadata** (visible to gateway & agent): `account_id`, `provider_id`, `method`, `label`, `description` (human note for agent disambiguation, e.g. *"read-only prod"* vs *"staging writes"*), `health` (`healthy | credentials_rejected | forbidden | upstream_unavailable | check_failed`), `last_checked_ms`.
   - **Secret material** (tokens, refresh tokens, client secrets, API keys): stored strictly inside the Vault boundary, **never** serialized into `Account`, tool listings, logs, MRTR `requestState`, or the `execute` script environment.
3. **Connector (Upstream)**: One registered tool source bound to 0..N **Account Slots**:
   - **MCP Remote** (`coax/lib/mcp/http_client.coax` + `auth_client.coax` — Streamable HTTP `2026-07-28` / `2025-11-25` + legacy fallback).
   - **MCP Local Stdio** (`coax/lib/mcp/stdio_client.coax` over `coax/lib/sys/proc.coax`).
   - **OpenAPI / HTTP Adapter** (declarative REST spec compiled to MCP tools, with credentials injected by the gateway transport layer).
   - **Native Coax Provider** (in-process OpenAgents tools: Realm, `oasim`, `lean-run`, `executor` Class-E modules, workspace tools — zero serialization overhead via `memory_transport.coax`).
4. **Egress Allowlisting** (harvested from `integrations/DEFINITIONS.md`):
   - Every HTTP/OpenAPI/remote-MCP connector declares an explicit origin/host allowlist derived from its provider spec (`depends_on` edges).
   - Pure `#authority` egress checker (`coax/lib/mcp/gateway/egress.coax`) blocks redirects or tool-constructed URLs targeting unlisted hosts, private/link-local RFC 1918/169.254 ranges (unless explicitly marked local), or DNS-rebinding origins.

---

## 5. Authorization, Grants & Tool-Owned Approvals (`#authority`)

All authorization and approval decisions live in pure, zero-allocation `#authority` Coax with **0 surviving mutants**:

### 5.1 Policy model (`coax/lib/mcp/gateway/policy.coax`)

```text
Action          :: DISCOVER | READ | RUN | MANAGE | DATA
ToolScope       :: ALL | READ_ONLY | SELECTED(tool_names[])
EventScope      :: ALL | SELECTED(event_names[])
RunTarget       :: APP_DEFAULT | ACCOUNT(account_id)

ConnectorPerm   :: { connector_id, tools: ToolScope, events: EventScope, targets: RunTarget[] }
GrantPolicy     :: FULL | SCOPED(perms: ConnectorPerm[], required_approval: UI_ONLY | CLIENT_OR_UI)
Grant           :: { grant_id, client_id, policy: GrantPolicy, mode: ElicitationMode }
```

Invariants verified by `#test` / `#forall`:
- **Default-deny** on any unlisted connector, tool, account target, or subscription event.
- **Hidden existence:** `search` and `skills` filter through `GrantPolicy` *before* formatting, so unauthorized connectors/accounts never leak into search results or error suggestions.
- **Non-downgradeable approvals:** if `GrantPolicy.required_approval == UI_ONLY`, requests attempting to negotiate `model` or `native` approval modes are refused (`GrantRefusal.APPROVAL_MODE`).
- **Target pinning:** `targeted_links(policy, connector, linked_accounts)` strips any account ID not explicitly permitted by `ConnectorPerm.targets`.

### 5.2 Tool-owned approval rules (`coax/lib/mcp/gateway/approval.coax`)

Evaluated **strictly after** JSON Schema 2020-12 input validation succeeds:

```text
ApprovalDecision :: APPROVED | DENIED | USER_APPROVAL
```

Rule resolution order (first restrictive decision wins):
1. Explicit deny rule (e.g. destructive pattern or forbidden parameter value) → `DENIED`.
2. Connector/tool rule (`always_ask`, `read_only_auto`, or declarative predicate over validated input fields, e.g. `amount > 100` or `method != "GET"` or MCP tool annotation `destructiveHint == true`) → `USER_APPROVAL`.
3. Any policy evaluation error or unknown tool annotation → **fail closed** to `USER_APPROVAL` (never silently `APPROVED`).

---

## 6. 4-Mode Elicitation & Upstream Forwarding (Bridging `2025-11-25` and `2026-07-28`)

Harvested from `executor`'s multi-mode elicitation design and upgraded to make **MCP `2026-07-28` MRTR** a first-class citizen:

| Mode | Wire mechanism | When used |
| --- | --- | --- |
| **`mrtr`** *(2026-07-28 primary)* | Returns `InputRequiredResult` (`resultType: "input_required"`, `elicitation` form/URL, and HMAC/AEAD-sealed `requestState` from `coax/lib/mcp/mrtr.coax`). Client re-issues request with `requestState` + `_meta["io.modelcontextprotocol/elicitationResponse"]`. | Any `2026-07-28` stateless client supporting MRTR. **Zero sticky session needed.** |
| **`native`** *(2025-11-25)* | Sends server→client JSON-RPC `elicitation/create` over the active SSE/stdio stream and awaits response within `expires_at`. | `2025-11-25` stateful clients advertising `capabilities.elicitation.form`. |
| **`ui`** *(Desktop / Browser)* | Suspends execution, creates a bounded `PendingInteraction` (`apr_...` / `elc_...`), pushes an approval/form card to OpenAgents Desktop/Web/iOS UI, and resumes when the human decides (or expires). | Local OpenAgents hosts or `UI_ONLY` grants where the human must approve outside the chat channel. |
| **`model`** *(universal fallback)* | Returns `PendingInteraction` as structured tool output instructing the agent to ask the user and call `resume(requestId, response)`. | Clients with neither MRTR nor native elicitation (and only when `GrantPolicy` permits `CLIENT_OR_UI`). |
| **`automatic`** *(default)* | Selects `mrtr` (`2026-07-28`) → `native` (`2025-11-25` + capability) → `ui` (if OpenAgents UI attached) → `model` (if permitted by grant; otherwise refuses). | Default gateway mode. |

### Upstream MCP elicitation forwarding

When a gateway `call` or `execute` invokes an **upstream** MCP server that itself returns:
- `2026-07-28` `InputRequiredResult`, or
- `2025-11-25` server→client `elicitation/create` (form or URL),

the gateway validates the upstream elicitation schema, enforces `GrantPolicy` (`authorize_elicitation`), pauses the active `call`/`execute` frame, and routes the prompt through the **exact same outer elicitation channel** (`mrtr` / `native` / `ui` / `model`). Once the human responds, the gateway forwards the reply to the upstream MCP server (never exposing upstream tokens or internal `requestState` to the downstream client).

---

## 7. Deterministic Sandboxed Code-Mode (`coax/lib/rlm/script.coax` Adaptation)

Instead of embedding V8 (`workerd`), OpenAgents adapts its own pure-Coax deterministic interpreter (`coax/lib/rlm/script.coax` + `coax/lib/rlm/policy.coax`) for the `execute` meta-tool:

### Why `script.coax` is a strict upgrade over `workerd` here

1. **Zero external runtime & total confinement:** Python-subset syntax (which LLMs write reliably) running in caller-owned arenas with no `import` I/O, no `open`, no shell, no sockets, and no wall clock.
2. **Native suspend-memo-replay:** In `coax/lib/rlm/run.coax`, the interpreter never performs I/O directly. When a script calls `tool(connector, name, args)` or `batch([(connector, name, args), ...])`:
   - If the call index is already in the execution **memo**, the interpreter returns the memoized JSON value immediately.
   - Otherwise, the interpreter **suspends deterministically**, hands the pending call batch to the gateway driver, which checks `GrantPolicy` → validates input schema → checks `ApprovalPolicy` → injects Vault credentials → calls the upstream MCP/HTTP/native tool → appends the sanitized result to the memo → restores the script snapshot and replays.
3. **Seamless MRTR & approval suspension:** Because program state is completely represented by `(code, memo, pending_call_index)`, pausing mid-script for human approval or upstream elicitation requires **no live OS thread or pinned V8 isolate**! For `2026-07-28` MRTR, a sealed digest/handle of the memoized call log can resume cleanly across stateless HTTP requests (with bounded server-side interaction store for large memos, expiring at `ttl_ms`).

### Explicit Code-Mode & Catalog budgets (`#authority` checked)

Harvested and tightened from `executor/packages/mcp/src/contracts/execute.ts` & `packages/sdk/src/contracts/declarations.ts`:

| Limit | Default | Enforcement |
| --- | --- | --- |
| `max_code_bytes` | `65_536` (64 KiB) | Refused before lexing. |
| `max_steps` | `250_000` interpreter steps | Halts script cleanly with `BudgetExceeded`. |
| `max_tool_calls` | `32` upstream calls per `execute` | Checked before each dispatch; prevents runaway loops. |
| `max_batch_concurrency` | `8` concurrent upstream calls | Bounded parallel poll slots. |
| `max_output_bytes` | `65_536` (64 KiB) returned to LLM | Truncated with structured warning + byte counts. |
| `max_active_executions` | `64` | Bounded table; HTTP `429` / JSON-RPC `-32029` when full. |
| `discovery_concurrency` | `8` upstreams refreshed in parallel | Prevents thundering herd on startup. |
| `discovery_wait_ms` | `10_000` ms | Partial search returns immediately; slow upstreams listed in `unavailable[]`. |
| **SWR catalog cache** | `fresh_ms = 30_000`, `max_stale_ms = 7d`, `refresh_ms = 30_000`, `load_ms = 45_000` | Keyed by `(connector_id, version, account_set_hash)`; honors upstream `2026-07-28` `CacheableResult` (`ttlMs`, `cacheScope`) when shorter. |

---

## 8. Client onboarding, catalog & verification assets to harvest

1. **IDE Config Importer (from `mcporter/src/config-imports.ts`):**
   - `openagents mcp import` discovers and normalizes existing MCP server definitions from Claude Desktop, Claude Code (`~/.claude.json`, `.mcp.json`), Cursor (`~/.cursor/mcp.json`), Codex, Windsurf, VS Code, and OpenCode, expanding `${ENV}` references safely without persisting plaintext secrets outside OpenAgents Vault.
2. **Record / Replay Fixtures (from `mcporter`):**
   - Built-in wire recorder wrapping `coax/lib/mcp/` transports to capture sanitized JSON-RPC + SSE transcripts from real third-party servers into deterministic offline replay tests.
3. **Connector Directory Schema (from `integrations` / `executor-apps`):**
   - Optional static JSON feed describing popular official MCP servers and OpenAPI surfaces (endpoint URL, transport, OAuth/API-key method, setup guide, `depends_on` host allowlist) so OpenAgents users can add GitHub, Linear, Stripe, Slack, etc. in one click.
4. **Realistic E2E Emulator Suite (from `emulate`):**
   - In CI (separate from `@modelcontextprotocol/conformance`), spin up `emulate`'s local stateful OAuth + DCR + MCP services (GitHub MCP, PlanetScale MCP, Miro MCP, Zapier MCP, generic MCP) to exercise our Coax MCP client + gateway account/OAuth/catalog flows against realistic third-party quirks with zero network dependency.
5. **MCP Apps Testing Pattern (from `sunpeak`):**
   - When implementing `coax/lib/mcp/ext/apps.coax` (`ui://` interactive UI resources), adopt `sunpeak`'s pinned tool-state simulation fixtures so Desktop/Web UI widgets render deterministically in CI without live LLM credits.

---

## 9. Overall strategy for harvesting and pushing this into OpenAgents

### Four non-negotiable rules

1. **Conformance first, gateway second (never interleave the two):**
   Do not start implementing gateway features until Layer 1 (`coax/lib/mcp/`) passes **100%** of `@modelcontextprotocol/conformance` for both `2025-11-25` and `2026-07-28`. Building gateway shortcuts before the protocol layer is locked creates leaky abstractions that break conformance later.
2. **Harvest concepts into `#authority` contracts first — zero porting of TS code:**
   Every harvested idea starts as a tiny, standalone, zero-heap `#authority` Coax contract (`policy.coax`, `approval.coax`, `egress.coax`, `elicitation.coax`, `catalog.coax`) proven with `#test` / `#forall` and **0 surviving mutants** under `./coax/bin/coax mutate`, before any I/O or UI is wired.
3. **Reuse OpenAgents' existing superpowers instead of foreign runtimes:**
   - Sandboxing & Code-Mode → `coax/lib/rlm/script.coax` (NOT `workerd` / V8 / Node).
   - Crypto, PKCE, JWT, sealing → `coax/lib/crypto/` & `coax/lib/workos/` (NOT JS auth libs).
   - Wire protocol & OAuth 2.1 → `coax/lib/mcp/` (NOT `effect/ai` or `@modelcontextprotocol/sdk`).
4. **Ship thin vertical slices gated by real E2E tests:**
   Every slice proves itself end-to-end (in-memory → stdio → Streamable HTTP → `emulate` OAuth servers → Desktop/CLI hosts) before adding the next capability.

### Concrete execution roadmap (mapping onto Phases 0–8)

| Stage | Milestone | What gets pushed in | Exit gate |
| --- | --- | --- | --- |
| **Phases 0–5** *(from [coax-architecture-and-plan.md](coax-architecture-and-plan.md))* | **100% Conformant `coax/lib/mcp/`** | Core `#authority` protocol, stdio, Streamable HTTP server & client, OAuth 2.1 client & resource server. *(While building Phase 4–5, keep `elicitation` delivery decoupled from transport and validate client against `emulate`'s local OAuth+MCP servers in addition to official conformance.)* | **100% pass** on `@modelcontextprotocol/conformance` (`2025-11-25` 30+18, `2026-07-28` 37+32) + green `emulate` smoke test. |
| **Phase 6** | **Official Extensions (`coax/lib/mcp/ext/`)** | `ext/skills.coax` (SEP-2640), `ext/tasks.coax`, `ext/auth_ext.coax`, `ext/server_card.coax`, `ext/apps.coax` (`ui://` tested with `sunpeak`-style pinned fixtures). | Passing conformance extension suites. |
| **Phase 7** | **Repo Migration & Host Client Manager** | Migrate all 5 existing ad-hoc MCP files (`apps/openagents-cli/{mcp,executor-mcp}.coax`, `coder/tui/{oasim,lean-run}-mcp.coax`, `coax/lib/agent/claude/mcp.coax`) onto `coax/lib/mcp/`; add `mcporter`-inspired IDE MCP config importer (`openagents mcp import`). | Zero ad-hoc MCP framing left in repo; one-command import from Claude/Cursor/Codex configs. |
| **Phase 8A** | **Gateway `#authority` Core (`coax/lib/mcp/gateway/`)** | Pure `#authority` `policy.coax`, `approval.coax`, `egress.coax`, `accounts.coax`, `catalog.coax` (SWR cache), `elicitation.coax` (4-mode router), `surface.coax` (`skills`/`search`/`describe`/`call`/`resume`). | `coax test` + `coax mutate` (**0 surviving mutants**) + in-memory multi-upstream gateway tests. |
| **Phase 8B** | **Code-Mode (`execute`) & Upstream Forwarding** | Adapt `coax/lib/rlm/script.coax` memo/suspend engine to gateway `execute`; wire MRTR `requestState` + `native`/`ui`/`model` approval & upstream elicitation forwarding; wire Vault account injection. | Multi-connector Code-Mode scripts pause/resume cleanly across all 4 elicitation modes; secret-leakage & SSRF fuzz tests pass 100%. |
| **Phase 8C** | **Product Surface & Connector Catalog** | `openagents gateway` local MCP endpoint (`127.0.0.1` + stdio) usable from OpenAgents Desktop, Coder, CLI, Claude Code, and Cursor; Desktop Vault account/approval UI; curated connector catalog feed (inspired by `integrations.sh` / `executor-apps`). | Single OpenAgents MCP gateway drives built-in Coax tools + imported IDE MCP servers + remote OAuth MCP servers with live approval sheets and zero leaked credentials. |
