# Khala Code Effect Integration Audit — Deep Dive

Date: 2026-07-01
Status: audit + recommendations. Successor to
`docs/audits/2026-06-28-effect-usage-audit.md` and
`docs/audits/2026-06-29-effect-usage-audit.md`, which scoped
`apps/openagents.com`, `apps/pylon`, and `packages/*` — and never covered
`clients/khala-code-desktop`, now the most active product surface. This audit
goes deeper: it is grounded in the Effect v4 source itself
(`projects/repos/effect-smol`, the code behind our pinned
`effect@4.0.0-beta.70`), effect-smol's own `.patterns/` rules, the full
`effect-solutions` topic set, and a file-by-file read of the desktop app and
every system it consumes. Documentation-only; no runtime changes.

## 1. Executive Summary

**The desktop app links against Effect but does not adopt it.** Only 5 of 55
source files in `clients/khala-code-desktop/src` import `effect` at all.
There are **zero** uses of `Context.Service`, `Layer`, `Config`, `Scope`,
`Clock`, or tagged errors anywhere in the app's own code. The genuine Effect
machinery it depends on — the deterministic delegate program, the tool
dispatcher, redaction, the MCP server — lives in `packages/khala-tools` and
`apps/pylon`, and the desktop reaches it through `Effect.runPromise` bridges
placed mid-flow, not at edges. The app's own control plane — subprocess
supervision (five independent hand-rolled implementations), the Codex
app-server JSON-RPC client, both chat runtimes, the 2,018-line RPC handler
surface, and the entire 2,598-line UI shell — is plain async with manual
`Map`s, `setTimeout`, deferred promises, bare `catch {}`, `JSON.parse … as
T`, and vanilla DOM.

The sharper finding: **every package the desktop consumes offers a
typed-Effect or Foldkit surface, and the desktop systematically takes the
imperative escape hatch out of each one.** It imports `icon-dom`'s
`iconElement` instead of `@openagentsinc/ui`'s Foldkit `iconView`/
`IconService`; it renders the Arbiter graph via `renderArbiterGraphHtml` →
`innerHTML` instead of the Foldkit `arbiterGraphFigure` vdom; it drives Pylon
as argv + stdout-JSON strings instead of a typed service; it locally
re-declares the Pylon lifecycle wire schema and then discards its own decode
back into `stringField` probing. Meanwhile `foldkit@^0.102.1` is already in
its `package.json`, `@openagentsinc/ui` is a full Foldkit component library
(workroom shell, sidebars, panels, buttons, drawers), and
**`apps/autopilot-desktop` is a working Electrobun + Foldkit app in this
repo** — the exact template, down to `src/ui/{main,model,message,view,
subscriptions}.ts` and a deterministic Effect test harness.

> **Clarification (owner direction, 2026-07-01):** `clients/khala-code-desktop`
> is the ONLY active desktop development target. `apps/autopilot-desktop` and
> the other desktop surfaces are postponed — do not route new desktop feature
> work there. Every reference to `apps/autopilot-desktop` in this audit means
> "source material / pattern template to copy FROM", never "surface to develop
> ON". Port its Foldkit program layout, subscriptions pattern, and
> deterministic test harness into `clients/khala-code-desktop` (or shared
> packages); leave the autopilot-desktop app itself untouched.

One layer down, `packages/khala-tools` has excellent Schema contracts and two
genuinely idiomatic modules (`permission-policy`, `fleet-delegate-program`),
but its execution substrate is Effect-flavored Promise glue: exactly one real
service/layer (`redaction.ts`), **zero** `Clock` or `Scope`/`acquireRelease`
uses in the whole package, ~28 `Effect.promise(async …)` wrappers, and an
`Effect.runPromise`-inside-`Effect.promise` anti-pattern in the exec path
that defeats interruption entirely.

