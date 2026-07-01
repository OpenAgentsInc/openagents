import { describe, expect, test } from "bun:test"

import {
  THREAD_SWITCH_BENCHMARK_HARNESS,
} from "../scripts/thread-switch-benchmark"

describe("thread switch benchmark harness", () => {
  test("names the repeatable Khala Code chat-switch performance benchmark", () => {
    expect(THREAD_SWITCH_BENCHMARK_HARNESS).toBe(
      "khala_code_thread_switch_performance_v1",
    )
  })
})
