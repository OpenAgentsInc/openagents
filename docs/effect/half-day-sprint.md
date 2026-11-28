# Half-Day Sprint Spec: `@effect/ui` + Effect v4 + Live `oa-worker` Spike

> **Time box:** 4 hours (single engineer or engineer + coding agent).
> **Objective:** Validate or falsify `@effect/ui` (no React) + Effect v4 as the Web UI & server stack driving our real compiled Coax coder (`oa-worker`) before committing to a full production build.

---

## 1. What This Sprint Must Prove

A synthetic hello-world counter proves nothing. By hour 4, a single local web app must run **one real end-to-end coding-agent session**:

1. **Real Coax worker underneath:** Spawns the actual compiled `oa-worker` binary (built from `openagents/runtime/oa-worker.coax`) over bidirectional stdin/stdout NDJSON (`mcworker-wire.coax`).
2. **Real token-by-token streaming in `@effect/ui`:** Streams `thought`, `reply`, `command`, `view`, `stats`, and `ended` frames over Server-Sent Events (SSE) into a `Query.StreamService` entry, rendered inside `@effect/ui-kit`'s `VirtualList` (tail-following) with built-in `Markdown.view`.
3. **Real bidirectional control:**
   - User approves or denies an interactive `MWW_ASK` / `MWW_PERMIT` card (`MWW_ANSWER` with exact `ticket`).
   - User sends a mid-turn `MWW_STEER` or `MWW_STOP` cancellation while tokens or tools are active.
4. **Effect UI ergonomics & agent verification:**
   - At least one `Story` test (using `Story.explore` to check overlapping approval/cancel settlement races) and one `Play` test (using `TestClock` in Node, no browser).
   - `@effect/ui-devtools` MCP server running in `vite dev` (`http://127.0.0.1:4774/mcp`) and queried from an agent (`list_sessions`, `get_state`, `diff_state`, `dispatch`).
5. **Effect v4 ecosystem check:** Confirm the exact Postgres / auth / deployment libraries we would use in production install and type-check cleanly alongside `effect ^4.1.6` under `exactOptionalPropertyTypes: true`.

### Explicitly Out of Scope for the 4-Hour Spike

- Authentication, Postgres persistence, billing, multi-tenancy, or remote GCE/Fly sandboxes (use a single local `oa-worker` child process on loopback).
- Custom diff editors, xterm.js terminal panes, worktree management, or `@effect/ui-native`.
- Modifying `openagents/runtime/oa-worker.coax`, `apps/openagents-desktop/`, or `coax/web/api/`.

---

## 2. Workspace Setup & Pinned Dependencies

Run the spike in a throwaway isolated directory outside the main checkout (for example `../oa-effect-spike/`) so it does not affect Coax build caches or `tools-check` allowlists before the Go/No-Go decision.

### Build the real `oa-worker` binary first

From an OpenAgents worktree (never the main checkout):

```sh
COAX_TOOL_BUILD_ONLY=1 coax/web/tool.sh openagents/runtime/oa-worker.coax
# Locate the built binary (path printed or under coax/web/build/tools/oa-worker-<coax_sha>)
export OA_WORKER_BIN="$(ls -t coax/web/build/tools/oa-worker-* | head -n 1)"
"$OA_WORKER_BIN" --auth
```

Verify `OA_WORKER_BIN --auth` prints `{"source":...,"ready":true,...}` against either local Psionic Inkling or OpenAgents Cloud / Vertex before starting the UI clock.

### Scaffold the `@effect/ui` + Vite 8 + Tailwind v4 app

Pin every `@effect/ui*` package to the exact reviewed commit SHA (`9d672abe`) so all peer references resolve to the same snapshot:

```sh
pnpm create vite ../oa-effect-spike --template vanilla-ts --no-interactive
cd ../oa-effect-spike
SHA=9d672abe
pnpm add \
  effect@^4.1.6 \
  @effect/platform-node \
  @effect/ui@https://pkg.pr.new/Effect-TS/ui/@effect/ui@${SHA} \
  @effect/ui-kit@https://pkg.pr.new/Effect-TS/ui/@effect/ui-kit@${SHA} \
  @effect/ui-devtools@https://pkg.pr.new/Effect-TS/ui/@effect/ui-devtools@${SHA}
pnpm add -D vite@^8 tailwindcss @tailwindcss/vite vitest @effect/vitest happy-dom
```

