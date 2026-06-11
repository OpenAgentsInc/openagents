import { describe, expect, test } from "bun:test"

import {
  acceptPylonWorkOffer,
  buildPylonWorkRequestBody,
  createPylonWorkRequest,
  listPylonWorkOffers,
  readPylonWorkStatus,
  workAcceptanceMemoryEntry,
  workRequestMemoryEntry,
} from "../src/work-requester"

describe("pylon work requester body", () => {
  test("builds ref-only Forum work request bodies from CLI input", () => {
    const body = buildPylonWorkRequestBody({
      budgetSats: 2_000,
      deadline: "2026-06-12T00:00:00.000Z",
      objective: "fix the failing sum test",
      repository: "https://github.com/OpenAgentsInc/openagents",
      verificationCommand: "bun test sum.test.ts",
    })

    expect(body).toMatchObject({
      budgetSats: 2_000,
      repositoryRefs: ["repo.public.github.OpenAgentsInc.openagents"],
      requiredCapabilityRefs: ["capability.pylon.local_claude_agent"],
      title: "Pylon work: fix the failing sum test",
    })
    expect(body.objectiveRef).toStartWith("objective.public.pylon_work.")
    expect(body.deadlineRef).toStartWith("deadline.public.pylon_work.")
    expect(body.verificationCommandRef).toStartWith("command.public.pylon_work.")
    expect(JSON.stringify(body)).not.toContain("bun test sum.test.ts")
  })

  test("rejects private or payment-shaped material before API calls", () => {
    expect(() =>
      buildPylonWorkRequestBody({
        budgetSats: 2_000,
        objective: "use bearer token secret",
      }),
    ).toThrow(/private, payment/)
    expect(() =>
      buildPylonWorkRequestBody({
        budgetSats: 2_000,
        objective: "safe public task",
        repository: "ssh://github.com/private/repo",
      }),
    ).toThrow(/private, payment/)
  })
})

describe("pylon work requester API", () => {
  test("request posts through the Forum work-request API with bearer identity and memory refs", async () => {
    const calls: Array<{ url: string; init: RequestInit }> = []
    const fetcher = async (url: URL | RequestInfo, init?: RequestInit) => {
      calls.push({ init: init ?? {}, url: String(url) })
      return new Response(JSON.stringify({
        topic: { topicId: "topic_1" },
        workRequest: {
          jobEventId: "a".repeat(64),
          topicId: "topic_1",
          workRequestId: "work_request_1",
        },
      }), { status: 201 })
    }

    const result = await createPylonWorkRequest(
      {
        agentToken: "agent-token-test",
        baseUrl: "https://openagents.test",
        fetch: fetcher,
        now: () => new Date("2026-06-10T23:30:00.000Z"),
      },
      {
        budgetSats: 2_000,
        objective: "fix a public failing test",
      },
    )

    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toBe("https://openagents.test/api/forum/work-requests")
    expect(calls[0]?.init.method).toBe("POST")
    expect((calls[0]?.init.headers as Record<string, string>).Authorization).toBe("Bearer agent-token-test")
    expect((calls[0]?.init.headers as Record<string, string>)["Idempotency-Key"]).toContain("pylon-work-request:")
    expect(JSON.parse(String(calls[0]?.init.body))).toMatchObject({
      budgetSats: 2_000,
      requiredCapabilityRefs: ["capability.pylon.local_claude_agent"],
    })

    expect(workRequestMemoryEntry({
      at: "2026-06-10T23:30:00.000Z",
      result,
    })).toEqual({
      at: "2026-06-10T23:30:00.000Z",
      kind: "work_request",
      refs: {
        jobEventId: "a".repeat(64),
        topicId: "topic_1",
        workRequestId: "work_request_1",
      },
      summary: "requested work work_request_1",
    })
  })

  test("offers, accept, and status use the request-ref API and write accept memory", async () => {
    const paths: string[] = []
    const methods: string[] = []
    const fetcher = async (url: URL | RequestInfo, init?: RequestInit) => {
      const parsed = new URL(String(url))
      paths.push(parsed.pathname)
      methods.push(init?.method ?? "GET")
      if (parsed.pathname.endsWith("/offers")) {
        return new Response(JSON.stringify({ offers: [{ quoteRef: "quote.public.one", amountMsats: 2_000_000 }] }))
      }
      if (parsed.pathname.endsWith("/acceptances")) {
        return new Response(JSON.stringify({ receiptRefs: ["receipt.labor_escrow.reserve.one"] }), { status: 201 })
      }
      return new Response(JSON.stringify({ workRequest: { state: "accepted" } }))
    }
    const options = {
      agentToken: "agent-token-test",
      baseUrl: "https://openagents.test",
      fetch: fetcher,
      now: () => new Date("2026-06-10T23:31:00.000Z"),
    }

    await expect(listPylonWorkOffers(options, "work_request_1")).resolves.toMatchObject({
      offers: [{ quoteRef: "quote.public.one" }],
    })
    const accepted = await acceptPylonWorkOffer(options, {
      quoteRef: "quote.public.one",
      requestRef: "work_request_1",
    })
    await expect(readPylonWorkStatus(options, "work_request_1")).resolves.toMatchObject({
      workRequest: { state: "accepted" },
    })

    expect(paths).toEqual([
      "/api/forum/work-requests/work_request_1/offers",
      "/api/forum/work-requests/work_request_1/acceptances",
      "/api/forum/work-requests/work_request_1",
    ])
    expect(methods).toEqual(["GET", "POST", "GET"])
    expect(workAcceptanceMemoryEntry({
      at: "2026-06-10T23:31:00.000Z",
      quoteRef: "quote.public.one",
      requestRef: "work_request_1",
      result: accepted,
    })).toEqual({
      at: "2026-06-10T23:31:00.000Z",
      kind: "work_acceptance",
      refs: {
        quoteRef: "quote.public.one",
        receiptRefs: ["receipt.labor_escrow.reserve.one"],
        requestRef: "work_request_1",
      },
      summary: "accepted work quote quote.public.one",
    })
  })

  test("write commands require an agent token and preserve typed refusal reasons", async () => {
    await expect(
      createPylonWorkRequest(
        { agentToken: "", baseUrl: "https://openagents.test" },
        { budgetSats: 2_000, objective: "fix a public test" },
      ),
    ).rejects.toThrow("OPENAGENTS_AGENT_TOKEN")

    await expect(
      acceptPylonWorkOffer(
        {
          agentToken: "agent-token-test",
          baseUrl: "https://openagents.test",
          fetch: async () =>
            new Response(JSON.stringify({ reason: "insufficient_available_balance" }), { status: 409 }),
        },
        { quoteRef: "quote.public.one", requestRef: "work_request_1" },
      ),
    ).rejects.toThrow("insufficient_available_balance")

    await expect(
      acceptPylonWorkOffer(
        {
          agentToken: "agent-token-test",
          baseUrl: "https://openagents.test",
          fetch: async () =>
            new Response(JSON.stringify({ reason: "quote_already_accepted" }), { status: 409 }),
        },
        { quoteRef: "quote.public.two", requestRef: "work_request_1" },
      ),
    ).rejects.toThrow("quote_already_accepted")
  })
})
