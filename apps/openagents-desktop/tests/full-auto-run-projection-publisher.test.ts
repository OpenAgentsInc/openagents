// FA-RUN-05 (#8981): proves the publish hook fires on real registry lifecycle
// transitions (start/transition/rerun), never on illegal/no-op results, and
// never transmits anything beyond the public-safe projection fields --
// exercised against the REAL `openFullAutoRunRegistry` (not a fake), the same
// fixture discipline as `full-auto-run-registry.test.ts`.
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { describe, expect, test } from "vite-plus/test"

import { openFullAutoRunRegistry, type FullAutoRun } from "../src/full-auto-run-registry.ts"
import {
  makeFullAutoRunProjectionPublisher,
  toFullAutoRunClientProjection,
  wrapFullAutoRunRegistryWithProjectionPublish,
} from "../src/full-auto-run-projection-publisher.ts"

const withTempDir = <A>(prefix: string, fn: (root: string) => Promise<A>): Promise<A> => {
  const root = mkdtempSync(path.join(tmpdir(), prefix))
  return fn(root).finally(() => rmSync(root, { recursive: true, force: true }))
}

/** Waits a tick so a fire-and-forget `Effect.runFork` publish (kicked off
 * synchronously inside a wrapped registry call) has a chance to resolve
 * before the test asserts on the captured fetch calls. */
const flush = () => new Promise<void>(resolve => setTimeout(resolve, 0))

describe("toFullAutoRunClientProjection", () => {
  test("derives a short workspaceLabel from a local path, never the raw path", () => {
    const run: FullAutoRun = {
      runRef: "run.full-auto.abc.def",
      objective: "Ship the mobile projection.",
      objectiveSource: "user",
      doneCondition: "The endpoint round-trips.",
      objectiveHistory: [],
      workspaceRef: "/Users/private/code/openagents",
      turnCap: 20,
      successfulAttempts: 0,
      failedAttempts: 0,
      state: "running",
      stateRevision: 1,
      createdAt: "2026-07-17T21:00:00.000Z",
      startedAt: "2026-07-17T21:00:00.000Z",
      title: "Full Auto",
      transitions: [{
        from: "draft", to: "running", actor: "control_api", at: "2026-07-17T21:00:00.000Z", reason: "start",
      }],
    }
    const projection = toFullAutoRunClientProjection(run)
    expect(projection?.workspaceLabel).toBe("openagents")
    expect(JSON.stringify(projection)).not.toContain("/Users/private")
  })

  test("never includes raw prompt/tool-output-shaped fields (only the fixed public-safe field set)", () => {
    const run: FullAutoRun = {
      runRef: "run.full-auto.abc.def",
      objective: "Ship the mobile projection.",
      objectiveSource: "user",
      doneCondition: "The endpoint round-trips.",
      objectiveHistory: [],
      turnCap: 20,
      successfulAttempts: 0,
      failedAttempts: 0,
      state: "running",
      stateRevision: 1,
      createdAt: "2026-07-17T21:00:00.000Z",
      title: "Full Auto",
      transitions: [{
        from: "draft", to: "running", actor: "owner_ui", at: "2026-07-17T21:00:00.000Z", reason: "start",
      }],
    }
    const projection = toFullAutoRunClientProjection(run)
    expect(projection === null ? [] : Object.keys(projection).sort()).toEqual([
      "doneCondition", "lastTransition", "lifecycleState", "objective",
      "runRef", "startedAt", "threadRef", "updatedAt", "workspaceLabel",
    ])
  })

  test("returns null for a run with no recorded transition (defensive)", () => {
    const run: FullAutoRun = {
      runRef: "run.full-auto.abc.def",
      objective: "x",
      objectiveSource: "user",
      doneCondition: "y",
      objectiveHistory: [],
      turnCap: 20,
      successfulAttempts: 0,
      failedAttempts: 0,
      state: "draft",
      stateRevision: 0,
      createdAt: "2026-07-17T21:00:00.000Z",
      title: "Full Auto",
      transitions: [],
    }
    expect(toFullAutoRunClientProjection(run)).toBeNull()
  })
})