This is precisely where the day-to-day headaches come from: interleaved
transcript state races, orphaned subprocesses on interrupt, silently dropped
notifications, token undercounts from fire-and-forget reporting, and flaky
sleep-based tests — every one traces to a place where the Effect model was
available and skipped.

## 2. Reference Base (What "Good" Means In v4)

Verified against the effect-smol source, not v3 memory:

- **The monorepo collapsed into core `effect`.** `Schema`, `Stream`, `Config`,
  `ConfigProvider`, `FileSystem`, `Path`, `Clock`, `Scope`, `Sink` are all
  top-level in `effect`. HTTP lives in `effect/unstable/http`; child
  processes in `effect/unstable/process`; testing helpers (`TestClock`,
  `FastCheck`) in `effect/testing`. `@effect/platform-bun` provides only
  layers (`BunServices.layer`, `BunHttpServer.layer`, `BunRuntime.runMain`).
- **Services**: `Context.Service` class syntax with `@path/Name` identifiers,
  methods returning `Effect` with `R = never`, layers as statics
  (`layer`, `testLayer`), provided once at the entry point
  (effect-smol `.patterns/effect.md:92`, `effect-solutions
  services-and-layers`).
- **Errors**: `Schema.TaggedErrorClass` (the actual v4 symbol; there is no
  plain `Schema.TaggedError`) for anything crossing a serialization boundary;
  `Data.TaggedError` for internal-only; errors are yieldable; recover with
  `catchTag`/`catchTags`; `Schema.Defect` wraps unknown platform errors;
  defects are for bugs, caught only at boundaries.
- **Boundaries**: `Schema.fromJsonString(schema)` fuses parse+decode;
  `Schema.decodeUnknownEffect` returns typed `SchemaError`; brand recurring
  primitives.
- **Subprocesses**: `ChildProcess.make` in `effect/unstable/process` is
  **scoped** — the process dies when the `Scope` closes; the handle exposes
  `stdin: Sink`, `stdout/stderr: Stream<Uint8Array>`, `exitCode`, `kill`.
  Requires `ChildProcessSpawner` (provided by `BunServices.layer`).
- **NDJSON**: `Stream.decodeText` → `Stream.splitLines` →
  `Schema.decodeUnknownEffect` per line → `Stream.runForEach`.
- **Config**: `Config.schema(schema, "ENV")`, `Config.redacted` for secrets,
  wrapped in a `Context.Service` so tests use `Layer.succeed`.
- **Time**: never `Date.now()`/`new Date()` in logic — `Clock`, so
  `TestClock.adjust` controls it (effect-smol `AGENTS.md:109`).
- **Testing**: `@effect/vitest` `it.effect` with auto TestContext; scopes
  auto-close at test end; never `Effect.runSync` in tests.
- **Style**: never `try/catch` inside `Effect.gen`; `return yield*` for
  terminal effects; `Effect.fn("name")` for traced reusable functions,
  `Effect.fnUntraced` instead of bare `(a) => Effect.gen(...)` wrappers;
  `.pipe()` for cross-cutting timeout/retry; and — per effect-smol's own
  simplicity rules — don't wrap pure functions in Effect at all.

## 3. Findings

### 3.1 The desktop app, file by file (condensed)

