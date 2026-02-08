import { Effect } from "effect"

import type { ProbeService } from "../effect/ProbeService.ts"
import type { TestContext } from "../runner/TestContext.ts"

export type EvalArg = string | (() => unknown)

export type WaitOptions = {
  readonly timeoutMs?: number
  readonly intervalMs?: number
}

export type Viewport = {
  readonly width: number
  readonly height: number
  readonly deviceScaleFactor?: number
}

export type Page = {
  readonly addInitScript: (source: string) => Effect.Effect<void, unknown, ProbeService | TestContext>
  readonly setViewport: (viewport: Viewport) => Effect.Effect<void, unknown, ProbeService | TestContext>
  readonly goto: (url: string) => Effect.Effect<void, unknown, ProbeService | TestContext>
  readonly click: (selector: string) => Effect.Effect<void, unknown, ProbeService | TestContext>
  readonly fill: (selector: string, value: string) => Effect.Effect<void, unknown, ProbeService | TestContext>
  readonly type: (selector: string, text: string) => Effect.Effect<void, unknown, ProbeService | TestContext>
  readonly evaluate: <A = unknown>(fnOrExpression: EvalArg) => Effect.Effect<A, unknown, ProbeService | TestContext>
  readonly waitForFunction: (
    fnOrExpression: EvalArg,
    options?: WaitOptions,
  ) => Effect.Effect<void, unknown, ProbeService | TestContext>
  readonly htmlSnapshot: () => Effect.Effect<string, unknown, ProbeService | TestContext>
  readonly screenshot: (filePath: string) => Effect.Effect<void, unknown, ProbeService | TestContext>
  readonly close: Effect.Effect<void, never, ProbeService | TestContext>
}
