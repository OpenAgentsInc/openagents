# OpenAgents Exa Web Search & Metered MCP Specification

**Status:** Implementing  
**Issues:** #14114 (metered REST proxy & billing), #14113 (native `WebSearch` tool), #14115 (metered `/api/v1/mcp/exa`)  
**Owner:** OpenAgents Cloud & Core  
**Scope:** Central metered Exa REST proxy (`/api/v1/search`, `/api/v1/fetch`), first-party metered Streamable HTTP MCP endpoint (`/api/v1/mcp/exa`), prepaid credit & x402 billing integration (`inference.*`), and native `WebSearch` core tool (`openagents/core/wire.coax`, `coder/tui/oa-worker.coax`).

---

## 1. Motivation & Design Principles

OpenAgents Core needs Claude Code–parity `WebSearch` across every supported model backend (Gemini, OpenAgents Cloud, OpenRouter, and local Inkling / Psionic Metal) without shipping a provider API key to Desktop or CLI binaries.

We also want to consume and expose Exa via MCP as our full MCP client ships, while operating **one centrally-managed `EXA_API_KEY`** billed through OpenAgents' existing prepaid Lightning/x402 credit system (`inference.credit`, `inference.calls`, `inference.models`).

### Why `oa-api` calls `api.exa.ai` directly instead of proxying `https://mcp.exa.ai/mcp`

Inspection of Exa's official `exa-mcp-server` (`src/toolRegistry.ts`, `src/tools/webSearch.ts`, `src/tools/webFetch.ts`) reveals two architectural facts:

1. **8 of 12 legacy Exa MCP tools are deprecated.** The active surface is just two core primitives (`web_search_exa` → `POST /search` and `web_fetch_exa` → `POST /contents`) plus opt-in advanced search and multi-step `agent_run` ($1–$20/run).
2. **`exa-mcp-server` strips `costDollars` from MCP responses** (only returning `_meta: { searchTime }`). Proxying `mcp.exa.ai/mcp` would blind OpenAgents' billing ledger to Exa's actual per-call spend while adding an extra network hop (`Client → oa-api → Vercel → api.exa.ai`).

Calling `https://api.exa.ai/search` and `https://api.exa.ai/contents` directly from `oa-api` yields:
- Exact per-call upstream cost via `costDollars.total` on every response.
- Single-hop latency and strict server-side parameter clamping on the shared key.
- Unified metering across **both** the native REST API (`/api/v1/search`, `/api/v1/fetch`) and OpenAgents' first-party metered MCP endpoint (`/api/v1/mcp/exa`).
- Zero-cost BYOA path: once Desktop's remote MCP + OAuth 2.1 client lands, users who want to use their own Exa account can connect directly to `https://mcp.exa.ai/mcp`.

---

## 2. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│ OpenAgents Desktop / CLI / oa-worker / External MCP Clients             │
│                                                                         │
│  [Native WebSearch tool]               [Streamable HTTP MCP Client]     │
│   POST /api/v1/search                   POST /api/v1/mcp/exa            │
│   (or BYOK EXA_API_KEY fallback)        (tools: web_search_exa,         │
│                                                 web_fetch_exa)          │
└───────────────────┬─────────────────────────────────┬───────────────────┘
                    │ NIP-98 or Account Session Bearer │
                    ▼                                 ▼
┌─────────────────────────────────────────────────────────────────────────┐
│ oa-api (coax/web/api/exa.c + exa-policy.coax + inference.c)             │
│  1. inference_auth_caller(): NIP-98 or logged-in account principal      │
│  2. wallet_credit_refused() + inference_rate_refresh()                  │
│  3. SQL inference.search_admit(): credit > 0 (402), fresh BTC/USD (503),│
│     daily request & spend caps (429)                                    │
│  4. #authority exa-policy.coax: clamp num_results, max_chars, urls,     │
│     domains, search type, SSRF URL check                                │
│  5. Upstream POST to https://api.exa.ai/{search,contents}               │
│     Header: x-api-key: $EXA_API_KEY                                     │
│     Header: Exa-Beta: dynamic-highlights-2026-08-28 (for highlights)    │
│  6. Parse exact costDollars.total -> upstream_usd_micros                │
│  7. SQL inference.search_finish(): retail = ceil(up * (10000+bps)/10000)│
│     debit inference.credit (msat), record inference.calls (kind='search')│
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Billing & Database Specification (`migrations/0220_exa_search_billing.sql`)

