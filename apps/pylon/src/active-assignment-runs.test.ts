import { describe, expect, test } from "bun:test"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  activeCodingRunCounts,
  activeCodingRunCountsByAccount,
  activeCodingRuns,
  activeCodingRunCountsFromAssignmentLeases,
  maxActiveCodingRunCounts,
  registerActiveCodingRun,
  UNKEYED_ACTIVE_RUN_ACCOUNT,
} from "./active-assignment-runs.js"

describe("active assignment run counts", () => {
  test("counts unexpired server Codex leases as busy capacity", () => {
    const now = new Date("2026-06-27T13:30:00.000Z")

    expect(
      activeCodingRunCountsFromAssignmentLeases(
        [
          {
            capabilityRefs: ["capability.pylon.local_codex"],
            expiresAt: "2026-06-27T13:31:00.000Z",
          },
          {
            capabilityRefs: ["capability.pylon.local_codex"],
            expiresAt: "2026-06-27T13:29:59.000Z",
          },
          {
            capabilityRefs: ["capability.pylon.local_claude_agent"],
            expiresAt: "2026-06-27T13:32:00.000Z",
          },
        ],
        { now },
      ),
    ).toEqual({ claude: 1, codex: 1 })
  })

  test("merges local and server counts conservatively without double counting", () => {
    expect(maxActiveCodingRunCounts({ codex: 1 }, { codex: 4, claude: 1 })).toEqual({
      claude: 1,
      codex: 4,
    })
  })

  test("prunes malformed local run files through the schema-backed JSON boundary", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pylon-active-runs-malformed-"))
    const paths = { activeAssignmentRuns: dir } as never
    try {
      await writeFile(join(dir, "bad.json"), "{")

      expect(await activeCodingRunCounts(paths)).toEqual({})
    } finally {
      await rm(dir, { force: true, recursive: true })
    }
  })

  test("lists fresh local run details sorted by start time", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pylon-active-runs-list-"))
    const paths = { activeAssignmentRuns: dir } as never
    try {
      await registerActiveCodingRun(paths, {
        assignmentRef: "assignment.public.khala_coding.b",
        leaseRef: "lease.b",
        now: new Date("2026-06-27T13:31:00.000Z"),
        service: "codex",
      })
      await registerActiveCodingRun(paths, {
        assignmentRef: "assignment.public.khala_coding.a",
        leaseRef: "lease.a",
        now: new Date("2026-06-27T13:30:00.000Z"),
        service: "codex",
      })

      expect(
        (await activeCodingRuns(paths, {
          now: new Date("2026-06-27T13:31:30.000Z"),
          ttlMs: 120_000,
        })).map(run => run.assignmentRef),
      ).toEqual([
        "assignment.public.khala_coding.a",
        "assignment.public.khala_coding.b",
      ])
    } finally {
      await rm(dir, { force: true, recursive: true })
    }
  })
})

describe("#6354 per-account active run counts", () => {
  test("buckets fresh local runs by account-ref hash", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pylon-active-runs-"))
    const paths = { activeAssignmentRuns: dir } as never
    try {
      await registerActiveCodingRun(paths, {
        accountRefHash: "account.pylon.codex.aaaa",
        assignmentRef: "assignment.public.khala_coding.a1",
        leaseRef: "lease.a1",
        service: "codex",
      })
      await registerActiveCodingRun(paths, {
        accountRefHash: "account.pylon.codex.aaaa",
        assignmentRef: "assignment.public.khala_coding.a2",
        leaseRef: "lease.a2",
        service: "codex",
      })
      await registerActiveCodingRun(paths, {
        accountRefHash: "account.pylon.codex.bbbb",
        assignmentRef: "assignment.public.khala_coding.b1",
        leaseRef: "lease.b1",
        service: "codex",
      })
      // No account hash -> unkeyed bucket.
      await registerActiveCodingRun(paths, {
        assignmentRef: "assignment.public.khala_coding.u1",
        leaseRef: "lease.u1",
        service: "codex",
      })

      const counts = await activeCodingRunCountsByAccount(paths)
      expect(counts.codex).toEqual({
        "account.pylon.codex.aaaa": 2,
        "account.pylon.codex.bbbb": 1,
        [UNKEYED_ACTIVE_RUN_ACCOUNT]: 1,
      })
    } finally {
      await rm(dir, { force: true, recursive: true })
    }
  })
})
