import { describe, expect, test } from "bun:test"

import {
  activeCodingRunCountsFromAssignmentLeases,
  maxActiveCodingRunCounts,
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
})
