import { describe, expect, test } from "bun:test"

import { parseAutopilotDeepLink } from "./deeplink-parse.js"

describe("autopilot deep link parsing", () => {
  test("parses session deep links into session refs", () => {
    expect(parseAutopilotDeepLink("autopilot://session/local.session.0001")).toEqual({
      kind: "session",
      sessionRef: "local.session.0001",
      nodeRef: null,
    })
  })

  test("parses node deep links into node refs", () => {
    expect(parseAutopilotDeepLink("autopilot://node/node.alpha")).toEqual({
      kind: "node",
      sessionRef: null,
      nodeRef: "node.alpha",
    })
  })

  test("parses ship deep links into session refs", () => {
    expect(parseAutopilotDeepLink("autopilot://ship/cloud.session.0002")).toEqual({
      kind: "ship",
      sessionRef: "cloud.session.0002",
      nodeRef: null,
    })
  })

  test("ignores query strings and fragments after the ref", () => {
    expect(parseAutopilotDeepLink("autopilot://session/local.session.0003?source=push#open")).toEqual({
      kind: "session",
      sessionRef: "local.session.0003",
      nodeRef: null,
    })
  })

  test("decodes escaped refs without using the URL global", () => {
    expect(parseAutopilotDeepLink("autopilot://node/node%2Fwith%20space")).toEqual({
      kind: "node",
      sessionRef: null,
      nodeRef: "node/with space",
    })
  })

  test("returns unknown for unsupported routes", () => {
    expect(parseAutopilotDeepLink("autopilot://agent/local.session.0001")).toEqual({
      kind: "unknown",
      sessionRef: null,
      nodeRef: null,
    })
  })

  test("returns unknown for missing refs", () => {
    expect(parseAutopilotDeepLink("autopilot://session/")).toEqual({
      kind: "unknown",
      sessionRef: null,
      nodeRef: null,
    })
  })

  test("returns unknown for non-string and non-autopilot inputs", () => {
    expect(parseAutopilotDeepLink(null)).toEqual({
      kind: "unknown",
      sessionRef: null,
      nodeRef: null,
    })
    expect(parseAutopilotDeepLink("https://openagents.com/session/local.session.0001")).toEqual({
      kind: "unknown",
      sessionRef: null,
      nodeRef: null,
    })
  })
})
