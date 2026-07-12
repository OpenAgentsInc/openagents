import { describe, expect, test } from "bun:test"
import { EventEmitter } from "node:events"
import type { Worker } from "node:worker_threads"

import { makeWorkspaceSearchHost } from "./workspace-search-host.ts"

class FixtureWorker extends EventEmitter {
  terminated = 0

  terminate(): Promise<number> {
    this.terminated += 1
    return Promise.resolve(0)
  }
}

const availablePage = (query: string) => ({
  state: "available" as const,
  grantRef: "workspace.grant.fixture",
  query,
  mode: "content" as const,
  matches: [],
  nextOffset: null,
  truncated: false,
  cache: { key: `workspace.search.fixture.${query}`, epoch: 0, freshness: "current" as const },
})

describe("Workspace search host", () => {
  test("correlates isolated workers and settles cancellation, disposal, and late events exactly once", async () => {
    const workers: FixtureWorker[] = []
    const inputs: unknown[] = []
    const host = makeWorkspaceSearchHost(
      "/private/root",
      "workspace.grant.fixture",
      new URL("file:///fixture/workspace-search-worker.js"),
      (_url, input) => {
        const worker = new FixtureWorker()
        workers.push(worker)
        inputs.push(input)
        return worker as unknown as Worker
      },
    )

    const completed = host.start({ query: "ready", mode: "content", epoch: 0 })
    expect(host.activeCount()).toBe(1)
    expect(inputs[0]).toEqual({
      root: "/private/root",
      grantRef: "workspace.grant.fixture",
      request: { query: "ready", mode: "content", epoch: 0 },
    })
    workers[0]!.emit("message", { ok: true, result: availablePage("ready") })
    expect(await completed.result).toEqual(availablePage("ready"))
    workers[0]!.emit("exit", 0)
    expect(host.activeCount()).toBe(0)
    expect(workers[0]!.terminated).toBe(0)

    const cancelled = host.start({ query: "cancel", mode: "content", epoch: 0 })
    cancelled.cancel()
    cancelled.cancel()
    workers[1]!.emit("message", { ok: true, result: availablePage("late") })
    expect(await cancelled.result).toEqual({ state: "unavailable", message: "Workspace search was cancelled." })
    expect(workers[1]!.terminated).toBe(1)

    const disposedA = host.start({ query: "a", mode: "path", epoch: 1 })
    const disposedB = host.start({ query: "b", mode: "path", epoch: 1 })
    expect(host.activeCount()).toBe(2)
    host.dispose()
    host.dispose()
    expect(await disposedA.result).toEqual({ state: "unavailable", message: "The selected workspace has been disposed." })
    expect(await disposedB.result).toEqual({ state: "unavailable", message: "The selected workspace has been disposed." })
    expect(workers[2]!.terminated).toBe(1)
    expect(workers[3]!.terminated).toBe(1)
    expect(host.activeCount()).toBe(0)

    const afterDispose = host.start({ query: "never", mode: "path", epoch: 2 })
    expect(await afterDispose.result).toEqual({ state: "unavailable", message: "The selected workspace has been disposed." })
    expect(workers).toHaveLength(4)
  })

  test("fails closed on malformed results and worker failures", async () => {
    const workers: FixtureWorker[] = []
    const host = makeWorkspaceSearchHost(
      "/private/root",
      "workspace.grant.fixture",
      new URL("file:///fixture/workspace-search-worker.js"),
      () => {
        const worker = new FixtureWorker()
        workers.push(worker)
        return worker as unknown as Worker
      },
    )
    const malformed = host.start({ query: "bad", mode: "content", epoch: 0 })
    workers[0]!.emit("message", { ok: true, result: { state: "available", root: "/private/root" } })
    expect((await malformed.result).state).toBe("unavailable")

    const failed = host.start({ query: "error", mode: "content", epoch: 0 })
    workers[1]!.emit("error", new Error("private detail"))
    expect(await failed.result).toEqual({ state: "unavailable", message: "Workspace search stopped unexpectedly." })
    expect(host.activeCount()).toBe(0)
    host.dispose()
  })
})
