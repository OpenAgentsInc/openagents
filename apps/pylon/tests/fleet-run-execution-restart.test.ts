import { NodeTestDatabase } from "@openagentsinc/sqlite-runtime/test"
import { describe, expect, test } from "vite-plus/test"
import { createHash } from "node:crypto"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Schema as S } from "effect"

import { hashPylonAccountRef } from "../src/account-registry.js"
import { projectFleetRunSupervisorObservation } from "../src/orchestration/fleet-run-execution-projection.js"
import {
  openPylonFleetRunExecutionReporter,
  PYLON_FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
  PylonFleetRunExecutionEventSchemaV2,
  type PylonFleetRunAnyExecutionBatch,
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
  {
    accountRef: "codex-owner",
    advertisedCapacity: 1,
    marginalCostClass: "subscription",
    workerKind: "codex",
  },
  {
    accountRef: "claude-owner",
    advertisedCapacity: 1,
    marginalCostClass: "free",
    workerKind: "claude",
  },
  {
    accountRef: "grok-owner",
    advertisedCapacity: 1,
    marginalCostClass: "not_measured",
    workerKind: "grok",
  },
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
  totalTokens: 18,
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
    const store = createPylonOrchestrationStore(new NodeTestDatabase(":memory:"))
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
        marginalCostClass: "subscription",
        verification: null,
        artifactRefs: [],
        proofRefs: [],
        authorityReceiptRefs: [],
        executionTarget: "owner_local",
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
    expect(() => S.decodeUnknownSync(PylonFleetRunExecutionEventSchemaV2)({
      ...failed,
      schema: PYLON_FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2,
      sequence: 2,
      eventRef: "event.pylon.fleet_run.0123456789abcdef01234567",
    }, { onExcessProperty: "error" })).not.toThrow()

    const unprovenCompletion = projectFleetRunSupervisorObservation({
      store,
      event: {
        kind: "dispatch",
        runRef,
        taskId: "task.public.unproven_completion",
        claimRef: "claim.public.unproven_completion",
        workUnitRef: "unit.public.unproven_completion",
        accountRef: "private-account-ref-must-not-project",
        accountRefHash: hashPylonAccountRef("grok", "grok-owner"),
        assignmentRef: "assignment.public.unproven_completion",
        blockerRefs: [],
        closeoutRef: "closeout.public.unproven_completion",
        status: "completed",
        usageEvidence: notMeasuredEvidence("assignment.public.unproven_completion"),
        workerKind: "grok",
        marginalCostClass: "not_measured",
        verification: null,
        artifactRefs: [],
        proofRefs: [],
        authorityReceiptRefs: [],
        executionTarget: "owner_local",
      },
    })[1]
    expect(unprovenCompletion).toMatchObject({
      kind: "work_terminal",
      terminalState: "failed",
      blockerRefs: ["blocker.pylon.fleet_run.evidence_incomplete"],
    })

    const completeEvent = {
      kind: "dispatch" as const,
      runRef,
      taskId: "task.public.cardinality",
      claimRef: "claim.public.cardinality",
      workUnitRef: "unit.public.cardinality",
      accountRef: "private-account-ref-must-not-project",
      accountRefHash: hashPylonAccountRef("grok", "grok-owner"),
      assignmentRef: "assignment.public.cardinality",
      blockerRefs: [],
      closeoutRef: "closeout.public.cardinality",
      status: "completed" as const,
      usageEvidence: notMeasuredEvidence("assignment.public.cardinality"),
      workerKind: "grok" as const,
      marginalCostClass: "not_measured" as const,
      verification: {
        truth: "passed" as const,
        verifierRef: "verifier.public.cardinality",
        evidenceRefs: ["verification.public.cardinality"],
      },
      artifactRefs: ["artifact.public.cardinality"],
      proofRefs: ["proof.public.cardinality"],
      authorityReceiptRefs: [claimRef],
      executionTarget: "owner_local" as const,
    }
    const invalidCardinalityEvents = [
      {
        ...completeEvent,
        verification: {
          ...completeEvent.verification,
          evidenceRefs: [
            "evidence.public.cardinality.duplicate",
            "evidence.public.cardinality.duplicate",
          ],
        },
      },
      {
        ...completeEvent,
        workerKind: "codex" as const,
        accountRefHash: hashPylonAccountRef("codex", "codex-owner"),
        usageEvidence: {
          ...exactEvidence({
            assignmentRef: completeEvent.assignmentRef,
            harnessKind: "codex",
          }),
          tokenRows: 101,
          tokenUsageRefs: Array.from(
            { length: 101 },
            (_, index) => `token_usage.public.cardinality.${index}`,
          ),
        },
      },
      {
        ...completeEvent,
        artifactRefs: Array.from(
          { length: 65 },
          (_, index) => `artifact.public.cardinality.${index}`,
        ),
      },
      {
        ...completeEvent,
        status: "failed" as const,
        blockerRefs: Array.from(
          { length: 33 },
          (_, index) => `blocker.public.cardinality.${index}`,
        ),
      },
      (() => {
        const unsafe = "Users/owner/private/blocker"
        const opaque = `blocker.pylon.fleet_run.opaque.${createHash("sha256")
          .update(unsafe)
          .digest("hex")
          .slice(0, 24)}`
        return {
          ...completeEvent,
          status: "failed" as const,
          blockerRefs: [unsafe, opaque],
        }
      })(),
      (() => {
        const unsafe = "Users/owner/private/artifact"
        const opaque = `artifact.public.pylon.opaque.${createHash("sha256")
          .update(unsafe)
          .digest("hex")
          .slice(0, 24)}`
        return {
          ...completeEvent,
          artifactRefs: [unsafe],
          proofRefs: [opaque],
        }
      })(),
      (() => {
        const unsafeUsageProof = "Users/owner/private/usage-proof"
        const collidingTerminalProof = `proof.public.pylon.opaque.${createHash("sha256")
          .update(unsafeUsageProof)
          .digest("hex")
          .slice(0, 24)}`
        return {
          ...completeEvent,
          workerKind: "codex" as const,
          accountRefHash: hashPylonAccountRef("codex", "codex-owner"),
          proofRefs: [collidingTerminalProof],
          usageEvidence: {
            ...exactEvidence({
              assignmentRef: completeEvent.assignmentRef,
              harnessKind: "codex",
            }),
            proofRefs: [unsafeUsageProof],
          },
        }
      })(),
      {
        ...completeEvent,
        workerKind: "codex" as const,
        accountRefHash: hashPylonAccountRef("codex", "codex-owner"),
        usageEvidence: {
          ...exactEvidence({
            assignmentRef: completeEvent.assignmentRef,
            harnessKind: "codex",
          }),
          tokenUsageRefs: [
            "usage.public.within_role_duplicate",
            "usage.public.within_role_duplicate",
          ],
        },
      },
    ]
    for (const event of invalidCardinalityEvents) {
      expect(projectFleetRunSupervisorObservation({ store, event })[1]).toMatchObject({
        kind: "work_terminal",
        terminalState: "failed",
        blockerRefs: ["blocker.pylon.fleet_run.evidence_cardinality_invalid"],
      })
    }

    const unsafeUsageAssignment = "Users/owner/private/usage-assignment"
    const collidingTerminalAssignment = `assignment.public.pylon.opaque.${createHash("sha256")
      .update(unsafeUsageAssignment)
      .digest("hex")
      .slice(0, 24)}`
    const rawAssignmentMismatch = projectFleetRunSupervisorObservation({
      store,
      event: {
        ...completeEvent,
        workerKind: "codex",
        accountRefHash: hashPylonAccountRef("codex", "codex-owner"),
        assignmentRef: collidingTerminalAssignment,
        usageEvidence: exactEvidence({
          assignmentRef: unsafeUsageAssignment,
          harnessKind: "codex",
        }),
      },
    })[1]
    expect(rawAssignmentMismatch).toMatchObject({
      kind: "work_terminal",
      terminalState: "failed",
      blockerRefs: ["blocker.pylon.fleet_run.evidence_identity_invalid"],
    })

    const independentRoleBounds = projectFleetRunSupervisorObservation({
      store,
      event: {
        ...completeEvent,
        workerKind: "codex" as const,
        accountRefHash: hashPylonAccountRef("codex", "codex-owner"),
        verification: {
          ...completeEvent.verification,
          evidenceRefs: Array.from(
            { length: 62 },
            (_, index) => `verification.public.cardinality.${index}`,
          ),
        },
        artifactRefs: Array.from(
          { length: 60 },
          (_, index) => `artifact.public.cardinality.${index}`,
        ),
        usageEvidence: {
          ...exactEvidence({
            assignmentRef: completeEvent.assignmentRef,
            harnessKind: "codex",
          }),
          tokenUsageRefs: Array.from(
            { length: 26 },
            (_, index) => `token_usage.public.cardinality.${index}`,
          ),
          proofRefs: Array.from(
            { length: 25 },
            (_, index) => `proof.public.cardinality.${index}`,
          ),
          closeoutChecklistRefs: Array.from(
            { length: 25 },
            (_, index) => `check.public.cardinality.closeout.${index}`,
          ),
          proofChecklistRefs: Array.from(
            { length: 25 },
            (_, index) => `check.public.cardinality.proof.${index}`,
          ),
        },
      },
    })[1]
    expect(independentRoleBounds).toMatchObject({
      kind: "work_terminal",
      terminalState: "accepted",
    })

    for (const sharedRef of [
      "evidence.public.shared_test_and_verification",
      "Users/owner/private/shared-test-and-verification",
    ]) {
      const sharedReceipt = projectFleetRunSupervisorObservation({
        store,
        event: {
          ...completeEvent,
          verification: {
            ...completeEvent.verification,
            evidenceRefs: [sharedRef],
          },
          artifactRefs: [sharedRef],
        },
      })[1]
      expect(sharedReceipt).toMatchObject({
        kind: "work_terminal",
        terminalState: "accepted",
      })
      if (sharedReceipt?.kind !== "work_terminal" || sharedReceipt.terminalState !== "accepted") {
        throw new Error("expected the shared receipt to remain accepted")
      }
      expect(sharedReceipt.verification.evidenceRefs[0]).toBe(sharedReceipt.artifactRefs[0])
      expect(JSON.stringify(sharedReceipt)).not.toContain("Users/owner/private")
    }

    const invalidRunTerminal = projectFleetRunSupervisorObservation({
      store,
      event: {
        kind: "terminal",
        runRef,
        terminalState: "failed",
        blockerRefs: Array.from(
          { length: 33 },
          (_, index) => `blocker.public.run_terminal.${index}`,
        ),
      },
    })[1]
    expect(invalidRunTerminal).toMatchObject({
      kind: "run_terminal",
      terminalState: "failed",
      blockerRefs: ["blocker.pylon.fleet_run.evidence_cardinality_invalid"],
    })
  })

  test("replays one durably bound approval request after reporter restart", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fc3-approval-restart-"))
    const databasePath = join(root, "orchestration.sqlite")
    const workUnitRef = "unit.public.approval_restart"
    const workClaimRef = "work_claim.public.approval_restart"
    const assignmentRef = "assignment.public.approval_restart"
    const approvalRef = "approval.public.approval_restart"
    try {
      const firstDatabase = new NodeTestDatabase(databasePath)
      const firstStore = createPylonOrchestrationStore(firstDatabase)
      firstStore.createFleetRun({
        runRef,
        objective: "Retain one exact approval request across restart.",
        workSource: "fixture",
        targetConcurrency: 1,
        workerKind: "codex",
        state: "running",
        startedAt: firstNow,
        now: firstNow,
        authorityBinding: {
          schema: "openagents.pylon.fleet_run_authority_binding.v1",
          source: "sarah_authority",
          authorityFingerprint: "c".repeat(64),
          claimRef,
          pylonRef,
          targetPreference: "owner_local",
          phase: "accepted",
        },
      })
      expect(firstStore.tryClaimWorkUnit({
        claimRef: workClaimRef,
        workUnitRef,
        runRef,
        workerAccountRef: "codex-owner",
        marginalCostClass: "subscription",
        ttl: 60_000,
        now: firstNow,
      })).not.toBeNull()
      firstStore.updateWorkClaimState(workClaimRef, "in_progress", firstNow)
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
      await firstReporter.record({
        schema: "openagents.pylon.fleet_run_execution_event.v2",
        kind: "run_started",
        observedAt: firstNow.toISOString(),
      })
      await firstReporter.record({
        schema: "openagents.pylon.fleet_run_execution_event.v2",
        kind: "work_progress",
        observedAt: firstNow.toISOString(),
        unitRef: workUnitRef,
        workClaimRef,
        assignmentRef,
        workerKind: "codex",
        accountRefHash: hashPylonAccountRef("codex", "codex-owner"),
        marginalCostClass: "subscription",
        blockerRefs: [],
      })
      firstStore.bindFleetRunSteeringApproval({
        approvalRef,
        pylonRef,
        runRef,
        claimRef,
        workUnitRef,
        workClaimRef,
        assignmentRef,
        workerKind: "codex",
        workerRef: "worker.pylon.codex.approval-restart",
        accountRefHash: hashPylonAccountRef("codex", "codex-owner"),
        toolClass: "write_file",
        now: firstNow,
      })
      // Simulate process death after the binding commits but before the live
      // callback can append its approval event. The reopened reporter must
      // rebuild that exact event from SQLite custody.
      firstDatabase.close()

      const secondDatabase = new NodeTestDatabase(databasePath)
      const secondStore = createPylonOrchestrationStore(secondDatabase)
      const delivered: PylonFleetRunAnyExecutionBatch[] = []
      const secondReporter = openPylonFleetRunExecutionReporter({
        store: secondStore,
        pylonRef,
        runRef,
        now: () => new Date(firstNow.getTime() + 1_000),
        remote: {
          append: async ({ batch }) => {
            delivered.push(batch)
            const lastSequence = batch.events.at(-1)?.sequence ?? 0
            return {
              schema: "openagents.pylon.fleet_run_execution_ack.v1",
              runRef,
              claimRef,
              acceptedThroughSequence: lastSequence,
              storedEventCount: batch.events.length,
              duplicateEventCount: 0,
              execution: {
                state: "running",
                lastSequence,
                counters: {
                  workUnitsTotal: 1,
                  activeAssignments: 1,
                  acceptedAssignments: 0,
                  failedAssignments: 0,
                  staleAssignments: 0,
                },
                startedAt: firstNow.toISOString(),
                updatedAt: firstNow.toISOString(),
                closeouts: [],
              },
            }
          },
        },
      })
      await secondReporter.flush()
      await secondReporter.flush()
      await secondReporter.close()

      const approvalEvents = delivered.flatMap(batch => batch.events)
        .filter(event => event.kind === "approval_requested")
      expect(approvalEvents).toEqual([expect.objectContaining({
        kind: "approval_requested",
        unitRef: workUnitRef,
        workClaimRef,
        assignmentRef,
        workerKind: "codex",
        workerRef: "worker.pylon.codex.approval-restart",
        accountRefHash: hashPylonAccountRef("codex", "codex-owner"),
        approvalRef,
        toolClass: "write_file",
        blockerRefs: ["blocker.pylon.fleet_run.approval_required"],
      })])
      expect(secondStore.listFleetRunExecutionOutbox(runRef, {
        pendingOnly: false,
      }).filter(entry => JSON.parse(entry.eventJson).kind === "approval_requested")).toHaveLength(1)
      secondDatabase.close()
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("resumes one mixed Codex/Claude/Grok run without duplicate claims and closes with exact evidence truth", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fc2-execution-restart-"))
    const databasePath = join(root, "orchestration.sqlite")
    try {
      const firstDatabase = new NodeTestDatabase(databasePath)
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
      const delivered: PylonFleetRunAnyExecutionBatch[] = []
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
      const secondDatabase = new NodeTestDatabase(databasePath)
      const secondStore = createPylonOrchestrationStore(secondDatabase)
      expect(secondStore.listWorkClaims({ runRef }).map(claim =>
        claim.marginalCostClass
      ).sort()).toEqual(["free", "not_measured", "subscription"])
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
            marginalCostClass: "subscription" as const,
            verification: {
              truth: "passed" as const,
              verifierRef: `verifier.public.${workerKind}.restart_fixture`,
              evidenceRefs: [`verification.public.${workerKind}.restart_fixture`],
            },
            artifactRefs: [`artifact.public.${workerKind}.restart_fixture`],
            proofRefs: [`proof.public.${workerKind}.restart_fixture`],
            authorityReceiptRefs: [claimRef],
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
      expect(events.filter(event => event.kind === "work_terminal").map(event =>
        event.kind === "work_terminal"
          ? `${event.workerKind}:${event.marginalCostClass}`
          : null
      ).sort()).toEqual([
        "claude:free",
        "codex:subscription",
        "grok:not_measured",
      ])
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