*(Note: `pnpm` prints unmet `@effect/*` peer warnings for `pkg.pr.new` URLs because a URL does not match `^0.0.0`; the installed snapshots still resolve cleanly.)*

### Mandatory `tsconfig.json` flags

`@effect/ui` requires both `strict` and `exactOptionalPropertyTypes` — without `exactOptionalPropertyTypes`, root host types widen to `unknown` and fail to type-check:

```json
{
  "compilerOptions": {
    "strict": true,
    "exactOptionalPropertyTypes": true,
    "types": ["@effect/ui/html/HtmlServerViteDocument"]
  }
}
```

Point the spike's `AGENTS.md` directly at `node_modules/@effect/ui/AGENTS.md` (which ships the matching `LLMS.md` guide and worked examples) so any coding agent writes idiomatic Effect UI v4 instead of hallucinating React or Foldkit 0.x APIs.

---

## 3. Target Directory Layout (Spike)

```text
oa-effect-spike/
├── AGENTS.md                       # Points to node_modules/@effect/ui/AGENTS.md
├── index.html                      # Static shell with <!--app-html-->
├── vite.config.ts                  # tailwindcss(), htmlServer(), DevToolsVite.plugin({ dispatch: true })
├── src/
│   ├── shared/
│   │   ├── wire.ts                 # Effect Schema codecs for mcworker-wire & SessionSnapshot
│   │   └── api.ts                  # Shared route/payload schemas for turn/approve/steer/stop/stream
│   ├── server/
│   │   ├── OaWorkerProcess.ts      # Effect Service spawning $OA_WORKER_BIN (NDJSON stdin/stdout)
│   │   ├── SessionHub.ts           # Per-session fold + PubSub broadcasting SessionSnapshot deltas
│   │   └── entry.server.ts         # HttpRouter (/api/* SSE + POSTs) + HtmlServerHttp.layerHttp
│   ├── client/
│   │   ├── queries/
│   │   │   └── SessionStream.ts    # Query.StreamService backed by EventSource / fetch SSE
│   │   ├── components/
│   │   │   ├── App.ts              # Root layout + Query subscription + error boundary
│   │   │   ├── Transcript.ts       # VirtualList (followEnd: true) + h.lazy message rows + Markdown.view
│   │   │   ├── MessageRow.ts       # Isolated row (thinking collapsible, markdown reply, tool calls)
│   │   │   ├── ApprovalCard.ts     # Inline permit/ask approval widget (Allow / Deny + ticket)
│   │   │   └── Composer.ts         # Isolated text input + Send / Steer / Stop buttons
│   │   └── entry.client.ts         # HtmlHydrate.hydrate + BrowserRouter + DevTools.layer
│   └── tests/
│       ├── ApprovalCard.story.test.ts  # Story + Story.explore concurrency check
│       └── ChatFlow.play.test.ts       # Full Play flow on TestHost + TestClock
```

---

## 4. Hour-by-Hour Execution Plan

### Hour 0:00 – 0:45 — Server Bridge (`OaWorkerProcess` + `SessionHub` + SSE)

1. Define `shared/wire.ts` with `Schema` codecs covering the subset of `coax/lib/agent/engine/mcworker-wire.coax` needed for a full turn:
   - **Owner → Worker (JSON lines written to `oa-worker` stdin):**
     - `{"task": string, "session": string, "cwd": string}` (`MWW_TASK`)
     - `{"answer": boolean, "ticket": number, "why"?: string}` (`MWW_ANSWER`)
     - `{"steer": string, "id": string, "expected_turn"?: number}` (`MWW_STEER`)
     - `{"stop": true}` (`MWW_STOP`)
   - **Worker → Owner (JSON lines read from `oa-worker` stdout):**
     - `ready`, `turn`, `thought`, `reply`, `command`, `view`, `ask`, `permit`, `todos`, `stats`, `meta`, `steer`, `ended`
