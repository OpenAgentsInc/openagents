import { describe, expect, test } from "vite-plus/test"

import { makeCodexHistoryHost, type CodexHistoryProcess } from "./codex-history-host.ts"

class FixtureProcess implements CodexHistoryProcess {
  posted: unknown[] = []
  terminated = false
  messageListener: (value: unknown) => void = () => {}
  exitListener: () => void = () => {}

  postMessage(value: unknown): void {
    this.posted.push(value)
  }

  onMessage(listener: (value: unknown) => void): void {
    this.messageListener = listener
  }

  onExit(listener: () => void): void {
    this.exitListener = listener
  }

  terminate(): void {
    this.terminated = true
  }
}

describe("Codex history process host", () => {
  test("reuses one worker, correlates responses, and settles pending reads on disposal", async () => {
    const worker = new FixtureProcess()
    let opens = 0
    const host = makeCodexHistoryHost(() => {
      opens++
      return worker
    })

    const first = host.run({ kind: "history_catalog", sessionsRoot: "/fixture" })
    expect(worker.posted).toEqual([{
      id: 1,
      request: { kind: "history_catalog", sessionsRoot: "/fixture" },
    }])
    worker.messageListener({ id: 1, ok: true, result: { roots: [] } })
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
