import { describe, expect, test } from "vite-plus/test"
import { EventEmitter } from "node:events"
import type { Worker } from "node:worker_threads"

import { makeWorkspaceSearchHost } from "./workspace-search-host.ts"
import type {
  IdePortableMutationAuthority,
  IdePortableMutationPermit,
} from "./ide/portable-mutation-authority.ts"

class FixtureWorker extends EventEmitter {
  terminated = 0

  constructor(private readonly terminateResult: Promise<number> = Promise.resolve(0)) {
    super()
  }

  terminate(): Promise<number> {
    this.terminated += 1
    return this.terminateResult
  }
}

const permit = (generation: number): IdePortableMutationPermit => ({
  _tag: "Portable",
  key: `portable:workspace.grant.fixture:session.fixture:work.fixture:attachment.${generation}:${generation}:target.fixture`,
  grantRef: "workspace.grant.fixture",
  sessionRef: "session.fixture",
  workContextRef: "work.fixture",
  attachmentRef: `attachment.${generation}`,
  generation,
  targetRef: "target.fixture",
})

const portableAuthority = (current: () => IdePortableMutationPermit | null): IdePortableMutationAuthority => ({
  authorize: grantRef => {
    const active = current()
    return active !== null && active.grantRef === grantRef
      ? { _tag: "Permitted", permit: active }
      : { _tag: "Refused", reason: "admission_unavailable" }
  },
  reauthorize: candidate => current()?.key === candidate.key,
})

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
    expect(workers[0]!.terminated).toBe(1)

    const cancelled = host.start({ query: "cancel", mode: "content", epoch: 0 })
    cancelled.cancel()
    cancelled.cancel()
    workers[1]!.emit("message", { ok: true, result: availablePage("late") })
    expect(await cancelled.result).toEqual({ state: "unavailable", message: "Workspace search was cancelled." })
    expect(workers[1]!.terminated).toBe(1)

    const disposedA = host.start({ query: "a", mode: "path", epoch: 1 })
    const disposedB = host.start({ query: "b", mode: "path", epoch: 1 })
    expect(host.activeCount()).toBe(2)
    await host.dispose()
    await host.dispose()
    expect(await disposedA.result).toEqual({ state: "unavailable", message: "Workspace search was quiesced." })
    expect(await disposedB.result).toEqual({ state: "unavailable", message: "Workspace search was quiesced." })
    expect(workers[2]!.terminated).toBe(1)
    expect(workers[3]!.terminated).toBe(1)
    expect(host.activeCount()).toBe(0)

    const afterDispose = host.start({ query: "never", mode: "path", epoch: 2 })
    expect(await afterDispose.result).toEqual({ state: "unavailable", message: "Workspace search is quiesced on this host." })
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
    await host.dispose()
  })

  test("revokes a blocked worker and suppresses late results across attachment generations", async () => {
    const workers: FixtureWorker[] = []
    const monitors: Array<Readonly<{ check: () => void; closed: () => boolean }>> = []
    let current: IdePortableMutationPermit | null = permit(1)
    const host = makeWorkspaceSearchHost(
      "/private/root",
      "workspace.grant.fixture",
      new URL("file:///fixture/workspace-search-worker.js"),
      () => {
        const worker = new FixtureWorker()
        workers.push(worker)
        return worker as unknown as Worker
      },
      {
        mutationAuthority: portableAuthority(() => current),
        monitorAuthority: check => {
          let closed = false
          monitors.push({ check, closed: () => closed })
          return { close: () => { closed = true } }
        },
      },
    )

    const blocked = host.start({ query: "blocked", mode: "content", epoch: 0 })
    current = permit(2)
    monitors[0]!.check()
    expect(await blocked.result).toEqual({ state: "unavailable", message: "The workspace search authority was revoked." })
    expect(workers[0]!.terminated).toBe(1)
    expect(monitors[0]!.closed()).toBe(true)
    workers[0]!.emit("message", { ok: true, result: availablePage("late-generation-1") })

    const replacement = host.start({ query: "generation-2", mode: "content", epoch: 0 })
    workers[1]!.emit("message", { ok: true, result: availablePage("generation-2") })
    expect(await replacement.result).toEqual(availablePage("generation-2"))

    const late = host.start({ query: "late", mode: "content", epoch: 0 })
    current = null
    workers[2]!.emit("message", { ok: true, result: availablePage("must-not-leak") })
    expect(await late.result).toEqual({
      state: "unavailable",
      message: "The workspace search authority was revoked before its result was admitted.",
    })
    expect(workers[2]!.terminated).toBe(1)
    await host.dispose()
  })

  test("quiesce is permanent, idempotent, and waits for the worker termination safe point", async () => {
    let finishTermination!: (code: number) => void
    const termination = new Promise<number>(resolve => { finishTermination = resolve })
    const worker = new FixtureWorker(termination)
    const host = makeWorkspaceSearchHost(
      "/private/root",
      "workspace.grant.fixture",
      new URL("file:///fixture/workspace-search-worker.js"),
      () => worker as unknown as Worker,
      { quiesceTimeoutMs: 1_000 },
    )
    const blocked = host.start({ query: "blocked", mode: "content", epoch: 0 })
    const first = host.quiesce()
    const second = host.dispose()
    expect(second).toBe(first)
    expect(await blocked.result).toEqual({ state: "unavailable", message: "Workspace search was quiesced." })
    expect(worker.terminated).toBe(1)
    let settled = false
    void first.then(() => { settled = true })
    await Promise.resolve()
    expect(settled).toBe(false)
    finishTermination(0)
    await expect(first).resolves.toEqual({ state: "quiesced" })
    expect(await host.start({ query: "after", mode: "path", epoch: 0 }).result).toEqual({
      state: "unavailable",
      message: "Workspace search is quiesced on this host.",
    })
  })
})