### 3.1 Catalog Rows in `inference.models`

| `name` | `upstream` | `kind` | `markup_bps` | `per_call_micros` (fallback retail) |
|---|---|---|---|---|
| `exa/search` | `https://api.exa.ai/search` | `search` | `2500` (+25%) | `6250` ($0.00625; $0.005 upstream × 1.25) |
| `exa/contents` | `https://api.exa.ai/contents` | `search` | `2500` (+25%) | `1250` ($0.00125; $0.001 upstream × 1.25) |

Both rows automatically appear in `GET /api/v1/inference/pricing`.

### 3.2 Admission: `inference.search_admit(who, model_name, cap_n, cap_usd_micros, free)`

1. Verifies `model_name` exists in `inference.models` with `kind = 'search'` (else `400`).
2. If `NOT free`:
   - Checks `inference.credit.msat > 0` for `who`; if `<= 0`, returns `402` with `credit = '/api/v1/inference/credit'` and `credit_msat`.
   - Checks `inference.rate_age() <= 3600`; if missing or stale, returns `503`.
3. Increments `inference.account_day.n` for `(who, current_date)` and enforces:
   - `d.n <= cap_n` (default `OA_EXA_DAILY_REQUESTS=250`, else `429`).
   - `d.usd_micros < cap_usd_micros` (default `OA_EXA_DAILY_USD_MICROS=2000000`, i.e., $2.00/day, else `429`).
4. Returns `{"status": 200, "model": m.name, "markup_bps": m.markup_bps, "per_call_micros": m.per_call_micros, "free": free}`.

### 3.3 Settlement: `inference.search_finish(who, model_name, http, upstream_usd_micros, ms_taken, free)`

1. On `http = 200`:
   - If `upstream_usd_micros > 0`:
     - `up_cost = upstream_usd_micros`
     - `cost = (up_cost * (10000 + markup_bps) + 9999) / 10000` (exact ceiling integer arithmetic)
   - Else (upstream omitted `costDollars`):
     - `cost = m.per_call_micros`
     - `up_cost = (cost * 10000) / (10000 + markup_bps)`
2. On `http != 200`: `cost = 0`, `up_cost = 0` (failed upstream requests are logged but never charged).
3. Inserts into `inference.calls` with `kind = 'search'`, `usd_micros = cost`, `upstream_usd_micros = up_cost`, `ms_taken`.
4. Updates `inference.account_day.usd_micros += cost`.
5. If `NOT free AND cost > 0`:
   - Converts `cost` USD micros to `msat = ceil(cost * 100000 / usd_per_btc)`.
   - Debits `inference.credit.msat -= msat`, sets `inference.calls.msat = msat`, and records spend in `auth.credit_spends`.
6. Returns `{"usd_micros": cost, "upstream_usd_micros": up_cost, "msat": msat}`.

### 3.4 Usage Reporting: `inference.usage_summary(who)`

Adds a `search` object alongside `total` and `decisions`:
```json
"search": {
  "calls": 12,
  "usd_micros": 75000,
  "msat": 83333
}
```

---

## 4. Pure `#authority` Policy (`coax/web/api/exa-policy.coax`)

All clamping, domain validation, SSRF guarding, and cost-parsing invariants live in a single `#authority` Coax module compiled to C (`exa-policy.generated.{c,h}`):

- **Shared-key guardrails:**
  - `exa_clamp_num_results(n)`: default `8`, clamped to `[1, 10]`.
  - `exa_clamp_max_chars(n)`: default `3000`, clamped to `[200, 12000]`.
  - `exa_valid_url_count(n)`: requires `1 <= n <= 5`.
  - `exa_valid_search_type(t)`: allows `""`, `"auto"`, `"fast"` (blocks unguarded `"deep"`, `"deep-reasoning"`, or `"agent"` spend spikes on the shared central key).
  - `exa_valid_category(c)`: allows `""`, `"company"`, `"research paper"`, `"news"`, `"personal site"`, `"financial report"`, `"people"`, `"tweet"`, `"github"`, `"pdf"`.
  - `exa_valid_domain(d)`: 1–253 ASCII chars (`[a-z0-9.-]`), must contain `.`, cannot start/end with `.` or `-`, no scheme/path/whitespace.
  - `exa_url_safe(url)`: requires `http://` or `https://` and rejects `localhost`, `127.*`, `::1`, `0.*`, `10.*`, `169.254.*`, `192.168.*`, `172.16-31.*`, `.local`, `.internal`.
