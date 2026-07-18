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
  toFullAutoRunClientReceiptSummary,
  wrapFullAutoRunRegistryWithProjectionPublish,
} from "../src/full-auto-run-projection-publisher.ts"
import type { FullAutoRunReceipt } from "../src/full-auto-run-report.ts"

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
      "accountRef", "doneCondition", "failedAttempts", "laneRef", "lastTransition",
      "lifecycleState", "objective", "receiptSummary", "rotationCount", "runRef",
      "startedAt", "successfulAttempts", "threadRef", "turnCap", "updatedAt", "workspaceLabel",
    ])
  })

  test("MOB-FA-02 (#8994): carries lane/account/cap/attempts from run.profile, defaulting rotationCount to 0 and receiptSummary to null", () => {
    const run: FullAutoRun = {
      runRef: "run.full-auto.abc.def",
      objective: "Ship the mobile projection.",
      objectiveSource: "user",
      doneCondition: "The endpoint round-trips.",
      objectiveHistory: [],
      profile: { lane: "codex-local", accountRef: "codex-2" },
      turnCap: 20,
      successfulAttempts: 7,
      failedAttempts: 1,
      state: "running",
      stateRevision: 1,
      createdAt: "2026-07-17T21:00:00.000Z",
      title: "Full Auto",
      transitions: [{
        from: "draft", to: "running", actor: "control_api", at: "2026-07-17T21:00:00.000Z", reason: "start",
      }],
    }
    const projection = toFullAutoRunClientProjection(run)
    expect(projection).toMatchObject({
      laneRef: "codex-local", accountRef: "codex-2",
      turnCap: 20, successfulAttempts: 7, failedAttempts: 1,
      rotationCount: 0, receiptSummary: null,
    })
  })

  test("MOB-FA-02 (#8994): a non-terminal run never surfaces a receiptSummary even if `extra` supplies one", () => {
    const run: FullAutoRun = {
      runRef: "run.full-auto.abc.def",
      objective: "x",
      objectiveSource: "user",
      doneCondition: "y",
      objectiveHistory: [],
      turnCap: 20,
      successfulAttempts: 0,
      failedAttempts: 0,
      state: "running",
      stateRevision: 1,
      createdAt: "2026-07-17T21:00:00.000Z",
      title: "Full Auto",
      transitions: [{
        from: "draft", to: "running", actor: "control_api", at: "2026-07-17T21:00:00.000Z", reason: "start",
      }],
    }
    const projection = toFullAutoRunClientProjection(run, {
      rotationCount: 3,
      receiptSummary: {
        schema: "full_auto_run.mobile_receipt.v1", runRef: run.runRef, objectiveDigest: "a".repeat(64),
        doneConditionDigest: "a".repeat(64), workspaceRefDigest: null, state: "completed", turnCap: 20,
        successfulAttempts: 0, failedAttempts: 0, providerIdentities: [], providerTransitionCount: 0,
        providerTransitionDispositions: [], livenessGapCount: 0, recoveryActionsUsed: [], verifiedRefCount: 0,
        claimedRefCount: 0, progressDisposition: "unknown", usageKnown: false, reportRevision: 1,
        createdAt: "2026-07-17T21:00:00.000Z",
      },
    })
    expect(projection?.rotationCount).toBe(3)
    expect(projection?.receiptSummary).toBeNull()
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

describe("toFullAutoRunClientReceiptSummary", () => {
  test("maps every FullAutoRunReceipt field to the client schema's mobile_receipt.v1 literal", () => {
    const receipt: FullAutoRunReceipt = {
      schema: "openagents.desktop.full_auto_run_receipt.v1",
      runRef: "run.full-auto.abc.def",
      threadRef: "thread.abc",
      objectiveDigest: "a".repeat(64),
      doneConditionDigest: "b".repeat(64),
      workspaceRefDigest: "c".repeat(64),
      state: "completed",
      startedAt: "2026-07-17T21:00:00.000Z",
      endedAt: "2026-07-17T22:00:00.000Z",
      turnCap: 20,
      successfulAttempts: 7,
      failedAttempts: 1,
      providerIdentities: ["codex-local"],
      providerTransitionCount: 1,
      providerTransitionDispositions: ["complete_within_bounds"],
      livenessGapCount: 0,
      recoveryActionsUsed: ["retry_now"],
      verifiedRefCount: 2,
      claimedRefCount: 1,
      progressDisposition: "unknown",
      usageKnown: false,
      reportRevision: 3,
      createdAt: "2026-07-17T22:00:00.000Z",
    }
    const summary = toFullAutoRunClientReceiptSummary(receipt)
    expect(summary.schema).toBe("full_auto_run.mobile_receipt.v1")
    expect(summary).toMatchObject({
      runRef: receipt.runRef,
      state: "completed",
      successfulAttempts: 7,
      failedAttempts: 1,
      providerIdentities: ["codex-local"],
      recoveryActionsUsed: ["retry_now"],
      reportRevision: 3,
    })
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
