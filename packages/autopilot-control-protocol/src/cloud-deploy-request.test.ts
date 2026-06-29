import { describe, expect, test } from "bun:test"

import { buildCloudDeployRequest } from "./cloud-deploy-request.js"

describe("cloud deploy request", () => {
  test("builds a cloud deploy request with artifact and repo refs", () => {
    expect(buildCloudDeployRequest({
      sessionRef: "session.desktop.0001",
      artifactRef: "artifact.bundle.0001",
      repoRef: "OpenAgentsInc/openagents#4932",
      clientRequestId: "client-request-0001",
    })).toEqual({
      ok: true,
      request: {
        verb: "cloud.deploy",
        sessionRef: "session.desktop.0001",
        artifactRef: "artifact.bundle.0001",
        repoRef: "OpenAgentsInc/openagents#4932",
        clientRequestId: "client-request-0001",
        idempotencyKey: "client-request-0001",
      },
    })
  })

  test("defaults missing optional refs to null", () => {
    expect(buildCloudDeployRequest({
      sessionRef: "session.mobile.0001",
      clientRequestId: "client-request-0002",
    })).toEqual({
      ok: true,
      request: {
        verb: "cloud.deploy",
        sessionRef: "session.mobile.0001",
        artifactRef: null,
        repoRef: null,
        clientRequestId: "client-request-0002",
        idempotencyKey: "client-request-0002",
      },
    })
  })

  test("uses clientRequestId as the idempotency key", () => {
    const result = buildCloudDeployRequest({
      sessionRef: "session.desktop.0002",
      clientRequestId: "retry-safe-request-0001",
    })

    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.request.idempotencyKey).toBe("retry-safe-request-0001")
    }
  })

  test("rejects an empty sessionRef", () => {
    expect(buildCloudDeployRequest({
      sessionRef: "",
      clientRequestId: "client-request-0003",
    })).toEqual({
      ok: false,
      error: "sessionRef is required",
    })
  })

  test("rejects a blank clientRequestId", () => {
    expect(buildCloudDeployRequest({
      sessionRef: "session.mobile.0002",
      clientRequestId: "   ",
    })).toEqual({
      ok: false,
      error: "clientRequestId is required",
    })
  })

  test("reports both missing required fields", () => {
    expect(buildCloudDeployRequest({
      sessionRef: " ",
      clientRequestId: "",
    })).toEqual({
      ok: false,
      error: "sessionRef and clientRequestId are required",
    })
  })
})
