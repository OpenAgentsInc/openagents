import { describe, expect, test } from "bun:test"

import { broadcastCancellation, broadcastResolution } from "../src/node/decision-broadcast"

const requestId = "decision-1"
const clientRefs = ["client-web", "client-desktop", "client-mobile"]

describe("decision broadcast", () => {
  test("resolution broadcasts resolved to resolver and resolved_elsewhere to other clients", () => {
    expect(
      broadcastResolution({
        requestId,
        resolvedVerb: "approve",
        resolvingClientRef: "client-desktop",
        clientRefs,
      }),
    ).toEqual([
      { clientRef: "client-web", message: "resolved_elsewhere" },
      { clientRef: "client-desktop", message: "resolved" },
      { clientRef: "client-mobile", message: "resolved_elsewhere" },
    ])
  })

  test("cancellation broadcasts cancelled to all clients", () => {
    expect(broadcastCancellation({ requestId, clientRefs })).toEqual([
      { clientRef: "client-web", message: "cancelled" },
      { clientRef: "client-desktop", message: "cancelled" },
      { clientRef: "client-mobile", message: "cancelled" },
    ])
  })
})