- **Exact decimal cost parsing (`exa_cost_dollars_to_micros`):**
  - Parses JSON numeric text from `costDollars.total` (e.g. `"0.005"` → `5000`, `"0.0125"` → `12500`, `"1.5"` → `1500000`, `"5e-3"` → `5000`, rounding fractional sub-micros up) using integer arithmetic only.
- **Retail price ceiling (`exa_retail_micros`):**
  - Matches SQL `inference.search_finish` identically.

---

## 5. HTTP & MCP API Surface (`coax/web/api/exa.c`)

All routes live under `/api/v1/` (covered by `auth.c`'s `R("*", "/api/v1/", true, AP_GUEST, ...)` rule) and authenticate via `inference_auth_caller()` (NIP-98 `"Authorization: Nostr ..."` OR OpenAgents account Bearer/Cookie session).

### 5.1 `POST /api/v1/search`

**Request JSON:**
```json
{
  "query": "Coax compiler effect system documentation",
  "num_results": 8,
  "type": "auto",
  "category": "github",
  "allowed_domains": ["github.com"],
  "blocked_domains": ["pinterest.com"],
  "start_published_date": "2026-01-01",
  "end_published_date": "2026-12-31",
  "highlights": true,
  "max_characters": 3000
}
```
- Also accepts Exa-native aliases (`include_domains`, `exclude_domains`, `numResults`, `maxCharacters`) and inline `category:<type>` prefix/token in `query` (matching `exa-mcp-server`).
- `allowed_domains` and `blocked_domains` are mutually exclusive (400 if both non-empty, matching Claude Code `WebSearchTool` parity). Max 20 domains.
- By default sets `contents: { highlights: { dynamic: true } }` and sends `Exa-Beta: dynamic-highlights-2026-08-28` so Exa selects concise highlights under a shared token budget. Setting `"highlights": false` requests `contents: { text: { maxCharacters } }`.

**Response JSON (HTTP 200):**
```json
{
  "results": [
    {
      "title": "...",
      "url": "https://...",
      "publishedDate": "2026-11-10",
      "author": "...",
      "highlights": ["..."],
      "text": "..."
    }
  ],
  "searchTime": 412.5,
  "billing": {
    "model": "exa/search",
    "upstream_usd_micros": 5000,
    "usd_micros": 6250,
    "msat": 6945,
    "free": false
  }
}
```

### 5.2 `POST /api/v1/fetch`

**Request JSON:**
```json
{
  "urls": ["https://docs.exa.ai/reference/search"],
  "max_characters": 3000
}
```
- Also accepts single `"url": "https://..."` convenience field and `"maxCharacters"`.
- Validates 1–5 URLs with `exa_url_safe()`.
- Calls `POST {OA_EXA_BASE_URL}/contents` with `{ urls, text: { maxCharacters } }`.
- Returns `{ results, statuses, searchTime, billing }` metered under `exa/contents`.

### 5.3 `POST /api/v1/mcp/exa` (First-Party Metered Streamable HTTP MCP)

Implements MCP Streamable HTTP (`2025-03-26`, JSON-RPC 2.0 over `POST /api/v1/mcp/exa`) backed directly by the metered `exa/search` and `exa/contents` handlers:

- `initialize`: returns `protocolVersion: "2025-03-26"`, `capabilities: { tools: {} }`, `serverInfo: { name: "openagents-exa", version: "1.0.0" }`.
- `notifications/initialized`: returns `202 Accepted` (or empty 200 JSON-RPC ack).
- `ping`: returns `{}`.
- `tools/list`: returns two tools with `readOnlyHint: true, openWorldHint: true, idempotentHint: true`:
  1. `web_search_exa` (`query`, optional `numResults`, `allowed_domains`, `blocked_domains`, `category`)
  2. `web_fetch_exa` (`urls`, optional `maxCharacters`)
- `tools/call`: dispatches to the exact same admission, clamping, Exa REST call, `costDollars` parsing, and `inference.search_finish` billing pipeline, returning MCP `content: [{ type: "text", text: "<formatted markdown>" }]` plus `_meta: { searchTime, billing }`.
- Returns JSON-RPC error `-32002` (with HTTP 402 or `isError: true` message) when prepaid credit is exhausted.

---

## 6. Native `WebSearch` Tool in OpenAgents Core

### 6.1 Wire Contract (`openagents/core/wire.coax`)

- `FLAWED_WIRE_NTOOLS` increases from `8` to `9`, adding `WebSearch` right alongside `WebFetch`:
  ```json
  {
    "type": "object",
    "properties": {
      "query": {"type": "string", "description": "Natural language search query"},
      "allowed_domains": {"type": "array", "items": {"type": "string"}, "description": "Only include results from these domains"},
      "blocked_domains": {"type": "array", "items": {"type": "string"}, "description": "Exclude results from these domains"},
      "num_results": {"type": "integer", "description": "Number of results (1-10, default 8)"},
      "category": {"type": "string", "enum": ["company", "research paper", "news", "personal site", "financial report", "people", "tweet", "github", "pdf"]}
    },
    "required": ["query"]
  }
  ```
- Concurrency-safe (`flawed_wire_tool_is_concurrency_safe("WebSearch") == true`).
- Read-only (permitted inside read-only subagents such as `Explore` and `Plan`).
- Permission policy: identical to `WebFetch` (`ALLOW` in Allow All / Auto / AcceptEdits / DontAsk / session_allow; `ASK` in Default / Plan).

### 6.2 Runtime Execution (`coder/tui/oa-worker.coax`)

`fww_tool_websearch` executes searches with zero provider key shipped in the binary:

1. **Primary path — OpenAgents Cloud Metered Proxy (`POST {base}/api/v1/search`):**
   - Resolves OpenAgents Cloud base URL (`OA_CLOUD_BASE_URL` or `https://openagents.com`) and credentials via `auth` / `fww_try_ours_auth`:
     - `OA_CLOUD_AUTH_HEADER`
     - `OA_CLOUD_TOKEN` / `~/.openagents/desktop/login.json` (`Authorization: Bearer ...`)
     - Local CLI Nostr signer via `fww_sign_ours_nip98` (`Authorization: Nostr ...`)
   - Sends JSON payload `{ query, num_results, allowed_domains, blocked_domains, category }`.
   - Formats returned `results[]` into clean markdown (`Title`, `URL`, `Published`, `Author`, `Highlights` / `Text`) with a concise citation reminder.
   - Surfaces actionable errors on `401` (log in with `openagents login` or Desktop), `402` (top up credit at `/api/v1/inference/credit`), `429` (daily cap reached), or `503` (search temporarily unavailable).
2. **Local BYOK fallback — Direct Exa (`POST {OA_EXA_BASE_URL or https://api.exa.ai}/search`):**
   - Used automatically when `OA_EXA_DIRECT=1` or when no OpenAgents Cloud credential is present and `EXA_API_KEY` is set in the environment.
   - Sends `x-api-key: $EXA_API_KEY` and `Exa-Beta: dynamic-highlights-2026-08-28` with `contents: { highlights: { dynamic: true } }`.

---

## 7. Configuration & Operations

### Environment Variables (`oa-api`)

| Variable | Default | Secret | Purpose |
|---|---|---|---|
| `EXA_API_KEY` | *(required for search)* | Yes | Central OpenAgents Exa API key (Secret Manager: `oa-exa-api-key`) |
| `OA_EXA_BASE_URL` | `https://api.exa.ai` | No | Upstream Exa base URL (overrideable in tests) |
| `OA_EXA_DAILY_REQUESTS` | `250` | No | Per-account daily Exa request cap |
| `OA_EXA_DAILY_USD_MICROS` | `2000000` | No | Per-account daily spend cap ($2.00) |

### Operational Checklist
- Store central Exa key in GCP Secret Manager as `oa-exa-api-key` (bound automatically by `coax/web/api/deploy.sh` when present).
- Verify Zero Data Retention (ZDR) terms on the central Exa organization account before advertising ZDR coverage for cloud search.
- Monitor `inference.calls WHERE kind = 'search'` margin (`sum(usd_micros) - sum(upstream_usd_micros)`) and error rates.
