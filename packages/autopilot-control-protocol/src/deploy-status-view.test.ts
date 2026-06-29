import { describe, expect, test } from "bun:test"

import { projectDeployStatus } from "./deploy-status-view.js"

describe("deploy status view projection", () => {
  test("projects a direct deployed payload", () => {
    expect(projectDeployStatus({
      state: "deployed",
      url: "https://example.workers.dev",
      deployedAt: "2026-06-13T12:34:56.000Z",
      message: "Live",
    })).toEqual({
      state: "deployed",
      url: "https://example.workers.dev",
      deployedAt: "2026-06-13T12:34:56.000Z",
      message: "Live",
    })
  })

  test("normalizes nested building aliases", () => {
    expect(projectDeployStatus({
      deployment: {
        deployment_state: "in_progress",
        preview_url: "https://preview.example.com",
        status_text: "Build step 2/4",
      },
    })).toEqual({
      state: "building",
      url: "https://preview.example.com",
      deployedAt: null,
      message: "Build step 2/4",
    })
  })

  test("normalizes queued aliases with default message", () => {
    expect(projectDeployStatus({
      deploy_status: {
        status: "pending",
      },
    })).toEqual({
      state: "queued",
      url: null,
      deployedAt: null,
      message: "Deployment queued",
    })
  })

  test("projects nested cloud result completion aliases", () => {
    expect(projectDeployStatus({
      result: {
        cloud: {
          status: "ignored",
        },
        deployment: {
          phase: "success",
          public_url: "https://cloud.openagents.com/app",
          completed_at: "2026-06-13T13:00:00.000Z",
        },
      },
    })).toEqual({
      state: "deployed",
      url: "https://cloud.openagents.com/app",
      deployedAt: "2026-06-13T13:00:00.000Z",
      message: "Deployment deployed",
    })
  })

  test("projects failed status and nested error message", () => {
    expect(projectDeployStatus({
      deploy: {
        state: "errored",
        error: {
          message: "Wrangler deploy failed",
        },
      },
    })).toEqual({
      state: "failed",
      url: null,
      deployedAt: null,
      message: "Wrangler deploy failed",
    })
  })

  test("returns a closed projection for bad input", () => {
    expect(projectDeployStatus(null)).toEqual({
      state: "unknown",
      url: null,
      deployedAt: null,
      message: "Deployment status unavailable",
    })
    expect(projectDeployStatus(["deployed"])).toEqual({
      state: "unknown",
      url: null,
      deployedAt: null,
      message: "Deployment status unavailable",
    })
  })

  test("rejects invalid field types and non-http urls", () => {
    expect(projectDeployStatus({
      state: "???",
      url: "not a url",
      deployedAt: 123,
    })).toEqual({
      state: "unknown",
      url: null,
      deployedAt: null,
      message: "Deployment status unknown",
    })
  })
})
