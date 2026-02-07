import { Effect } from "effect"

export type RunId = string
export type TestId = string
export type SpanId = string

export type TestStatus = "passed" | "failed" | "skipped"

export type TestEvent =
  | {
      readonly type: "run.started"
      readonly runId: RunId
      readonly ts: number
      readonly baseUrl: string
    }
  | {
      readonly type: "run.finished"
      readonly runId: RunId
      readonly ts: number
      readonly status: Exclude<TestStatus, "skipped">
      readonly durationMs: number
    }
  | {
      readonly type: "test.started"
      readonly runId: RunId
      readonly ts: number
      readonly testId: TestId
      readonly tags: ReadonlyArray<string>
    }
  | {
      readonly type: "test.finished"
      readonly runId: RunId
      readonly ts: number
      readonly testId: TestId
      readonly status: Exclude<TestStatus, "skipped">
      readonly durationMs: number
      readonly error?: {
        readonly name: string
        readonly message: string
      }
    }
  | {
      readonly type: "span.started"
      readonly runId: RunId
      readonly ts: number
      readonly testId: TestId
      readonly spanId: SpanId
      readonly parentSpanId?: SpanId
      readonly name: string
      readonly kind: "test" | "step" | "service"
    }
  | {
      readonly type: "span.finished"
      readonly runId: RunId
      readonly ts: number
      readonly testId: TestId
      readonly spanId: SpanId
      readonly status: Exclude<TestStatus, "skipped">
      readonly durationMs: number
      readonly error?: {
        readonly name: string
        readonly message: string
      }
    }
  | {
      readonly type: "artifact.created"
      readonly runId: RunId
      readonly ts: number
      readonly testId: TestId
      readonly kind: "screenshot" | "html" | "events"
      readonly path: string
    }
  | {
      readonly type: "server.started"
      readonly runId: RunId
      readonly ts: number
      readonly baseUrl: string
    }
  | {
      readonly type: "server.stopped"
      readonly runId: RunId
      readonly ts: number
    }
  | {
      readonly type: "log"
      readonly runId: RunId
      readonly ts: number
      readonly level: "debug" | "info" | "warn" | "error"
      readonly message: string
      readonly data?: unknown
    }

export type TestCase<R = never> = {
  readonly id: TestId
  readonly tags: ReadonlyArray<string>
  readonly timeoutMs?: number
  readonly steps: Effect.Effect<void, unknown, R>
}

export type BrowserLaunchOptions = {
  readonly headless: boolean
  readonly slowMoMs?: number
}
