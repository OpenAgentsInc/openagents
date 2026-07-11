import { describe, expect, test } from "bun:test"

import {
  decodeDesktopOperationContext,
  makeDesktopCorrelationJournal,
  makeDesktopOperationContext,
} from "./desktop-operation-context.ts"

describe("Desktop operation correlation", () => {
  test("preserves exact public-safe refs through the bounded journal", () => {
    const events: unknown[] = []
    const journal = makeDesktopCorrelationJournal(event => events.push(event))
    const context = makeDesktopOperationContext({
      operationRef: "operation.desktop.start.1",
      sessionRef: "session.desktop.1",
      correlationRef: "correlation.desktop.1",
      runRef: "run.desktop.1",
    })
    for (const stage of ["ipc.received", "gateway.received", "sync.intent", "ipc.returned"] as const) {
      journal.record(stage, context)
    }
    expect(journal.complete(context.correlationRef)).toBe(true)
    expect(events).toHaveLength(4)
    expect(events.every(event => JSON.stringify(event).includes("run.desktop.1"))).toBe(true)
    expect(JSON.stringify(events)).not.toMatch(/token|password|authorization|\/Users\//iu)
    journal.dispose()
    expect(journal.stages(context.correlationRef)).toEqual([])
  })

  test("rejects path, URL, secret-shaped, and oversized refs", () => {
    for (const operationRef of [
      "/Users/private/work",
      "https://example.test/run",
      "token=secret-value",
      `operation.${"x".repeat(300)}`,
    ]) {
      expect(decodeDesktopOperationContext({
        operationRef,
        sessionRef: "session.desktop.1",
        correlationRef: "correlation.desktop.1",
      })).toBeNull()
    }
  })
})
