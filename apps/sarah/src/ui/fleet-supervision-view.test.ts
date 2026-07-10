import { describe, expect, test } from "bun:test"
import { Schema } from "@effect-native/core/effect"
import { ViewSchema } from "@effect-native/core"
import {
  decodeFleetApprovalEntity,
  decodeFleetAssignmentEntity,
  decodeFleetAttemptEntity,
  decodeFleetRunEntity,
  decodeFleetWorkerEntity,
  decodeFleetWorkUnitEntity,
} from "@openagentsinc/khala-sync"

import { projectSarahFleetOwnerRun } from "../contracts/fleet-owner-projection.ts"
import {
  SarahFleetApprovalDecisionRequested,
  SarahFleetAuditToggled,
  SarahFleetEvidenceOpened,
  SarahFleetRunControlRequested,
  SarahFleetWorkUnitOpened,
  sarahFleetRunSupervisionView,
} from "./fleet-supervision-view.ts"
import { SARAH_OWNER_FLEET_INTERACTIVE } from "./owner-fleet-interaction.ts"

const interactive = { interactionMode: SARAH_OWNER_FLEET_INTERACTIVE } as const

const NOW = Date.parse("2026-07-09T20:00:00.000Z")
const CLAIM_REF = `claim.sarah_fleet_run.${"a".repeat(24)}`

const run = decodeFleetRunEntity({
  runId: "fleet.run.sarah.8639",
  status: "running",
  desiredSlots: 3,
  workerKind: "auto",
  startedAt: "2026-07-09T19:50:00.000Z",
  counters: {
    workUnitsTotal: 4,
    activeAssignments: 2,
    completedAssignments: 1,
    failedAssignments: 0,
    blockedAssignments: 1,
  },
  updatedAt: "2026-07-09T19:59:58.000Z",
})

const assignments = [
  decodeFleetAssignmentEntity({
    assignmentRef: "assignment.fc3.claude",
    issueRef: "#8633",
    status: "accepted",
    updatedAt: "2026-07-09T19:59:45.000Z",
  }),
  decodeFleetAssignmentEntity({
    assignmentRef: "assignment.fc3.codex",
    issueRef: "#8637",
    status: "running",
    updatedAt: "2026-07-09T19:59:30.000Z",
  }),
  decodeFleetAssignmentEntity({
    assignmentRef: "assignment.fc3.grok",
    issueRef: "#8639",
    status: "accepted_work",
    closeoutClass: "accepted_work",
    updatedAt: "2026-07-09T19:59:50.000Z",
  }),
]

const workers = [
  decodeFleetWorkerEntity({
    workerId: "worker.fc3.claude",
    phase: "blocked",
    harnessKind: "claude",
    assignmentRef: "assignment.fc3.claude",
    accountRefHash: "account.pylon.claude.22222222",
    lastProgressAt: "2026-07-09T19:59:45.000Z",
    updatedAt: "2026-07-09T19:59:45.000Z",
  }),
  decodeFleetWorkerEntity({
    workerId: "worker.fc3.codex",
    phase: "dispatched",
    harnessKind: "codex",
    assignmentRef: "assignment.fc3.codex",
    accountRefHash: "account.pylon.codex.11111111",
    lastProgressAt: "2026-07-09T19:59:30.000Z",
    updatedAt: "2026-07-09T19:59:30.000Z",
  }),
  decodeFleetWorkerEntity({
    workerId: "worker.fc3.grok",
    phase: "completed",
    harnessKind: "grok",
    assignmentRef: "assignment.fc3.grok",
    accountRefHash: "account.pylon.grok.33333333",
    lastProgressAt: "2026-07-09T19:59:50.000Z",
    updatedAt: "2026-07-09T19:59:50.000Z",
  }),
]

const approvals = [
  decodeFleetApprovalEntity({
    approvalRef: "approval.fc3.claude",
    status: "pending",
    workerId: "worker.fc3.claude",
    toolClass: "write_file",
    openedAt: "2026-07-09T19:59:45.000Z",
    updatedAt: "2026-07-09T19:59:45.000Z",
  }),
]

