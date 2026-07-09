import { describe, expect, test } from "bun:test"
import { Schema } from "@effect-native/core/effect"
import { ViewSchema } from "@effect-native/core"
import {
  decodeFleetApprovalEntity,
  decodeFleetAssignmentEntity,
  decodeFleetRunEntity,
  decodeFleetWorkerEntity,
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

const NOW = Date.parse("2026-07-09T20:00:00.000Z")

const run = decodeFleetRunEntity({
  runId: "fleet.run.sarah.8639",
  status: "running",
  desiredSlots: 3,
  workerKind: "auto",
  startedAt: "2026-07-09T19:50:00.000Z",
  counters: {
    workUnitsTotal: 3,
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

const projection = projectSarahFleetOwnerRun(
  {
    run,
    assignments,
    workers,
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
    const view = sarahFleetRunSupervisionView(projection)
    expect(Schema.decodeUnknownSync(ViewSchema)(view)).toEqual(view)
    expect(view).toMatchObject({
      _tag: "Card",
      a11y: {
        role: "region",
        label: "Fleet run. Running. 3 work units. 1 pending approvals.",
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
      "Claude worker",
      "Codex worker",
      "Grok worker",
    ])
    expect(graph?.nodes?.map((node) => node.id)).toEqual([
      "run:fleet.run.sarah.8639",
      "work:#8633:assignment.fc3.claude",
      "work:#8637:assignment.fc3.codex",
      "work:#8639:assignment.fc3.grok",
      "worker:worker.fc3.claude",
      "worker:worker.fc3.codex",
      "worker:worker.fc3.grok",
    ])
    expect(graph?.a11y?.label).toBe(
      "Fleet run map. 3 work units and 3 workers.",
    )
    expect(graph?.edges).toHaveLength(6)

    expect(
      findByKey(view, "fleet-supervision-assignment.fc3.codex-progress"),
    ).toMatchObject({
      label: "Reconnecting · no fresh progress for 30s",
      tone: "warn",
      a11y: {
        label: "Progress: Reconnecting · no fresh progress for 30s",
      },
    })
    expect(
      findByKey(view, "fleet-supervision-assignment.fc3.codex-heading")
        ?.direction,
    ).toEqual({ base: "column", sm: "row" })
    expect(
      findByKey(view, "fleet-supervision-assignment.fc3.codex")?.a11y,
    ).toMatchObject({ role: "listitem" })
    expect(
      findByKey(view, "fleet-supervision-assignment.fc3.claude-progress"),
    ).toMatchObject({ label: "Claude worker blocked", tone: "warn" })
    expect(
      graph?.nodes?.find(
        (node) => node.id === "worker:worker.fc3.claude",
      ),
    ).toMatchObject({ status: "pending" })
  })

  test("renders only projection-supplied run controls with exact typed refs", () => {
    const view = sarahFleetRunSupervisionView(projection)
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

    const resumeOnly = sarahFleetRunSupervisionView({
      ...projection,
      run: {
        ...projection.run,
        status: "paused",
        availableControls: ["resume"],
      },
    })
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

  test("renders pending decisions only and preserves approval target identity", () => {
    const resolvedApproval = {
      ...projection.approvals[0]!,
      approvalRef: "approval.fc3.resolved",
      status: "allowed" as const,
      availableDecisions: [] as const,
      summary: "Approval allowed",
    }
    const view = sarahFleetRunSupervisionView({
      ...projection,
      approvals: [...projection.approvals, resolvedApproval],
    })

    const allow = findByKey(view, "fleet-supervision-approval.fc3.claude-allow")
    const deny = findByKey(view, "fleet-supervision-approval.fc3.claude-deny")
    expect(
      Schema.decodeUnknownSync(
        SarahFleetApprovalDecisionRequested.payloadSchema,
      )(payloadOf(allow)),
    ).toEqual({
      runRef: "fleet.run.sarah.8639",
      approvalRef: "approval.fc3.claude",
      workUnitRef: "#8633",
      workerRef: "worker.fc3.claude",
      decision: "allow",
    })
    expect(payloadOf(deny)).toMatchObject({ decision: "deny" })
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
    const view = sarahFleetRunSupervisionView(projection)
    const codexAudit = findByKey(
      view,
      "fleet-supervision-assignment.fc3.codex-audit",
    )
    expect(codexAudit).toMatchObject({
      _tag: "Accordion",
      expandedIds: [],
      items: [{ id: "references", header: "Audit references" }],
    })
    expect(
      findByKey(
        view,
        "fleet-supervision-assignment.fc3.codex-verification-status",
      )?.label,
    ).toBe("Verification not reported")
    expect(
      findByKey(view, "fleet-supervision-assignment.fc3.codex-verification"),
    ).toBeNull()

    const grokVerification = findByKey(
      view,
      "fleet-supervision-assignment.fc3.grok-verification",
    )
    expect(
      Schema.decodeUnknownSync(SarahFleetEvidenceOpened.payloadSchema)(
        payloadOf(grokVerification),
      ),
    ).toEqual({
      runRef: "fleet.run.sarah.8639",
      workUnitRef: "#8639",
      assignmentRef: "assignment.fc3.grok",
      workerRef: "worker.fc3.grok",
      evidenceKind: "verification",
      evidenceRef: "assignment.fc3.grok",
    })
    expect(
      findByKey(
        view,
        "fleet-supervision-assignment.fc3.grok-verification-status",
      ),
    ).toMatchObject({ label: "Verification available", tone: "info" })

    const opened = findByKey(
      view,
      "fleet-supervision-assignment.fc3.grok-open",
    )
    expect(
      Schema.decodeUnknownSync(SarahFleetWorkUnitOpened.payloadSchema)(
        payloadOf(opened),
      ),
    ).toEqual({
      runRef: "fleet.run.sarah.8639",
      workUnitRef: "#8639",
      assignmentRef: "assignment.fc3.grok",
      workerRef: "worker.fc3.grok",
    })

    const expandedView = sarahFleetRunSupervisionView(projection, {
      expandedAuditWorkUnitRefs: ["#8639"],
    })
    const expandedAudit = findByKey(
      expandedView,
      "fleet-supervision-assignment.fc3.grok-audit",
    )
    expect(expandedAudit?.expandedIds).toEqual(["references"])
    expect(
      Schema.decodeUnknownSync(SarahFleetAuditToggled.payloadSchema)(
        ((expandedAudit?.onToggle as { payload?: { value?: unknown } })?.payload)
          ?.value,
      ),
    ).toEqual({
      runRef: "fleet.run.sarah.8639",
      workUnitRef: "#8639",
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

    const unassigned = sarahFleetRunSupervisionView({
      ...projection,
      workUnits: [
        {
          ...projection.workUnits[0]!,
          workerRef: null,
          progress: {
            status: "not_assigned",
            summary: "Work unit not assigned",
          },
          verification: {
            status: "not_reported",
            verificationRef: null,
            summary: "Verification not reported",
          },
          closeout: {
            status: "open",
            closeoutRef: null,
            closeoutClass: null,
            summary: "Closeout open",
          },
        },
      ],
      workers: [],
      approvals: [],
    })
    expect(
      findByKey(
        unassigned,
        "fleet-supervision-assignment.fc3.claude-worker-name",
      )?.content,
    ).toBe("Worker not assigned")
    expect(
      findByKey(
        unassigned,
        "fleet-supervision-fleet.run.sarah.8639-approvals-empty",
      )?.content,
    ).toBe("No pending approvals.")
  })
})
