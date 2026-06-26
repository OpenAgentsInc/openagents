import { describe, expect, test } from "bun:test"

import {
  acceptPylonWorkOffer,
  assertPublicSafe,
  buildPylonAutopilotWorkRequestBody,
  buildPylonWorkRequestBody,
  createPylonWorkRequest,
  listPylonWorkOffers,
  readPylonAutopilotWorkEvents,
  readPylonAutopilotWorkStatus,
  readPylonWorkStatus,
  reviewPylonAutopilotWork,
  submitPylonAutopilotWork,
  workAcceptanceMemoryEntry,
  workRequestMemoryEntry,
} from "../src/work-requester"

describe("pylon work requester body", () => {
  test("allows bounded verifier filenames that contain sk-adjacent substrings", () => {
    expect(() =>
      assertPublicSafe("src/inference/hydralisk-adapter.test.ts", "work request verification command"),
    ).not.toThrow()
  })

  test("still rejects realistic sk-prefixed credential-shaped values", () => {
    expect(() =>
      assertPublicSafe("sk-1234567890abcdef1234567890abcdef", "work request verification command"),
    ).toThrow(/private, payment/)
  })

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

describe("pylon autopilot work order body", () => {
  test("builds the shared Autopilot work-order request projection from CLI input", () => {
    const body = buildPylonAutopilotWorkRequestBody({
      branch: "main",
      budgetCents: 2500,
      commit: "1745cd4b54b8a12a50922f80b5d345314c91d70d",
      objective: "fix the public failing test",
      repository: "https://github.com/OpenAgentsInc/openagents",
      verificationCommand: "bun test sum.test.ts",
    }) as {
      paymentPolicy: { buyerPaymentMode: string; maxSpendCents: number }
      promiseRef: { promiseId: string }
      tasks: Array<{
        checkout: { commitSha: string; verificationCommand: { args: string[] } }
        requestedAdapter?: string
        requestedAdapterProfileRef?: string
        repository: { fullName: string }
      }>
    }

    expect(body.paymentPolicy).toMatchObject({
      buyerPaymentMode: "l402",
      maxSpendCents: 2500,
    })
    expect(body.promiseRef.promiseId).toBe("autopilot.mission_briefing.v1")
    expect(body.tasks[0]?.repository.fullName).toBe("OpenAgentsInc/openagents")
    expect(body.tasks[0]?.checkout.commitSha).toBe("1745cd4b54b8a12a50922f80b5d345314c91d70d")
    expect(body.tasks[0]?.checkout.verificationCommand.args).toEqual([
      "bun",
      "test",
      "sum.test.ts",
    ])
  })

  test("rejects unsafe Autopilot work-order input before API calls", () => {
    expect(() =>
      buildPylonAutopilotWorkRequestBody({
        budgetCents: 0,
        commit: "1745cd4b54b8a12a50922f80b5d345314c91d70d",
        objective: "use secret bearer material",
      }),
    ).toThrow(/private, payment/)
    expect(() =>
      buildPylonAutopilotWorkRequestBody({
        budgetCents: 0,
        commit: "1745cd4b54b8a12a50922f80b5d345314c91d70d",
        objective: "safe public work",
        repository: "ssh://github.com/OpenAgentsInc/private",
      }),
    ).toThrow(/private, payment/)
  })

  test("requires an explicit real commit for Autopilot work-order submissions", () => {
    expect(() =>
      buildPylonAutopilotWorkRequestBody({
        budgetCents: 0,
        objective: "safe public work",
      }),
    ).toThrow(/--commit/)
    expect(() =>
      buildPylonAutopilotWorkRequestBody({
        budgetCents: 0,
        commit: "1111111111111111111111111111111111111111",
        objective: "safe public work",
      }),
    ).toThrow(/not a placeholder/)
    expect(() =>
      buildPylonAutopilotWorkRequestBody({
        budgetCents: 0,
        commit: "main",
        objective: "safe public work",
      }),
    ).toThrow(/40-character/)
  })

  test("carries Codex and Fable adapter intent through the work-order request body", () => {
    const codex = buildPylonAutopilotWorkRequestBody({
      adapter: "codex",
      budgetCents: 0,
      commit: "1745cd4b54b8a12a50922f80b5d345314c91d70d",
      objective: "safe public codex work",
    }) as { tasks: Array<{ requestedAdapter?: string; requestedAdapterProfileRef?: string }> }
    const fable = buildPylonAutopilotWorkRequestBody({
      adapter: "fable",
      budgetCents: 0,
      commit: "1745cd4b54b8a12a50922f80b5d345314c91d70d",
      objective: "safe public fable work",
    }) as { tasks: Array<{ requestedAdapter?: string; requestedAdapterProfileRef?: string }> }

    expect(codex.tasks[0]?.requestedAdapter).toBe("codex")
    expect(codex.tasks[0]?.requestedAdapterProfileRef).toBeUndefined()
    expect(fable.tasks[0]?.requestedAdapter).toBe("claude_agent")
    expect(fable.tasks[0]?.requestedAdapterProfileRef).toBe("profile.claude_agent.fable")
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

  test("submit, status, events, and review use the Autopilot work-order API", async () => {
    const calls: Array<{ body: unknown; method: string; path: string }> = []
    const fetcher = async (url: URL | RequestInfo, init?: RequestInit) => {
      const parsed = new URL(String(url))
      calls.push({
        body: init?.body === undefined ? null : JSON.parse(String(init.body)),
        method: init?.method ?? "GET",
        path: parsed.pathname,
      })
      if (parsed.hostname === "api.github.com") {
        return new Response(JSON.stringify({ sha: "1745cd4b54b8a12a50922f80b5d345314c91d70d" }))
      }
      if (parsed.pathname.endsWith("/events")) {
        return new Response(JSON.stringify({ events: [], generatedAt: "2026-06-11T00:00:00.000Z" }))
      }
      if (parsed.pathname.endsWith("/review")) {
        return new Response(JSON.stringify({ work: { state: "accepted", workOrderRef: "autopilot_work_order.test" } }), { status: 201 })
      }
      if (parsed.pathname === "/api/autopilot/work") {
        return new Response(JSON.stringify({
          error: "payment_required",
          work: { state: "payment_required", workOrderRef: "autopilot_work_order.test" },
        }), { status: 402 })
      }
      return new Response(JSON.stringify({ work: { state: "delivered", workOrderRef: "autopilot_work_order.test" } }))
    }
    const options = {
      agentToken: "agent-token-test",
      baseUrl: "https://openagents.test",
      fetch: fetcher,
      now: () => new Date("2026-06-11T00:00:00.000Z"),
    }

    await expect(submitPylonAutopilotWork(options, {
      adapter: "codex",
      budgetCents: 2500,
      commit: "1745cd4b54b8a12a50922f80b5d345314c91d70d",
      objective: "fix public work",
    })).resolves.toMatchObject({
      pylonSubmission: {
        pinnedCheckout: {
          commitSha: "1745cd4b54b8a12a50922f80b5d345314c91d70d",
        },
      },
      work: { state: "payment_required" },
    })
    await expect(readPylonAutopilotWorkStatus(options, "autopilot_work_order.test")).resolves.toMatchObject({
      work: { state: "delivered" },
    })
    await expect(readPylonAutopilotWorkEvents(options, "autopilot_work_order.test")).resolves.toMatchObject({
      events: [],
    })
    await expect(reviewPylonAutopilotWork(options, {
      action: "accept",
      workOrderRef: "autopilot_work_order.test",
    })).resolves.toMatchObject({
      work: { state: "accepted" },
    })

    expect(calls.map((call) => `${call.method} ${call.path}`)).toEqual([
      "GET /repos/OpenAgentsInc/openagents/commits/1745cd4b54b8a12a50922f80b5d345314c91d70d",
      "POST /api/autopilot/work",
      "GET /api/autopilot/work/autopilot_work_order.test",
      "GET /api/autopilot/work/autopilot_work_order.test/events",
      "POST /api/autopilot/work/autopilot_work_order.test/review",
    ])
    expect(calls[1]?.body).toMatchObject({
      tasks: [
        {
          checkout: {
            commitSha: "1745cd4b54b8a12a50922f80b5d345314c91d70d",
          },
          requestedAdapter: "codex",
        },
      ],
    })
    expect(calls[4]?.body).toMatchObject({
      action: "accept",
      decisionRefs: ["review.pylon_cli.accept.autopilot_work_order_test"],
    })
  })

  test("unresolvable Autopilot work-order commits fail before submission", async () => {
    const calls: Array<{ method: string; path: string }> = []
    const fetcher = async (url: URL | RequestInfo, init?: RequestInit) => {
      const parsed = new URL(String(url))
      calls.push({
        method: init?.method ?? "GET",
        path: parsed.pathname,
      })
      if (parsed.hostname === "api.github.com") {
        return new Response(JSON.stringify({ message: "Not Found" }), { status: 404 })
      }
      return new Response(JSON.stringify({ work: { state: "created" } }), { status: 201 })
    }

    await expect(submitPylonAutopilotWork({
      agentToken: "agent-token-test",
      baseUrl: "https://openagents.test",
      fetch: fetcher,
      now: () => new Date("2026-06-11T00:00:00.000Z"),
    }, {
      budgetCents: 0,
      commit: "1745cd4b54b8a12a50922f80b5d345314c91d70d",
      objective: "fix public work",
    })).rejects.toThrow(/was not found/)

    expect(calls.map((call) => `${call.method} ${call.path}`)).toEqual([
      "GET /repos/OpenAgentsInc/openagents/commits/1745cd4b54b8a12a50922f80b5d345314c91d70d",
    ])
  })
})
