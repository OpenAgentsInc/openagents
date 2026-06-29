import { describe, expect, test } from "bun:test"

import { pylonCountFromFleetStatus } from "../src/shared/pylon-status.js"

describe("openagents desktop pylon status", () => {
  test("counts connected pylons from the operator fleet spread", () => {
    expect(
      pylonCountFromFleetStatus({
        fleet: {
          spread: [
            { pylonRef: "pylon.one", heartbeatFresh: true },
            { pylonRef: "pylon.two", status: "online" },
            { pylonRef: "pylon.stale", heartbeatFresh: false },
          ],
        },
      }),
    ).toBe(2)
  })

  test("falls back to zero for missing fleet data", () => {
    expect(pylonCountFromFleetStatus({})).toBe(0)
  })
})
