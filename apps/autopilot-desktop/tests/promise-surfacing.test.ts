import { describe, expect, test } from "bun:test"
import {
  resolvePromiseSurfacingSettings,
  surfacePromiseGapReport,
} from "../src/bun/promise-surfacing"
import {
  buildPromiseSurfacingDraft,
  validatePromiseSurfacingInput,
  type PromiseSurfacingInput,
} from "../src/shared/promise-surfacing"

const report: PromiseSurfacingInput = {
  promiseId: "autopilot.builtin_compute_agent.v1",
  surface: "Autopilot",
  claimText: "Click Go online without a user API key.",
  expectedBehavior: "A hosted agent starts.",
  observedBehavior: "The app reports hosted compute is unconfigured.",
  evidenceOrSteps: "Open Agent pane, click Go online, observe blocker ref.",
  environment: "macOS arm64 rc tester",
  impact: "Normal users cannot reach a working agent.",
  suggestedState: "yellow",
}

const ledger = {
  registryVersion: "2026-06-15.4",
  generatedAt: "2026-06-15T00:00:00.000Z",
  promises: [
    {
      promiseId: report.promiseId,
      state: "green",
      safeCopy: "Built-in agent is available to eligible users.",
      evidenceRefs: ["apps/autopilot-desktop/src/shared/builtin-agent.ts"],
      blockerRefs: [],
    },
  ],
}

describe("promise surfacing (#5065)", () => {
  test("validates exact report fields before posting", () => {
    const invalid = validatePromiseSurfacingInput({
      ...report,
      promiseId: "",
      evidenceOrSteps: "",
    })
    expect(invalid.ok).toBe(false)
    expect(invalid.errors).toContain("promiseId is required")
    expect(invalid.errors).toContain("evidenceOrSteps is required")
  })

  test("draft includes ledger verdict and forum posture", () => {
    const draft = buildPromiseSurfacingDraft({
      report,
      ledger,
      relatedTopics: [
        {
          topicId: "topic.promise.1",
          title: `[Promise Report] ${report.promiseId}`,
          url: "https://openagents.test/forum/t/topic.promise.1",
        },
      ],
      observedAt: "2026-06-15T01:00:00.000Z",
    })

    expect(draft.title).toBe(`[Promise Report] ${report.promiseId}`)
    expect(draft.ledgerVerdict).toBe("ledger_claims_fixed_report_new_mismatch")
    expect(draft.bodyText).toContain("Surface only. Do not ship code")
    expect(draft.bodyText).toContain("relatedExactPromiseForumTopics: 1")
  })

  test("drafts but does not post when no registered-agent token is configured", async () => {
    const calls: string[] = []
    const fetchImpl = async (url: string | URL | Request) => {
      const href = String(url)
      calls.push(href)
      if (href.endsWith("/api/public/product-promises")) {
        return Response.json(ledger)
      }
      if (href.endsWith("/api/forum/forums/product-promises/topics")) {
        return Response.json({ topics: [] })
      }
      return new Response("not found", { status: 404 })
    }

    const result = await surfacePromiseGapReport({
      settings: resolvePromiseSurfacingSettings({
        OPENAGENTS_COM_BASE_URL: "https://openagents.test",
      }),
      report,
      fetchImpl: fetchImpl as typeof fetch,
    })

    expect(result.ok).toBe(false)
    expect(result.mode).toBe("drafted")
    expect(result.draft?.title).toBe(`[Promise Report] ${report.promiseId}`)
    expect(result.blockerRefs).toContain("env.OPENAGENTS_AGENT_TOKEN")
    expect(calls.filter(url => url.includes("/topics")).length).toBe(1)
  })

  test("posts Product Promises topic with configured agent token", async () => {
    const requests: Array<{ url: string; init?: RequestInit }> = []
    const fetchImpl = async (url: string | URL | Request, init?: RequestInit) => {
      const href = String(url)
      requests.push({ url: href, init })
      if (href.endsWith("/api/public/product-promises")) {
        return Response.json(ledger)
      }
      if (href.endsWith("/api/forum/forums/product-promises/topics") && !init) {
        return Response.json({ topics: [] })
      }
      if (href.endsWith("/api/forum/forums/product-promises/topics")) {
        return Response.json({
          topic: {
            topicId: "topic.promise.created",
            url: "https://openagents.test/forum/t/topic.promise.created",
          },
        }, { status: 201 })
      }
      return new Response("not found", { status: 404 })
    }

    const result = await surfacePromiseGapReport({
      settings: resolvePromiseSurfacingSettings({
        OPENAGENTS_COM_BASE_URL: "https://openagents.test",
        OPENAGENTS_AGENT_TOKEN: "oa_agent_test",
      }),
      report,
      fetchImpl: fetchImpl as typeof fetch,
    })

    const post = requests.find(request => request.init?.method === "POST")
    expect(result.ok).toBe(true)
    expect(result.mode).toBe("posted")
    expect(result.topicUrl).toBe(
      "https://openagents.test/forum/t/topic.promise.created",
    )
    expect(post?.init?.headers).toMatchObject({
      authorization: "Bearer oa_agent_test",
    })
    expect(String(post?.init?.body)).toContain(report.promiseId)
  })
})
