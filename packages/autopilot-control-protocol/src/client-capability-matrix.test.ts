import { describe, expect, test } from "bun:test"

import { clientCapabilities, type ClientSurface } from "./client-capability-matrix.js"

const readOnlyVerbs = [
  "bridge.clients.list",
  "session.list",
  "session.subscribe",
  "session.snapshot",
  "session.history",
  "artifact.read",
  "capability.list",
]

describe("client capability matrix", () => {
  test("mobile defaults to read-only bridge access", () => {
    expect(clientCapabilities("mobile")).toEqual({
      client: "mobile",
      verbs: readOnlyVerbs,
      canSpawn: false,
      canCancel: false,
      canApprove: false,
      readOnlyOnly: true,
    })
  })

  test("desktop defaults to read-only bridge access", () => {
    expect(clientCapabilities("desktop")).toEqual({
      client: "desktop",
      verbs: readOnlyVerbs,
      canSpawn: false,
      canCancel: false,
      canApprove: false,
      readOnlyOnly: true,
    })
  })

  test("web defaults to read-only bridge access", () => {
    expect(clientCapabilities("web")).toEqual({
      client: "web",
      verbs: readOnlyVerbs,
      canSpawn: false,
      canCancel: false,
      canApprove: false,
      readOnlyOnly: true,
    })
  })

  test("all client surfaces expose the same conservative read verbs", () => {
    const surfaces = ["mobile", "desktop", "web"] satisfies ClientSurface[]
    expect(surfaces.map((surface) => clientCapabilities(surface).verbs)).toEqual([
      readOnlyVerbs,
      readOnlyVerbs,
      readOnlyVerbs,
    ])
  })

  test("effectful bridge verbs are not advertised while read-only", () => {
    const verbs = clientCapabilities("mobile").verbs
    expect(verbs).not.toContain("session.spawn")
    expect(verbs).not.toContain("session.cancel")
    expect(verbs).not.toContain("decision.resolve")
    expect(verbs).not.toContain("turn.steer")
    expect(verbs).not.toContain("turn.interrupt")
  })

  test("returns a defensive verbs copy", () => {
    const first = clientCapabilities("desktop")
    first.verbs.push("session.cancel")

    expect(clientCapabilities("desktop").verbs).toEqual(readOnlyVerbs)
  })
})