const attempts = [
  decodeFleetAttemptEntity({
    attemptRef: "work_claim.fc3.claude",
    workUnitRef: "unit.fc3.claude",
    intakeClaimRef: CLAIM_REF,
    pylonRef: "pylon-owner-1",
    workerKind: "claude",
    state: "running",
    progressClass: "blocked",
    assignmentRef: "assignment.fc3.claude",
    accountRefHash: `account.pylon.claude_agent.${"2".repeat(24)}`,
    capacityClass: "owner_local",
    marginalCostClass: "subscription",
    verification: { truth: "pending" },
    artifactRefs: [],
    proofRefs: [],
    authorityReceiptRefs: [],
    closeoutRef: null,
    usageEvidence: { truth: "pending" },
    blockerRefs: ["blocker.fc3.claude"],
    lastEventRef: `event.pylon.fleet_run.${"1".repeat(24)}`,
    startedAt: "2026-07-09T19:55:00.000Z",
    lastObservedAt: "2026-07-09T19:59:45.000Z",
    remoteObservedAt: "2026-07-09T19:59:44.000Z",
    terminalAt: null,
    updatedAt: "2026-07-09T19:59:45.000Z",
  }),
  decodeFleetAttemptEntity({
    attemptRef: "work_claim.fc3.codex",
    workUnitRef: "unit.fc3.codex",
    intakeClaimRef: CLAIM_REF,
    pylonRef: "pylon-owner-1",
    workerKind: "codex",
    state: "running",
    progressClass: "active",
    assignmentRef: "assignment.fc3.codex",
    accountRefHash: `account.pylon.codex.${"1".repeat(24)}`,
    capacityClass: "owner_local",
    marginalCostClass: "subscription",
    verification: { truth: "pending" },
    artifactRefs: [],
    proofRefs: [],
    authorityReceiptRefs: [],
    closeoutRef: null,
    usageEvidence: { truth: "pending" },
    blockerRefs: [],
    lastEventRef: `event.pylon.fleet_run.${"2".repeat(24)}`,
    startedAt: "2026-07-09T19:55:00.000Z",
    lastObservedAt: "2026-07-09T19:59:30.000Z",
    // A future remote clock cannot make the attempt fresh.
    remoteObservedAt: "2026-07-09T20:59:59.000Z",
    terminalAt: null,
    updatedAt: "2026-07-09T19:59:30.000Z",
  }),
  decodeFleetAttemptEntity({
    attemptRef: "work_claim.fc3.grok",
    workUnitRef: "unit.fc3.grok",
    intakeClaimRef: CLAIM_REF,
    pylonRef: "pylon-owner-1",
    workerKind: "grok",
    state: "succeeded",
    progressClass: "terminal",
    assignmentRef: "assignment.fc3.grok",
    accountRefHash: `account.pylon.grok.${"3".repeat(24)}`,
    capacityClass: "owner_local",
    marginalCostClass: "api_metered",
    verification: {
      truth: "passed",
      verifierRef: "verifier.fc3.grok",
      evidenceRefs: ["test.fc3.grok"],
    },
    artifactRefs: ["artifact.fc3.grok"],
    proofRefs: ["proof.fc3.grok"],
    authorityReceiptRefs: ["authority.fc3.grok"],
    closeoutRef: "closeout.fc3.grok",
    usageEvidence: {
      schema: "openagents.pylon.fleet_run_usage_evidence.v1",
      truth: "not_measured",
      harnessKind: "grok",
      evidenceRef: "evidence.fc3.grok",
      assignmentRef: "assignment.fc3.grok",
      receiptRef: "receipt.fc3.grok",
      tokenUsageRefs: [],
      caveatRefs: ["caveat.fc3.grok.not_measured"],
    },
    blockerRefs: [],
    lastEventRef: `event.pylon.fleet_run.${"3".repeat(24)}`,
    startedAt: "2026-07-09T19:55:00.000Z",
    lastObservedAt: "2026-07-09T19:59:50.000Z",
    remoteObservedAt: "2026-07-09T19:59:49.000Z",
    terminalAt: "2026-07-09T19:59:50.000Z",
    updatedAt: "2026-07-09T19:59:50.000Z",
  }),
]

const workUnits = [
  decodeFleetWorkUnitEntity({
    workUnitRef: "unit.fc3.claude",
    issueRef: "#8633",
    dependsOnRefs: [],
    state: "running",
    latestAttemptRef: "work_claim.fc3.claude",
    acceptedAttemptRef: null,
    updatedAt: "2026-07-09T19:59:45.000Z",
  }),
  decodeFleetWorkUnitEntity({
    workUnitRef: "unit.fc3.codex",
    issueRef: "#8637",
    dependsOnRefs: [],
    state: "running",
    latestAttemptRef: "work_claim.fc3.codex",
    acceptedAttemptRef: null,
    updatedAt: "2026-07-09T19:59:30.000Z",
  }),
  decodeFleetWorkUnitEntity({
    workUnitRef: "unit.fc3.grok",
    issueRef: "#8639",
    dependsOnRefs: [],
    state: "succeeded",
    latestAttemptRef: "work_claim.fc3.grok",
    acceptedAttemptRef: "work_claim.fc3.grok",
    updatedAt: "2026-07-09T19:59:50.000Z",
  }),
  decodeFleetWorkUnitEntity({
    workUnitRef: "unit.fc3.planned",
    issueRef: "#8640",
    dependsOnRefs: ["unit.fc3.grok"],
    state: "planned",
    latestAttemptRef: null,
    acceptedAttemptRef: null,
    updatedAt: "2026-07-09T19:59:55.000Z",
  }),
]

