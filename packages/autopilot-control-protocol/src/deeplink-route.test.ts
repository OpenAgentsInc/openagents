import { describe, expect, test } from "bun:test"

import { resolveDeepLinkRoute } from "./deeplink-route.js"

describe("autopilot deep link route resolution", () => {
  test("routes session deep links to SessionDetail with the session ref", () => {
    expect(resolveDeepLinkRoute("autopilot://session/local.session.0001")).toEqual({
      screen: "SessionDetail",
      params: { sessionRef: "local.session.0001" },
    })
  })

  test("routes node deep links to Nodes with the node ref", () => {
    expect(resolveDeepLinkRoute("autopilot://node/node.alpha")).toEqual({
      screen: "Nodes",
      params: { nodeRef: "node.alpha" },
    })
  })

  test("routes ship deep links to Nodes with the session ref", () => {
    expect(resolveDeepLinkRoute("autopilot://ship/cloud.session.0002")).toEqual({
      screen: "Nodes",
      params: { sessionRef: "cloud.session.0002" },
    })
  })

  test("uses parsed refs after query strings, fragments, and escaping are handled", () => {
    expect(resolveDeepLinkRoute("autopilot://session/local.session%2Fwith%20space?source=push#open")).toEqual({
      screen: "SessionDetail",
      params: { sessionRef: "local.session/with space" },
    })
  })

  test("returns no route for unsupported deep link kinds", () => {
    expect(resolveDeepLinkRoute("autopilot://agent/local.session.0001")).toEqual({
      screen: null,
      params: {},
    })
  })

  test("returns no route for missing refs", () => {
    expect(resolveDeepLinkRoute("autopilot://session/")).toEqual({
      screen: null,
      params: {},
    })
  })

  test("returns no route for non-string and non-autopilot inputs", () => {
    expect(resolveDeepLinkRoute(null)).toEqual({
      screen: null,
      params: {},
    })
    expect(resolveDeepLinkRoute("https://openagents.com/session/local.session.0001")).toEqual({
      screen: null,
      params: {},
    })
  })
})
