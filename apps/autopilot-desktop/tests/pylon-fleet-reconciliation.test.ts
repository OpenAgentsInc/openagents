import { describe, expect, test } from "bun:test"
import {
  classifyAssignmentLogText,
  emptyAssignmentLogSummary,
  reconcilePylonFleet,
  summarizeAssignmentLogTexts,
} from "../src/shared/pylon-fleet-reconciliation"

describe("pylon fleet reconciliation (#7593)", () => {
  test("classifies recent assignment logs into accepted and rejected closeouts", () => {
    expect(
      classifyAssignmentLogText(
        '{"event":"assignment_run.completed","status":"accepted"}',
      ),
    ).toBe("accepted")
    expect(
      classifyAssignmentLogText(
        '{"event":"assignment_run.completed","status":"rejected"}',
      ),
    ).toBe("rejected")
    expect(
      summarizeAssignmentLogTexts([
        '{"event":"assignment_run.accepted"}',
        '{"ok": false, "error":"dispatch_gate_blocked"}',
        "",
      ]),
    ).toMatchObject({
      empty: 1,
      failed_before_accept: 1,
      running_or_unknown: 1,
    })
  })

  test("detects marker without live Codex child as stale unknown", () => {
    const logs = emptyAssignmentLogSummary()
    logs.accepted = 4
    logs.rejected = 2
    const projection = reconcilePylonFleet({
      availableCodexSlots: 1,
      fetchedAt: "2026-06-29T15:05:00.000Z",
      khalaRequestWrappers: 1,
      liveCodexExecCount: 1,
      logs,
      markers: [
        {
          accountRefHash: "acctaaa111",
          assignmentRef: "assignment.alpha",
          leaseRef: "lease.alpha",
          refreshedAt: "2026-06-29T15:04:40.000Z",
          service: "codex",
        },
        {
          accountRefHash: "acctbbb222",
          assignmentRef: "assignment.bravo",
          leaseRef: "lease.bravo",
          refreshedAt: "2026-06-29T15:04:20.000Z",
          service: "codex",
        },
      ],
      presences: [
        {
          blockerRefs: [],
          lastHeartbeatAt: "2026-06-29T15:04:50.000Z",
          pylonRef: "pylon.local",
        },
      ],
      tokenFailureCount: 3,
    })

    expect(projection.counts).toMatchObject({
      accepted: 4,
      assigned: 2,
      executing: 1,
      khalaRequestWrappers: 1,
      pylons: 1,
      rejected: 2,
      stale: 1,
      tokenFailures: 3,
    })
    expect(projection.capacity.state).toBe("verified")
    expect(projection.assignments.map(row => row.state)).toEqual([
      "executing",
      "stale_unknown",
    ])
  })

  test("marks capacity stale when the last heartbeat is too old", () => {
    const projection = reconcilePylonFleet({
      fetchedAt: "2026-06-29T15:10:00.000Z",
      khalaRequestWrappers: 0,
      liveCodexExecCount: 0,
      logs: emptyAssignmentLogSummary(),
      markers: [],
      presences: [
        {
          blockerRefs: [],
          lastHeartbeatAt: "2026-06-29T15:00:00.000Z",
          pylonRef: "pylon.local",
        },
      ],
      tokenFailureCount: 0,
    })
    expect(projection.capacity.state).toBe("stale")
    expect(projection.capacity.ageSeconds).toBe(600)
  })
})
