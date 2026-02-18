import * as Fs from "node:fs/promises"
import * as Path from "node:path"

import { Cause, Effect, Layer, Scope } from "effect"

import { BrowserService, BrowserServiceLive, BrowserServiceNone } from "../browser/BrowserService.ts"
import {
  EffuseTestConfig,
  EffuseTestConfigLive,
  type EffuseTestConfigOverrides,
} from "../config/EffuseTestConfig.ts"
import { ProbeServiceLive, ProbeService } from "../effect/ProbeService.ts"
import { CurrentSpanId } from "../effect/span.ts"
import type { SpanId, TestCase, TestStatus } from "../spec.ts"
import { openagentsComSuite } from "../suites/openagents-com.ts"
import { startViewerServer } from "../viewer/server.ts"
import { TestContext } from "./TestContext.ts"
import { startWranglerDev } from "./TestServer.ts"

const toError = (cause: unknown): Error =>
  cause instanceof Error ? cause : new Error(String(cause))

export class RunnerError extends Error {
  readonly operation: string
  override readonly cause: unknown

  constructor(operation: string, cause: unknown) {
    const err = toError(cause)
    super(`[Runner] ${operation}: ${err.message}`)
    this.name = "RunnerError"
    this.operation = operation
    this.cause = cause
  }
}

const tryRunnerPromise = <A>(operation: string, f: () => Promise<A>) =>
  Effect.tryPromise({
    try: f,
    catch: (cause) => new RunnerError(operation, cause),
  })

export type RunnerOptions = {
  readonly projectDir: string
  readonly serverPort: number
  readonly baseUrl?: string
  readonly viewerPort: number
  readonly headless: boolean
  readonly watch: boolean
  readonly grep?: string
  readonly tags?: ReadonlyArray<string>
  readonly configOverrides?: EffuseTestConfigOverrides
}

const defaultArtifactsRoot = (runId: string) =>
  Path.resolve(process.cwd(), "../../output/effuse-test", runId)

type TestEnv = ProbeService | BrowserService | TestContext | EffuseTestConfig | Scope.Scope

const selectSuite = (
  projectDir: string,
): Effect.Effect<ReadonlyArray<TestCase<TestEnv>>, RunnerError, EffuseTestConfig> =>
  Effect.gen(function* () {
    const normalized = projectDir.replaceAll("\\", "/")
    if (normalized.endsWith("/apps/openagents.com"))
      return yield* openagentsComSuite() as Effect.Effect<
        ReadonlyArray<TestCase<TestEnv>>,
        RunnerError,
        EffuseTestConfig
      >
    return yield* Effect.fail(
      new RunnerError("select suite", `Unsupported projectDir: ${projectDir}`),
    )
  })

const filterTests = (tests: ReadonlyArray<TestCase<TestEnv>>, options: RunnerOptions): ReadonlyArray<TestCase<TestEnv>> => {
  let out = tests
  if (options.grep) {
    const re = new RegExp(options.grep)
    out = out.filter((t) => re.test(t.id))
  }

  // Safety default: never run production-targeting tests unless explicitly requested.
  const wantsProd = options.tags?.includes("prod") ?? false
  if (!wantsProd) {
    out = out.filter((t) => !t.tags.includes("prod"))
  }

  // Visual snapshots are opt-in (local baselines, heavier runtime).
  const wantsVisual = options.tags?.includes("visual") ?? false
  if (!wantsVisual) {
    out = out.filter((t) => !t.tags.includes("visual"))
  }

  if (options.tags && options.tags.length > 0) {
    const required = new Set(options.tags)
    out = out.filter((t) => t.tags.some((tag) => required.has(tag)))
  }
  return out
}

