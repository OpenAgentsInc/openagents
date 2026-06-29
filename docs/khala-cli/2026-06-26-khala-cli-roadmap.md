# Khala CLI — Roadmap (the build plan)

> Status: **internal execution roadmap, 2026-06-26.** Pairs with
> [`2026-06-26-khala-cli-audit.md`](./2026-06-26-khala-cli-audit.md). Direction
> and implementation record; flips no promise state.
>
> Superseded note, 2026-06-27: this roadmap is the original execution record.
> The shipped v0.1.16 client lives in `clients/khala-cli/`, uses normal terminal
> scrollback/raw-mode input instead of OpenTUI, and has grown beyond the M0-M4
> plan to include login/logout, metadata commands, public token counter,
> optional local Codex delegation, and the owner-authenticated Artanis channel.
> Treat the milestone text below as history, not current product copy.

A single Bun + Effect client that reads one line of input (OpenTUI) and streams
a Khala answer to stdout. It now ships from `clients/khala-cli/`; the original
milestone plan remains below as the execution record.

## Where it lives

- **Actual home:** `clients/khala-cli/` as a tiny standalone client package in
  the monorepo, entrypoint `clients/khala-cli/src/index.ts` with a
  `#!/usr/bin/env bun` shebang. Its `package.json` exposes a `khala` bin, and
  the root package exposes `bun run khala`.
- **Original proposed home:** `apps/khala-cli/`.
- **Rejected lighter alternative:** a single `scripts/khala.ts`; the shipped
  client is a package because it needs a `khala` bin and isolated tests.
- **Dependency:** `@opentui/core` (published npm build; see M0 native-core note).
  Effect + Effect Schema as already used across the repo.

## Design shape (Effect/Bun)

The whole program is one Effect pipeline per turn:

```
read line (OpenTUI InputRenderable, resolves on ENTER)
  → append { role: "user", content } to the running messages array
  → POST the messages to the chosen endpoint (fetch, Bun-native)
  → consume the SSE body as a stream of frames
  → for each text delta: write to stdout (no newline)
  → on `done`/`[DONE]`: append the assembled assistant text to messages, newline
  → loop
```

- **State:** one in-memory `messages: KhalaChatMessage[]`. Stateless wire — the
  whole array is re-sent each turn (matches `/api/khala/chat`'s contract and the
  audit's bounds). Trim oldest turns if we approach
  `KHALA_CHAT_MAX_MESSAGES`/`MAX_TOTAL_CHARS`.
- **Schema:** Effect Schema mirrors of the request (`{messages:[{role,content}]}`)
  and the SSE frame payloads (`{text}` / `{done}` / `{error}`). Keep them local;
  do not import worker internals.
- **Errors:** map HTTP `400/429/502` envelopes and the terminal `error` frame to
  a single tagged `KhalaCliError` with a human line printed to stderr; exit
  non-zero. Never crash on a malformed frame — skip and continue, fail only on
  the terminal `error`.

## Endpoint modes (from the audit)

- **`--public` (default):** `POST /api/khala/chat`, no auth, 3-event SSE
  (`delta`/`done`/`error`). Simplest; the recommended default.
- **`--api` (opt-in):** `POST /api/v1/chat/completions`, `model
  openagents/khala`, `Authorization: Bearer $OPENAGENTS_AGENT_TOKEN`, OpenAI SSE
  (`delta.content` … `[DONE]`). If no token is present and the user passes
  `--api`, offer to mint one via `POST /api/keys/free` (one call, returns the raw
  bearer once; honor the `404`-when-disarmed and per-IP/day bounds).
- **`--base-url`:** override `https://openagents.com` for staging/local.

Flags only; no interactive config. Base URL + mode + optional token resolved
once at startup.

## Milestones

### M0 — Scaffold + native-core check
- Create `clients/khala-cli/` (`package.json`, `tsconfig`, `src/index.ts` shebang).
- `bun install @opentui/core`; confirm the published package resolves its
  **prebuilt native binary** on this machine without requiring a local Zig
  toolchain (the audit flags this as the one environment risk). If a build
  machine lacks the prebuilt binary, document the `bun install` requirement.
- **Done-when:** `bun run --cwd clients/khala-cli khala -- --help` prints usage and exits 0.

### M1 — OpenTUI input, echo only (no network)
- Stand up the OpenTUI input: `createCliRenderer()` → one `InputRenderable` →
  resolve a Promise/Effect on `InputRenderableEvents.ENTER`. Decide
  minimal-renderer vs key-handler-only mode here (audit §3); default to
  minimal-renderer.
- Echo the submitted line back, loop until EOF / `Ctrl-C` / a `/exit` line.
- **Done-when:** typing a line (with paste + cursor editing working) echoes it
  back; `Ctrl-C` exits cleanly with the terminal restored (no stuck raw mode).

### M2 — Public lane streaming (the core deliverable)
- Wire the default `/api/khala/chat` path: build the messages array, POST, parse
  the `delta`/`done`/`error` SSE wire, stream `text` to stdout.
- Append the assembled assistant reply to `messages`; support multi-turn.
- Enforce the audit's bounds client-side (message/total char caps, non-empty,
  last-is-user) so a too-long turn fails locally with a clear message instead of
  a server `400`.
