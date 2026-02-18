import { Effect } from "effect"

import { EffuseTestConfig } from "../config/EffuseTestConfig.ts"
import type { TestCase } from "../spec.ts"

/**
 * E2E suite for apps/openagents.com (Laravel web app).
 * Add tests here as needed; runner supports --project ../../apps/openagents.com.
 */
export const openagentsComSuite = (): Effect.Effect<
  ReadonlyArray<TestCase<unknown>>,
  never,
  EffuseTestConfig
> =>
  Effect.gen(function* () {
    yield* EffuseTestConfig
    return [] as ReadonlyArray<TestCase<unknown>>
  })
