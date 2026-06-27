# Khala CLI

> Status: **current shipped-surface note, 2026-06-27.** The local package lives
> at `clients/khala-cli/`. The 2026-06-26 OpenTUI single-line plan below is
> superseded by the shipped v0.1.16 client. This page is supporting evidence for
> `khala.cli_terminal_client.v1`; the product promise source of truth remains
> `apps/openagents.com/workers/api/src/product-promises.ts`.

`@openagentsinc/khala` is a Bun + Effect terminal client for talking to
**Khala** — OpenAgents' OpenAI-compatible inference orchestrator — from a
terminal. The shipped `khala` command opens a normal terminal scrollback chat
with raw-mode input, `>` prompts, `Khala:` turns, local slash commands, and
headless `--prompt` / stdin modes for scripts.

The guiding constraint from the owner stayed the same: **"just basic input."**
The implementation changed: the CLI does **not** use OpenTUI in the shipped
package. It uses normal terminal scrollback plus local input handling in
`clients/khala-cli/src/input.ts`.

Current v0.1.16 surface:

- interactive chat and headless one-turn mode;
- `/info`, `/msginfo`, `/tokens`, `/feedback`, `/changelog`, `/version`, and
  `/help`;
- `khala login` / `khala logout` and matching slash commands for OpenAgents
  device auth;
- optional local Codex workspace delegation (`khala auth codex`, `khala codex`,
  `/codex`) when local credentials are connected;
- owner-authenticated Artanis operator channel (`khala --artanis`, `/artanis`);
- public Khala token counter and package changelog utilities;
- background npm update check unless `KHALA_NO_AUTO_UPDATE=1`.

## Documents

| Doc | What it covers |
| --- | --- |
| [`2026-06-26-khala-cli-audit.md`](./2026-06-26-khala-cli-audit.md) | Historical audit of the first simple CLI plan and server surfaces. Its OpenTUI implementation notes are superseded by the v0.1.16 shipped client. |
| [`2026-06-26-khala-cli-roadmap.md`](./2026-06-26-khala-cli-roadmap.md) | Historical execution roadmap. Use `clients/khala-cli/README.md` and `clients/khala-cli/src/changelog.ts` for current behavior. |

## TL;DR

- **Endpoint (default, zero-auth):** `POST https://openagents.com/api/khala/chat`
  — public, unauthenticated, SSE-streaming demo lane. Body is
  `{ messages: [{ role, content }] }`; frames are `delta` / `done` / `error`.
- **Endpoint (authenticated, OpenAI-compatible):**
  `POST https://openagents.com/api/v1/chat/completions` with
  `Authorization: Bearer oa_agent_…`, model `openagents/khala`. A free,
  rate-limited key is mintable in one call via `POST /api/keys/free`.
- **Implementation:** `clients/khala-cli/`, package bin `khala`.
- **Input:** normal terminal scrollback/raw-mode input, not OpenTUI.
- **Runtime:** Bun + Effect (matches the rest of `apps/openagents.com`).
- **Output:** stream assistant text deltas straight to stdout. That's it.

## Non-goals

- No multi-pane TUI, alternate-screen app, billing UI, or wallet flows.
- Device auth exists for owner/operator flows; it is not a billing/account
  console.
- No new server endpoints — the CLI is a pure client of surfaces that already ship.
- Not a replacement for `pylon khala request` (that is the typed
  Khala->Pylon->Codex **delegation** path; the CLI's local Codex flow is an
  optional local-workspace delegation path).
