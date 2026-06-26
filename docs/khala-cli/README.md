# Khala CLI

> Status: **internal design docs, 2026-06-26.** This is a build plan, not a
> shipped binary and not a product promise. It flips no promise state and ships
> no code by itself.

A dirt-simple command-line client for talking to **Khala** — OpenAgents'
OpenAI-compatible inference orchestrator — from a terminal. One prompt in,
streamed answer out. No TUI app, no panes, no dashboards: a single Bun script,
written with Effect, that reads a line of input (via OpenTUI's input primitive)
and streams the reply to stdout.

The guiding constraint from the owner: **"just basic input."** OpenTUI is used
only for the one thing it is worth using here — a correct, paste-safe,
cursor-aware single-line input — and nothing else. We deliberately do **not**
build a full-screen retained-mode interface.

## Documents

| Doc | What it covers |
| --- | --- |
| [`2026-06-26-khala-cli-audit.md`](./2026-06-26-khala-cli-audit.md) | What already exists: the two Khala HTTP surfaces a CLI can hit, their exact request/response shapes, auth, bounds, and how OpenTUI fits. The "what we're building on" reference. |
| [`2026-06-26-khala-cli-roadmap.md`](./2026-06-26-khala-cli-roadmap.md) | The build plan: where the script lives, the Effect/Bun shape, the milestone sequence (M0–M4), and the done-criteria for each. |

## TL;DR

- **Endpoint (default, zero-auth):** `POST https://openagents.com/api/khala/chat`
  — public, unauthenticated, SSE-streaming demo lane. Body is
  `{ messages: [{ role, content }] }`; frames are `delta` / `done` / `error`.
- **Endpoint (authenticated, OpenAI-compatible):**
  `POST https://openagents.com/api/v1/chat/completions` with
  `Authorization: Bearer oa_agent_…`, model `openagents/khala`. A free,
  rate-limited key is mintable in one call via `POST /api/keys/free`.
- **Input:** `@opentui/core`'s `InputRenderable` (single-line, paste-safe).
- **Runtime:** Bun + Effect (matches the rest of `apps/openagents.com`).
- **Output:** stream assistant text deltas straight to stdout. That's it.

## Non-goals

- No multi-pane TUI, no scrollback widget, no syntax-highlighted markdown render.
- No session persistence, accounts, billing UI, or wallet flows.
- No new server endpoints — the CLI is a pure client of surfaces that already ship.
- Not a replacement for `pylon khala request` (that is the typed
  Khala→Pylon→Codex **delegation** path; this is the plain **chat** path).
