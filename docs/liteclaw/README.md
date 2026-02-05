# LiteClaw Documentation

This folder documents the LiteClaw runtime, programmatic surfaces, and Sky-mode behavior. Start here if you need to understand how the worker, tools, extensions, and export format fit together.

## Overview

LiteClaw is a Cloudflare Workers runtime backed by Durable Objects and the Cloudflare Agents SDK. The UI in `apps/web` talks to the LiteClaw worker over the Agents SDK websocket, and the Durable Object owns the canonical transcript and memory.

Primary code locations:

- `apps/liteclaw-worker/` runs the Agents SDK worker and Durable Object.
- `apps/web/` renders the Hatchery and `/chat/{id}` UI.
- `apps/liteclaw-local-agent/` is the optional tunnel executor for workspace tools.

Related docs:

- `docs/liteclaw/spec.md` for scope and roadmap.
- `docs/liteclaw/sky-export-compat.md` for export compatibility.
- `docs/liteclaw/tunnel.md` for tunnel setup and security.
- `docs/liteclaw/cloudflare-tunnel.md` for Cloudflare Tunnel architecture and Access patterns.

## Runtime Flow

1. The browser connects to `WS /agents/chat/{id}`.
2. The LiteClaw DO (class `Chat`) receives messages via `AIChatAgent`.
3. The DO streams model deltas back over the websocket and persists messages in SQLite.
4. Sky-mode instrumentation, tool receipts, and run/event metadata are appended in parallel when `LITECLAW_SKY_MODE=1`.

Default model: `@cf/openai/gpt-oss-120b` via Workers AI. This model is tool-capable and should emit real tool calls when the tool policy allows it.

## Programmatic Endpoints

These surfaces are treated as stable and are required for automation.

- `WS /agents/chat/{id}` for streaming chat.
- `GET /agents/chat/{id}/get-messages` for transcript rehydration.
- `GET /agents/chat/{id}/export` for Sky JSONL export.
- `GET|POST /agents/chat/{id}/tool-policy` for per-thread tool policy.
- `GET|POST /agents/chat/{id}/extensions` for extension policy.
- `GET|POST /agents/chat/{id}/extensions/catalog` for extension catalog updates.

Admin endpoints accept the `x-liteclaw-admin-secret` header. Tool policy uses `LITECLAW_TOOL_ADMIN_SECRET` (fallback to `LITECLAW_EXTENSION_ADMIN_SECRET`), and extension admin uses `LITECLAW_EXTENSION_ADMIN_SECRET`.

Error responses are JSON when possible:

```json
{
  "ok": false,
  "code": "error_code",
  "message": "Human-readable message",
  "thread_id": "chat-id",
  "run_id": "optional-run-id"
}
```

## Sky Mode and Export

Sky mode is enabled by `LITECLAW_SKY_MODE=1`. It adds run/event/receipt logging and a JSONL export format. The export endpoint emits JSON lines with these types:

- `liteclaw.export` header line
- `memory`
- `message`
- `run`
- `event`
- `receipt`

Schema validation lives in `apps/liteclaw-worker/src/sky/contracts.ts`. Export compatibility and expectations are documented in `docs/liteclaw/sky-export-compat.md`.

## Tools

LiteClaw tools are gated by a per-thread policy and per-run budgets.

Built-in tools:

- `http.fetch` for allowlisted HTTP requests.
- `summarize` for internal summarization.
- `extract` for structured extraction.
- `workspace.read`, `workspace.write`, `workspace.edit` for file operations.

Tool policies:

- `none` disables tools.
- `read-only` allows read tools only.
- `read-write` allows all tools.

Policy is stored per thread in `sky_tool_policy` and can be managed via `/tool-policy`.

## Extensions

Extensions are manifest-driven and loaded from a catalog with allowlist enforcement.

Key rules:

- Policies require pinned versions (`id@version`).
- Tools must be declared in `manifest.tools`.
- Prompt-only extensions are allowed when `system_prompt` is present and no tools are defined.

Built-in sample:

- `sky.echo@0.1.0` adds the `extension.echo` tool.

Extension policies are stored in `sky_extension_policy` and updated via `/extensions`. Catalog entries are stored in `sky_extensions` and updated via `/extensions/catalog`.

## Configuration Reference

These env vars are used by the LiteClaw worker. Defaults are the current runtime defaults in `apps/liteclaw-worker/src/server.ts`.

