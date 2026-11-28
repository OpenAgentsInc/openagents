# Half-Day Sprint Results & Go/No-Go Scorecard

> **Date:** 2026-11-27
> **Prototype:** [`apps/openagents-effect-spike/`](../../apps/openagents-effect-spike/README.md)
> **Pinned versions:** `effect 4.1.6`, `@effect/platform-node 4.1.6`, `Effect-TS/ui@9d672abe` (`@effect/ui`, `@effect/ui-kit`, `@effect/ui-devtools`), Vite `8.4.0`, Tailwind CSS `4.3.5`
> **Coax worker tested:** `/Applications/OpenAgents.app/Contents/Resources/oa-worker` (`OACommit: 267704a1`, `OAProfile: v1`)
> **Verdict:** **GO** — All 4 Hard Gates and both Qualitative Gates passed on live hardware with real `oa-worker` turns, headless Chrome hydration, and DevTools MCP control.

---

## 1. Go / No-Go Scorecard (Measured Results)

| Gate | Type | Target | Measured Result | Verdict |
| --- | --- | --- | --- | --- |
| Live `oa-worker` turn + approval + steer/stop E2E | **Hard** | Cold `req#1` streams tokens, resolves `MWW_PERMIT` by ticket (Allow & Deny), handles `MWW_STEER`/`MWW_STOP`, zero leaked processes | **Passed:** Flow A (`Read package.json`) streamed 9 SSE frames (`ttftMs: 3847`, `43` tokens, `view` tool call, markdown reply `` `@openagents/effect-spike` ``). Flow B1 (`MWW_PERMIT` ticket `#1`, `option: "allow_once"`) ran `echo SPRINT_APPROVAL_ALLOW_42` (`exit: 0`). Flow B2 (`MWW_PERMIT` ticket `#3`, `answer: false`) blocked `echo SPRINT_APPROVAL_DENY_99` (`toolsCount: 0`, `ranDeniedCmd: false`). Flow C returned `200` on `/steer` and `/stop` and settled to `ready`. All 3 spawned worker PIDs (`33067`, `33229`, `33306`) were cleanly reaped on shutdown. | **PASS** |
| Streaming smoothness & browser hydration | **Hard** | No input lag while typing in isolated `Composer`; `VirtualList` tail-follows with zero console/hydration errors | **Passed:** Headless Chrome (`--headless=new --mute-audio`) hydrated `GET /` with `0` console errors and `0` exceptions, connected live SSE (`ready · pid 33689`), typed and submitted in `[data-testid="composer-input"]`, and rendered `CHROME_E2E_PONG` + `TTFT 2469ms · 198 tok/s` inside `VirtualList`. | **PASS** |
| Effect v4 backend ecosystem compatibility | **Hard** | Postgres client, HTTP/SSE Web Handler (`HttpRouter.toWebHandler`), and auth primitives pass `tsc --noEmit` under `strict` + `exactOptionalPropertyTypes` | **Passed:** Core `effect` (`4.1.6` / latest `4.1.7`) ships `./http`, `./http-api`, `./sql`, `./rpc`, `./cluster`, `./workflow`, `./eventlog`, `./persistence`, `./jose`, `./socket`, `./process`, `./observability`. Verified `@effect/sql-pg@^4.1.7` + `effect/sql` installs cleanly and passes `tsc --noEmit` with `strict` + `exactOptionalPropertyTypes`. Standalone `dist/server/index.js` served `/api/health` (`200`, 13 ms) and `/` SSR (`200`, 8 ms, 3,272 bytes). | **PASS** |
| Deterministic tests + DevTools MCP | **Hard** | `Story.explore` + `Play` pass in Node (`< 2s`), and DevTools MCP `get_state`/`dispatch` controls live browser tab | **Passed:** `vitest run` passed **7/7 tests across 3 files in 605 ms** (`wire.test.ts`, `ApprovalCard.story.test.ts` with **`Story.explore` returning `closed: true`**, `ChatFlow.play.test.ts` on `TestHost` + `TestClock` + `ssr` hydration check). Live MCP (`http://127.0.0.1:4774/mcp`) verified `list_sessions`, `list_instances`, `get_state(App)`, `list_entries`, and `dispatch({ _tag: "ToggledGated" })` flipping the live Chrome DOM from `approvals: on` to `approvals: off`. | **PASS** |
| Authoring & agent ergonomics | **Qualitative** | Adding `TodosPanel` (`MWW_TODOS`) and full chat UI is clean without fighting types once patterns are known | **Passed:** Total spike is ~1,400 LOC of strict TS across 17 files (`0` `tsc --noEmit` errors, `0` unreasoned files in `coax/web/tool.sh tools-check`). Adding `TodosPanel` took < 5 minutes. | **PASS** |
| Production bundle & SSR build | **Qualitative** | Clean `vite build` with `serverOutDir`, fast build, reasonable gzipped client bundle (~150 kB) | **Passed (note):** Client built in **122 ms** (`151.86 kB` gzipped JS + `3.96 kB` gzipped CSS — 1.86 kB over the 150 kB target because the spike eagerly bundles full `MarkdownParser` + `VirtualList` + `Collapsible` + `effect` v4 without `Component.lazy` splitting); SSR `dist/server/index.js` built in **36 ms** (`188.46 kB` gzipped). | **PASS** |

