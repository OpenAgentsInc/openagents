import { describe, expect, test } from "vite-plus/test"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { openPylonFleetRunRuntime } from "../src/orchestration/fleet-run-runtime.js"

const fixedNow = new Date("2026-07-10T00:00:00.000Z")
const deliveredAt = new Date("2026-07-10T00:00:01.000Z")
const runRef = "fleet_run.sarah.0123456789abcdef0123"
const claimRef = "claim.sarah_fleet_run.0123456789abcdef01234567"

const eventJson = (
  sequence: number,
  eventRef: string,
  kind: "run_started" | "run_terminal",
): string => JSON.stringify({
  schema: "openagents.pylon.fleet_run_execution_event.v1",
  sequence,
  eventRef,
  kind,
  observedAt: fixedNow.toISOString(),
  ...(kind === "run_terminal" ? { terminalState: "completed", blockerRefs: [] } : {}),
})

describe("Pylon FleetRun execution outbox", () => {
  test("assigns gapless sequences, replays exact events, and retains pending rows across reopen", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fleet-run-outbox-"))
    const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
    const startedRef = "event.pylon.fleet_run.0123456789abcdef01234567"
    const terminalRef = "event.pylon.fleet_run.89abcdef0123456701234567"

    try {
      const first = await openPylonFleetRunRuntime({ env, now: () => fixedNow })
      first.store.createFleetRun({
        runRef,
        objective: "Run one accepted Sarah fixture through the durable outbox.",
        workSource: "fixture",
        authorityBinding: {
          schema: "openagents.pylon.fleet_run_authority_binding.v1",
          source: "sarah_authority",
          authorityFingerprint: "a".repeat(64),
          claimRef,
          pylonRef: "pylon.public.fc2.outbox",
          targetPreference: "owner_local",
          phase: "accepted",
        },
        targetConcurrency: 1,
        workerKind: "auto",
        state: "running",
        now: fixedNow,
      })

      const started = first.store.enqueueFleetRunExecutionOutbox({
        runRef,
        claimRef,
        eventRef: startedRef,
        eventJsonForSequence: sequence => eventJson(sequence, startedRef, "run_started"),
        now: fixedNow,
      })
      expect(started.sequence).toBe(1)
      expect(JSON.parse(started.eventJson)).toMatchObject({ sequence: 1, eventRef: startedRef })

      const replay = first.store.enqueueFleetRunExecutionOutbox({
        runRef,
        claimRef,
        eventRef: startedRef,
        eventJsonForSequence: sequence => eventJson(sequence, startedRef, "run_started"),
        now: deliveredAt,
      })
      expect(replay).toEqual(started)

      const terminal = first.store.enqueueFleetRunExecutionOutbox({
        runRef,
        claimRef,
        eventRef: terminalRef,
        eventJsonForSequence: sequence => eventJson(sequence, terminalRef, "run_terminal"),
        now: deliveredAt,
      })
      expect(terminal.sequence).toBe(2)
      expect(first.store.listFleetRunExecutionOutbox(runRef)).toHaveLength(2)
      expect(
        first.store.markFleetRunExecutionOutboxDelivered(runRef, claimRef, 1, deliveredAt),
      ).toBe(1)
      const deliveryBatchRef = "batch.pylon.fleet_run.0123456789abcdef01234567"
      expect(() => first.store.reserveFleetRunExecutionOutboxBatch({
        runRef,
        claimRef,
        firstSequence: 2,
        lastSequence: Number.MAX_SAFE_INTEGER + 1,
        deliveryBatchRef,
      })).toThrow("reservation is invalid")
      expect(first.store.reserveFleetRunExecutionOutboxBatch({
        runRef,
        claimRef,
        firstSequence: 2,
        lastSequence: 2,
        deliveryBatchRef,
      })).toEqual([
        expect.objectContaining({ sequence: 2, deliveryBatchRef }),
      ])
      await first.close()

      const reopened = await openPylonFleetRunRuntime({ env, now: () => deliveredAt })
      try {
        expect(reopened.store.listFleetRunExecutionOutbox(runRef)).toEqual([
          expect.objectContaining({
            sequence: 2,
            eventRef: terminalRef,
            deliveryBatchRef,
            deliveredAt: null,
          }),
        ])
        expect(reopened.store.listFleetRunExecutionOutbox(runRef, { pendingOnly: false })).toEqual([
          expect.objectContaining({ sequence: 1, eventRef: startedRef, deliveredAt: deliveredAt.toISOString() }),
          expect.objectContaining({ sequence: 2, eventRef: terminalRef, deliveredAt: null }),
        ])
        expect(() => reopened.store.enqueueFleetRunExecutionOutbox({
          runRef,
          claimRef,
          eventRef: terminalRef,
          eventJsonForSequence: sequence => eventJson(sequence, terminalRef, "run_started"),
          now: deliveredAt,
        })).toThrow("conflicting bytes")
      } finally {
        await reopened.close()
      }
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("refuses events not bound to the exact accepted authority claim", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fleet-run-outbox-authority-"))
    const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
    try {
      const runtime = await openPylonFleetRunRuntime({ env, now: () => fixedNow })
      try {
        runtime.store.createFleetRun({
          runRef,
          objective: "Keep imported authority fail closed until server acceptance.",
          workSource: "fixture",
          authorityBinding: {
            schema: "openagents.pylon.fleet_run_authority_binding.v1",
            source: "sarah_authority",
            authorityFingerprint: "b".repeat(64),
            claimRef,
            pylonRef: "pylon.public.fc2.outbox",
            targetPreference: "owner_local",
            phase: "imported",
          },
          targetConcurrency: 1,
          workerKind: "codex",
          state: "running",
          now: fixedNow,
        })
        expect(() => runtime.store.enqueueFleetRunExecutionOutbox({
          runRef,
          claimRef,
          eventRef: "event.pylon.fleet_run.aaaaaaaaaaaaaaaaaaaaaaaa",
          eventJsonForSequence: sequence => eventJson(
            sequence,
            "event.pylon.fleet_run.aaaaaaaaaaaaaaaaaaaaaaaa",
            "run_started",
          ),
          now: fixedNow,
        })).toThrow("exact accepted authority claim")
      } finally {
        await runtime.close()
      }
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })
})
