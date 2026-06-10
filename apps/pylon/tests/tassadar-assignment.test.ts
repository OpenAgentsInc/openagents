import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import { executeTassadarAssignment, tassadarPayloadFrom } from "../src/assignment"
import type { PylonAssignmentLease } from "../src/assignment"

const fixture = JSON.parse(
  readFileSync(
    new URL(
      "../../../packages/tassadar-executor/fixtures/tassadar-poc-loop-sum-v1.json",
      import.meta.url,
    ),
    "utf8",
  ),
)

const leaseWith = (tassadar: Record<string, unknown>): PylonAssignmentLease => ({
  schema: "openagents.pylon.assignment_lease.v0.3",
  assignmentRef: "assignment.tassadar_poc.test",
  leaseRef: "assignment.tassadar_poc.test",
  goal: "goal.tassadar_poc.test",
  paymentMode: "no-spend",
  capabilityRefs: [],
  codingAssignment: {
    kind: "tassadar_executor_trace_homework",
    tassadar,
  } as never,
  expiresAt: new Date(Date.now() + 60_000).toISOString(),
})

describe("tassadar executor-trace assignment gate", () => {
  test("executes the dispatched workload and reports the matching trace digest", async () => {
    const result = await executeTassadarAssignment(
      leaseWith({
        expectedTraceDigest: fixture.expectedTraceDigest,
        fixtureId: fixture.fixtureId,
        model: fixture.model,
        steps: fixture.steps,
      }),
      new Date("2026-06-10T00:00:00.000Z"),
    )
    expect(result).not.toBeNull()
    expect(result?.status).toBe("accepted")
    expect(result?.blockerRefs).toEqual([])
    expect(result?.artifactRefs[0]).toBe(
      `artifact.tassadar_poc.trace_digest.${fixture.expectedTraceDigest}`,
    )
  })

  test("flags a digest mismatch instead of accepting silently", async () => {
    const result = await executeTassadarAssignment(
      leaseWith({
        expectedTraceDigest: "not-the-real-digest",
        model: fixture.model,
        steps: fixture.steps,
      }),
      new Date("2026-06-10T00:00:00.000Z"),
    )
    expect(result?.status).toBe("rejected")
    expect(result?.blockerRefs).toEqual([
      "blocker.assignment.tassadar_trace_digest_mismatch",
    ])
  })

  test("ignores non-tassadar coding assignments", async () => {
    expect(tassadarPayloadFrom({ kind: "autopilot_coding" })).toBeNull()
    expect(tassadarPayloadFrom(null)).toBeNull()
  })

  test("typed execution refusals become rejected closeout evidence", async () => {
    const result = await executeTassadarAssignment(
      leaseWith({
        model: fixture.model,
        steps: [[0, 1, 2]],
      }),
      new Date("2026-06-10T00:00:00.000Z"),
    )
    expect(result?.status).toBe("rejected")
    expect(result?.blockerRefs).toEqual([
      "blocker.assignment.tassadar_execution_refused",
    ])
  })
})

describe("transit shape restoration", () => {
  test("initialChannelWrites restores to seed_writes before execution", async () => {
    const { seed_writes, ...rest } = fixture.model
    const result = await executeTassadarAssignment(
      leaseWith({
        expectedTraceDigest: fixture.expectedTraceDigest,
        model: { ...rest, initialChannelWrites: seed_writes },
        steps: fixture.steps,
      }),
      new Date("2026-06-10T00:00:00.000Z"),
    )
    expect(result?.status).toBe("accepted")
  })
})
