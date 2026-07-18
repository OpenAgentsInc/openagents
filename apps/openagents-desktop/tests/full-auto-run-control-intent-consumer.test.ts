// MOB-FA-02 (#8994): proves the Desktop-side control-intent consumer
// actually applies a mobile-dispatched Pause/Resume/Stop through the SAME
// `full-auto-run-actions.ts` functions the loopback control API and owner
// UI use (never a bypass), attributes the transition to `actor: "mobile"`
// with `disabledBy: "mobile"` on the thread record, and reports a typed
// outcome for every intent it sees -- applied, rejected, or (only when the
// server itself is unreachable) deferred to the next tick, but never a
// silent drop of an intent it actually fetched.
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { describe, expect, test } from "vite-plus/test"
import { Effect } from "effect"

import type { FullAutoRunControlIntent } from "@openagentsinc/khala-sync"

import type { FullAutoControlCapabilities } from "../src/full-auto-control-server.ts"
import type { FullAutoRunActionContext } from "../src/full-auto-run-actions.ts"
import { FULL_AUTO_OWNER_UI_CALLER_LABEL, startFullAutoRunAction } from "../src/full-auto-run-actions.ts"
import {
  applyFullAutoRunControlIntent,
  makeFullAutoRunControlIntentConsumer,
} from "../src/full-auto-run-control-intent-consumer.ts"
import { openFullAutoRegistry } from "../src/full-auto-registry.ts"
import { openFullAutoRunRegistry, type FullAutoRunRegistry } from "../src/full-auto-run-registry.ts"
import { openFullAutoRunReportStore } from "../src/full-auto-run-report.ts"

const GRANTED_WORKSPACE = "/granted/full-auto/workspace"
const TIMESTAMP = "2026-07-18T02:00:00.000Z"

const withTempDir = <A>(prefix: string, fn: (root: string) => Promise<A>): Promise<A> => {
  const root = mkdtempSync(path.join(tmpdir(), prefix))
  return fn(root).finally(() => rmSync(root, { recursive: true, force: true }))
}

const makeCapabilities = (root: string): Readonly<{
  capabilities: FullAutoControlCapabilities
  runRegistry: FullAutoRunRegistry
  registry: ReturnType<typeof openFullAutoRegistry>
}> => {
  const registry = openFullAutoRegistry(path.join(root, "registry.json"), () => new Date(TIMESTAMP))
  const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"), () => new Date(TIMESTAMP))
  const reportStore = openFullAutoRunReportStore(path.join(root, "reports.json"))
  const capabilities: FullAutoControlCapabilities = {
    registry,
    runRegistry,
    reportStore,
    interruptLiveTurn: () => false,
    resolveWorkspaceRef: () => GRANTED_WORKSPACE,
    triggerReconciliation: async () => undefined,
    liveState: () => null,
    listTurns: () => [],
    appendSystemNote: () => undefined,
    createThread: title => `thread.${title ?? "full-auto"}`,
    listLanes: async () => [],
    isLaneEligible: laneRef => laneRef === "codex-local",
  }
  return { capabilities, runRegistry, registry }
}

const startedRun = (capabilities: FullAutoControlCapabilities) => {
  const ctx: FullAutoRunActionContext = {
    capabilities,
    now: () => new Date(TIMESTAMP),
    actor: "owner_ui",
    callerLabel: FULL_AUTO_OWNER_UI_CALLER_LABEL,
  }
  const outcome = startFullAutoRunAction(ctx, {
    title: "Full Auto",
    objective: "Ship the mobile control intent consumer.",
    doneCondition: "Pause/Resume/Stop from the phone actually transitions the run.",
    workspaceRef: GRANTED_WORKSPACE,
    lane: "codex-local",
  })
  if (!outcome.ok) throw new Error("failed to start a fixture run")
  return outcome.value
}

const mobileContext = (capabilities: FullAutoControlCapabilities): FullAutoRunActionContext => ({
  capabilities,
  now: () => new Date(TIMESTAMP),
  actor: "mobile",
  callerLabel: "a mobile Pause/Resume/Stop request",
})

const pendingIntent = (input: Readonly<{ intentId: string; runRef: string; action: "pause" | "resume" | "stop" }>): FullAutoRunControlIntent => ({
  schema: "full_auto_run.control_intent.v1",
  intentId: input.intentId,
  idempotencyKey: `idem.${input.intentId}`,
  runRef: input.runRef,
  action: input.action,
  surface: "mobile",
  createdAt: TIMESTAMP,
  status: "pending",
  appliedAt: null,
  rejectionReason: null,
  resultLifecycleState: null,
})