const projection = projectSarahFleetOwnerRun(
  {
    run,
    assignments,
    workers,
    workUnits,
    attempts,
    approvals,
    inboxFlags: [],
  },
  NOW,
)

type AnyNode = { readonly _tag?: string; readonly [key: string]: unknown }

const findByKey = (node: unknown, key: string): AnyNode | null => {
  if (node === null || typeof node !== "object") return null
  if (Array.isArray(node)) {
    for (const child of node) {
      const found = findByKey(child, key)
      if (found !== null) return found
    }
    return null
  }
  const record = node as AnyNode
  if (record.key === key) return record
  for (const value of Object.values(record)) {
    const found = findByKey(value, key)
    if (found !== null) return found
  }
  return null
}

const findByTag = (node: unknown, tag: string): AnyNode | null => {
  if (node === null || typeof node !== "object") return null
  if (Array.isArray(node)) {
    for (const child of node) {
      const found = findByTag(child, tag)
      if (found !== null) return found
    }
    return null
  }
  const record = node as AnyNode
  if (record._tag === tag) return record
  for (const value of Object.values(record)) {
    const found = findByTag(value, tag)
    if (found !== null) return found
  }
  return null
}

const payloadOf = (node: AnyNode | null): unknown =>
  ((node?.onPress as { payload?: { value?: unknown } } | undefined)?.payload)
    ?.value

