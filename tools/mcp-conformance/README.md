# `tools/mcp-conformance/` — Coax MCP Conformance & Interop Harnesses

Verification harnesses for `coax/lib/mcp/` across both **`2025-11-25`** (stateful) and **`2026-07-28`** (stateless GA).

## Phase 2 (Stdio Transport & Interop)

- `stdio-server.coax` — Dual-era Coax stdio MCP server (`echo`, `add`, `fail` tools; `memo://hello` + `memo://{id}` resources; `greet` prompt; completions; `--2025-only` / `--2026-only` flags).
- `stdio-client.coax` — Dual-era Coax stdio MCP client CLI that spawns any MCP stdio server, runs `server/discover` → `initialize` backward-compat probe, exercises `tools/list`, `tools/call`, `resources/list`, `resources/read`, `prompts/list`, `prompts/get`, `ping`, and `notifications/cancelled`.
- `run.sh` — Runs:
  1. Coax stdio client ↔ Coax stdio server (`2026-07-28` mode).
  2. Coax stdio client ↔ Coax stdio server (`--2025-only`, verifying `server/discover` → `initialize` fallback + `notifications/initialized` + `ping`).
  3. Coax stdio client ↔ official `@modelcontextprotocol/server-everything` (when installed/built locally).
  4. Official `@modelcontextprotocol/inspector --cli` ↔ Coax stdio server (when installed locally).

## Phases 3–5 (Streamable HTTP + OAuth Conformance)

- `everything-server.coax` (Phase 3) — Streamable HTTP conformance server targeting 100% pass on `@modelcontextprotocol/conformance server --requirements 2025-11-25` (30/30) and `--requirements 2026-07-28` (37/37).
- `everything-client.coax` (Phases 4–5) — Streamable HTTP + OAuth 2.1 conformance client targeting 100% pass on `@modelcontextprotocol/conformance client --requirements 2025-11-25` (18/18) and `--requirements 2026-07-28` (32/32).