describe("wrapFullAutoRunRegistryWithProjectionPublish", () => {
  test("publishes on Start (draft -> running)", () => withTempDir("fa-run-pub-", async root => {
    const calls: Array<Readonly<{ method: string; body: unknown }>> = []
    const publisher = makeFullAutoRunProjectionPublisher({
      sessionReady: () => true,
      credential: () => ({ ownerUserId: "owner-a", accessToken: "token-a", refreshToken: "refresh-a" }),
      baseUrl: "https://openagents.com",
      fetchImpl: (async (_input, init) => {
        calls.push({ method: String(init?.method), body: JSON.parse(String(init?.body)) })
        return Response.json({
          ok: true,
          projection: { schema: "full_auto_run.mobile_projection.v1", privateMaterialExcluded: true, generatedAt: "2026-07-17T21:00:00.000Z", run: null },
        })
      }) as typeof fetch,
    })
    const raw = openFullAutoRunRegistry(path.join(root, "runs.json"), () => new Date("2026-07-17T21:00:00.000Z"))
    const wrapped = wrapFullAutoRunRegistryWithProjectionPublish(raw, publisher)

    const result = wrapped.startNew({
      title: "Full Auto", objective: "Ship the mobile projection.", doneCondition: "It round-trips.",
      objectiveSource: "user", actor: "owner_ui", reason: "owner started",
    })
    expect(result.ok).toBe(true)
    await flush()

    expect(calls).toHaveLength(1)
    expect(calls[0]?.method).toBe("POST")
    expect((calls[0]?.body as { run: { lifecycleState: string } }).run.lifecycleState).toBe("running")
  }))

  test("publishes on every subsequent lifecycle transition (Pause, Resume, Stop)", () => withTempDir("fa-run-pub-", async root => {
    const calls: Array<unknown> = []
    const publisher = makeFullAutoRunProjectionPublisher({
      sessionReady: () => true,
      credential: () => ({ ownerUserId: "owner-a", accessToken: "token-a", refreshToken: "refresh-a" }),
      baseUrl: "https://openagents.com",
      fetchImpl: (async (_input, init) => {
        calls.push(JSON.parse(String(init?.body)))
        return Response.json({
          ok: true,
          projection: { schema: "full_auto_run.mobile_projection.v1", privateMaterialExcluded: true, generatedAt: "2026-07-17T21:00:00.000Z", run: null },
        })
      }) as typeof fetch,
    })
    const raw = openFullAutoRunRegistry(path.join(root, "runs.json"), () => new Date("2026-07-17T21:00:00.000Z"))
    const wrapped = wrapFullAutoRunRegistryWithProjectionPublish(raw, publisher)

    const started = wrapped.startNew({
      title: "Full Auto", objective: "Ship the mobile projection.", doneCondition: "It round-trips.",
      objectiveSource: "user", actor: "owner_ui", reason: "owner started",
    })
    expect(started.ok).toBe(true)
    if (!started.ok) return
    await flush()

    wrapped.transition(started.run.runRef, { to: "paused", actor: "owner_ui", reason: "owner paused" })
    await flush()
    wrapped.transition(started.run.runRef, { to: "running", actor: "owner_ui", reason: "owner resumed" })
    await flush()
    wrapped.transition(started.run.runRef, { to: "stopped", actor: "owner_ui", reason: "owner stopped" })
    await flush()

    expect(calls).toHaveLength(4)
    const states = calls.map(body => (body as { run: { lifecycleState: string } }).run.lifecycleState)
    expect(states).toEqual(["running", "paused", "running", "stopped"])
  }))

  test("never publishes for an illegal transition (registry refuses before publish is reached)", () => withTempDir("fa-run-pub-", async root => {
    const calls: Array<unknown> = []
    const publisher = makeFullAutoRunProjectionPublisher({
      sessionReady: () => true,
      credential: () => ({ ownerUserId: "owner-a", accessToken: "token-a", refreshToken: "refresh-a" }),
      baseUrl: "https://openagents.com",
      fetchImpl: (async () => {
        calls.push(true)
        return Response.json({ ok: true, projection: {} })
      }) as typeof fetch,
    })
    const raw = openFullAutoRunRegistry(path.join(root, "runs.json"), () => new Date("2026-07-17T21:00:00.000Z"))
    const wrapped = wrapFullAutoRunRegistryWithProjectionPublish(raw, publisher)

    const started = wrapped.startNew({
      title: "Full Auto", objective: "Ship the mobile projection.", doneCondition: "It round-trips.",
      objectiveSource: "user", actor: "owner_ui", reason: "owner started",
    })
    expect(started.ok).toBe(true)
    if (!started.ok) return
    await flush()
    calls.length = 0

    // Resume is only legal from Paused (FA-AC-44) -- illegal from Running.
    const illegal = wrapped.transition(started.run.runRef, { to: "running", actor: "owner_ui", reason: "bogus resume" })
    expect(illegal.ok).toBe(false)
    await flush()
    expect(calls).toHaveLength(0)
  }))

  test("does not publish while signed out or the session is not ready", () => withTempDir("fa-run-pub-", async root => {
    const calls: Array<unknown> = []
    const publisher = makeFullAutoRunProjectionPublisher({
      sessionReady: () => false,
      credential: () => null,
      baseUrl: "https://openagents.com",
      fetchImpl: (async () => {
        calls.push(true)
        return Response.json({ ok: true, projection: {} })
      }) as typeof fetch,
    })
    const raw = openFullAutoRunRegistry(path.join(root, "runs.json"), () => new Date("2026-07-17T21:00:00.000Z"))
    const wrapped = wrapFullAutoRunRegistryWithProjectionPublish(raw, publisher)

    wrapped.startNew({
      title: "Full Auto", objective: "Ship the mobile projection.", doneCondition: "It round-trips.",
      objectiveSource: "user", actor: "owner_ui", reason: "owner started",
    })
    await flush()
    expect(calls).toHaveLength(0)
  }))

  test("a publish network failure never throws out of the wrapped registry call", () => withTempDir("fa-run-pub-", async root => {
    const publisher = makeFullAutoRunProjectionPublisher({
      sessionReady: () => true,
      credential: () => ({ ownerUserId: "owner-a", accessToken: "token-a", refreshToken: "refresh-a" }),
      baseUrl: "https://openagents.com",
      fetchImpl: (async () => {
        throw new Error("network is down")
      }) as typeof fetch,
    })
    const raw = openFullAutoRunRegistry(path.join(root, "runs.json"), () => new Date("2026-07-17T21:00:00.000Z"))
    const wrapped = wrapFullAutoRunRegistryWithProjectionPublish(raw, publisher)

    expect(() => wrapped.startNew({
      title: "Full Auto", objective: "Ship the mobile projection.", doneCondition: "It round-trips.",
      objectiveSource: "user", actor: "owner_ui", reason: "owner started",
    })).not.toThrow()
    await flush()
  }))
})