const runEffect = Effect.fn("effuseTest.runner.run")(function* (options: RunnerOptions) {
  return yield* Effect.scoped(
    Effect.gen(function* () {
      const config = yield* EffuseTestConfig
      const runId = crypto.randomUUID()
      const runStart = Date.now()
      const artifactsRoot = defaultArtifactsRoot(runId)
      yield* tryRunnerPromise("fs.mkdir(artifacts root)", () =>
        Fs.mkdir(artifactsRoot, { recursive: true }),
      )

      const tests = filterTests(
        yield* selectSuite(options.projectDir),
        options,
      )
      if (tests.length === 0) {
        yield* Effect.logWarning("No tests selected")
        return
      }

      const needsBrowser = tests.some((t) => t.tags.includes("browser"))

      const viewer = options.watch
        ? yield* startViewerServer(options.viewerPort).pipe(
            Effect.tap((v) => Effect.logInfo(`Viewer: ${v.url}`)),
          )
        : undefined

      const eventsPath = Path.join(artifactsRoot, "events.jsonl")
      const probeLayer = ProbeServiceLive({
        eventsPath,
        broadcast: viewer?.broadcast,
      })

      const baseUrl = options.baseUrl?.replace(/\/+$/, "")
      if (baseUrl && !/^https?:\/\//.test(baseUrl)) {
        return yield* Effect.fail(
          new RunnerError("validate --base-url", `--base-url must start with http(s)://, got: ${baseUrl}`),
        )
      }

      const server = baseUrl
        ? ({ baseUrl } satisfies { readonly baseUrl: string })
        : yield* startWranglerDev({
            projectDir: options.projectDir,
            port: options.serverPort,
            env: config.childProcessEnv,
          })
      const browserLayer = needsBrowser
        ? BrowserServiceLive({
            headless: options.headless,
            chromePath: config.chromePath,
          })
        : BrowserServiceNone

      const mainLayer = Layer.mergeAll(probeLayer, browserLayer)

      const main = Effect.gen(function* () {
        const probe = yield* ProbeService
        const browser = yield* BrowserService

        yield* probe.emit({
          type: "run.started",
          runId,
          ts: Date.now(),
          baseUrl: server.baseUrl,
        })
        yield* probe.emit({
          type: "server.started",
          runId,
          ts: Date.now(),
          baseUrl: server.baseUrl,
        })
        yield* probe.emit({
          type: "artifact.created",
          runId,
          ts: Date.now(),
          testId: "run",
          kind: "events",
          path: eventsPath,
        })

        let overall: TestStatus = "passed"

        for (const test of tests) {
          const testId = test.id
          const testStart = Date.now()
          const testArtifactsDir = Path.join(artifactsRoot, sanitizePath(testId))
          yield* tryRunnerPromise("fs.mkdir(test artifacts dir)", () =>
            Fs.mkdir(testArtifactsDir, { recursive: true }),
          )

          const ctxLayer = Layer.succeed(TestContext, {
            runId,
            testId,
            baseUrl: server.baseUrl,
            artifactsDir: testArtifactsDir,
          })

          yield* probe.emit({
            type: "test.started",
            runId,
            ts: testStart,
            testId,
            tags: test.tags,
          })

          const testSpanId = crypto.randomUUID() as SpanId
          yield* probe.emit({
            type: "span.started",
            runId,
            ts: Date.now(),
            testId,
            spanId: testSpanId,
            name: testId,
            kind: "test",
          })

          const program = Effect.locally(CurrentSpanId, testSpanId)(
            test.steps.pipe(
              test.timeoutMs != null
                ? Effect.timeoutFail({
                    duration: `${test.timeoutMs} millis`,
                    onTimeout: () => new Error(`Timed out after ${test.timeoutMs}ms`),
                  })
                : (a) => a,
            ),
          ).pipe(Effect.provide(ctxLayer))

          const exit = yield* program.pipe(Effect.exit)

          if (exit._tag === "Success") {
            yield* probe.emit({
              type: "span.finished",
              runId,
              ts: Date.now(),
              testId,
              spanId: testSpanId,
              status: "passed",
              durationMs: Date.now() - testStart,
            })
            yield* probe.emit({
              type: "test.finished",
              runId,
              ts: Date.now(),
              testId,
              status: "passed",
              durationMs: Date.now() - testStart,
            })
          } else {
            overall = "failed"

            const error = { name: "TestFailure", message: Cause.pretty(exit.cause) }

            // Best-effort failure artifacts.
            const screenshotPath = Path.join(testArtifactsDir, "failure.png")
            const htmlPath = Path.join(testArtifactsDir, "failure.html")
            const captured = yield* Effect.locally(CurrentSpanId, testSpanId)(
              browser
                .captureFailureArtifacts({ screenshotPath, htmlPath })
                .pipe(Effect.provide(ctxLayer), Effect.catchAll(() => Effect.succeed(false))),
            )
            if (captured) {
              yield* probe.emit({
                type: "artifact.created",
                runId,
                ts: Date.now(),
                testId,
                kind: "screenshot",
                path: screenshotPath,
              })
              yield* probe.emit({
                type: "artifact.created",
                runId,
                ts: Date.now(),
                testId,
                kind: "html",
                path: htmlPath,
              })
            }

            yield* probe.emit({
              type: "span.finished",
              runId,
              ts: Date.now(),
              testId,
              spanId: testSpanId,
              status: "failed",
              durationMs: Date.now() - testStart,
              error,
            })
            yield* probe.emit({
              type: "test.finished",
              runId,
              ts: Date.now(),
              testId,
              status: "failed",
              durationMs: Date.now() - testStart,
              error,
            })
          }
        }

        yield* probe.emit({
          type: "server.stopped",
          runId,
          ts: Date.now(),
        })
        yield* probe.emit({
          type: "run.finished",
          runId,
          ts: Date.now(),
          status: overall === "passed" ? "passed" : "failed",
          durationMs: Date.now() - runStart,
        })

        yield* probe.flush

        if (overall !== "passed") {
          return yield* Effect.fail(new RunnerError("test run", "One or more tests failed"))
        }
      }).pipe(Effect.provide(mainLayer))

      yield* main
    }),
  ).pipe(
    Effect.provide(EffuseTestConfigLive(options.configOverrides)),
    Effect.mapError((cause) =>
      cause instanceof RunnerError ? cause : new RunnerError("run", cause),
    ),
  )
})

export const run = (options: RunnerOptions): Effect.Effect<void, RunnerError> =>
  runEffect(options)

const sanitizePath = (s: string): string => s.replaceAll(/[^a-zA-Z0-9._-]+/g, "_").slice(0, 160)
