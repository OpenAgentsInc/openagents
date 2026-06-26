# Khala CLI — Audit (what we build on)

> Status: **internal audit, 2026-06-26.** Reference for the build plan in
> [`2026-06-26-khala-cli-roadmap.md`](./2026-06-26-khala-cli-roadmap.md). Not a
> product promise; flips no promise state.

The goal is a "dirt-simple" terminal client for Khala: read one line, stream the
answer. Before designing it, this audits the two server surfaces a CLI can hit
today, the input library we were asked to use (OpenTUI), and the constraints
each imposes. Everything below already exists in the repo or in
`projects/repos/`; the CLI invents no new server behavior.

## 1. The two Khala surfaces a CLI can hit

There are exactly two HTTP entry points that turn a prompt into a streamed Khala
answer. Both are already implemented and deployed. A CLI should support the
first by default and the second as an opt-in.

### 1a. Public demo lane — `POST /api/khala/chat` (zero-auth, simplest)

- **Source:** `apps/openagents.com/workers/api/src/khala-chat-routes.ts` and
  `…/khala-chat-program.ts`.
- **URL:** `https://openagents.com/api/khala/chat`, method `POST`.
- **Auth:** none. Public, unauthenticated, per-IP token-bucket rate limited.
- **Request body** (`KhalaChatRequest`):
  ```jsonc
  { "messages": [ { "role": "user" | "assistant", "content": "…" } ] }
  ```
  The conversation is **stateless** — the client re-sends the whole list each
  turn. The newest user message is last. Only `user`/`assistant` roles cross the
  boundary; the system prompt is rebuilt server-side and can never be supplied
  by the client.
- **Response:** Server-Sent Events. A narrow, self-describing wire:
  - `event: delta` → `data: { "text": "…" }` — one per content increment.
  - `event: done`  → `data: { "done": true }` — terminal, once.
  - `event: error` → `data: { "error": "…" }` — terminal, on failure.
  - Response headers: `content-type: text/event-stream`, `cache-control: no-store`.
- **Bounds (cheap abuse guards, enforced before any stream byte):**
  - `KHALA_CHAT_MAX_MESSAGE_CHARS = 8_000` per message.
  - `KHALA_CHAT_MAX_MESSAGES = 40` messages.
  - `KHALA_CHAT_MAX_TOTAL_CHARS = 24_000` for the whole conversation.
  - Last message must be a `user` message; no message may be empty.
- **Error envelopes (JSON, pre-stream):** `400 bad_request`,
  `400 validation_error`, `429 rate_limited`, `502 inference_unavailable`.

This is the right default for a "dirt-simple" CLI: no key, no balance, no
headers beyond `content-type`, and a trivial three-event SSE wire.

### 1b. OpenAI-compatible lane — `POST /api/v1/chat/completions` (authenticated)

- **Source:** OpenAPI at
  `apps/openagents.com/workers/api/src/openagents-openapi.ts` (`/api/v1/*`).
- **URL:** `https://openagents.com/api/v1/chat/completions` (canonical under
  `/api`; the bare `/v1/chat/completions` path is a non-breaking alias).
- **Auth:** `Authorization: Bearer oa_agent_…`.
- **Model id:** `openagents/khala` (inside the ecosystem the slug is `khala`;
  external clients use `openagents/khala`). One public model only.
- **Wire:** standard OpenAI Chat Completions request/response, with normal
  OpenAI SSE when `stream: true` (`data: {choices:[{delta:{content}}]}` … `[DONE]`).
- **Model discovery:** `GET /api/v1/models` — public, pre-purchase, returns the
  one public model plus its `oa_free_tier_eligible` flag and `oa_free_tier`
  quota object.
- **Free key, no payment, one call:** `POST /api/keys/free` mints a free,
  rate-limited `oa_agent_` key and returns the raw bearer **once**. A free-tier
  key can call `openagents/khala` with no credit balance, within a per-key daily
  free quota (request + served-token caps, reset each UTC day). Free usage is
  still receipt-first metered as a zero-credit debit. Gated by
  `INFERENCE_FREE_TIER_ENABLED`; returns `404` until free mode is armed. Minting
  is bounded per client IP per UTC day.

This lane is worth supporting because it is the same surface real agents,
OpenCode, Aider, etc. use (see the roadmap's Phase-5 recipes in
`docs/khala/2026-06-26-khala-open-issues-master-roadmap.md`). But it requires a
key and a slightly richer SSE parser, so it is opt-in, not the default.

### Surface comparison

