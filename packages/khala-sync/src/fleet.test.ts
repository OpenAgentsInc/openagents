import { describe, expect, test } from "bun:test"
import {
  canonicalJson,
  decodeFleetAccountEntity,
  decodeFleetAssignmentEntity,
  decodeFleetInboxFlagEntity,
  decodeFleetRunEntity,
  decodeFleetWorkerEntity,
  encodeFleetInboxFlagEntity,
  encodeFleetRunEntity,
  FLEET_ENTITY_TYPES,
} from "./index.js"

/**
 * Fleet entity contracts (KS-6.1). The load-bearing property here is
 * SPEC §7 invariant 9: these shapes must structurally REFUSE raw private
 * material — emails, filesystem paths, bearer strings — so a redaction bug
 * upstream fails to decode instead of replicating.
 */

const FORBIDDEN = /token|apiKey|authorization|\/Users\//i

const validRun = {
  counters: {
    activeAssignments: 2,
    blockedAssignments: 0,
    completedAssignments: 3,
    failedAssignments: 1,
    workUnitsTotal: 10,
  },
  desiredSlots: 4,
  runId: "fleet-run.pylon.supervisor.abc123",
  startedAt: "2026-07-04T15:00:00.000Z",
  status: "running",
  updatedAt: "2026-07-04T15:20:11.412Z",
  workerKind: "codex",
}

describe("fleet entity contracts", () => {
  test("entity type names are the closed public set", () => {
    expect([...FLEET_ENTITY_TYPES]).toEqual([
      "fleet_run",
      "fleet_worker",
      "fleet_assignment",
      "fleet_account",
      "fleet_inbox_flag",
      "fleet_approval",
      "fleet_steer",
    ])
  })

  test("fleet_run decodes and re-encodes canonically", () => {
    const entity = decodeFleetRunEntity(validRun)
    expect(entity.status).toBe("running")
    expect(canonicalJson(encodeFleetRunEntity(entity))).not.toMatch(FORBIDDEN)
  })

  test("fleet_run rejects out-of-range desiredSlots and unknown status", () => {
    expect(() =>
      decodeFleetRunEntity({ ...validRun, desiredSlots: -1 }),
    ).toThrow()
    expect(() =>
      decodeFleetRunEntity({ ...validRun, desiredSlots: 5000 }),
    ).toThrow()
    expect(() =>
      decodeFleetRunEntity({ ...validRun, status: "exploded" }),
    ).toThrow()
  })

  test("refs structurally refuse emails, paths, and whitespace", () => {
    // runId with a filesystem path
    expect(() =>
      decodeFleetRunEntity({ ...validRun, runId: "/Users/alice/run" }),
    ).toThrow()
    // workerId with an email
    expect(() =>
      decodeFleetWorkerEntity({
        phase: "idle",
        updatedAt: "2026-07-04T15:20:11.412Z",
        workerId: "alice@example.com",
      }),
    ).toThrow()
    // assignmentRef with whitespace
    expect(() =>
      decodeFleetAssignmentEntity({
        assignmentRef: "assignment with spaces",
        status: "offered",
        updatedAt: "2026-07-04T15:20:11.412Z",
      }),
    ).toThrow()
  })

  test("accountRefHash accepts ONLY the public hash-ref shape", () => {
    const base = {
      readiness: "ready",
      updatedAt: "2026-07-04T15:20:11.412Z",
    }
    expect(
      decodeFleetAccountEntity({
        ...base,
        accountRefHash: "account.pylon.codex.4e5f6a7b8c9d0e1f2a3b4c5d",
      }).readiness,
    ).toBe("ready")
    // raw email — refused
    expect(() =>
      decodeFleetAccountEntity({
        ...base,
        accountRefHash: "alice@example.com",
      }),
    ).toThrow()
    // non-hex tail (a raw handle, not a digest) — refused
    expect(() =>
      decodeFleetAccountEntity({
        ...base,
        accountRefHash: "account.pylon.codex.alice_handle",
      }),
    ).toThrow()
    // home path — refused
    expect(() =>
      decodeFleetAccountEntity({
        ...base,
        accountRefHash: "/Users/alice/.codex/auth.json",
      }),
    ).toThrow()
  })

  test("fleet_assignment issueRef accepts #N and owner/repo#N only", () => {
    const base = {
      assignmentRef: "assignment.public.issue8302.1",
      status: "offered",
      updatedAt: "2026-07-04T15:20:11.412Z",
    }
    expect(
      decodeFleetAssignmentEntity({ ...base, issueRef: "#8302" }).issueRef,
    ).toBe("#8302")
    expect(
      decodeFleetAssignmentEntity({
        ...base,
        issueRef: "OpenAgentsInc/openagents#8302",
      }).issueRef,
    ).toBe("OpenAgentsInc/openagents#8302")
    expect(() =>
      decodeFleetAssignmentEntity({
        ...base,
        issueRef: "https://github.com/OpenAgentsInc/openagents/issues/8302",
      }),
    ).toThrow()
  })

  test("fleet_worker accepts the operator-desired paused phase", () => {
    expect(
      decodeFleetWorkerEntity({
        phase: "paused",
        updatedAt: "2026-07-04T15:20:11.412Z",
        workerId: "dispatch-context.pylon.supervisor.9ab31c44",
      }).phase,
    ).toBe("paused")
  })

  test("fleet_inbox_flag decodes, and refs/kinds refuse private material", () => {
    const validFlag = {
      acknowledgedAt: "2026-07-04T15:21:00.000Z",
      flagRef: "inbox-flag.run_blocked.4f2a9c1d",
      kind: "run_blocked",
      openedAt: "2026-07-04T15:18:02.000Z",
      status: "acknowledged",
      updatedAt: "2026-07-04T15:21:00.000Z",
    }
    const entity = decodeFleetInboxFlagEntity(validFlag)
    expect(entity.status).toBe("acknowledged")
    expect(canonicalJson(encodeFleetInboxFlagEntity(entity))).not.toMatch(
      FORBIDDEN,
    )
    // flagRef with a filesystem path — refused
    expect(() =>
      decodeFleetInboxFlagEntity({
        ...validFlag,
        flagRef: "/Users/alice/inbox",
      }),
    ).toThrow()
    // flagRef with an email — refused
    expect(() =>
      decodeFleetInboxFlagEntity({
        ...validFlag,
        flagRef: "alice@example.com",
      }),
    ).toThrow()
    // kind must be a bounded lower_snake_case token
    expect(() =>
      decodeFleetInboxFlagEntity({ ...validFlag, kind: "Run Blocked!" }),
    ).toThrow()
    // status is a closed set
    expect(() =>
      decodeFleetInboxFlagEntity({ ...validFlag, status: "dismissed" }),
    ).toThrow()
  })

  test("timestamps must be ISO-8601 UTC", () => {
    expect(() =>
      decodeFleetRunEntity({ ...validRun, updatedAt: "yesterday" }),
    ).toThrow()
    expect(() =>
      decodeFleetRunEntity({
        ...validRun,
        updatedAt: "2026-07-04T15:20:11+02:00",
      }),
    ).toThrow()
  })
})
