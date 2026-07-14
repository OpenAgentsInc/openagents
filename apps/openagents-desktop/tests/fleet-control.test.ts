import { describe, expect, test } from "vite-plus/test"

import { decodeFleetStageRequest } from "../src/fleet-contract.ts"
import { submitFleetBrief } from "../src/fleet-control.ts"

describe("desktop Fleet brief contract", () => {
  test("accepts one bounded objective and rejects malformed or oversized input", () => {
    expect(decodeFleetStageRequest({ objective: "  Ship desktop chat  " })).toEqual({
      objective: "Ship desktop chat",
    })
    expect(decodeFleetStageRequest({ objective: "   " })).toBeNull()
    expect(decodeFleetStageRequest({ objective: "x".repeat(1_001) })).toBeNull()
    expect(decodeFleetStageRequest({ objective: 12 })).toBeNull()
  })
})

describe("desktop local Pylon dispatch", () => {
  test("submits the bounded brief through the host-held bearer and returns only public status", async () => {
    const result = await submitFleetBrief(
      { objective: "Ship the Fleet chat" },
      {
        readToken: () => "secret-never-returned",
        baseUrl: "http://127.0.0.1:4716",
        fetch: (async (request, init) => {
          expect(request).toBe("http://127.0.0.1:4716/command")
          expect(init?.headers).toEqual({
            authorization: "Bearer secret-never-returned",
            "content-type": "application/json",
          })
          expect(JSON.parse(String(init?.body))).toEqual({
            type: "intent.submit",
            title: "Desktop fleet deployment brief",
            body: "Ship the Fleet chat",
            submittedByClientRef: "openagents-desktop",
          })
          return new Response(JSON.stringify({ ok: true, result: { status: "received" } }))
        }) as typeof fetch,
      },
    )
    expect(result).toEqual({
      state: "accepted",
      message: "Local Pylon accepted the fleet brief. Watch for an authority-backed FleetRun receipt before treating work as deployed.",
      intentStatus: "received",
    })
  })

  test("fails closed when the host has no local Pylon token", async () => {
    const result = await submitFleetBrief({ objective: "Ship it" }, { readToken: () => null })
    expect(result.state).toBe("unavailable")
    expect(result.intentStatus).toBeNull()
  })
})