| Env var | Purpose | Default |
| --- | --- | --- |
| `LITECLAW_SKY_MODE` | Enable Sky logging and export. | `"0"` |
| `LITECLAW_TOOL_POLICY` | Default per-thread tool policy. | `none` |
| `LITECLAW_TOOL_CHOICE` | Tool choice mode (`auto`, `none`, `required`, or `tool:<name>`). | `auto` |
| `LITECLAW_TOOL_MAX_CALLS` | Max tool calls per run. | `4` |
| `LITECLAW_TOOL_MAX_OUTBOUND_BYTES` | Max tool output bytes per run. | `200000` |
| `LITECLAW_TOOL_ADMIN_SECRET` | Admin secret for `/tool-policy`. | none |
| `LITECLAW_EXECUTOR_KIND` | Tool executor (`workers` or `tunnel`). If unset and tunnel vars are configured, defaults to `tunnel`. | `workers` |
| `LITECLAW_TUNNEL_URL` | Tunnel base URL for local tools. | none |
| `LITECLAW_TUNNEL_TOKEN` | Token used for tunnel auth and receipts. | none |
| `LITECLAW_TUNNEL_TIMEOUT_MS` | Tunnel request timeout. | `8000` |
| `LITECLAW_TUNNEL_ACCESS_CLIENT_ID` | Optional Cloudflare Access client id. | none |
| `LITECLAW_TUNNEL_ACCESS_CLIENT_SECRET` | Optional Cloudflare Access client secret. | none |
| `LITECLAW_HTTP_ALLOWLIST` | Comma-separated host allowlist for `http.fetch`. | none |
| `LITECLAW_HTTP_MAX_BYTES` | Max bytes returned by `http.fetch`. | `50000` |
| `LITECLAW_HTTP_TIMEOUT_MS` | HTTP timeout for `http.fetch`. | `8000` |
| `LITECLAW_EXTENSION_ALLOWLIST` | Allowlisted extensions. | none |
| `LITECLAW_EXTENSION_DEFAULTS` | Default enabled extensions per thread. | none |
| `LITECLAW_EXTENSION_CATALOG_URL` | External catalog URL (fallback). | none |
| `LITECLAW_EXTENSION_CATALOG_JSON` | Inline catalog JSON. | none |
| `LITECLAW_EXTENSION_CATALOG_KEY` | KV/R2 catalog key. | `extensions/catalog.json` |
| `LITECLAW_EXTENSION_ADMIN_SECRET` | Admin secret for `/extensions*`. | none |
| `LITECLAW_EXTENSION_KV` | KV binding for catalog storage. | none |
| `LITECLAW_EXTENSION_BUCKET` | R2 binding for catalog storage. | none |

Local executor env vars live in `apps/liteclaw-local-agent/README.md` and `docs/liteclaw/tunnel.md`.

Note: When using Cloudflare Access service tokens via API, the Access policy must include `service_token.token_id` in the `include` rule (see `docs/liteclaw/cloudflare-tunnel.md`).

## Local Dev and Testing

Worker development:

```bash
cd apps/liteclaw-worker
npm run dev
```

Tests:

```bash
cd apps/liteclaw-worker
npm test
```

Smoke harness (requires `wrangler dev` running and admin secrets configured):

```bash
cd apps/liteclaw-worker
npm run smoke
```

The smoke harness is implemented in `apps/liteclaw-worker/scripts/liteclaw-smoke.ts` and validates:

- Websocket streaming and TTFT.
- Transcript persistence.
- Tool policy changes.
- Tool receipts and export JSONL schema validation.
- Extension policy and catalog updates.
- Tunnel receipts when `LITECLAW_EXECUTOR_KIND=tunnel`.

Tunnel smoke harness (local agent only):

```bash
cd apps/liteclaw-local-agent
LITECLAW_TUNNEL_URL=https://local-tools.example.com \
LITECLAW_TUNNEL_TOKEN=replace-me \
node scripts/tunnel-smoke.js
```

Bun handshake script (minimal tunnel check):

```bash
cd apps/nydus
LITECLAW_TUNNEL_URL=https://local-tools.example.com \
LITECLAW_TUNNEL_TOKEN=replace-me \
CF_ACCESS_CLIENT_ID=optional \
CF_ACCESS_CLIENT_SECRET=optional \
bun run index.ts
```

Cloud demo (agent message triggers local tools):

```bash
cd apps/nydus
LITECLAW_AGENT_BASE_URL=https://openagents.com \
LITECLAW_TOOL_ADMIN_SECRET=replace-me \
NYDUS_LOCAL_ROOT=/path/to/your/repo \
bun run index.ts cloud
```

## Data Model Highlights

LiteClaw relies on SQLite tables inside the Durable Object:

- `cf_ai_chat_agent_messages` for canonical chat messages.
- `sky_runs`, `sky_events`, `sky_receipts` for Sky-mode telemetry.
- `sky_memory` for summary memory.
- `sky_tool_policy` and `sky_extension_policy` for per-thread policies.
- `sky_extensions` for catalog storage.
- `sky_workspaces` and `sky_workspace_files` for workspace state.

## Troubleshooting

Common issues:

- `Tool access is read-only for this thread.` Use `/tool-policy` to set `read-write`.
- `HTTP host is not in the allowlist.` Set `LITECLAW_HTTP_ALLOWLIST`.
- `Tunnel executor is not configured.` Ensure `LITECLAW_TUNNEL_URL` and `LITECLAW_TUNNEL_TOKEN` are set.
- `Extensions not allowed or missing.` Ensure allowlist and catalog entries exist and include pinned versions.

For tunnel-specific troubleshooting, see `docs/liteclaw/tunnel.md`.