| File | Class | Key evidence |
| --- | --- | --- |
| `bun/index.ts` | plain async | top-level await, `Bun.serve`, 15 env reads; RPC body `as {args?}` + bare `catch{}` → `args=[]` (L136-141) |
| `bun/codex-app-server-client.ts` | plain async | hand-rolled JSON-RPC: `Map<JsonRpcId, Pending>` (L169), per-request `setTimeout` (L225), `JSON.parse(line) as JsonRpcResponse & JsonRpcNotification` (L300); no `effect` import |
| `bun/codex-app-server-chat-runtime.ts` | plain async | 1,176 lines; manual deferred promise + timeout + subscribe/unsubscribe assembly (L937-1096); field-probing param walks |
| `bun/rpc-handlers.ts` | plain async | ~60 async handlers; failures as `{ok:false, error: string}`; probe failures swallowed to `[]` (L742) |
| `bun/khala-chat-runtime.ts` | Effect-adjacent | `Effect.runPromise` mid-loop per tool call/message (L1241-1305); manual SSE parser; sequential tool loop |
| `bun/khala-codex-fleet-tools.ts` | Effect-adjacent (best) | real `Effect.gen`, `Stream` line reader (L1202-1230), Schema union decode (L288) — but async top-level fns, `Effect.promise` closures mutating outer vars (L734-792) |
| `bun/codex-{harness-status,rate-limits,token-usage-telemetry}.ts`, `apple-fm-sidecar.ts` | plain async | four more copy-paste `spawn` + `setTimeout` + `kill` + `JSON.parse as` blocks; fixed-interval `setInterval` sync without backoff (telemetry L905) |
| `shared/rpc.ts` | plain TS | 939 lines of interfaces; **the entire 57-method RPC contract has zero runtime validation on either transport** |
| `shared/headless-events.ts` | Schema, type-only | `S.Struct`/`S.Union` defined (L9-85) but only for `typeof X.Type`; serializer is `JSON.stringify` (L189), never `S.encode` |
| `ui/main.ts` | vanilla DOM | 2,598 lines; ~18 module-level mutable `let`s (`messages`, `pendingTurn`, `composerState`, `activeCodexThreadId`, …); 28 `createElement`, full `replaceChildren` re-render per event |
| `ui/{fleet-status,codex-thread-sidebar,codex-settings-panel,gym-pane,sidebar,inbox,transcript-render}.ts` | vanilla DOM | `mount*(root, deps)` closures; manual listeners |
| `ui/{fleet-board,gym-graph}-renderer.ts` | Foldkit-string | `renderArbiterGraphHtml` → `innerHTML` — the string helper, not the TEA loop |

`Effect.runPromise` bridges are never at one clean edge; they appear inside
the chat tool loop, inside `spawnCodexInstances`, and inside tool `execute`
progress callbacks.

### 3.2 The seven escape-hatch seams (consumed layer)

1. **Shell rendering**: imperative DOM instead of a Foldkit program — zero
   `foldkit` imports despite the declared dependency.
2. **Icons**: `@openagentsinc/ui/icon-dom` `iconElement` instead of
   `iconView`/`IconService` (which exists as a real `Context.Service`
   returning `Effect<Html>` — `packages/ui/src/icon.ts:3826`).
3. **UI kit**: hand-rolled panels/sidebars/menus instead of
   `@openagentsinc/ui`'s workroom shell, navigation, panel, button, drawer,
   and split-pane Foldkit components (`packages/ui/README.md:38-60`).
4. **Graph**: `renderArbiterGraphHtml` string → `innerHTML` instead of
   `arbiterGraphFigure` Foldkit vdom (`packages/arbiter-effect/src/foldkit.ts:290`).
5. **Pylon**: subprocess argv + `--json` stdout parsing (`runPylonCommand`,
   `khala-codex-fleet-tools.ts:1044`) instead of any typed service interface.
6. **Lifecycle stream**: Pylon's `AssignmentRunLifecycleEvent` is a plain TS
   type (`apps/pylon/src/assignment.ts:193`); the desktop re-declares its own
   Schema (`khala-codex-fleet-tools.ts:261`), decodes it (L2673), then
   **down-casts the decoded value back to `Record` and re-reads it via
   `stringField` across four field-name variants** (L2680-2712) inside
   `catch → null`. Three drifting definitions of one wire contract.
7. **Tool execution**: `executeKhalaTool` returns `Effect<Result, never>` —
   errors collapsed to data at the boundary — and khala-tools internally
   calls `await Effect.runPromise(...)` inside `Effect.promise(async …)`
   (`exec-command.ts:130-141`), so no interruption or context ever reaches
   the desktop.