---

## 2. Key Technical Findings Caught During the Sprint

These real issues surfaced during live testing and should guide Phase 1 production implementation:

1. **`oa-worker` permission gating (`MWW_ASK` vs `MWW_PERMIT`):**
   - `oa-worker` defaults `OA_AGENT_PERMISSION_MODE` to `bypass` unless set at process spawn (`OA_AGENT_PERMISSION_MODE=default`).
   - In `oa-worker.coax`, `MWW_ASK` (`{"kind":"ask",...}`) is emitted *after* `proc_start` as a non-blocking command-started progress notice, whereas **`MWW_PERMIT` (`{"kind":"permit",...}`)** is the actual blocking approval/question gate (carrying JSON option objects `[{"optionId":"allow_once",...},{"optionId":"allow_session",...}]`).
   - Mapping `MWW_ASK` as running progress and `MWW_PERMIT` as the blocking `ApprovalCard` fixed Allow/Deny semantics cleanly.
2. **Effect v4 `HttpRouter.add` vs `HttpRouter.addAll`:**
   - `HttpRouter.add(method, path, handler)` treats handler requirements `R` as **per-request** dependencies (`Request.From<"Requires", R>`), not Layer build-time requirements. Capturing `SessionHub` once inside `HttpRouter.addAll(Effect.gen(function*() { const hub = yield* SessionHub; return [...] }))` resolves it cleanly at Layer build time with zero per-request lookup failures.
3. **`@effect/ui` authoring conventions that save time upfront:**
   - Component `view` always takes `(h, model)` — props live on `h.props` (a 3-argument `view` falls into the headless component overload).
   - Command output mapping uses standalone `Command.map(cmd, fn)`, not `cmd.map(fn)`.
   - `@effect/ui-kit` `Collapsible`'s `c.panel(...)` returns `Element | undefined` (omitted when closed), so put `.children(...)` inside the element passed to `c.panel(...)`.
   - `HtmlServerVite` requires sibling output directories (`build.outDir: "dist/client"` + `serverOutDir: "dist/server"`).
4. **Bundle trimming headroom:**
   - The 151.86 kB gzipped client bundle includes `MarkdownParser` + `@effect/ui-kit` + `effect` v4 in one chunk. For production, server-side or lazy-loaded (`Component.lazy`) panes (`DiffTab`, `FileViewerTab`, `TerminalTab`) will keep the initial route well under 120 kB gzipped.

---

## 3. Recommended Next Step (Phase 1 Production Kickoff)

Proceed to **Phase 1** of [`full-architecture.md`](full-architecture.md):
1. Build the long-lived Linux Sandbox Bridge (`mcworker-wire` over authenticated WSS/stdio) wrapping `oa-worker` inside `oa-boat` sandboxes.
2. Back `SessionHub` with `@effect/sql-pg` (`chat.turn_events_v2` + `chat.sync_operations_v2`) emitting canonical `openagents.chat.event.v2` frames with `Last-Event-ID` SSE resume so Web (`@effect/ui`), Desktop (`OpenAgents.app`), and iOS share live cloud conversations.
