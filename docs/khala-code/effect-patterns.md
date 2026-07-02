# Khala Code Desktop Effect Patterns

Status: approved pattern reference for new Khala Code Desktop Effect work.
Scope: `clients/khala-code-desktop`, `packages/khala-qa-harness`, and the
desktop-owned parts of `packages/khala-tools`.

Use the Effect v4 idioms already present in this repository. Do not introduce
parallel wrapper APIs, custom mini-runtimes, or untyped promise helpers when an
existing Effect pattern below fits the seam.

## Schema-First Contracts And Derived Types

Use this when data crosses a process, RPC, CLI, JSONL, test-fixture, or
projection boundary. Define the runtime contract once with `Schema`, derive the
TypeScript type from it with `typeof X.Type`, and decode at the boundary.

Approved desktop-native examples:

- `clients/khala-code-desktop/src/shared/rpc.ts` defines RPC failures,
  backend projections, harness status, metrics, and request/response payloads as
  `S.Struct`, `S.Union`, `S.Literal`, and `S.Literals`, then exports types such
  as `KhalaCodeDesktopRpcBridgeFailure = typeof
  KhalaCodeDesktopRpcBridgeFailure.Type` and `KhalaCodeDesktopChatTurnRequest =
  typeof RpcChatTurnRequest.Type`.
- `clients/khala-code-desktop/src/shared/headless-events.ts` defines headless
  JSONL events as `KhalaCodeHeadlessThreadEvent = S.Union(...)` and derives
  `type KhalaCodeHeadlessThreadEvent = typeof
  KhalaCodeHeadlessThreadEvent.Type`.
- `packages/khala-qa-harness/src/scenario.ts` is the right contract shape for
  QA scenario actions and oracles; new unions in this file should keep following
  the schema-first style and derive their exported types from the schema where
  possible.

Replaces this anti-pattern:

- Hand-maintained interfaces plus ad hoc field extraction. Current migration
  seams include `clients/khala-code-desktop/src/bun/codex-app-server-client.ts`,
  where stdout JSON is parsed with `JSON.parse(line) as JsonRpcResponse &
  JsonRpcNotification`, and
  `clients/khala-code-desktop/src/bun/codex-app-server-chat-runtime.ts`, where
  helpers such as `stringField`, `objectField`, and `arrayField` probe unknown
  app-server payloads. Those seams are acceptable historical bridges, not the
  pattern for new boundaries.

## Context Services And Layers

Use this when code needs environment, clocks, randomness, redaction, transport,
or another dependency that should be swappable in tests. Model the dependency as
a `Context.Service`; provide live and test layers explicitly.

Approved desktop-native examples:

- `clients/khala-code-desktop/src/bun/khala-code-config.ts` defines
  `KhalaCodeConfig extends Context.Service`, `KhalaCodeConfigLive`, and
  `KhalaCodeConfig.testProfile(...)`. Tests in
  `clients/khala-code-desktop/tests/khala-code-config.test.ts` inject the test
  layer with `Effect.provide(...)`.
- `packages/khala-qa-harness/src/deterministic-env.ts` defines
  `KhalaQaTransport extends Context.Service`, a `stubTransportLayer`, and
  `TestEnvironmentLayer(...)`.
- `packages/khala-tools/src/runtime.ts` defines `KhalaToolRuntimeService` for
  time, sleep, and IDs, with live and deterministic implementations.
- `packages/khala-tools/src/redaction.ts` defines
  `KhalaPrivacyRedactionService` as the owner-local redaction service.

Replaces this anti-pattern:

- Direct reads from `process.env`/`Bun.env`, direct `Date.now()` calls, or hidden
  module-level singletons inside logic. Current pre-migration seams include
  `clients/khala-code-desktop/src/bun/claude-app-sdk-chat-runtime.ts`, where
  session and turn IDs still use `Date.now()`, and older env parsing paths in
  `clients/khala-code-desktop/src/bun/codex-token-usage-telemetry.ts` that
  should move behind `KhalaCodeConfig` as touched.