### 3.3 khala-tools substrate debt

**T7.4 update (2026-07-01):** the first substrate pass landed for
`packages/khala-tools`. `KhalaToolRuntimeService` now provides injected
Clock/random for dispatcher duration accounting and event IDs, with a
deterministic runtime used by tests. `makeKhalaToolServices` now exposes a
Layer-backed `KhalaToolServicesService` while preserving the plain factory for
existing callers. The one-shot unsandboxed and macOS Seatbelt exec paths wrap
process groups and mkdtemp seatbelt profiles in `Effect.acquireRelease`, and
`exec-command.ts` keeps permission, workdir, process, and artifact calls in the
Effect chain instead of nesting `Effect.runPromise` inside `Effect.promise`.

Remaining debt in this subsection is outside T7.4's landed slice: older
helpers such as todo/network timestamps and some long-lived session pumps still
need deeper service migration, and `session-rollout.ts` still uses direct
wall-clock/UUID helpers.
- The bright spots to grow from: `permission-policy.ts:54-98` (real
  `Effect.gen` + `catchTag` composition) and `fleet-delegate-program.ts`
  (typed `KhalaFleetDelegateModuleError`, Schema-class parameters, pure
  deterministic control flow — its only gap is taking modules as a plain
  argument instead of a layer).

### 3.4 Strongest existing patterns (standardize these)

1. `collectStreamLines` (`khala-codex-fleet-tools.ts:1202-1230`):
   `Stream.fromAsyncIterable → decodeText → splitLines → runForEach` — this
   is already the canonical v4 NDJSON shape; make it *the* subprocess reader.
2. The delegate program's typed-module-error + trace shape — the model to
   extend inward into chat turns and RPC handlers.
3. Union-of-tagged-structs + `S.decodeUnknownSync` for wire decode
   (`khala-codex-fleet-tools.ts:261-288`) — generalize, and stop discarding
   the decoded type.
4. Schema-first type derivation in `shared/headless-events.ts` — promote
   from type-only to real `S.encode`/`S.decode` at the NDJSON edge.
5. The pervasive DI seams (`spawnFn`, `runner`, injected `fetch`/
   `setInterval`) — the natural lift points into real `Context`/`Layer`
   services.

## 4. Top Debt, Ranked By Risk

Each item names the failure mode it invites today:

1. **RPC contract unvalidated on both transports** (`shared/rpc.ts` plain TS;
   `index.ts:137` and `ui/main.ts:94` both cast `.json()`). UI↔host drift
   surfaces as deep `TypeError`s, not boundary errors — and the QA
   framework's schema oracle has nothing to decode with.
2. **App-server protocol decoded by cast + field probing**
   (`codex-app-server-client.ts:300`). A Codex field rename silently drops
   token usage or turn ids → undercounted tokens, stuck turns.
3. **Notification fan-out with no error isolation**
   (`codex-app-server-client.ts:287`): one throwing subscriber aborts
   delivery to the rest — dropped MCP/ecosystem notifications after any
   projector exception.
4. **Pylon subprocess leak on interrupt**: `defaultCommandRunner` runs inside
   uninterruptible `Effect.promise` with no finalizer
   (`khala-codex-fleet-tools.ts:779-825, 1115`) — interrupting a delegate
   orphans Pylon children. The same class exists in khala-tools'
   `process-sandbox-macos.ts`.
5. **Fire-and-forget token reporting** (`codex-app-server-chat-runtime.ts:993`
   `.catch(() => undefined)`, `Promise.allSettled` reconcile) — silent token
   undercount against the exact-accounting product invariant.
6. **JSON-RPC timeout doesn't cancel the remote turn**
   (`codex-app-server-client.ts:225`) — codex keeps burning tokens on a
   locally "timed-out" request.
