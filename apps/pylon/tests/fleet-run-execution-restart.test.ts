import { Database } from "bun:sqlite"
import { describe, expect, test } from "bun:test"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Schema as S } from "effect"

import { hashPylonAccountRef } from "../src/account-registry.js"
import { projectFleetRunSupervisorObservation } from "../src/orchestration/fleet-run-execution-projection.js"
import {
  openPylonFleetRunExecutionReporter,
  PylonFleetRunExecutionEventSchema,
  type PylonFleetRunExecutionBatch,
  type PylonFleetRunExecutionHttpPort,
  type PylonFleetRunExecutionReporter,
} from "../src/orchestration/fleet-run-execution-reporter.js"
import {
  tickFleetRunSupervisor,
  type FleetRunSupervisorAccount,
  type FleetRunSupervisorObservedEvent,
  type FleetRunSupervisorRunner,
} from "../src/orchestration/fleet-run-supervisor.js"
import type { PylonFleetRunUsageEvidence } from "../src/orchestration/fleet-run-usage-evidence.js"
import { createPylonOrchestrationStore } from "../src/orchestration/store.js"
import { fixtureCandidates, planWorkCandidates } from "../src/orchestration/work-planner.js"

const runRef = "fleet_run.sarah.abcdef0123456789abcd"
const claimRef = "claim.sarah_fleet_run.abcdef0123456789abcdef01"
const pylonRef = "pylon.public.fc2.restart_projection"
const firstNow = new Date("2026-07-09T20:00:00.000Z")

const accounts: readonly FleetRunSupervisorAccount[] = [
  { accountRef: "codex-owner", advertisedCapacity: 1, workerKind: "codex" },
  { accountRef: "claude-owner", advertisedCapacity: 1, workerKind: "claude" },
  { accountRef: "grok-owner", advertisedCapacity: 1, workerKind: "grok" },
]

const planner = (store: ReturnType<typeof createPylonOrchestrationStore>) => ({
  plan: async (input: { readonly now: Date }) =>
    planWorkCandidates("fixture", fixtureCandidates({ kind: "fixture", count: 3 }), {
      claimRegistry: store,
      now: input.now,
    }),
})

const exactEvidence = (input: {
  readonly assignmentRef: string
  readonly harnessKind: "claude" | "codex"
}): PylonFleetRunUsageEvidence => ({
  schema: "openagents.pylon.fleet_run_usage_evidence.v1",
  truth: "exact",
  harnessKind: input.harnessKind,
  evidenceRef: `evidence.public.${input.harnessKind}.fixture`,
  assignmentRef: input.assignmentRef,
  pylonRef,
  provider: input.harnessKind === "codex"
    ? "pylon-codex-own-capacity"
    : "pylon-claude-own-capacity",
  model: input.harnessKind === "codex"
    ? "openagents/pylon-codex"
    : "openagents/pylon-claude",
  demandKind: "own_capacity",
  demandSource: "khala_coding_delegation",
  inputTokens: 11,
  outputTokens: 7,
  reasoningTokens: 3,
  cacheReadTokens: 2,
  totalTokens: 21,
  tokenRows: 1,
  tokenUsageRefs: [`token_usage.public.${input.harnessKind}.fixture`],
  proofRefs: [`proof.public.${input.harnessKind}.fixture`],
  closeoutChecklistRefs: [`check.public.${input.harnessKind}.closeout`],
  proofChecklistRefs: [`check.public.${input.harnessKind}.proof`],
})

const notMeasuredEvidence = (assignmentRef: string): PylonFleetRunUsageEvidence => ({
  schema: "openagents.pylon.fleet_run_usage_evidence.v1",
  truth: "not_measured",
  harnessKind: "grok",
  evidenceRef: "evidence.public.grok.fixture",
  assignmentRef,
  receiptRef: "receipt.public.grok.fixture",
  tokenUsageRefs: [],
  caveatRefs: ["caveat.public.grok.usage_not_measured"],
})

const lifecycleSink = (
  reporter: PylonFleetRunExecutionReporter,
  store: ReturnType<typeof createPylonOrchestrationStore>,
) => async (event: FleetRunSupervisorObservedEvent): Promise<void> => {
  for (const projected of projectFleetRunSupervisorObservation({ event, store })) {
    await reporter.record(projected)
  }
}