describe("FC-3 Effect Native fleet supervision view", () => {
  test("renders a valid accessible run card and stable run/work-unit/worker canvas", () => {
    const view = sarahFleetRunSupervisionView(projection, interactive)
    expect(Schema.decodeUnknownSync(ViewSchema)(view)).toEqual(view)
    expect(view).toMatchObject({
      _tag: "Card",
      a11y: {
        role: "region",
        label: "Fleet run. Running. 4 work units. 1 pending approvals.",
      },
    })

    const graph = findByTag(view, "GraphFigure") as {
      nodes?: ReadonlyArray<{ id: string; label: string; status?: string }>
      edges?: ReadonlyArray<{ from: string; to: string }>
      a11y?: { label?: string }
    } | null
    expect(graph?.nodes?.map((node) => node.label)).toEqual([
      "Fleet run",
      "#8633",
      "#8637",
      "#8639",
      "#8640",
      "Claude worker",
      "Codex worker",
      "Grok worker",
    ])
    expect(graph?.nodes?.map((node) => node.id)).toEqual([
      "run:fleet.run.sarah.8639",
      "work:unit.fc3.claude",
      "work:unit.fc3.codex",
      "work:unit.fc3.grok",
      "work:unit.fc3.planned",
      "worker:worker.fc3.claude",
      "worker:worker.fc3.codex",
      "worker:worker.fc3.grok",
    ])
    expect(graph?.a11y?.label).toBe(
      "Fleet run map. 4 work units and 3 workers.",
    )
    expect(graph?.edges).toHaveLength(7)

    expect(
      findByKey(view, "fleet-supervision-unit.fc3.codex-progress"),
    ).toMatchObject({
      label: "Reconnecting · no fresh progress for 30s",
      tone: "warn",
      a11y: {
        label: "Progress: Reconnecting · no fresh progress for 30s",
      },
    })
    expect(
      findByKey(view, "fleet-supervision-unit.fc3.codex-heading")
        ?.direction,
    ).toEqual({ base: "column", sm: "row" })
    expect(
      findByKey(view, "fleet-supervision-unit.fc3.codex")?.a11y,
    ).toMatchObject({ role: "listitem" })
    expect(
      findByKey(view, "fleet-supervision-unit.fc3.claude-progress"),
    ).toMatchObject({ label: "Claude attempt blocked", tone: "warn" })
    expect(
      graph?.nodes?.find(
        (node) => node.id === "worker:worker.fc3.claude",
      ),
    ).toMatchObject({ status: "pending" })
  })

  test("renders only projection-supplied run controls with exact typed refs", () => {
    const view = sarahFleetRunSupervisionView(projection, interactive)
    const pause = findByKey(
      view,
      "fleet-supervision-fleet.run.sarah.8639-control-pause",
    )
    const payload = payloadOf(pause)
    expect(
      Schema.decodeUnknownSync(SarahFleetRunControlRequested.payloadSchema)(
        payload,
      ),
    ).toEqual({ runRef: "fleet.run.sarah.8639", action: "pause" })
    expect(() =>
      Schema.decodeUnknownSync(SarahFleetRunControlRequested.payloadSchema)({
        runRef: "/Users/alice/private/repo",
        action: "pause",
      }),
    ).toThrow()

    const resumeOnly = sarahFleetRunSupervisionView(
      {
        ...projection,
        run: {
          ...projection.run,
          status: "paused",
          availableControls: ["resume"],
        },
      },
      interactive,
    )
    expect(
      findByKey(
        resumeOnly,
        "fleet-supervision-fleet.run.sarah.8639-control-resume",
      ),
    ).not.toBeNull()
    expect(
      findByKey(
        resumeOnly,
        "fleet-supervision-fleet.run.sarah.8639-control-pause",
      ),
    ).toBeNull()

    const stopped = sarahFleetRunSupervisionView({
      ...projection,
      run: {
        ...projection.run,
        status: "stopped",
        availableControls: [],
      },
    })
    const stoppedGraph = findByTag(stopped, "GraphFigure") as {
      nodes?: ReadonlyArray<{ id: string; status?: string }>
    } | null
    expect(
      stoppedGraph?.nodes?.find(
        (node) => node.id === "run:fleet.run.sarah.8639",
      )?.status,
    ).toBe("idle")
    expect(
      findByKey(stopped, "fleet-supervision-fleet.run.sarah.8639-status"),
    ).toMatchObject({ label: "Stopped", tone: "neutral" })
  })

  test("never makes a worker-slot approval actionable across retry attempts", () => {
    const resolvedApproval = {
      ...projection.approvals[0]!,
      approvalRef: "approval.fc3.resolved",
      status: "allowed" as const,
      availableDecisions: [] as const,
      summary: "Approval allowed",
    }
    const view = sarahFleetRunSupervisionView(
      {
        ...projection,
        approvals: [...projection.approvals, resolvedApproval],
      },
      interactive,
    )

    const allow = findByKey(view, "fleet-supervision-approval.fc3.claude-allow")
    const deny = findByKey(view, "fleet-supervision-approval.fc3.claude-deny")
    expect(allow).toBeNull()
    expect(deny).toBeNull()
    expect(
      Schema.decodeUnknownSync(
        SarahFleetApprovalDecisionRequested.payloadSchema,
      )({
        runRef: "fleet.run.sarah.8639",
        approvalRef: "approval.fc3.exact_attempt_fixture",
        workUnitRef: "unit.fc3.claude",
        workerRef: "worker.fc3.claude",
        decision: "allow",
      }),
    ).toEqual({
      runRef: "fleet.run.sarah.8639",
      approvalRef: "approval.fc3.exact_attempt_fixture",
      workUnitRef: "unit.fc3.claude",
      workerRef: "worker.fc3.claude",
      decision: "allow",
    })
    expect(
      findByKey(
        view,
        "fleet-supervision-approval.fc3.claude-decisions-empty",
      )?.content,
    ).toBe("Decision options not reported.")
    expect(
      findByKey(view, "fleet-supervision-approval.fc3.resolved"),
    ).toBeNull()
    expect(() =>
      Schema.decodeUnknownSync(
        SarahFleetApprovalDecisionRequested.payloadSchema,
      )({
        runRef: "fleet.run.sarah.8639",
        workUnitRef: "#8633",
        workerRef: "worker.fc3.claude",
        decision: "allow",
      }),
    ).toThrow()
  })

  test("keeps audit refs secondary and never invents verification evidence", () => {
    const view = sarahFleetRunSupervisionView(projection, interactive)
    const codexAudit = findByKey(
      view,
      "fleet-supervision-unit.fc3.codex-audit",
    )
    expect(codexAudit).toMatchObject({
      _tag: "Accordion",
      expandedIds: [],
      items: [{ id: "references", header: "Audit references" }],
    })
    expect(
      findByKey(
        view,
        "fleet-supervision-unit.fc3.codex-verification-status",
      )?.label,
    ).toBe("Verification not reported")
    expect(
      findByKey(view, "fleet-supervision-unit.fc3.codex-verification"),
    ).toBeNull()

    const grokVerification = findByKey(
      view,
      "fleet-supervision-unit.fc3.grok-verification",
    )
    expect(
      Schema.decodeUnknownSync(SarahFleetEvidenceOpened.payloadSchema)(
        payloadOf(grokVerification),
      ),
    ).toEqual({
      runRef: "fleet.run.sarah.8639",
      workUnitRef: "unit.fc3.grok",
      assignmentRef: "assignment.fc3.grok",
      workerRef: "worker.fc3.grok",
      evidenceKind: "verification",
      evidenceRef: "verifier.fc3.grok",
    })
    expect(
      findByKey(
        view,
        "fleet-supervision-unit.fc3.grok-verification-status",
      ),
    ).toMatchObject({ label: "Verification passed", tone: "info" })

    const opened = findByKey(
      view,
      "fleet-supervision-unit.fc3.grok-open",
    )
    expect(
      Schema.decodeUnknownSync(SarahFleetWorkUnitOpened.payloadSchema)(
        payloadOf(opened),
      ),
    ).toEqual({
      runRef: "fleet.run.sarah.8639",
      workUnitRef: "unit.fc3.grok",
      assignmentRef: "assignment.fc3.grok",
      workerRef: "worker.fc3.grok",
    })

    const expandedView = sarahFleetRunSupervisionView(projection, {
      expandedAuditWorkUnitRefs: ["unit.fc3.grok"],
      interactionMode: SARAH_OWNER_FLEET_INTERACTIVE,
    })
    const expandedAudit = findByKey(
      expandedView,
      "fleet-supervision-unit.fc3.grok-audit",
    )
    expect(expandedAudit?.expandedIds).toEqual(["references"])
    expect(
      Schema.decodeUnknownSync(SarahFleetAuditToggled.payloadSchema)(
        ((expandedAudit?.onToggle as { payload?: { value?: unknown } })?.payload)
          ?.value,
      ),
    ).toEqual({
      runRef: "fleet.run.sarah.8639",
      workUnitRef: "unit.fc3.grok",
      assignmentRef: "assignment.fc3.grok",
      workerRef: "worker.fc3.grok",
    })
  })

  test("ignores raw/private extras and renders honest missing-worker states", () => {
    const unsafe = {
      ...projection,
      rawLogs: "PRIVATE RAW LOG SENTINEL",
      workUnits: projection.workUnits.map((workUnit) => ({
        ...workUnit,
        privatePrompt: "PRIVATE PROMPT SENTINEL",
      })),
      workers: projection.workers.map((worker) => ({
        ...worker,
        workspacePath: "/Users/alice/private/repo",
      })),
    } as typeof projection
    const serialized = JSON.stringify(sarahFleetRunSupervisionView(unsafe))
    expect(serialized).not.toContain("PRIVATE RAW LOG SENTINEL")
    expect(serialized).not.toContain("PRIVATE PROMPT SENTINEL")
    expect(serialized).not.toContain("/Users/alice/private/repo")

    const plannedWorkUnit = projection.workUnits.find(
      (workUnit) => workUnit.workUnitRef === "unit.fc3.planned",
    )!
    const unassigned = sarahFleetRunSupervisionView(
      {
        ...projection,
        workUnits: [plannedWorkUnit],
        workers: [],
        approvals: [],
      },
      interactive,
    )
    expect(
      findByKey(
        unassigned,
        "fleet-supervision-unit.fc3.planned-worker-name",
      )?.content,
    ).toBe("Worker not assigned")
    const plannedOpen = findByKey(
      unassigned,
      "fleet-supervision-unit.fc3.planned-open",
    )
    expect(plannedOpen).toBeNull()
    const plannedAudit = findByKey(
      unassigned,
      "fleet-supervision-unit.fc3.planned-audit",
    )
    expect(
      Schema.decodeUnknownSync(SarahFleetAuditToggled.payloadSchema)(
        ((plannedAudit?.onToggle as { payload?: { value?: unknown } } | undefined)
          ?.payload)?.value,
      ),
    ).toEqual({
      runRef: "fleet.run.sarah.8639",
      workUnitRef: "unit.fc3.planned",
      assignmentRef: null,
      workerRef: null,
    })
    const plannedGraph = findByTag(unassigned, "GraphFigure") as {
      nodes?: ReadonlyArray<{ id: string }>
    }
    expect(plannedGraph.nodes?.some((node) => node.id === "work:unit.fc3.planned")).toBe(
      true,
    )
    expect(JSON.stringify(unassigned)).not.toContain("fleet-supervision-null")
    expect(JSON.stringify(unassigned)).not.toContain("Assignment: null")
    expect(
      findByKey(
        unassigned,
        "fleet-supervision-fleet.run.sarah.8639-approvals-empty",
      )?.content,
    ).toBe("No pending approvals.")
  })
})