| | `/api/khala/chat` | `/api/v1/chat/completions` |
| --- | --- | --- |
| Auth | none | `Bearer oa_agent_…` |
| Body | `{messages:[{role,content}]}` | full OpenAI chat-completions |
| Model field | implicit (`khala`) | `openagents/khala` required |
| SSE wire | `delta`/`done`/`error` (`{text}`) | OpenAI `delta.content` + `[DONE]` |
| Metered/billed | demo, IP-rate-limited | yes (free-tier or credits) |
| Best for | the dirt-simple default | parity with real agent clients |

## 2. What NOT to confuse this with

- **`pylon khala request --workflow codex_agent_task`** (`apps/pylon/src/khala-requester.ts`)
  is the typed **delegation** path that routes a coding assignment to a linked
  local Pylon/Codex. It is a different contract (assignments, leases, closeouts,
  token-usage reconciliation) and is **not** what this CLI is. This CLI is the
  plain conversational chat path.
- **The homepage counter** `GET /api/public/khala-tokens-served` is a public
  projection, not a chat endpoint. Out of scope.

## 3. OpenTUI — what we use, and only that

Reference clone: `projects/repos/opentui` (`@opentui/core`, v0.3.x, MIT). It is
a native terminal-UI core (Zig + TypeScript bindings) that powers OpenCode in
production. It is a big library; the owner's instruction is "just basic input,"
so we use exactly **one** primitive.

- **`InputRenderable`** (`packages/core/src/renderables/Input.ts`): a
  single-line text input. Height is always 1, no wrapping, newlines stripped,
  **Enter submits**. It handles the things a naive `readline` gets wrong —
  cursor movement, bracketed-paste decoding (`decodePasteBytes` /
  `stripAnsiSequences`), and key bindings inherited from `TextareaRenderable`.
  Events (`InputRenderableEvents`): `INPUT`, `CHANGE`, `ENTER`.
- **`createCliRenderer(config)`** (`packages/core/src/renderer.ts`): the entry
  point that gives you a `renderer` whose `root` you `.add()` the input to. The
  example `packages/examples/src/input-demo.ts` shows the minimal shape:
  `createCliRenderer()` → `new InputRenderable(renderer, {…})` →
  `renderer.root.add(input)` → `input.on(InputRenderableEvents.ENTER, value => …)`.
- **Install:** `bun install @opentui/core`. Note from the upstream README: the
  package builds against a **native Zig core**; published npm builds ship
  prebuilt binaries, so a consumer normally does not need Zig installed — only
  contributors building the core from source do. The roadmap calls out verifying
  this on the target machine as an M0 task.

### The one design tension to resolve

`createCliRenderer` is built to drive a **retained-mode, full-screen** terminal
app, which is more than "dirt-simple" wants. Two honest options, decided in the
roadmap (M1):

1. **Minimal-renderer mode (preferred):** create the renderer, add a single
   `InputRenderable`, but keep streamed model output flowing to **plain stdout**
   below/around the input rather than building a managed output widget. Use
   OpenTUI for input correctness only.
2. **Key-handler-only mode (fallback):** if driving the renderer for a
   one-line prompt proves heavier than warranted, drop to OpenTUI's lower-level
   keypress parser (`packages/core/src/lib/parse.keypress*.ts` /
   `stdin-parser.ts`) for paste-safe line editing without a renderer. Still
   "uses opentui," still no TUI.

Either keeps the surface to a single input line, satisfying both "use opentui"
and "no tui, no fancy anything."

## 4. Runtime + house style

- **Bun + Effect.** `apps/openagents.com` and `apps/pylon` are Bun/Effect/Effect
  Schema. The CLI matches: an Effect program for the request/stream pipeline,
  Effect Schema for the (tiny) request/response shapes, a Bun shebang
  (`#!/usr/bin/env bun`) entrypoint like `apps/pylon/src/index.ts`.
- **No secrets in tracked files.** The optional bearer key comes from the
  environment (e.g. `OPENAGENTS_AGENT_TOKEN`) or a one-shot `POST /api/keys/free`
  mint; it is never written to the repo, logged, or echoed.

## 5. Audit conclusion

Everything the CLI needs already exists:

1. A zero-auth streaming chat endpoint with a trivial 3-event SSE wire
   (`/api/khala/chat`) — the default.
2. An authenticated OpenAI-compatible endpoint + a one-call free-key mint for
   parity with real clients (`/api/v1/chat/completions`, `/api/keys/free`) —
   opt-in.
3. A single OpenTUI input primitive that gives correct, paste-safe line input
   without forcing a full TUI.

No server work is required. The remaining work is purely a small client script,
sequenced in the roadmap.
