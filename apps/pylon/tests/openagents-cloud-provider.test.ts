import { describe, expect, test } from "bun:test"

import { buildCloudSessionRequest } from "../src/openagents-cloud-provider"

describe("buildCloudSessionRequest", () => {
  test("builds an OpenAgents Cloud request", () => {
    const request = buildCloudSessionRequest({
      objective: "ship the cloud execution provider",
      verify: ["bun test"],
      workspaceRef: "workspace.pylon",
    })

    expect(request.providerKind).toBe("openagents_cloud")
  })

  test("uses a hashed objective ref without raw objective text", () => {
    const objective = "do not embed this raw objective"
    const request = buildCloudSessionRequest({
      objective,
      verify: [],
      workspaceRef: "workspace.pylon",
    })

    expect(request.objectiveRef.startsWith("objective.")).toBe(true)
    expect(request.objectiveRef).not.toContain(objective)
  })

  test("defaults timeout to 600 seconds", () => {
    const request = buildCloudSessionRequest({
      objective: "use default timeout",
      verify: [],
      workspaceRef: "workspace.pylon",
    })

    expect(request.timeoutSeconds).toBe(600)
  })

  test("clamps timeout to the supported range", () => {
    expect(
      buildCloudSessionRequest({
        objective: "clamp low timeout",
        verify: [],
        workspaceRef: "workspace.pylon",
        timeoutSeconds: 0,
      }).timeoutSeconds,
    ).toBe(1)

    expect(
      buildCloudSessionRequest({
        objective: "clamp high timeout",
        verify: [],
        workspaceRef: "workspace.pylon",
        timeoutSeconds: 1201,
      }).timeoutSeconds,
    ).toBe(1200)
  })
})