- Map `429 rate_limited` / `502 inference_unavailable` to friendly stderr lines.
- **Done-when:** `echo`-style session: ask a question, watch the answer stream
  token-by-token, ask a follow-up that depends on context, get a coherent reply;
  clean exit.

### M3 — Authenticated OpenAI-compatible lane (`--api`)
- Add the `/api/v1/chat/completions` path with `model openagents/khala` and the
  bearer header; parse OpenAI SSE (`choices[].delta.content`, terminal `[DONE]`).
- Token resolution order: `--token` flag → `OPENAGENTS_AGENT_TOKEN` env →
  offer `POST /api/keys/free` mint (print the minted key once, tell the user to
  export it; never write it to disk).
- Optional `--models` to print `GET /api/v1/models`.
- **Done-when:** with a free-tier key, a streamed chat round-trips through
  `/api/v1/chat/completions`; with no token and free mode disarmed, the CLI
  prints the `404`/"free tier not armed" reason cleanly instead of throwing.

### M4 — Polish + ship
- `--no-stream` (single JSON response) for piping; `--system`-free by design
  (server owns the system prompt — document this).
- README usage block; `bin` wired so `bun run khala` works from the repo.
- Tests: a deterministic SSE-parser unit test (feed canned `delta`/`done`/`error`
  bytes, assert assembled text and terminal handling) and a bounds-validator
  test. Keep network calls out of unit tests.
- **Done-when:** the full relevant test suite + `check:deploy` are green, README
  shows a copy-pasteable session, and `clients/khala-cli` is committed and pushed to
  `main`.

## Sequence (flat)

`M0 scaffold` → `M1 opentui input/echo` → `M2 public-lane stream (core)` →
`M3 --api authed lane + free key` → `M4 tests + polish + ship`.

M2 is the milestone that delivers the owner's actual ask ("just hits khala API,
basic input, stream out"). M3/M4 are parity + hardening and can land later
without blocking M2's usefulness.

## Done-criteria for the whole CLI

1. One line in → streamed Khala answer out, in a terminal, over
   `/api/khala/chat`, with no auth and no config.
2. Input is OpenTUI's `InputRenderable` (paste/cursor-correct), and nothing
   heavier than a single input line — no TUI app.
3. Bun + Effect + Effect Schema, matching repo house style; no secrets tracked.
4. Optional authenticated `/api/v1/chat/completions` parity for real clients.
5. Tests + `check:deploy` green; committed and pushed to `main`.

## Open decisions (resolve as you build)

- **Package vs single script** — resolved to `clients/khala-cli/` so the command
  can expose a real `khala` bin and headless tests.
- **Minimal-renderer vs key-handler-only** OpenTUI mode (audit §3) — default to
  minimal-renderer; fall back if it fights "dirt-simple."
- **Conversation trimming policy** when approaching the public-lane bounds —
  simplest is drop-oldest-pair; revisit only if it confuses multi-turn context.
