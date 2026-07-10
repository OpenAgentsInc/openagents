import { describe, expect, test } from "bun:test"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  makePylonFleetRunExecutionHttpPort,
  openPylonFleetRunExecutionReporter,
  PYLON_FLEET_RUN_EXECUTION_ACK_SCHEMA,
  PYLON_FLEET_RUN_EXECUTION_BATCH_SCHEMA,
  type PylonFleetRunExecutionAck,
} from "../src/orchestration/fleet-run-execution-reporter.js"
import { openPylonFleetRunRuntime } from "../src/orchestration/fleet-run-runtime.js"

const fixedNow = new Date("2026-07-10T00:10:00.000Z")
const runRef = "fleet_run.sarah.0123456789abcdef0123"
const claimRef = "claim.sarah_fleet_run.0123456789abcdef01234567"
const pylonRef = "pylon.public.fc2.execution"

const ack = (acceptedThroughSequence: number): PylonFleetRunExecutionAck => ({
  schema: PYLON_FLEET_RUN_EXECUTION_ACK_SCHEMA,
  runRef,
  claimRef,
  acceptedThroughSequence,
  storedEventCount: acceptedThroughSequence,
  duplicateEventCount: 0,
  execution: {
    state: acceptedThroughSequence === 0 ? "pending" : "running",
    counters: {
      workUnitsTotal: 1,
      activeAssignments: 0,
      acceptedAssignments: 0,
      failedAssignments: 0,
      staleAssignments: 0,
    },
    updatedAt: fixedNow.toISOString(),
  },
})

const seedAcceptedRun = async (env: NodeJS.ProcessEnv) => {
  const runtime = await openPylonFleetRunRuntime({ env, now: () => fixedNow })
  runtime.store.createFleetRun({
    runRef,
    objective: "Project the exact accepted Sarah FleetRun execution lifecycle.",
    workSource: "fixture",
    authorityBinding: {
      schema: "openagents.pylon.fleet_run_authority_binding.v1",
      source: "sarah_authority",
      authorityFingerprint: "c".repeat(64),
      claimRef,
      pylonRef,
      targetPreference: "owner_local",
      phase: "accepted",
    },
    targetConcurrency: 1,
    workerKind: "auto",
    state: "running",
    now: fixedNow,
  })
  return runtime
}

describe("Pylon FleetRun execution reporter", () => {
  test("retains a failed append and replays its exact sequence after reopen", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fleet-run-reporter-"))
    const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
    try {
      const first = await seedAcceptedRun(env)
      const failed = openPylonFleetRunExecutionReporter({
        store: first.store,
        pylonRef,
        runRef,
        now: () => fixedNow,
        remote: { append: () => Promise.reject(new Error("offline")) },
      })
      await failed.record({
        schema: "openagents.pylon.fleet_run_execution_event.v1",
        kind: "run_started",
        observedAt: fixedNow.toISOString(),
      })
      expect(first.store.listFleetRunExecutionOutbox(runRef)).toEqual([
        expect.objectContaining({ sequence: 1, deliveredAt: null }),
      ])
      await failed.close()
      await first.close()

      const reopened = await openPylonFleetRunRuntime({ env, now: () => fixedNow })
      const seen: number[][] = []
      const reporter = openPylonFleetRunExecutionReporter({
        store: reopened.store,
        pylonRef,
        runRef,
        now: () => fixedNow,
        remote: {
          append: async ({ batch }) => {
            seen.push(batch.events.map(event => event.sequence))
            return ack(batch.events.at(-1)?.sequence ?? 0)
          },
        },
      })
      try {
        expect((await reporter.flush())?.acceptedThroughSequence).toBe(1)
        expect(seen).toEqual([[1]])
        expect(reopened.store.listFleetRunExecutionOutbox(runRef)).toEqual([])

        // The stable event ref makes a repeated start byte-identical and does
        // not generate another server call after the original ack.
        await reporter.record({
          schema: "openagents.pylon.fleet_run_execution_event.v1",
          kind: "run_started",
          observedAt: fixedNow.toISOString(),
        })
        expect(seen).toEqual([[1]])
        expect(reopened.store.listFleetRunExecutionOutbox(runRef, { pendingOnly: false })).toHaveLength(1)
      } finally {
        await reporter.close()
        await reopened.close()
      }
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("posts a strict bearer batch to the exact Pylon/run route", async () => {
    const calls: Array<{ url: string; init: RequestInit }> = []
    const port = makePylonFleetRunExecutionHttpPort({
      agentToken: "oa_agent_fixture",
      baseUrl: "https://openagents.test",
      fetchImpl: async (url, init) => {
        calls.push({ url: String(url), init: init ?? {} })
        return Response.json(ack(1))
      },
    })
    const result = await port.append({
      pylonRef,
      runRef,
      batch: {
        schema: PYLON_FLEET_RUN_EXECUTION_BATCH_SCHEMA,
        claimRef,
        events: [{
          schema: "openagents.pylon.fleet_run_execution_event.v1",
          sequence: 1,
          eventRef: "event.pylon.fleet_run.0123456789abcdef01234567",
          kind: "run_started",
          observedAt: fixedNow.toISOString(),
        }],
      },
    })

    expect(result.acceptedThroughSequence).toBe(1)
    expect(calls).toHaveLength(1)
    expect(new URL(calls[0]!.url).pathname).toBe(
      `/api/pylons/${pylonRef}/fleet-runs/${runRef}/events`,
    )
    expect(calls[0]!.init.method).toBe("POST")
    expect((calls[0]!.init.headers as Record<string, string>).Authorization).toBe(
      "Bearer oa_agent_fixture",
    )
    expect(JSON.parse(String(calls[0]!.init.body))).toMatchObject({
      schema: PYLON_FLEET_RUN_EXECUTION_BATCH_SCHEMA,
      claimRef,
      events: [{ sequence: 1, kind: "run_started" }],
    })
  })

  test("refuses bearer transport over non-loopback HTTP", () => {
    expect(() => makePylonFleetRunExecutionHttpPort({
      agentToken: "oa_agent_fixture",
      baseUrl: "http://openagents.test",
    })).toThrow("unavailable")
  })
})