## Scoped Resource Lifecycles

Use this for subprocesses, SDK query handles, temp resources, timers tied to a
run, and anything that must be closed on interruption or failure. Acquire the
resource with `Effect.acquireRelease(...)`, run it in a `Scope`, and put cleanup
in the release action.

Approved desktop-native examples:

- `clients/khala-code-desktop/src/bun/claude-app-sdk-chat-runtime.ts` wraps the
  Claude SDK query handle in `Effect.acquireRelease(...)`, closes the handle,
  aborts the controller, and removes the active turn in the release action; the
  async iterable is consumed inside `Effect.scoped`.
- `clients/khala-code-desktop/src/bun/fleet-run-supervisor.ts` uses
  `Scope.addFinalizer(...)` so a supervisor stop closes the active loop and
  removes the one-supervisor-per-Pylon guard.
- `clients/khala-code-desktop/src/bun/khala-codex-fleet-tools.ts` creates a
  `Scope` for an active fleet run and later closes it with
  `Scope.close(scope, Exit.void)` in `releaseActive(...)`.
- `packages/khala-tools/src/process-sandbox-macos.ts` uses
  `Effect.scoped(...)` with `Effect.acquireRelease(...)` for the sandbox profile
  and spawned process group in one-shot command execution.

Replaces this anti-pattern:

- Starting subprocesses with loose `spawn`/`Bun.spawn`, storing handles in maps,
  and relying on scattered `try`/`catch`, `setTimeout`, or manual cleanup. Current
  migration seams include `clients/khala-code-desktop/src/bun/codex-app-server-client.ts`
  request timeouts and subscriber cleanup, and
  `clients/khala-code-desktop/src/bun/khala-codex-fleet-tools.ts` helper paths
  that still combine manual timers with process killing.

## Error Handling: Typed Failures Vs Defects

Use typed failures for expected operational outcomes the caller can handle:
unavailable process, failed remote post, decode failure, unknown tool, refused
request, timeout, or missing fixture. Let defects stay defects when the program
is internally inconsistent and should not be recovered as normal control flow.

Approved desktop-native examples:

- `clients/khala-code-desktop/src/bun/claude-app-sdk-chat-runtime.ts` defines
  `ClaudeSdkRuntimeError` with `Data.TaggedError(...)` and maps SDK query errors
  through `Effect.tryPromise({ catch: claudeSdkRuntimeError })`.
- `clients/khala-code-desktop/src/bun/codex-token-usage-telemetry.ts` models
  retryable report failures and final inbox-flag failures with
  `KhalaCodeDesktopTokenUsageReportFailure` and
  `KhalaCodeDesktopTokenUsagePersistentFailure`. The reporter retries with
  `Schedule.exponential(...)` and then returns a typed persistent failure.
- `clients/khala-code-desktop/src/bun/claude-app-sdk-chat-runtime.ts` inspects
  the failed `Exit` with `Cause.isFailReason` before projecting token-usage
  failure flags.
- `packages/khala-tools/src/dispatcher.ts` catches only
  `KhalaToolRuntimeError` with `Effect.catchTag(...)` where tool failures should
  become tool-result data.
- `packages/khala-tools/src/process-sandbox-macos.ts` uses `Effect.orDie` only
  for sandbox cleanup failure, which is release-path cleanup and not a normal
  user-facing failure.

Replaces this anti-pattern:

- Bare `catch {}` blocks and `{ ok: false, error: string }` as the only typed
  surface. Current seams include thread mutation methods in
  `clients/khala-code-desktop/src/bun/codex-app-server-chat-runtime.ts`, which
  catch unknown errors and return string errors, and subscriber isolation in
  `clients/khala-code-desktop/src/bun/codex-app-server-client.ts`, which logs
  thrown subscriber errors as diagnostics.

## Config

