import { describe, expect, test } from "bun:test"
import {
  QA_ASYNC_PLACEMENT_CONTRACT_VERSION,
  buildQaAsyncPlacementAssignment,
  missingQaAsyncConfig,
  normalizeQaAsyncControlEndpoint,
  parseGitHubRepository,
  qaAsyncRunId,
  triggerQaAsyncGce,
  type QaAsyncConfig,
} from "./qa-async-gce-trigger"

const fixtureConfig = (): QaAsyncConfig => ({
  authGrantRef: "codex-auth-grant_qa",
  controlToken: "control-token-secret",
  controlUrl: "https://control.openagents.internal",
  lane: "cloud-gcp",
  metadata: {
    branch: "main",
    changedFiles: ["apps/qa-runner/src/byo.ts", "apps/openagents.com/apps/web/src/page.tsx"],
    commitSha: "d91ea3e3792ce4b2ae54e8b45f7449d45e3ec91b",
    repository: "OpenAgentsInc/openagents",
  },
  ownerRef: "owner://openagents/internal-qa",
  prNumber: "6224",
  proBaseUrl: "https://openagents.com",
  providerAccountRef: "provider-account_qa",
  targetUrl: "https://openagents.com",
  timeoutMs: 10_000,
})

describe("qa async GCE trigger config", () => {
  test("requires the owner-armed control endpoint and refs", () => {
    expect(missingQaAsyncConfig({})).toEqual([
      "OA_QA_ASYNC_CONTROL_URL",
      "OA_QA_ASYNC_CONTROL_TOKEN",
      "OA_QA_ASYNC_PROVIDER_ACCOUNT_REF",
      "OA_QA_ASYNC_AUTH_GRANT_REF",
    ])
  })

  test("parses GitHub repository remotes", () => {
    expect(parseGitHubRepository("https://github.com/OpenAgentsInc/openagents.git")).toBe(
      "OpenAgentsInc/openagents",
    )
    expect(parseGitHubRepository("git@github.com:OpenAgentsInc/openagents.git")).toBe(
      "OpenAgentsInc/openagents",
    )
    expect(parseGitHubRepository("OpenAgentsInc/openagents")).toBe("OpenAgentsInc/openagents")
  })

  test("normalizes base URLs to the placement start route", () => {
    expect(normalizeQaAsyncControlEndpoint("https://control.example")).toBe(
      "https://control.example/v1/placement/start",
    )
    expect(normalizeQaAsyncControlEndpoint("https://control.example/v1/placement")).toBe(
      "https://control.example/v1/placement/start",
    )
    expect(normalizeQaAsyncControlEndpoint("https://control.example/api")).toBe(
      "https://control.example/api/v1/placement/start",
    )
  })
})

describe("qa async GCE placement payload", () => {
  test("builds a GCE placement assignment for the exact pushed commit", () => {
    const config = fixtureConfig()
    const assignment = buildQaAsyncPlacementAssignment(config, 1)

    expect(assignment).toMatchObject({
      auth_grant_ref: "codex-auth-grant_qa",
      contract_version: QA_ASYNC_PLACEMENT_CONTRACT_VERSION,
      created_at_ms: 1,
      lane: "cloud-gcp",
      owner_ref: "owner://openagents/internal-qa",
      provider_account_ref: "provider-account_qa",
      repository: "OpenAgentsInc/openagents@d91ea3e3792ce4b2ae54e8b45f7449d45e3ec91b",
      run_id: "qa-push-main-d91ea3e3792c",
      sandbox_mode: "danger_full_access",
      wallet_authority: false,
    })
    expect(assignment.goal).toContain("openagents/khala")
    expect(assignment.goal).toContain("apps/qa-runner/src/pr-comment-run.ts")
    expect(assignment.goal).toContain("https://openagents.com/trace/{uuid}")
    expect(assignment.goal).toContain("apps/qa-runner/src/byo.ts")
  })

  test("uses a stable idempotent run id per branch and commit", () => {
    expect(qaAsyncRunId(fixtureConfig().metadata)).toBe("qa-push-main-d91ea3e3792c")
  })
})

describe("qa async GCE trigger request", () => {
  test("posts the placement assignment with bearer auth but no token in the body", async () => {
    const requests: Array<Readonly<{ body: string; headers: Headers; url: string }>> = []
    const fetcher: typeof fetch = async (
      input: RequestInfo | URL,
      init?: RequestInit,
    ): Promise<Response> => {
      requests.push({
        body: String(init?.body ?? ""),
        headers: new Headers(init?.headers),
        url: String(input),
      })
      return Response.json({
        externalRunId: "shc-codex:oa-gce-ephemeral-qa:qa-push-main-d91ea3e3792c",
        status: "queued",
      })
    }

    const verdict = await triggerQaAsyncGce(fixtureConfig(), fetcher)

    expect(verdict.status).toBe("queued")
    expect(requests[0]?.url).toBe("https://control.openagents.internal/v1/placement/start")
    expect(requests[0]?.headers.get("authorization")).toBe("Bearer control-token-secret")
    expect(requests[0]?.headers.get("content-type")).toBe("application/json")
    expect(requests[0]?.body).not.toContain("control-token-secret")
    expect(JSON.parse(requests[0]?.body ?? "{}")).toMatchObject({
      contract_version: QA_ASYNC_PLACEMENT_CONTRACT_VERSION,
      lane: "cloud-gcp",
      wallet_authority: false,
    })
  })

  test("returns a loud failure verdict for rejected control requests", async () => {
    const fetcher: typeof fetch = async (): Promise<Response> =>
      Response.json({ error: "no capacity" }, { status: 503 })

    const verdict = await triggerQaAsyncGce(fixtureConfig(), fetcher)

    expect(verdict.status).toBe("failed")
    expect(verdict.exitCode).toBe(1)
    expect(verdict.reason).toContain("HTTP 503")
    expect(verdict.reason).toContain("no capacity")
  })
})
