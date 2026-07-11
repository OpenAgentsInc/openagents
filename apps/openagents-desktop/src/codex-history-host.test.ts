import { describe, expect, test } from "bun:test"
import { EventEmitter } from "node:events"
import type { Worker } from "node:worker_threads"

import { makeCodexHistoryHost } from "./codex-history-host.ts"

class FixtureWorker extends EventEmitter {
  posted: unknown[] = []
  terminated = false

  postMessage(value: unknown): void {
    this.posted.push(value)
  }

  terminate(): Promise<number> {
    this.terminated = true
    return Promise.resolve(0)
  }
}

describe("Codex history process host", () => {
  test("reuses one worker, correlates responses, and settles pending reads on disposal", async () => {
    const worker = new FixtureWorker()
    let opens = 0
    const host = makeCodexHistoryHost(
      new URL("file:///fixture/codex-history-worker.js"),
      () => {
        opens++
        return worker as unknown as Worker
      },
    )

    const first = host.run({ kind: "history_catalog", sessionsRoot: "/fixture" })
    expect(worker.posted).toEqual([{
      id: 1,
      request: { kind: "history_catalog", sessionsRoot: "/fixture" },
    }])
    worker.emit("message", { id: 1, ok: true, result: { roots: [] } })
    expect(await first).toEqual({ roots: [] })

    const pending = host.run({ kind: "history_page", sessionsRoot: "/fixture", threadRef: "thread", offset: 0, limit: 50 })
    host.dispose()
    host.dispose()
    expect(await pending).toBeNull()
    expect(opens).toBe(1)
    expect(worker.terminated).toBe(true)
    expect(await host.run({ kind: "history_catalog", sessionsRoot: "/fixture" })).toBeNull()
    expect(opens).toBe(1)
  })
})
