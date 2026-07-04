import { describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"
import { mkdtemp } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { dispatchEligibility } from "./coordinator.js"
import {
  applyFleetIntentToStore,
  enforcePendingFleetIntents,
  type ReadPendingFleetIntentsLike,
} from "./fleet-intent-enforcement.js"
import type { FleetIntentRow } from "./fleet-intents.js"
import { createPylonOrchestrationStore, type PylonOrchestrationStore } from "./store.js"

const RUN_REF = "fleet-run.pylon.supervisor.abc123"
const WORKER_ID = "dispatch-context.pylon.supervisor.9ab31c44"

const memoryStore = (): PylonOrchestrationStore =>
  createPylonOrchestrationStore(new Database(":memory:"))

const seedRun = (store: PylonOrchestrationStore, state: "running" | "paused" = "running"): void => {
  store.createFleetRun({
    objective: "fixture supervisor run",
    runRef: RUN_REF,
    state,
    targetConcurrency: 3,
    workerKind: "codex",
    workSource: "github_backlog",
  })
}

const seedWorker = (store: PylonOrchestrationStore): void => {
  store.createDispatchContext({
    assigneeHandle: "acct-1",
    id: WORKER_ID,
    lastHeartbeatAt: new Date(),
    runnerKind: "codex",
  })
}

let nextIntentId = 100
const intent = (overrides: Partial<FleetIntentRow> & { intent: FleetIntentRow["intent"] }): FleetIntentRow => {
  nextIntentId += 1
  return {
    createdAt: "2026-07-04T15:20:11.412Z",
    desiredSlots: null,
    flagRef: null,
    id: nextIntentId,
    mutationRef: `mutation:cg-1:c-1:${nextIntentId}`,
    requestedByUserId: "user-1",
    runId: RUN_REF,
    scope: `scope.fleet_run.${RUN_REF}`,
    workerId: null,
    ...overrides,
  } as FleetIntentRow
}

const pageReader = (
  pages: Array<{ intents: FleetIntentRow[]; nextAfter: number; upToDate?: boolean }>,
): { reader: ReadPendingFleetIntentsLike; calls: Array<{ after: number | undefined }> } => {
  const calls: Array<{ after: number | undefined }> = []
  let index = 0
  const reader: ReadPendingFleetIntentsLike = (options) => {
    calls.push({ after: options.after })
    const page = pages[Math.min(index, pages.length - 1)]!
    index += 1
    return Promise.resolve({
      intents: page.intents,
      nextAfter: page.nextAfter,
      ok: true,
      upToDate: page.upToDate ?? true,
    })
  }
  return { calls, reader }
}

const enforce = (
  store: PylonOrchestrationStore,
  reader: ReadPendingFleetIntentsLike,
  logLines: string[] = [],
) =>
  enforcePendingFleetIntents(store, {
    adminToken: "admin-secret",
    baseUrl: "https://openagents.com",
    log: (line) => logLines.push(line),
    readImpl: reader,
  })

describe("applyFleetIntentToStore", () => {
  test("set_desired_slots persists the operator cap and steers effective slots", () => {
    const store = memoryStore()
    seedRun(store)
    const result = applyFleetIntentToStore(store, intent({ desiredSlots: 2, intent: "set_desired_slots" }))
    expect(result.outcome).toBe("applied")
    expect(store.getFleetRunDesiredSlotsCap(RUN_REF)).toBe(2)
    expect(store.getFleetRun(RUN_REF)?.targetConcurrency).toBe(2)
    expect(store.effectiveFleetRunDesiredSlots(RUN_REF)).toBe(2)
  })

  test("set_desired_slots 0 caps effective slots at zero without faking a positive targetConcurrency", () => {
    const store = memoryStore()
    seedRun(store)
    const result = applyFleetIntentToStore(store, intent({ desiredSlots: 0, intent: "set_desired_slots" }))
    expect(result.outcome).toBe("applied")
    expect(store.getFleetRunDesiredSlotsCap(RUN_REF)).toBe(0)
    expect(store.getFleetRun(RUN_REF)?.targetConcurrency).toBe(3)
    expect(store.effectiveFleetRunDesiredSlots(RUN_REF)).toBe(0)
  })

  test("pause and resume steer the run state with operator provenance", () => {
    const store = memoryStore()
    seedRun(store)
    expect(applyFleetIntentToStore(store, intent({ intent: "pause" })).outcome).toBe("applied")
    const paused = store.getFleetRun(RUN_REF)
    expect(paused?.state).toBe("paused")
    expect(paused?.stateSource).toBe("operator")
    expect(store.effectiveFleetRunDesiredSlots(RUN_REF)).toBe(0)
    expect(applyFleetIntentToStore(store, intent({ intent: "resume" })).outcome).toBe("applied")
    expect(store.getFleetRun(RUN_REF)?.state).toBe("running")
    expect(store.effectiveFleetRunDesiredSlots(RUN_REF)).toBe(3)
  })

  test("stop is terminal, releases the run's live claims, and zeroes effective slots", () => {
    const store = memoryStore()
    seedRun(store)
    const claim = store.tryClaimWorkUnit({
      claimRef: "claim.fixture.1",
      runRef: RUN_REF,
      ttl: 60_000,
      workerAccountRef: "acct-1",
      workUnitRef: "work-unit.github-issue.fixture",
    })
    expect(claim).not.toBeNull()
    const result = applyFleetIntentToStore(store, intent({ intent: "stop" }))
    expect(result.outcome).toBe("applied")
    expect(result.detail).toContain("released 1 live claim")
    expect(store.getFleetRun(RUN_REF)?.state).toBe("stopped")
    expect(store.getWorkClaim("claim.fixture.1")?.state).toBe("released")
    expect(store.effectiveFleetRunDesiredSlots(RUN_REF)).toBe(0)
    // A stopped run never auto-revives: pause/resume/set_desired_slots go stale.
    expect(applyFleetIntentToStore(store, intent({ intent: "resume" })).outcome).toBe("skipped_stale")
    expect(
      applyFleetIntentToStore(store, intent({ desiredSlots: 5, intent: "set_desired_slots" })).outcome,
    ).toBe("skipped_stale")
  })

  test("pause_worker/resume_worker gate dispatch eligibility with worker_paused", () => {
    const store = memoryStore()
    seedRun(store)
    seedWorker(store)
    expect(
      applyFleetIntentToStore(store, intent({ intent: "pause_worker", workerId: WORKER_ID })).outcome,
    ).toBe("applied")
    const pausedContext = store.getDispatchContext(WORKER_ID)!
    expect(pausedContext.paused).toBe(true)
    expect(dispatchEligibility(pausedContext, { now: new Date() })).toEqual({
      ok: false,
      reason: "worker_paused",
    })
    expect(
      applyFleetIntentToStore(store, intent({ intent: "resume_worker", workerId: WORKER_ID })).outcome,
    ).toBe("applied")
    const resumedContext = store.getDispatchContext(WORKER_ID)!
    expect(resumedContext.paused).toBe(false)
    expect(dispatchEligibility(resumedContext, { now: new Date() }).ok).toBe(true)
  })

  test("unknown runs and workers are skipped honestly, malformed intents fail", () => {
    const store = memoryStore()
    expect(applyFleetIntentToStore(store, intent({ intent: "pause" })).detail).toBe("unknown_run")
    expect(applyFleetIntentToStore(store, intent({ intent: "pause" })).outcome).toBe("skipped_stale")
    expect(
      applyFleetIntentToStore(store, intent({ intent: "pause_worker", workerId: "dispatch-context.pylon.supervisor.nope" })).outcome,
    ).toBe("skipped_stale")
    seedRun(store)
    expect(
      applyFleetIntentToStore(store, intent({ desiredSlots: null, intent: "set_desired_slots" })).outcome,
    ).toBe("failed")
    expect(applyFleetIntentToStore(store, intent({ intent: "pause_worker" })).outcome).toBe("failed")
  })

  test("acknowledge_inbox_flag is an honest recorded no-op (no pylon-local attention items yet)", () => {
    const store = memoryStore()
    seedRun(store)
    const result = applyFleetIntentToStore(
      store,
      intent({ flagRef: "flag.public.fixture.1", intent: "acknowledge_inbox_flag" }),
    )
    expect(result.outcome).toBe("applied")
    expect(result.detail).toContain("no pylon-local attention items")
  })
})

describe("enforcePendingFleetIntents", () => {
  test("applies a page, records outcomes in the store, and persists the watermark", async () => {
    const store = memoryStore()
    seedRun(store)
    const pause = intent({ intent: "pause" })
    const slots = intent({ desiredSlots: 1, intent: "set_desired_slots" })
    const { calls, reader } = pageReader([{ intents: [pause, slots], nextAfter: slots.id }])
    const logLines: string[] = []
    const result = await enforce(store, reader, logLines)
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.outcomes.map((o) => o.outcome)).toEqual(["applied", "applied"])
      expect(result.nextAfter).toBe(slots.id)
    }
    expect(calls[0]!.after).toBe(0)
    expect(store.getFleetIntentWatermark()).toBe(slots.id)
    expect(store.getFleetRun(RUN_REF)?.state).toBe("paused")
    const recorded = store.listFleetIntentOutcomes({ runRef: RUN_REF })
    expect(recorded.map((o) => [o.intentId, o.outcome])).toEqual([
      [pause.id, "applied"],
      [slots.id, "applied"],
    ])
    expect(recorded.every((o) => o.mutationRef.startsWith("mutation:"))).toBe(true)
    expect(logLines.length).toBe(2)
    expect(JSON.stringify(logLines)).not.toContain("admin-secret")
  })

  test("watermark persists across a store restart (same db file, new store)", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pylon-intent-enforce-"))
    const dbPath = join(dir, "orchestration.sqlite")
    const first = createPylonOrchestrationStore(new Database(dbPath))
    seedRun(first)
    const pause = intent({ intent: "pause" })
    const firstResult = await enforce(first, pageReader([{ intents: [pause], nextAfter: pause.id }]).reader, [])
    expect(firstResult.ok).toBe(true)

    const reopened = createPylonOrchestrationStore(new Database(dbPath))
    expect(reopened.getFleetIntentWatermark()).toBe(pause.id)
    const { calls, reader } = pageReader([{ intents: [], nextAfter: pause.id }])
    const secondResult = await enforce(reopened, reader, [])
    expect(secondResult.ok).toBe(true)
    expect(calls[0]!.after).toBe(pause.id)
  })

  test("exactly-once under redelivery: a re-served intent is deduped, not re-applied", async () => {
    const store = memoryStore()
    seedRun(store)
    const pause = intent({ intent: "pause" })
    const page = { intents: [pause], nextAfter: pause.id }
    const first = await enforce(store, pageReader([page]).reader, [])
    expect(first.ok).toBe(true)
    // Operator resumes out-of-band; a redelivered stale pause page must NOT re-pause.
    store.updateFleetRunState(RUN_REF, "running")
    store.setFleetIntentWatermark(0) // simulate a poller replay from an older watermark
    const second = await enforce(store, pageReader([page]).reader, [])
    expect(second.ok).toBe(true)
    if (second.ok) {
      expect(second.outcomes).toHaveLength(1)
      expect(second.outcomes[0]!.deduped).toBe(true)
    }
    expect(store.getFleetRun(RUN_REF)?.state).toBe("running")
    expect(store.listFleetIntentOutcomes().filter((o) => o.intentId === pause.id)).toHaveLength(1)
  })

  test("failure isolation: one bad intent records failed and never wedges the rest", async () => {
    const store = memoryStore()
    seedRun(store)
    seedWorker(store)
    const malformed = intent({ desiredSlots: null, intent: "set_desired_slots" })
    const throwing = intent({ intent: "pause_worker", workerId: WORKER_ID })
    const healthy = intent({ intent: "pause" })
    const original = store.setDispatchContextPaused.bind(store)
    store.setDispatchContextPaused = () => {
      throw new Error("injected store failure")
    }
    try {
      const result = await enforce(
        store,
        pageReader([{ intents: [malformed, throwing, healthy], nextAfter: healthy.id }]).reader,
        [],
      )
      expect(result.ok).toBe(true)
      if (result.ok) {
        expect(result.outcomes.map((o) => o.outcome)).toEqual(["failed", "failed", "applied"])
        expect(result.outcomes[1]!.detail).toContain("injected store failure")
      }
    } finally {
      store.setDispatchContextPaused = original
    }
    expect(store.getFleetRun(RUN_REF)?.state).toBe("paused")
    expect(store.getFleetIntentWatermark()).toBe(healthy.id)
    expect(store.getFleetIntentOutcome(malformed.id)?.outcome).toBe("failed")
    expect(store.getFleetIntentOutcome(throwing.id)?.outcome).toBe("failed")
    expect(store.getFleetIntentOutcome(healthy.id)?.outcome).toBe("applied")
  })

  test("intents for unknown runs are consumed as skipped_stale, watermark still advances", async () => {
    const store = memoryStore()
    const foreign = intent({ intent: "pause", runId: "fleet-run.pylon.supervisor.someone-else" })
    const result = await enforce(store, pageReader([{ intents: [foreign], nextAfter: foreign.id }]).reader, [])
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.outcomes[0]!.outcome).toBe("skipped_stale")
      expect(result.outcomes[0]!.detail).toBe("unknown_run")
    }
    expect(store.getFleetIntentWatermark()).toBe(foreign.id)
  })

  test("a poll failure leaves the watermark untouched and applies nothing", async () => {
    const store = memoryStore()
    seedRun(store)
    store.setFleetIntentWatermark(7)
    const reader: ReadPendingFleetIntentsLike = () =>
      Promise.resolve({ error: "unauthorized", ok: false, reason: null, status: 401 })
    const logLines: string[] = []
    const result = await enforce(store, reader, logLines)
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error).toBe("unauthorized")
      expect(result.watermark).toBe(7)
    }
    expect(store.getFleetIntentWatermark()).toBe(7)
    expect(store.listFleetIntentOutcomes()).toHaveLength(0)
    expect(store.getFleetRun(RUN_REF)?.state).toBe("running")
    expect(JSON.stringify([result, logLines])).not.toContain("admin-secret")
  })
})
