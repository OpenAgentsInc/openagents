import { describe, expect, test } from "bun:test"

import {
  buildThreadSwitchBenchmarkSessionCatalog,
  THREAD_SWITCH_BENCHMARK_HARNESS,
} from "../scripts/thread-switch-benchmark"
import { sessionCatalogEntryToThreadSummary } from "../src/shared/session-catalog"

describe("thread switch benchmark harness", () => {
  test("names the repeatable Khala Code chat-switch performance benchmark", () => {
    expect(THREAD_SWITCH_BENCHMARK_HARNESS).toBe(
      "khala_code_thread_switch_performance_v1",
    )
  })

  test("mocks the current sessionCatalog thread-sidebar contract", () => {
    const catalog = buildThreadSwitchBenchmarkSessionCatalog() as {
      entries: Array<Parameters<typeof sessionCatalogEntryToThreadSummary>[0]>
      ok: true
      schemaVersion: string
    }
    const summaries = catalog.entries.map(sessionCatalogEntryToThreadSummary)

    expect(catalog.schemaVersion).toBe("khala-code-desktop.session-catalog.v1")
    expect(catalog.entries[0]).toMatchObject({
      harnessKind: "codex",
      sessionRef: "thread-a",
      threadRef: "thread-a",
      title: "Alpha benchmark",
    })
    expect(summaries.map(summary => summary.id)).toEqual([
      "thread-a",
      "thread-b",
      "thread-c",
      "thread-d",
      "thread-e",
    ])
  })
})