Use this when a setting is controlled by env, secrets, local files, or test
profiles. New desktop config belongs in `KhalaCodeConfig` unless it is truly
local to a test fixture. Secret values use `Config.redacted(...)` and should not
be printable.

Approved desktop-native examples:

- `clients/khala-code-desktop/src/bun/khala-code-config.ts` separates
  `KhalaCodePlainEnvKeys` from `KhalaCodeSecretEnvKeys`, parses them with
  `Config.schema(...)` and `Config.redacted(...)`, and exposes `config.env` for
  child process interop.
- `clients/khala-code-desktop/tests/khala-code-config.test.ts` verifies the full
  declared key surface, secret redaction in printable output, and layer-backed
  test injection.

Replaces this anti-pattern:

- Sprinkling new `process.env.X` reads through runtime logic, or parsing secret
  files in unrelated modules. Existing transitional seams include
  `clients/khala-code-desktop/src/bun/codex-token-usage-telemetry.ts`, which
  still resolves token-usage env and local secret paths directly, and should be
  folded into the config service as that area is touched.

## Stream Event Pipelines

Use this for lifecycle lines, SDK event iterables, SSE/NDJSON, and UI update
pipelines. Convert the source to a `Stream`, decode/transform one event at a
time, then collect or run a sink. Keep malformed input local to the pipeline and
do not let one bad frame break unrelated subscribers.

Approved desktop-native examples:

- `clients/khala-code-desktop/src/ui/fleet-worker-cards.ts` consumes fleet
  lifecycle NDJSON with `Stream.fromAsyncIterable(...)`, `Stream.decodeText()`,
  `Stream.splitLines`, and `Stream.runForEach(...)` before projecting worker
  card frames.
- `clients/khala-code-desktop/src/bun/claude-app-sdk-chat-runtime.ts` consumes
  the Claude SDK async iterable with `Stream.fromAsyncIterable(...)` and projects
  each SDK message into desktop turn events.
- `packages/khala-qa-harness/src/rpc-driver.ts` exposes driver events as a
  `Stream.Stream<KhalaCodeQaAppEvent, KhalaCodeQaDriverFailure>`, keeping the
  QA driver boundary stream-shaped even when a fixture has no events.

Replaces this anti-pattern:

- Manual chunk buffers, newline loops, and arrays of callback subscribers.
  Current seams include `clients/khala-code-desktop/src/bun/codex-app-server-client.ts`,
  which manually buffers stdout and loops subscribers, and older fleet helper
  code in `clients/khala-code-desktop/src/bun/khala-codex-fleet-tools.ts` that
  still mixes promise loops with streamed process output.

## TestClock-Based Testing

Use this for new Effect service tests that involve time, sleeps, retries,
deadlines, random exploration, or polling. Tests should advance virtual time with
`TestClock.adjust(...)` instead of sleeping in real time.

Approved desktop-native examples:

- `packages/khala-qa-harness/src/deterministic-env.ts` exports
  `TestEnvironmentLayer(...)`, which merges `TestClock.layer()` with a scripted
  transport layer, and `withSeed(...)` for reproducible Effect random values.
- `packages/khala-qa-harness/src/vitest/deterministic-env.test.ts` uses
  `@effect/vitest` `it.effect(...)`, reads `currentMillis`, advances
  `TestClock.adjust("2 seconds")`, and asserts deterministic transport calls.
- `packages/khala-tools/src/runtime.ts` supplies a deterministic runtime service
  for code that has not yet moved to `TestClock` directly.

Replaces this anti-pattern:

- `await new Promise(resolve => setTimeout(resolve, ...))`, wall-clock polling
  loops, and `Date.now()` assertions in tests. Existing seams include
  `clients/khala-code-desktop/tests/claude-app-sdk-chat-runtime.test.ts`,
  `clients/khala-code-desktop/tests/preview-bridge.test.ts`, and
  `packages/khala-tools/src/exec-command.test.ts`, which should be converted to
  TestClock-backed tests as their owners touch them.