7. **`Effect.promise` closures mutating shared outer variables** in
   `spawnCodexInstances` (`khala-codex-fleet-tools.ts:734-792`) — stale state
   bleeds across delegate-module retries.
8. **`ui/main.ts` global mutable state mutated from concurrent async paths**
   (~18 module `let`s, `messages` reassigned in 6+ handlers, full re-render
   per event) — the interleaved-transcript race class, and the reason the
   thread-switch benchmark needed hand-built performance plumbing.
9. **Five divergent subprocess implementations** (app-server client, Pylon
   runner, harness-status, rate-limits, apple-fm sidecar) with inconsistent
   timeout/kill semantics.
10. **No config service**: ~55 env keys read ad hoc with duplicated parsing
    helpers; mis-typed flags silently default differently per read site.
11. **`Effect.runPromise` inside `Effect.promise`** in khala-tools exec/patch
    (`exec-command.ts:130-141`) — nested runtimes; interruption and context
    severed.
12. **Zero Clock/TestClock**: dispatcher durations, cooldown logic, retry
    timers, and 39 test files all on wall-clock; time-dependent tests use
    real sleeps (`apple-fm-sidecar.test.ts:157`, `chat-runtime` tests).
13. **Swallow-to-empty status probes** (`rpc-handlers.ts:742` `catch → []`)
    — real harness/auth failures rendered as "no blockers".
14. **Attachment temp files never cleaned** (`rpc-handlers.ts:196`) —
    unbounded disk growth.
15. **Corrupt session-state file bricks all thread ops**
    (`codex-app-server-chat-runtime.ts:389-396` rethrows non-ENOENT).

## 5. Recommendations

The strategy mirrors the prior audits' successful pattern — make the right
path easy, then migrate the highest-risk seams — but is now specific to the
desktop and sequenced so each phase pays for itself. Phases 1–2 are
independently shippable without touching rendering; phase 3 is the Foldkit
migration they de-risk.

### Phase 1 — Contracts and boundaries (days, highest leverage)

1. **Schema-first `shared/rpc.ts`.** Define every RPC request/response as
   `Schema.Struct`/`Schema.Class` (deriving today's TS types via
   `typeof X.Type` so nothing else changes), decode on both transports
   (preview bridge body + `main.ts` client), and type handler failures as a
   `Schema.Union` of tagged errors instead of `{ok:false, error: string}`.
   This single change gives the QA framework its schema oracle, kills debt
   #1, and makes #13's swallow-to-empty impossible to hide.
2. **One shared Pylon wire-event contract.** Promote
   `AssignmentRunLifecycleEvent` and `PylonKhalaSpawnWorkerEvent` to Effect
   Schema in a shared package; both Pylon-emit and desktop-consume decode
   with it (`Schema.fromJsonString` per NDJSON line). Delete the desktop's
   local re-declaration and the `stringField` fallback probing (#2 of the
   consumed-layer list; kills three drifting definitions).
3. **`KhalaCodeConfig` service.** All ~55 env keys behind one
   `Context.Service` using `Config.schema(...)` + `Config.redacted` for
   tokens, with `Layer.succeed` test profiles. Existing pure helpers stay;
   env access moves.
4. **Notification isolation now** (small, urgent): wrap each subscriber
   dispatch so one throw cannot abort the rest, pending the phase-2 rewrite.

### Phase 2 — The process and protocol spine (a week)

5. **One scoped subprocess service.** Build a `KhalaProcess` service on
   `effect/unstable/process` `ChildProcess.make` (+ `BunServices.layer`):
   scoped lifetime (kill on scope close — fixes leak-on-interrupt), `stdin`
   Sink / `stdout` Stream, timeout via `Effect.timeout` + `kill`
   escalation policy in one place. Replace all five hand-rolled spawn
   implementations. The existing `spawnFn`/`runner` DI seams are the exact
   insertion points.
