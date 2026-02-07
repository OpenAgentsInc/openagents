import { Context } from "effect"

import type { RunId, TestId } from "../spec.ts"

export type TestContextData = {
  readonly runId: RunId
  readonly testId: TestId
  readonly baseUrl: string
  readonly artifactsDir: string
}

export class TestContext extends Context.Tag("@openagentsinc/effuse-test/TestContext")<
  TestContext,
  TestContextData
>() {}