describe("applyFullAutoRunControlIntent", () => {
  test("pause: transitions the run, attributes actor:mobile, and disables the thread with disabledBy:mobile", () =>
    withTempDir("fa-control-consumer-", async root => {
      const { capabilities, registry } = makeCapabilities(root)
      const run = startedRun(capabilities)
      const report = applyFullAutoRunControlIntent(
        mobileContext(capabilities),
        pendingIntent({ intentId: "intent.mobile.1", runRef: run.runRef, action: "pause" }),
      )
      expect(report).toMatchObject({ intentId: "intent.mobile.1", status: "applied", resultLifecycleState: "paused" })
      const updatedRun = capabilities.runRegistry.get(run.runRef)
      expect(updatedRun?.transitions.at(-1)).toMatchObject({ to: "paused", actor: "mobile" })
      expect(registry.record(run.threadRef!)?.disabledBy).toBe("mobile")
    }))

  test("resume: transitions a paused run back to running, attributed to mobile", () =>
    withTempDir("fa-control-consumer-", async root => {
      const { capabilities } = makeCapabilities(root)
      const run = startedRun(capabilities)
      applyFullAutoRunControlIntent(
        mobileContext(capabilities),
        pendingIntent({ intentId: "intent.mobile.1", runRef: run.runRef, action: "pause" }),
      )
      const report = applyFullAutoRunControlIntent(
        mobileContext(capabilities),
        pendingIntent({ intentId: "intent.mobile.2", runRef: run.runRef, action: "resume" }),
      )
      expect(report).toMatchObject({ status: "applied", resultLifecycleState: "running" })
    }))

  test("stop: transitions the run to stopped, attributed to mobile", () =>
    withTempDir("fa-control-consumer-", async root => {
      const { capabilities } = makeCapabilities(root)
      const run = startedRun(capabilities)
      const report = applyFullAutoRunControlIntent(
        mobileContext(capabilities),
        pendingIntent({ intentId: "intent.mobile.1", runRef: run.runRef, action: "stop" }),
      )
      expect(report).toMatchObject({ status: "applied", resultLifecycleState: "stopped" })
    }))

  test("an intent for an unknown runRef is rejected as run_not_found, never a silent no-op", () =>
    withTempDir("fa-control-consumer-", async root => {
      const { capabilities } = makeCapabilities(root)
      const report = applyFullAutoRunControlIntent(
        mobileContext(capabilities),
        pendingIntent({ intentId: "intent.mobile.1", runRef: "run.full-auto.ghost", action: "pause" }),
      )
      expect(report).toEqual({ intentId: "intent.mobile.1", status: "rejected", rejectionReason: "run_not_found" })
    }))

  test("resume on a non-paused run is rejected as illegal_transition, never silently applied", () =>
    withTempDir("fa-control-consumer-", async root => {
      const { capabilities } = makeCapabilities(root)
      const run = startedRun(capabilities)
      const report = applyFullAutoRunControlIntent(
        mobileContext(capabilities),
        pendingIntent({ intentId: "intent.mobile.1", runRef: run.runRef, action: "resume" }),
      )
      expect(report).toEqual({ intentId: "intent.mobile.1", status: "rejected", rejectionReason: "illegal_transition" })
      // The run must stay exactly where it was -- a refused mobile intent
      // never mutates state.
      expect(capabilities.runRegistry.get(run.runRef)?.state).toBe("running")
    }))
})

describe("makeFullAutoRunControlIntentConsumer.tick", () => {
  test("pulls pending intents, applies them, and reports the outcome back over the same route", () =>
    withTempDir("fa-control-consumer-", async root => {
      const { capabilities } = makeCapabilities(root)
      const run = startedRun(capabilities)
      const reportedOutcomes: Array<unknown> = []
      const consumer = makeFullAutoRunControlIntentConsumer({
        sessionReady: () => true,
        credential: () => ({ ownerUserId: "owner-a", accessToken: "token-a", refreshToken: "refresh-a" }),
        baseUrl: "https://openagents.com",
        actionContext: () => mobileContext(capabilities),
        fetchImpl: (async (input, init) => {
          const url = String(input)
          expect(url).toBe("https://openagents.com/api/full-auto-runs/control-intents")
          if (init?.method === "GET" || init?.method === undefined) {
            return Response.json({
              ok: true,
              intents: [pendingIntent({ intentId: "intent.mobile.1", runRef: run.runRef, action: "pause" })],
            })
          }
          const body = JSON.parse(String(init?.body))
          reportedOutcomes.push(body.outcome)
          return Response.json({ ok: true, intent: { ...pendingIntent({ intentId: "intent.mobile.1", runRef: run.runRef, action: "pause" }), status: "applied" } })
        }) as typeof fetch,
      })
      await Effect.runPromise(consumer.tick())
      expect(reportedOutcomes).toEqual([
        { intentId: "intent.mobile.1", status: "applied", resultLifecycleState: "paused" },
      ])
      expect(capabilities.runRegistry.get(run.runRef)?.state).toBe("paused")
    }))

  test("does not fetch while signed out or the session is not ready", () =>
    withTempDir("fa-control-consumer-", async root => {
      const { capabilities } = makeCapabilities(root)
      const calls: Array<unknown> = []
      const consumer = makeFullAutoRunControlIntentConsumer({
        sessionReady: () => false,
        credential: () => null,
        baseUrl: "https://openagents.com",
        actionContext: () => mobileContext(capabilities),
        fetchImpl: (async () => {
          calls.push(true)
          return Response.json({ ok: true, intents: [] })
        }) as typeof fetch,
      })
      await Effect.runPromise(consumer.tick())
      expect(calls).toHaveLength(0)
    }))

  test("a network failure during tick never throws out of the caller", () =>
    withTempDir("fa-control-consumer-", async root => {
      const { capabilities } = makeCapabilities(root)
      const consumer = makeFullAutoRunControlIntentConsumer({
        sessionReady: () => true,
        credential: () => ({ ownerUserId: "owner-a", accessToken: "token-a", refreshToken: "refresh-a" }),
        baseUrl: "https://openagents.com",
        actionContext: () => mobileContext(capabilities),
        fetchImpl: (async () => {
          throw new Error("network is down")
        }) as typeof fetch,
      })
      await expect(Effect.runPromise(consumer.tick())).resolves.toBeUndefined()
    }))
})