describe("FleetRun execution projection restart receipt", () => {
  test("projects a failed unit as terminal without inventing unavailable proof", () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    store.createFleetRun({
      runRef,
      objective: "Project a bounded failed-work fixture.",
      workSource: "fixture",
      targetConcurrency: 1,
      workerKind: "grok",
      state: "running",
      startedAt: firstNow,
      now: firstNow,
      authorityBinding: {
        schema: "openagents.pylon.fleet_run_authority_binding.v1",
        source: "sarah_authority",
        authorityFingerprint: "d".repeat(64),
        claimRef,
        pylonRef,
        targetPreference: "owner_local",
        phase: "accepted",
      },
    })
    const projected = projectFleetRunSupervisorObservation({
      store,
      event: {
        kind: "dispatch",
        runRef,
        taskId: "task.public.failed_fixture",
        claimRef: "claim.public.failed_fixture",
        workUnitRef: "unit.public.failed_fixture",
        accountRef: "private-account-ref-must-not-project",
        accountRefHash: null,
        assignmentRef: null,
        blockerRefs: ["blocker.pylon.fleet_runner.grok_verification_failed"],
        closeoutRef: null,
        status: "failed",
        usageEvidence: null,
        workerKind: "grok",
      },
    })
    const failed = projected[1]
    expect(failed).toMatchObject({
      kind: "work_terminal",
      terminalState: "failed",
      blockerRefs: ["blocker.pylon.fleet_runner.grok_verification_failed"],
    })
    expect(failed).not.toHaveProperty("assignmentRef")
    expect(JSON.stringify(projected)).not.toContain("private-account-ref")
    expect(() => S.decodeUnknownSync(PylonFleetRunExecutionEventSchema)({
      ...failed,
      sequence: 2,
      eventRef: "event.pylon.fleet_run.0123456789abcdef01234567",
    }, { onExcessProperty: "error" })).not.toThrow()
  })

  test("resumes one mixed Codex/Claude/Grok run without duplicate claims and closes with exact evidence truth", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fc2-execution-restart-"))
    const databasePath = join(root, "orchestration.sqlite")
    try {
      const firstDatabase = new Database(databasePath)
      const firstStore = createPylonOrchestrationStore(firstDatabase)
      firstStore.createFleetRun({
        runRef,
        objective: "Execute the bounded mixed-harness restart fixture.",
        workSource: "fixture",
        targetConcurrency: 3,
        workerKind: "auto",
        state: "running",
        startedAt: firstNow,
        now: firstNow,
        counters: { workUnitsTotal: 3 },
        authorityBinding: {
          schema: "openagents.pylon.fleet_run_authority_binding.v1",
          source: "sarah_authority",
          authorityFingerprint: "b".repeat(64),
          claimRef,
          pylonRef,
          targetPreference: "owner_local",
          phase: "accepted",
        },
      })
      const offline: PylonFleetRunExecutionHttpPort = {
        append: async () => {
          throw new Error("fixture transport offline")
        },
      }
      const firstReporter = openPylonFleetRunExecutionReporter({
        store: firstStore,
        pylonRef,
        runRef,
        remote: offline,
        now: () => firstNow,
      })
      const assignments = new Map<string, string>()
      await tickFleetRunSupervisor({
        store: firstStore,
        pylonRef,
        runRef,
        planner: planner(firstStore),
        capacity: { accounts: async () => accounts },
        runner: {
          dispatch: async input => {
            const assignmentRef = `assignment.public.${input.workerKind}.restart_fixture`
            assignments.set(input.taskId, assignmentRef)
            return {
              assignmentRef,
              lifecycle: [],
              status: "accepted",
              accountRefHash: null,
              closeoutRef: null,
              usageEvidence: null,
            }
          },
        },
        clock: { now: () => firstNow },
        onLifecycle: lifecycleSink(firstReporter, firstStore),
      })
      expect(firstStore.listTasks("dispatched")).toHaveLength(3)
      expect(firstStore.listWorkClaims({ runRef })).toHaveLength(3)
      expect(firstStore.listFleetRunExecutionOutbox(runRef, { pendingOnly: true })).not.toHaveLength(0)
      await firstReporter.close()
      firstDatabase.close()

      const secondNow = new Date(firstNow.getTime() + 1_000)
      const delivered: PylonFleetRunExecutionBatch[] = []
      const remote: PylonFleetRunExecutionHttpPort = {
        append: async ({ batch }) => {
          delivered.push(batch)
          return {
            schema: "openagents.pylon.fleet_run_execution_ack.v1",
            runRef,
            claimRef,
            acceptedThroughSequence: batch.events.at(-1)?.sequence ?? 0,
            storedEventCount: batch.events.length,
            duplicateEventCount: 0,
            execution: {
              state: batch.events.some(event => event.kind === "run_terminal")
                ? "completed"
                : "running",
              lastSequence: batch.events.at(-1)?.sequence ?? 0,
              counters: {
                workUnitsTotal: 3,
                activeAssignments: 0,
                acceptedAssignments: 3,
                failedAssignments: 0,
                staleAssignments: 0,
              },
              startedAt: firstNow.toISOString(),
              updatedAt: secondNow.toISOString(),
              closeouts: [],
            },
          }
        },
      }
      const secondDatabase = new Database(databasePath)
      const secondStore = createPylonOrchestrationStore(secondDatabase)
      const secondReporter = openPylonFleetRunExecutionReporter({
        store: secondStore,
        pylonRef,
        runRef,
        remote,
        now: () => secondNow,
      })
      await secondReporter.flush()
      let redispatches = 0
      const runner: FleetRunSupervisorRunner = {
        dispatch: async () => {
          redispatches += 1
          throw new Error("restart must reconcile, not redispatch")
        },
        reconcile: async ({ activeAssignments }) => activeAssignments.map(active => {
          const assignmentRef = assignments.get(active.taskId)
          if (assignmentRef === undefined) throw new Error("missing fixture assignment")
          const workerKind = active.taskId.includes("claude")
            ? "claude"
            : active.taskId.includes("grok")
              ? "grok"
              : secondStore.getTask(active.taskId)?.spec.runnerKind === "claude_agent"
                ? "claude"
                : secondStore.getTask(active.taskId)?.spec.runnerKind === "grok_cli"
                  ? "grok"
                  : "codex"
          const accountProvider = workerKind === "claude" ? "claude_agent" : workerKind
          const usageEvidence = workerKind === "grok"
            ? notMeasuredEvidence(assignmentRef)
            : exactEvidence({ assignmentRef, harnessKind: workerKind })
          return {
            taskId: active.taskId,
            assignmentRef,
            lifecycle: [],
            status: "completed" as const,
            accountRefHash: hashPylonAccountRef(accountProvider, active.accountRef),
            closeoutRef: `closeout.public.${workerKind}.restart_fixture`,
            usageEvidence,
          }
        }),
      }
      const result = await tickFleetRunSupervisor({
        store: secondStore,
        pylonRef,
        runRef,
        planner: planner(secondStore),
        capacity: { accounts: async () => accounts },
        runner,
        clock: { now: () => secondNow },
        onLifecycle: lifecycleSink(secondReporter, secondStore),
      })
      await secondReporter.flush()

      expect(result.run.state).toBe("completed")
      expect(redispatches).toBe(0)
      expect(secondStore.listWorkClaims({ runRef })).toHaveLength(3)
      expect(new Set(secondStore.listWorkClaims({ runRef }).map(claim => claim.claimRef)).size).toBe(3)
      expect(secondStore.listTasks("completed")).toHaveLength(3)
      expect(secondStore.listFleetRunExecutionOutbox(runRef, { pendingOnly: true })).toEqual([])
      const events = delivered.flatMap(batch => batch.events)
      expect(events.map(event => event.sequence)).toEqual(
        Array.from({ length: events.length }, (_, index) => index + 1),
      )
      expect(events.filter(event => event.kind === "work_terminal")).toHaveLength(3)
      expect(events.filter(event => event.kind === "work_terminal").map(event =>
        event.kind === "work_terminal" ? event.usageEvidence.truth : null
      ).sort()).toEqual(["exact", "exact", "not_measured"])
      expect(events.at(-1)?.kind).toBe("run_terminal")
      expect(JSON.stringify(events)).not.toContain("codex-owner")
      expect(JSON.stringify(events)).not.toContain(root)
      await secondReporter.close()
      secondDatabase.close()
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })
})
