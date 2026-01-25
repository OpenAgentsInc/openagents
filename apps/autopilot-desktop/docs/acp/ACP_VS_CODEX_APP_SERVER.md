# ACP vs Codex App-Server

This document compares the Agent Client Protocol (ACP) to the Codex app-server
protocol and explains how `codex-acp` fits between them. It is intended as a
practical assessment for Autopilot integrations.

## Executive Summary

- **Codex app-server** is richer and Codex-specific. It exposes a deep
  thread/turn/item model with fine-grained streaming events, approvals, account
  status, and config/skills endpoints.
- **ACP** is standardized and portable across agents. It offers a simpler
  session/prompt model with well-defined updates and tool calls, but it does
  not include Codex-specific telemetry or many app-server events.
- **codex-acp** is an adapter that speaks ACP. It does not require `codex
  app-server` internally, so event streams and session IDs differ unless you
  deliberately run both.

## Protocol Overview

### Codex app-server

- **Transport**: JSON-RPC over stdio (JSONL), omits `jsonrpc` field.
- **Handshake**: `initialize` request, then `initialized` notification.
- **Core model**: `thread` → `turn` → `item`.
- **Streaming**: many granular notifications (`thread/started`, `turn/started`,
  `item/started`, `item/*/delta`, `turn/completed`), plus `codex/event/*`,
  token usage, and rate limits.
- **Approvals**: server-initiated request methods such as
  `item/commandExecution/requestApproval`.
- **Extras**: config APIs, skills list, MCP server status, account endpoints.

Source: `/Users/christopherdavid/code/codex/codex-rs/app-server/README.md`

### ACP

- **Transport**: JSON-RPC 2.0 with explicit `jsonrpc` field.
- **Handshake**: `initialize`, optional `authenticate`.
- **Core model**: `session/new` + `session/prompt`.
- **Streaming**: `session/update` notifications for agent message chunks,
  thought chunks, plans, tool calls, and tool call updates.
- **Approvals**: `session/request_permission`.
- **Extras**: extension methods and `_meta` for custom data.

Source: `/Users/christopherdavid/code/agent-client-protocol/docs/protocol/overview.mdx`

## High-Level Mapping

| Concept | Codex app-server | ACP |
| --- | --- | --- |
| Start conversation | `thread/start` | `session/new` |
| Send user input | `turn/start` | `session/prompt` |
| Streaming output | `item/*/delta` notifications | `session/update` |
| Completion | `turn/completed` | `session/prompt` response `stopReason` |
| Tool call | `item/commandExecution/*` | `session/update` tool call entries |
| Approval | `item/*/requestApproval` | `session/request_permission` |

Notes:
- ACP does not mirror the thread/turn/item hierarchy directly.
- ACP uses a single `session/update` stream instead of many specialized
  notification methods.

## Event Coverage Differences

Autopilot’s event comparison work shows large gaps when using ACP alone:

- **App-server** produces many event types (thread, turn, item, codex/event,
  token usage, rate limits).
- **ACP** typically shows only `session/update` notifications unless you use
  the ACP library to decode and map them.

Reference: `docs/acp/ACP_EVENT_COMPARISON.md`

Implication: If you switch to ACP-only without extensions, you lose Codex’s
rich telemetry (rate limits, token usage, detailed item lifecycle) unless
codex-acp is extended to emit custom notifications.

## Approvals and Tooling

Codex app-server uses specialized approval requests tied to item lifecycle
events. ACP uses a standardized permission request (`session/request_permission`)
and expects tool lifecycle to be reported via `session/update`.

For a UI, this means:
- **App-server**: show item proposal and wait for approval request.
- **ACP**: show tool call entry and respond to `session/request_permission`.

## Extensibility Differences

- **App-server**: Codex-specific notifications (e.g., `codex/event/*`) are
  first-class but non-standard outside Codex.
- **ACP**: custom requests/notifications must be prefixed with `_` and should
  be advertised via capabilities or `_meta`.

Source: `/Users/christopherdavid/code/agent-client-protocol/docs/protocol/extensibility.mdx`

## How `codex-acp` Fits In

`codex-acp` is an adapter that implements ACP for Codex. It does **not** use
the Codex app-server internally; it uses Codex Rust libraries directly. That
means:

- ACP sessions created by `codex-acp` are separate from app-server threads.
- Event streams can differ from app-server (fewer or different notification
  types).
- You will not see Codex app-server-specific events unless you run the
  app-server in parallel and merge those events yourself.

Reference: `docs/codex/CODEX_ACP_ARCHITECTURE.md`

## Practical Implications for Autopilot

1. **ACP-only path (codex-acp only)**:
   - Pros: protocol standardization, multi-agent alignment.
   - Cons: loss of Codex-specific telemetry (rate limits, token usage,
     codex/event stream), and fewer lifecycle signals unless `codex-acp` emits
     extensions.

2. **App-server-only path**:
   - Pros: maximum Codex feature coverage.
   - Cons: Codex-specific protocol, not portable to other agents.

3. **Dual-protocol path (codex-acp + app-server)**:
   - Pros: standardized ACP events plus Codex-specific telemetry.
   - Cons: two processes and event merge complexity.

## Recommendation Snapshot

- If you want **best Codex feature fidelity**, keep app-server in the loop or
  add ACP extensions to `codex-acp`.
- If you want **multi-agent interoperability**, favor ACP and accept that you
  need extensions for Codex-specific data (rate limits, token usage, app-server
  event stream).

## Related Docs

- `docs/acp/ACP_EVENT_COMPARISON.md`
- `docs/acp/ACP_ASSESSMENT.md`
- `docs/codex/CODEX_ACP_ARCHITECTURE.md`
