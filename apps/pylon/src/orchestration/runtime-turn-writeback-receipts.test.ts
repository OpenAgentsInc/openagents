import { describe, expect, test } from "bun:test"

import type { PublishAssignmentPullRequestResult } from "../codex-pr-publisher.js"
import {
  KHALA_AGENT_COMPUTER_WRITEBACK_INGEST_PATH,
  KHALA_AGENT_COMPUTER_WRITEBACK_SCHEMA_VERSION,
  recordRuntimeTurnWriteback,
  reportableWritebackOutcome,
} from "./runtime-turn-writeback-receipts.js"

const opened: PublishAssignmentPullRequestResult = {
  branch: "pylon/assignment-abcd",
  branchUrl: "https://github.com/octocat/repo/tree/pylon/assignment-abcd",
  changedCount: 3,
  prNumber: 7,
  prUrl: "https://github.com/octocat/repo/pull/7",
  reused: false,
  state: "opened",
}

const branchPushed: PublishAssignmentPullRequestResult = {
  branch: "pylon/assignment-abcd",
  branchUrl: "https://github.com/octocat/repo/tree/pylon/assignment-abcd",
  changedCount: 2,
  state: "branch_pushed",
}

const makeFetch = (
  response: Response,
): {
  fetchImpl: typeof globalThis.fetch
  requests: Array<{ body: Record<string, unknown>; headers: Headers; url: string }>
} => {
  const requests: Array<{
    body: Record<string, unknown>
    headers: Headers
    url: string
  }> = []
  const fetchImpl = Object.assign(
    async (
      url: Parameters<typeof globalThis.fetch>[0],
      init?: Parameters<typeof globalThis.fetch>[1],
    ) => {
      requests.push({
        body: JSON.parse(String(init?.body)),
        headers: new Headers(init?.headers),
        url: String(url),
      })
      return response.clone()
    },
    { preconnect: globalThis.fetch.preconnect },
  )
  return { fetchImpl, requests }
}

describe("reportableWritebackOutcome", () => {
  test("maps an opened PR to pull_request_opened", () => {
    expect(reportableWritebackOutcome(opened)).toEqual({
      branch: opened.branch,
      branchUrl: opened.branchUrl,
      changedFileCount: 3,
      pullRequestNumber: 7,
      pullRequestUrl: "https://github.com/octocat/repo/pull/7",
      status: "pull_request_opened",
    })
  })

  test("maps a reused PR to pull_request_reused", () => {
    const reused = reportableWritebackOutcome({ ...opened, reused: true })
    expect(reused?.status).toBe("pull_request_reused")
  })

  test("maps a branch push to branch_pushed with no PR fields", () => {
    expect(reportableWritebackOutcome(branchPushed)).toEqual({
      branch: branchPushed.branch,
      branchUrl: branchPushed.branchUrl,
      changedFileCount: 2,
      status: "branch_pushed",
    })
  })

  test("does not report no_change / skipped / failed outcomes", () => {
    expect(reportableWritebackOutcome({ state: "no_change" })).toBeUndefined()
    expect(
      reportableWritebackOutcome({ reasonRef: "r", state: "skipped" }),
    ).toBeUndefined()
    expect(
      reportableWritebackOutcome({ reasonRef: "push_failed", state: "failed" }),
    ).toBeUndefined()
  })
})

describe("recordRuntimeTurnWriteback", () => {
  test("posts an opened PR outcome to the writeback ingest route", async () => {
    const { fetchImpl, requests } = makeFetch(
      new Response(
        JSON.stringify({
          decision: "recorded",
          eventId: "event.private.agent_computer.writeback.1",
          ok: true,
          status: "pull_request_opened",
        }),
        { status: 200 },
      ),
    )
    const result = await recordRuntimeTurnWriteback({
      agentToken: "oa_agent_fixture",
      baseUrl: "https://openagents.com",
      fetchImpl,
      ownerUserId: "github:14167547",
      repositoryFullName: "octocat/repo",
      result: opened,
      turnId: "turn-1",
    })
    expect(result).toEqual({
      decision: "recorded",
      eventId: "event.private.agent_computer.writeback.1",
      ok: true,
      status: "pull_request_opened",
    })
    expect(requests).toHaveLength(1)
    expect(requests[0]!.url).toBe(
      `https://openagents.com${KHALA_AGENT_COMPUTER_WRITEBACK_INGEST_PATH}`,
    )
    expect(requests[0]!.headers.get("authorization")).toBe(
      "Bearer oa_agent_fixture",
    )
    expect(requests[0]!.body).toEqual({
      outcome: {
        branch: opened.branch,
        branchUrl: opened.branchUrl,
        changedFileCount: 3,
        pullRequestNumber: 7,
        pullRequestUrl: "https://github.com/octocat/repo/pull/7",
        repositoryFullName: "octocat/repo",
        status: "pull_request_opened",
      },
      ownerUserId: "github:14167547",
      schemaVersion: KHALA_AGENT_COMPUTER_WRITEBACK_SCHEMA_VERSION,
      turnId: "turn-1",
    })
  })

  test("does not post when the outcome is not reportable", async () => {
    const { fetchImpl, requests } = makeFetch(new Response("{}", { status: 200 }))
    const result = await recordRuntimeTurnWriteback({
      agentToken: "oa_agent_fixture",
      baseUrl: "https://openagents.com",
      fetchImpl,
      ownerUserId: "github:14167547",
      repositoryFullName: "octocat/repo",
      result: { state: "no_change" },
      turnId: "turn-1",
    })
    expect(result).toEqual({
      decision: "not_reportable",
      ok: true,
      state: "no_change",
    })
    expect(requests).toHaveLength(0)
  })

  test("surfaces a permission_blocked decision as a typed non-success", async () => {
    const { fetchImpl } = makeFetch(
      new Response(
        JSON.stringify({
          decision: "permission_blocked",
          message: "Connect a GitHub account with repository write access.",
          ok: false,
          reason: "github_write_connection_required",
          recordedEventId: "event.private.agent_computer.writeback.9",
        }),
        { status: 200 },
      ),
    )
    const result = await recordRuntimeTurnWriteback({
      agentToken: "oa_agent_fixture",
      baseUrl: "https://openagents.com",
      fetchImpl,
      ownerUserId: "github:14167547",
      repositoryFullName: "octocat/repo",
      result: branchPushed,
      turnId: "turn-1",
    })
    expect(result).toEqual({
      decision: "permission_blocked",
      ok: false,
      reason: "github_write_connection_required",
      recordedEventId: "event.private.agent_computer.writeback.9",
    })
  })

  test("maps a 401 to a typed unauthorized error", async () => {
    const { fetchImpl } = makeFetch(new Response("{}", { status: 401 }))
    const result = await recordRuntimeTurnWriteback({
      agentToken: "oa_agent_fixture",
      baseUrl: "https://openagents.com",
      fetchImpl,
      ownerUserId: "github:14167547",
      repositoryFullName: "octocat/repo",
      result: opened,
      turnId: "turn-1",
    })
    expect(result.ok).toBe(false)
    if (!result.ok && result.decision === "error") {
      expect(result.error).toBe("unauthorized")
      expect(result.status).toBe(401)
    }
  })
})
