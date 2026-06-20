import { describe, expect, test } from "bun:test"

import { projectGrantedCapabilities } from "./bridge-capability-grant.js"

describe("bridge capability grant projection", () => {
  test("projects null claims to an empty read-only grant", () => {
    expect(projectGrantedCapabilities(null)).toEqual({
      verbs: [],
      readOnly: true,
      canSpawn: false,
      canApprove: false,
    })
  })

  test("ignores unknown stored capabilities defensively", () => {
    expect(projectGrantedCapabilities({ capabilities: ["observe_public", "root", "cancel"] }).verbs).toEqual([
      "bridge.pair.exchange",
      "bridge.revoke",
      "bridge.clients.list",
      "session.list",
      "session.subscribe",
      "session.snapshot",
      "session.history",
      "turn.interrupt",
      "session.cancel",
      "capability.list",
    ])
  })

  test("marks observe and artifact grants as read-only", () => {
    const projection = projectGrantedCapabilities({ capabilities: ["observe_public", "read_artifact"] })

    expect(projection.readOnly).toBe(true)
    expect(projection.verbs).toContain("session.list")
    expect(projection.verbs).toContain("artifact.read")
    expect(projection.verbs).not.toContain("turn.steer")
    expect(projection.verbs).not.toContain("decision.resolve")
  })

  test("projects approval capability into decision resolve and canApprove", () => {
    const projection = projectGrantedCapabilities({ capabilities: ["observe_private", "answer_decision"] })

    expect(projection.readOnly).toBe(false)
    expect(projection.canApprove).toBe(true)
    expect(projection.canSpawn).toBe(false)
    expect(projection.verbs).toContain("decision.resolve")
  })

  test("projects instruction capability into steering and canSpawn", () => {
    const projection = projectGrantedCapabilities({ capabilities: ["observe_private", "send_instruction"] })

    expect(projection.readOnly).toBe(false)
    expect(projection.canSpawn).toBe(true)
    expect(projection.canApprove).toBe(false)
    expect(projection.verbs).toContain("turn.steer")
    expect(projection.verbs).not.toContain("decision.resolve")
  })

  test("projects pause, resume, cancel, and interrupt from control capabilities", () => {
    const projection = projectGrantedCapabilities({ capabilities: ["observe_public", "cancel", "pause_resume"] })

    expect(projection.readOnly).toBe(false)
    expect(projection.verbs).toContain("turn.interrupt")
    expect(projection.verbs).toContain("session.cancel")
    expect(projection.verbs).toContain("session.pause")
    expect(projection.verbs).toContain("session.resume")
  })
})
