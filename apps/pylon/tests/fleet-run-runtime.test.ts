import { describe, expect, test } from "bun:test"
import { existsSync } from "node:fs"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  openPylonFleetRunRuntime,
  PYLON_FLEET_RUN_DATABASE_FILENAME,
} from "../src/orchestration/fleet-run-runtime.js"

const fixedNow = new Date("2026-07-09T20:00:00.000Z")

describe("Pylon-home FleetRun runtime", () => {
  test("run and claim survive close/reopen; status reconstructs without duplicate work", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fleet-run-runtime-"))
    const pylonHome = join(root, "pylon-home")
    const defaultCodexHome = join(root, "default-codex-home")
    const env = {
      PYLON_HOME: pylonHome,
      CODEX_HOME: defaultCodexHome,
    } as NodeJS.ProcessEnv
    const runRef = "fleet_run.fc2.durable_runtime"
    const workUnitRef = "fixture:durable-runtime:1"

    try {
      const first = await openPylonFleetRunRuntime({ env, now: () => fixedNow })
      expect(first.databasePath).toBe(
        join(pylonHome, PYLON_FLEET_RUN_DATABASE_FILENAME),
      )
      expect(first.databasePath).not.toBe(":memory:")
      expect(first.databasePath.startsWith(defaultCodexHome)).toBe(false)
      expect(existsSync(first.databasePath)).toBe(true)

      first.store.createFleetRun({
        runRef,
        objective: "Prove one bounded durable FleetRun unit.",
        workSource: "fixture",
        targetConcurrency: 1,
        workerKind: "codex",
        state: "running",
        startedAt: fixedNow,
        now: fixedNow,
      })
      first.store.createTask({
        id: workUnitRef,
        threadId: runRef,
        spec: {
          title: "Durable runtime fixture",
          prompt: "Run the public-safe fixture.",
          fleetRunRef: runRef,
        },
        status: "dispatched",
        now: fixedNow,
      })
      expect(first.store.tryClaimWorkUnit({
        claimRef: "claim.fc2.durable_runtime.1",
        workUnitRef,
        runRef,
        workerAccountRef: "codex-owner-isolated",
        ttl: 60_000,
        now: fixedNow,
      })).not.toBeNull()
      await first.close()
      await first.close()
      await expect(first.manager.status(runRef)).rejects.toThrow(
        "fleet run manager is closed",
      )

      const reopened = await openPylonFleetRunRuntime({ env, now: () => fixedNow })
      try {
        const snapshot = await reopened.manager.status(runRef)
        if (Array.isArray(snapshot)) throw new Error("expected one FleetRun snapshot")
        expect(snapshot.active).toBe(false)
        expect(snapshot.run.runRef).toBe(runRef)
        expect(snapshot.run.state).toBe("running")
        expect(snapshot.run.counters.activeAssignments).toBe(1)

        const claims = reopened.store.listWorkClaims({ runRef })
        expect(claims).toHaveLength(1)
        expect(claims[0]).toMatchObject({
          claimRef: "claim.fc2.durable_runtime.1",
          state: "claimed",
          workUnitRef,
        })
        expect(reopened.store.tryClaimWorkUnit({
          claimRef: "claim.fc2.durable_runtime.duplicate",
          workUnitRef,
          runRef,
          workerAccountRef: "codex-owner-isolated-2",
          ttl: 60_000,
          now: fixedNow,
        })).toBeNull()
        expect(reopened.store.listWorkClaims({ runRef })).toHaveLength(1)

        const publicStatus = JSON.stringify(snapshot)
        expect(publicStatus).not.toContain(pylonHome)
        expect(publicStatus).not.toContain(defaultCodexHome)
      } finally {
        await reopened.close()
      }
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })
})