6. **Codex app-server client as an Effect service.** `CodexAppServer` as
   `Context.Service`: requests as `Effect.fn` with typed
   `Schema.TaggedErrorClass` errors (`RpcTimeout { method, error:
   Schema.Defect }`), responses/notifications Schema-decoded (generate
   candidate schemas from `codex app-server generate-ts` — the parity
   contract already pins the reference), notifications exposed as a
   `Stream` via `Stream.callback` (isolating subscribers by construction),
   timeout policy that also fires `turn/interrupt` (#6), and supervision
   (start/restart/dispose) as scoped acquire/release. The chat runtime's
   deferred-promise/turn assembly collapses into `Effect.gen` over that
   service.
7. **A typed `PylonService`.** `request`, `runAssignment`,
   `lifecycle: Stream<AssignmentRunLifecycleEvent>` — backed by the
   subprocess service and the shared schema from phase 1, replacing
   argv+stdout string coupling. Stub layer for fixtures; this is also what
   the fleet-fan-out instructions' supervisor should consume.
8. **khala-tools substrate fixes** (T7.4 landed 2026-07-01): dispatcher
   durations and event IDs now use injected runtime Clock/random; one-shot
   sandbox/exec process groups use `acquireRelease`; `exec-command.ts` keeps
   permission/process/artifact calls in the Effect chain; and
   `makeKhalaToolServices` has a Layer-backed service form following
   `redaction.ts`. Follow-up work should migrate the remaining older helpers
   called out in §3.3.
9. **Make token reporting an Effect with retry + typed failure** (
   `Effect.retry(Schedule.exponential(...))`, escalate persistent failure to
   an Inbox flag) — closes #5 and #18 against the exact-accounting
   invariant. Attachment temp files become scoped resources (#14).

### Phase 3 — The UI becomes a Foldkit program (the big one, staged)

10. **Adopt the `apps/autopilot-desktop` template — in
    `clients/khala-code-desktop`** (autopilot-desktop itself stays postponed;
    it is copy-from material only): `src/ui/{main,model,
    message,view,subscriptions}.ts`, `Runtime.run`. The state mapping is
    textbook TEA: `messages`/`pendingTurn`/`thinkingTurnId`/
    `activeCodexThreadId`/`transcriptPinnedToEnd` become the Model; the
    streamed `chatTurnEvent`s, keypresses, and Pylon lifecycle events become
    Messages through one `update` funnel — structurally eliminating the
    interleaved-write race class (#8). External sources (Electrobun RPC,
    hotkeys, lifecycle stream) become `Stream.callback +
    Effect.acquireRelease` subscriptions, copying
    `autopilot-desktop/src/ui/subscriptions.ts` verbatim.
11. **Compose the shell from `@openagentsinc/ui` Foldkit components**
    (workroom shell, sidebars, panels, `iconView`/`IconService`) and switch
    the Gym pane to `arbiterGraphFigure` vdom — retiring `icon-dom`,
    `menu-dom`, and `innerHTML` graph injection.
12. **Stage it panel-by-panel** via `Runtime.embed` with Schema-typed Ports:
    new panels (the fleet-fan-out cockpit is the perfect first candidate)
    are written as Foldkit programs embedded in the existing shell; the
    transcript migrates last. Do not attempt a big-bang rewrite of
    `main.ts`.

### Phase 4 — Testing and guardrails

13. **`@effect/vitest` + TestClock for new Effect services**; port the
    autopilot-desktop deterministic harness (`TestEnvironmentLayer`,
    `withSeed`, `stubTransportLayer`) as `packages/khala-qa-harness`'s
    deterministic layer (dovetails with the QA framework doc's §8). Replace
    real-sleep tests as files are touched.
14. **Extend the report-only architecture scan** (recommended by the 06-29
    audit) to `clients/khala-code-desktop` and `packages/khala-tools`:
    flag new `JSON.parse … as`, bare `catch {}`, direct env reads,
    `Date.now()` in logic, `Effect.runPromise` outside allowlisted edges,
    and `setTimeout`-based process kills. Promote the desktop's bun layer
    to hard-fail once phases 1–2 land.
15. **Write the pattern doc** the 06-28 audit asked for, now with
    desktop-native approved examples: `collectStreamLines`, the delegate
    program, `permission-policy`, `redaction.ts`, and the new
    `CodexAppServer`/`PylonService`/`KhalaCodeConfig` services.

### v4-correct sketches (for the implementing agent)

```ts
// Service (v4 canon — Context.Service class form, layer as static)
import { Effect, Layer, Schema, Stream } from "effect"
import * as Context from "effect/Context"
import * as ChildProcess from "effect/unstable/process/ChildProcess"

class RpcTimeout extends Schema.TaggedErrorClass<RpcTimeout>()("RpcTimeout", {
  method: Schema.String,
  error: Schema.Defect,
}) {}

class CodexAppServer extends Context.Service<CodexAppServer, {
  readonly request: (method: string, params: unknown) =>
    Effect.Effect<unknown, RpcTimeout | CodexRpcError>
  readonly notifications: Stream.Stream<CodexNotification>
}>()("@khala-code/CodexAppServer") {
  static readonly layer = Layer.effect(CodexAppServer, Effect.gen(function* () {
    const handle = yield* ChildProcess.make("codex", ["app-server", "--stdio"])
    // stdin: Sink for requests; stdout NDJSON:
    // handle.stdout.pipe(Stream.decodeText(), Stream.splitLines(),
    //   Stream.mapEffect(Schema.decodeUnknownEffect(CodexMessageFromJson)))
    // fork the read loop with Effect.forkScoped — dies with the scope.
    ...
  }))
}
```

Per effect-smol's own rules while writing this: no `try/catch` inside
`Effect.gen`; `return yield*` on terminal effects; `Effect.fn("name")` for
traced service methods; `Effect.fnUntraced` instead of bare gen-wrapper
functions; pure projection/render helpers stay plain functions — do not
Effect-wrap what isn't effectful.

## 6. How This Sequences With The Other Fable Plans

Phases 1–2 here are prerequisites the other plans quietly assume: the
fleet fan-out instructions' `FleetRunSupervisor` should be built on the
`PylonService` + scoped subprocess service (not more `runPylonCommand`
strings), and the QA framework's schema/consistency oracles need the
Schema-first RPC contract to exist. The Claude bring-up plan
(`2026-07-01-claude-code-parity-and-codex-synergies.md`) is the natural
first *consumer* of this spine going the other way: `ClaudeChatRuntime` is
greenfield, so it should be written as the desktop's first real Effect
service from day one (Stream + acquireRelease + Schema-decoded SDK
boundary) rather than migrated later. Recommended order: Phase 1 → fan-out
Lane A/B on the new spine → Phase 2 alongside cockpit work → Phase 3
starting with the cockpit as the first embedded Foldkit program → Phase 4
continuously. The Foldkit migration is the one item that should *not* block
episode 245 or the throughput work — it rides behind them, panel by panel.
The full cross-plan schedule is the unified [`ROADMAP.md`](./ROADMAP.md).

## 7. Invariants While Migrating

- Do not weaken any public-safety, exact-accounting, or isolated-home
  behavior to make a migration cleaner; typed errors must carry public-safe
  reasons only.
- Keep the errors-as-data shape at the *tool-result* boundary (the
  `KhalaToolResultStatus` contract is deliberate); restore typed error
  channels *beneath* it.
- Migrate module-by-module with tests green at every landing (`bun run
  --cwd clients/khala-code-desktop verify` plus touched-package suites);
  no big-bang rewrites; the dirty-checkout worktree discipline applies.
- Consult `effect-solutions` before writing Effect code (per
  `apps/openagents.com/AGENTS.md`), and treat effect-smol `.patterns/` as
  authoritative where they disagree.