2. Implement `OaWorkerProcess` as a scoped Effect `Service`:
   - Spawns `process.env.OA_WORKER_BIN` with `stdio: ["pipe", "pipe", "pipe"]` and `SSH_CONNECTION=1` (safe probe hygiene).
   - Splits `stdout` on `\n`, parses JSON lines through `Schema`, and ignores unknown `kind` values forward-compatibly (matching `mcworker-wire.coax`'s contract).
   - Waits for initial `{"kind":"ready"}` before marking the session ready.
   - Guarantees clean teardown in the Effect `Scope` finalizer (`{"stop":true}\n` → `SIGTERM` → `SIGKILL` after 3s).
3. Implement `SessionHub` folding worker frames into an immutable `SessionSnapshot` (`status`, `activeTurn`, `messages[]`, `pendingPermit`, `todos`, `stats`) and publishing snapshots over an Effect `PubSub` / `Stream`.
4. Wire `entry.server.ts` with `HttpRouter`:
   - `GET /api/sessions/:id/stream` → `text/event-stream` emitting `SessionSnapshot` JSON frames (with periodic `: ping` keepalive).
   - `POST /api/sessions/:id/turn` → writes `MWW_TASK` to worker stdin.
   - `POST /api/sessions/:id/approve` → writes `MWW_ANSWER` (`{ answer, ticket, why }`).
   - `POST /api/sessions/:id/steer` → writes `MWW_STEER`.
   - `POST /api/sessions/:id/stop` → writes `MWW_STOP`.
   - Mount `HtmlServerHttp.layerHttp(App, ...)` via `HtmlServerVite.plugin({ entry: "/src/server/entry.server.ts" })` in `vite.config.ts`.

**Hour 0:45 checkpoint:** `curl -N http://localhost:5173/api/sessions/demo/stream` receives the initial `ready` snapshot and live `thought`/`reply`/`ended` events after `curl -X POST .../turn`.

---

### Hour 0:45 – 2:00 — `@effect/ui` Frontend (Streaming Transcript + Isolated Composer)

1. **`SessionStream` (`Query.StreamService`):**
   - Declare `SessionStream extends Query.StreamService<SessionStream>()("chat/SessionStream", { args: { sessionId: Schema.String }, success: SessionSnapshot, error: SessionError })`.
   - Its `toLayer` opens the SSE stream (`EventSource` or streaming `fetch` + `AbortSignal` tied to Effect fiber interruption) and emits each decoded `SessionSnapshot`.
   - Components read the live snapshot via `h.queries` without copying streaming chunks into their own Model.
2. **Component isolation (critical for `@effect/ui`'s re-render model):**
   - `@effect/ui` re-runs a component's `view` on each message to that component. Keep the **Composer** as a separate `Html.component` so every keystroke re-renders *only* the textarea/buttons, never the transcript.
   - Render the transcript inside `@effect/ui-kit`'s **`VirtualList`** (`contentAlignment: "end"`, `followEnd: true`, `scrollTo: { target: VirtualListScrollTarget.End() }`, keyed by message id) and wrap completed rows with `h.lazy` so streaming tokens only re-render the active tail row.
   - Render assistant prose with built-in **`Markdown.view(h, MarkdownParser.parseMarkdown(text))`**, thinking blocks with `@effect/ui-kit` **`Collapsible`**, and completed tool invocations (`command` / `view`) with compact collapsible pills.
3. **Interactive Approvals & Controls:**
   - When `snapshot.pendingPermit` is non-null, render `ApprovalCard` displaying the requested `command` / `title` / `options` and emitting `Allow` or `Deny` via a `Command.Service` (`mode: "switch"` or `"queue"`) that POSTs to `/api/sessions/:id/approve`.
   - While `snapshot.status === "running"`, switch the Composer primary action to **Steer** (`POST .../steer`) alongside a **Stop** button (`POST .../stop`).

**Hour 2:00 checkpoint:** Opening `http://localhost:5173` renders the SSR page, hydrates cleanly, and streams a real `oa-worker` response smoothly with markdown formatting and auto-scrolling.

---

### Hour 2:00 – 3:00 — Live End-to-End Flow & Stress Test

Run three real user scenarios against the live `oa-worker` process:

1. **Cold `req#1` streaming + tool call:** Prompt `oa-worker` to inspect a file in a scratch directory and summarize it. Verify thinking tokens, `view`/`command` rows, streamed markdown, and `stats` (TTFT / tokens/sec) render in real time with zero dropped frames while typing in the Composer.
2. **Interactive approval round-trip:** Trigger a shell command requiring approval (`MWW_ASK` / `MWW_PERMIT`), click **Allow** (and repeat with **Deny**), and verify `oa-worker` resumes or aborts cleanly on the matching `ticket`.
3. **Mid-turn steer & cancel:** Start a long generation, send a mid-turn `MWW_STEER`, then trigger `MWW_STOP` on a second turn; verify the worker emits `ended` (`stopped`) and the UI returns to idle cleanly with no dangling SSE or child-process leaks.

---

### Hour 3:00 – 4:00 — Deterministic Tests, DevTools MCP & Dependency Audit

1. **Unit & Concurrency Tests (`pnpm vitest --run`):**
   - `ApprovalCard.story.test.ts`: Verify `Story.explore` over overlapping `ClickedAllow`, `ClickedDeny`, and `ClickedStop` messages + command settlements proves no double-submission or stale-ticket UI state.
   - `ChatFlow.play.test.ts`: Mount `App` on `TestHost.layer()` with an in-memory `SessionStream.toLayer(...)` emitting synthetic chunks over `TestClock`; type a prompt, submit, approve a permit card, and assert the rendered transcript and ARIA states without launching a browser.
2. **Agent-driven DevTools MCP check:**
   - Enable `DevToolsVite.plugin({ dispatch: true })` and `DevTools.layer({ position: "BottomRight" })`.
   - Call `http://127.0.0.1:4774/mcp` (`list_sessions`, `list_instances`, `get_state`, `diff_state`, `dispatch`) against the live browser tab to confirm our agents can inspect and drive the UI as cleanly as `scripts/agent.mjs` / `coax drive`.
3. **Effect v4 Server Ecosystem Audit (20 min):**
   - Verify the exact server dependencies needed for production (`@effect/sql-pg` or `drizzle-orm`, auth/session library, Vercel/Fly adapter, `ws`/SSE support) have working Effect v4 releases or clean zero-friction wrappers under `exactOptionalPropertyTypes: true`.

---

## 5. Go / No-Go Scorecard

Commit to the full architecture in [`full-architecture.md`](full-architecture.md) only if **every Hard Gate passes** and the qualitative ergonomics feel like a net speedup:

| Gate | Type | Pass Threshold |
| --- | --- | --- |
| Live `oa-worker` turn + approval + steer/stop E2E | **Hard** | Cold `req#1` streams tokens, resolves `MWW_ASK`/`MWW_PERMIT` by ticket, handles `MWW_STEER` and `MWW_STOP` cleanly with zero leaked worker processes. |
| Streaming smoothness & scroll behavior | **Hard** | Rapid token streaming + concurrent typing in `Composer` produces no input lag; `VirtualList` follows tail when at bottom and stops auto-scrolling when the user scrolls up. |
| Effect v4 backend dependency compatibility | **Hard** | Postgres client, HTTP/SSE server (`HttpRouter.toWebHandler`), and auth layer install and pass `tsc --noEmit` with `strict` + `exactOptionalPropertyTypes`. |
| Deterministic tests + DevTools MCP | **Hard** | `Story.explore` + `Play` tests pass in Node (`< 2s`), and DevTools MCP `get_state` / `dispatch` works against the running tab. |
| Authoring & agent ergonomics | **Qualitative** | Both human and coding agent (pointed at `node_modules/@effect/ui/AGENTS.md`) can add a new UI panel (e.g., `TodosPanel` from `MWW_TODOS`) in < 15 min without fighting `Schema` / `h.*` types. |
| Production bundle & SSR sanity | **Qualitative** | `vite build` succeeds with `serverOutDir`, client gzipped JS is reasonable (< 150 kB for the spike), and SSR HTML hydrates with zero mismatch warnings. |

**Fallback if No-Go:** Keep the **Effect v4 server + Linux `oa-worker` sandbox control plane** unchanged (all backend work carries over 100%) and swap only the browser view layer to React + Vite/Next.js consuming the exact same `openagents.chat.event.v2` SSE/POST endpoints.
