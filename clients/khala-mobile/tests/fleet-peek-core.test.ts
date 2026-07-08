import { decodeKhalaFleetIntent } from "@openagentsinc/khala-fleet-intents"
import {
  decodeFleetApprovalEntity,
  decodeFleetRunEntity,
  decodeFleetSteerEntity,
  decodeFleetWorkerEntity,
} from "@openagentsinc/khala-sync"
import { describe, expect, test } from "bun:test"

import {
  deriveFleetPeekViewModel,
  harnessOfWorker,
  makeApprovalDecisionIntent,
  makeRunControlIntent,
  makeSteerMessageIntent,
  type FleetIntentIds,
} from "../src/sync/fleet-peek-core"

const nowIso = "2026-07-08T18:00:00.000Z"

const worker = (input: {
  id: string
  phase: string
  harnessKind?: string
  accountRefHash?: string
}) =>
  decodeFleetWorkerEntity({
    ...(input.harnessKind === undefined ? {} : { harnessKind: input.harnessKind }),
    ...(input.accountRefHash === undefined
      ? {}
      : { accountRefHash: input.accountRefHash }),
    phase: input.phase,
    updatedAt: nowIso,
    workerId: input.id,
  })

const approval = (input: { approvalRef: string; status: string; workerId?: string }) =>
  decodeFleetApprovalEntity({
    approvalRef: input.approvalRef,
    status: input.status,
    updatedAt: nowIso,
    ...(input.workerId === undefined ? {} : { workerId: input.workerId }),
  })

const ids = (n: string): FleetIntentIds => ({
  createdAt: nowIso,
  idempotencyKey: `idem.${n}`,
  intentId: `intent.${n}`,
})

describe("harnessOfWorker", () => {
  test("prefers the explicit harnessKind", () => {
    expect(
      harnessOfWorker(worker({ harnessKind: "grok", id: "w1", phase: "dispatched" })),
    ).toBe("grok")
  })

  test("falls back to the account lane when harnessKind is absent", () => {
    expect(
      harnessOfWorker(
        worker({
          accountRefHash: "account.pylon.claude.aa11bb22cc33dd44",
          id: "w1",
          phase: "dispatched",
        }),
      ),
    ).toBe("claude")
  })

  test("is `unknown` when neither signal is present", () => {
    expect(harnessOfWorker(worker({ id: "w1", phase: "idle" }))).toBe("unknown")
  })
})

describe("deriveFleetPeekViewModel", () => {
  const run = decodeFleetRunEntity({
    counters: {
      activeAssignments: 3,
      blockedAssignments: 1,
      completedAssignments: 4,
      failedAssignments: 0,
      workUnitsTotal: 8,
    },
    desiredSlots: 3,
    runId: "fleet.mh6.vm",
    startedAt: nowIso,
    status: "running",
    updatedAt: nowIso,
    workerKind: "auto",
  })

  test("groups per-harness worker cards and surfaces pending approvals", () => {
    const vm = deriveFleetPeekViewModel({
      approvals: [
        approval({ approvalRef: "approval.1", status: "pending", workerId: "w.claude" }),
        approval({ approvalRef: "approval.2", status: "allowed" }),
      ],
      run,
      steers: [],
      workers: [
        worker({ harnessKind: "codex", id: "w.codex", phase: "dispatched" }),
        worker({ harnessKind: "claude", id: "w.claude", phase: "blocked" }),
        worker({ harnessKind: "grok", id: "w.grok", phase: "dispatched" }),
      ],
    })
    expect(vm.runStatus).toBe("running")
    expect(vm.workers).toHaveLength(3)
    expect(vm.harnessCounts).toEqual({ claude: 1, codex: 1, grok: 1, unknown: 0 })
    expect(vm.pendingApprovals.map((a) => a.approvalRef)).toEqual(["approval.1"])
    expect(vm.resolvedApprovals.map((a) => a.approvalRef)).toEqual(["approval.2"])
    // running run: pause / drain / stop are the sensible controls
    expect(vm.availableRunControls).toEqual(["pause", "drain", "stop"])
  })

  test("a paused run offers resume; a missing run is `unknown` with all controls", () => {
    const paused = deriveFleetPeekViewModel({
      approvals: [],
      run: decodeFleetRunEntity({ ...run, status: "paused" }),
      steers: [],
      workers: [],
    })
    expect(paused.availableRunControls).toEqual(["resume", "drain", "stop"])

    const none = deriveFleetPeekViewModel({
      approvals: [],
      run: null,
      steers: [],
      workers: [],
    })
    expect(none.runStatus).toBe("unknown")
    expect(none.desiredSlots).toBe(0)
    expect(none.counters).toBeNull()
  })

  test("steer receipts sort newest-first and stay body-free", () => {
    const older = decodeFleetSteerEntity({
      bodyCarrier: "inline",
      createdAt: "2026-07-08T17:00:00.000Z",
      steerRef: "steer.old",
      updatedAt: "2026-07-08T17:00:00.000Z",
    })
    const newer = decodeFleetSteerEntity({
      bodyCarrier: "ref",
      createdAt: nowIso,
      steerRef: "steer.new",
      updatedAt: nowIso,
    })
    const vm = deriveFleetPeekViewModel({
      approvals: [],
      run,
      steers: [older, newer],
      workers: [],
    })
    expect(vm.recentSteers.map((s) => s.steerRef)).toEqual(["steer.new", "steer.old"])
  })
})

describe("typed intent factories build wire-valid KhalaFleetIntent values", () => {
  test("run control", () => {
    const intent = makeRunControlIntent({
      action: "pause",
      ids: ids("pause"),
      runRef: "fleet.mh6.vm",
    })
    // round-trips through the MH-0 decoder — proves the phone builds exactly
    // the value the server mutator consumes.
    const decoded = decodeKhalaFleetIntent(intent)
    expect(decoded.kind).toBe("fleet_run_control")
    if (decoded.kind === "fleet_run_control") {
      expect(decoded.action).toBe("pause")
      expect(decoded.runRef).toBe("fleet.mh6.vm")
    }
    expect(decoded.origin.surface).toBe("mobile")
  })

  test("approval decision", () => {
    const decoded = decodeKhalaFleetIntent(
      makeApprovalDecisionIntent({
        approvalRef: "approval.1",
        decision: "allow",
        ids: ids("approve"),
        runRef: "fleet.mh6.vm",
      }),
    )
    expect(decoded.kind).toBe("approval_decision")
    if (decoded.kind === "approval_decision") {
      expect(decoded.approvalRef).toBe("approval.1")
      expect(decoded.decision).toBe("allow")
    }
  })

  test("steer message carries the body and optional target", () => {
    const decoded = decodeKhalaFleetIntent(
      makeSteerMessageIntent({
        body: "focus on the failing test first",
        ids: ids("steer"),
        runRef: "fleet.mh6.vm",
        targetRef: "worker.claude.1",
      }),
    )
    expect(decoded.kind).toBe("steer_message")
    if (decoded.kind === "steer_message") {
      expect(decoded.body).toBe("focus on the failing test first")
      expect(decoded.targetRef).toBe("worker.claude.1")
    }
  })
})
