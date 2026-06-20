import { describe, expect, test } from "bun:test"

import { buildShipStatusMessage } from "./ship-status-roundtrip.js"

describe("ship status round-trip message", () => {
  test("routes queued OTA status back to the originating client", () => {
    expect(buildShipStatusMessage({
      originClientRef: "client.mobile.0001",
      mode: "ota",
      state: "queued",
      version: "v1.2.3",
      url: null,
    })).toEqual({
      clientRef: "client.mobile.0001",
      kind: "ship_status",
      text: "OTA v1.2.3 ship queued.",
      terminal: false,
    })
  })

  test("marks building rebuild status as non-terminal", () => {
    expect(buildShipStatusMessage({
      originClientRef: "client.mobile.0002",
      mode: "rebuild",
      state: "building",
      version: "v2.0.0",
      url: null,
    })).toEqual({
      clientRef: "client.mobile.0002",
      kind: "ship_status",
      text: "Rebuild v2.0.0 ship building.",
      terminal: false,
    })
  })

  test("marks published status as terminal and includes the public URL", () => {
    expect(buildShipStatusMessage({
      originClientRef: "client.mobile.0003",
      mode: "ota",
      state: "published",
      version: "v2.1.0",
      url: "https://openagents.com/releases/v2.1.0",
    })).toEqual({
      clientRef: "client.mobile.0003",
      kind: "ship_status",
      text: "OTA v2.1.0 ship published. https://openagents.com/releases/v2.1.0",
      terminal: true,
    })
  })

  test("marks failed status as terminal", () => {
    expect(buildShipStatusMessage({
      originClientRef: "client.mobile.0004",
      mode: "rebuild",
      state: "failed",
      version: "v2.1.1",
      url: null,
    })).toEqual({
      clientRef: "client.mobile.0004",
      kind: "ship_status",
      text: "Rebuild v2.1.1 ship failed.",
      terminal: true,
    })
  })

  test("keeps text concise when version and URL are not known", () => {
    expect(buildShipStatusMessage({
      originClientRef: "client.mobile.0005",
      mode: "ota",
      state: "published",
      version: null,
      url: null,
    })).toEqual({
      clientRef: "client.mobile.0005",
      kind: "ship_status",
      text: "OTA ship published.",
      terminal: true,
    })
  })
})
