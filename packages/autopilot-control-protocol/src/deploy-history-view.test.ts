import { describe, expect, test } from "bun:test"

import {
  projectDeployHistory,
  type DeployHistoryView,
} from "./deploy-history-view.js"

describe("deploy history view projection", () => {
  test("projects and sorts a direct deploy history by at descending", () => {
    expect(projectDeployHistory({
      deployHistory: [
        {
          ref: "deploy.old",
          state: "deployed",
          at: "2026-06-13T12:00:00.000Z",
          url: "https://old.example.com",
        },
        {
          ref: "deploy.new",
          state: "building",
          at: "2026-06-13T12:05:00.000Z",
          url: "https://new.example.com",
        },
      ],
    })).toEqual({
      entries: [
        {
          ref: "deploy.new",
          state: "building",
          at: "2026-06-13T12:05:00.000Z",
          url: "https://new.example.com",
        },
        {
          ref: "deploy.old",
          state: "deployed",
          at: "2026-06-13T12:00:00.000Z",
          url: "https://old.example.com",
        },
      ],
      latest: {
        ref: "deploy.new",
        state: "building",
      },
      total: 2,
    } satisfies DeployHistoryView)
  })

  test("projects snake case deployment aliases", () => {
    expect(projectDeployHistory({
      deployment_history: [
        {
          deployment_id: "deployment.1",
          deployment_state: "success",
          deployed_at: "2026-06-13T13:00:00.000Z",
          public_url: "https://app.openagents.com",
        },
      ],
    })).toEqual({
      entries: [
        {
          ref: "deployment.1",
          state: "success",
          at: "2026-06-13T13:00:00.000Z",
          url: "https://app.openagents.com",
        },
      ],
      latest: {
        ref: "deployment.1",
        state: "success",
      },
      total: 1,
    } satisfies DeployHistoryView)
  })

  test("reads nested result cloud history records", () => {
    expect(projectDeployHistory({
      result: {
        cloud: {
          deployments: [
            {
              deployRef: "cloud.deploy.1",
              status: "failed",
              completedAt: "2026-06-13T14:00:00.000Z",
              previewUrl: "https://preview.openagents.com",
            },
          ],
        },
      },
    })).toEqual({
      entries: [
        {
          ref: "cloud.deploy.1",
          state: "failed",
          at: "2026-06-13T14:00:00.000Z",
          url: "https://preview.openagents.com",
        },
      ],
      latest: {
        ref: "cloud.deploy.1",
        state: "failed",
      },
      total: 1,
    } satisfies DeployHistoryView)
  })

  test("accepts a direct array payload", () => {
    expect(projectDeployHistory([
      {
        id: "array.deploy.1",
        phase: "ready",
        timestamp: "2026-06-13T15:00:00.000Z",
      },
    ])).toEqual({
      entries: [
        {
          ref: "array.deploy.1",
          state: "ready",
          at: "2026-06-13T15:00:00.000Z",
          url: null,
        },
      ],
      latest: {
        ref: "array.deploy.1",
        state: "ready",
      },
      total: 1,
    } satisfies DeployHistoryView)
  })

  test("skips malformed entries and rejects non-http urls", () => {
    expect(projectDeployHistory({
      history: [
        null,
        "bad",
        {
          ref: "missing-state",
          at: "2026-06-13T16:00:00.000Z",
        },
        {
          ref: "missing-at",
          state: "queued",
        },
        {
          ref: "deploy.valid",
          state: "queued",
          at: "2026-06-13T16:01:00.000Z",
          url: "ftp://example.com/app",
        },
      ],
    })).toEqual({
      entries: [
        {
          ref: "deploy.valid",
          state: "queued",
          at: "2026-06-13T16:01:00.000Z",
          url: null,
        },
      ],
      latest: {
        ref: "deploy.valid",
        state: "queued",
      },
      total: 1,
    } satisfies DeployHistoryView)
  })

  test("returns a closed empty projection for bad input", () => {
    expect(projectDeployHistory(undefined)).toEqual({
      entries: [],
      latest: null,
      total: 0,
    } satisfies DeployHistoryView)
    expect(projectDeployHistory({ history: "not-an-array" })).toEqual({
      entries: [],
      latest: null,
      total: 0,
    } satisfies DeployHistoryView)
  })
})
