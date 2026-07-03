import { describe, expect, test } from "bun:test"

import {
  PYLON_KHALA_SPAWN_PLAN_SCHEMA,
  buildPylonKhalaSpawnPlan,
  repeatedKhalaSpawnObjectives,
  runPylonKhalaSpawnPlan,
  type PylonKhalaSpawnPlan,
} from "./khala-spawn.js"
import type { PylonKhalaProofResult, PylonKhalaRequestResult } from "./khala-requester.js"

const accountHashA = "account.pylon.codex.aaaaaaaaaaaa"
const accountHashB = "account.pylon.codex.bbbbbbbbbbbb"
const claudeAccountHash = "account.pylon.claude.aaaaaaaaaaaa"

const plan: PylonKhalaSpawnPlan = {
  schema: PYLON_KHALA_SPAWN_PLAN_SCHEMA,
  advertisedCodexAccounts: [
    {
      accountKey: "aaaaaaaaaaaa",
      accountRefHash: accountHashA,
      available: 1,
      busy: 0,
      queued: 0,
      ready: 1,
    },
  ],
  advertisedCodexAvailability: 1,
  advertisedWorkerAvailability: 1,
  baseUrl: "https://openagents.example",
  blockerRefs: [],
  dispatchBreakers: [],
  maxParallel: 1,
  objectiveCount: 1,
  readyCodexAccountCount: 1,
  readyWorkerAccountCount: 1,
  requestedCount: 1,
  slots: [
    {
      account: {
        accountRef: "codex",
        accountRefHash: accountHashA,
      },
      commands: {
        proof: "pylon khala proof --assignment-ref <assignmentRef> --json",
        request: "pylon khala request --workflow codex_agent_task --fixture --json",
        runNoSpend:
          'pylon assignment run-no-spend --base-url "https://openagents.example" --account "codex" --assignment-ref <assignmentRef> --lifecycle-ndjson --json',
      },
      objective: {
        objective: "Implement OpenAgents issue #6366 from the Khala roadmap.",
        objectiveRef: "objective.khala_spawn.01",
      },
      requestInput: {
        prompt: "Implement OpenAgents issue #6366 from the Khala roadmap.",
        targetAccountRefHash: accountHashA,
        targetPylonRef: "pylon.owner.codex",
        workflow: "codex_agent_task",
      },
      slotIndex: 0,
    },
  ],
  targetPylonRef: "pylon.owner.codex",
  workerKind: "codex",
  workflow: "codex_agent_task",
}

describe("Khala spawn per-account planning", () => {
  test("targets the selected public account hash and skips ready accounts with no advertised free slots", () => {
    const result = buildPylonKhalaSpawnPlan({
      accounts: {
        accounts: [
          {
            accountRef: "codex-a",
            accountRefHash: accountHashA,
            blockerRefs: [],
            homeState: "present",
            provider: "codex",
            readiness: { state: "ready" },
          },
          {
            accountRef: "codex-b",
            accountRefHash: accountHashB,
            blockerRefs: [],
            homeState: "present",
            provider: "codex",
            readiness: { state: "ready" },
          },
        ],
      } as never,
      advertisedCodexAccounts: [
        {
          accountKey: "aaaaaaaaaaaa",
          accountRefHash: accountHashA,
          available: 0,
          busy: 1,
          queued: 0,
          ready: 1,
        },
        {
          accountKey: "bbbbbbbbbbbb",
          accountRefHash: accountHashB,
          available: 2,
          busy: 0,
          queued: 0,
          ready: 2,
        },
      ],
      baseUrl: "https://openagents.example",
      fixture: true,
      objectives: repeatedKhalaSpawnObjectives({
        count: 2,
        objective: "Run the bounded fixture.",
      }),
      targetPylonRef: "pylon.owner.codex",
    })

    expect(result.readyCodexAccountCount).toBe(2)
    expect(result.maxParallel).toBe(2)
    expect(result.slots).toHaveLength(2)
    expect(result.slots.map(slot => slot.account.accountRef)).toEqual(["codex-b", "codex-b"])
    expect(result.slots.map(slot => slot.requestInput.targetAccountRefHash)).toEqual([accountHashB, accountHashB])
    expect(result.slots[0]?.commands.request).toContain('--account-ref "codex-b"')
  })

  test("spreads initial concurrent slots across advertised ready accounts", () => {
    const result = buildPylonKhalaSpawnPlan({
      accounts: {
        accounts: [
          {
            accountRef: "codex-a",
            accountRefHash: accountHashA,
            blockerRefs: [],
            homeState: "present",
            provider: "codex",
            readiness: { state: "ready" },
          },
          {
            accountRef: "codex-b",
            accountRefHash: accountHashB,
            blockerRefs: [],
            homeState: "present",
            provider: "codex",
            readiness: { state: "ready" },
          },
        ],
      } as never,
      advertisedCodexAccounts: [
        {
          accountKey: "aaaaaaaaaaaa",
          accountRefHash: accountHashA,
          available: 5,
          busy: 0,
          queued: 0,
          ready: 5,
        },
        {
          accountKey: "bbbbbbbbbbbb",
          accountRefHash: accountHashB,
          available: 5,
          busy: 0,
          queued: 0,
          ready: 5,
        },
      ],
      baseUrl: "https://openagents.example",
      fixture: true,
      maxParallel: 5,
      objectives: repeatedKhalaSpawnObjectives({
        count: 5,
        objective: "Run the bounded fixture.",
      }),
      targetPylonRef: "pylon.owner.codex",
    })

    expect(result.maxParallel).toBe(5)
    expect(result.slots.map((slot) => slot.account.accountRef)).toEqual([
      "codex-a",
      "codex-b",
      "codex-a",
      "codex-b",
      "codex-a",
    ])
    expect(result.slots.map((slot) => slot.requestInput.targetAccountRefHash)).toEqual([
      accountHashA,
      accountHashB,
      accountHashA,
      accountHashB,
      accountHashA,
    ])
  })

  test("keeps full long prompts while bounding request objective summaries", () => {
    const longObjective = [
      "Coordinate a public OpenAgents fleet packet.",
      "Review issue 7762 and related progress streaming notes.",
      "Map implementation files, run focused tests, and close out with evidence refs.",
      "Repeatable context:",
      "x".repeat(1_100),
    ].join(" ")

    const result = buildPylonKhalaSpawnPlan({
      accounts: {
        accounts: [
          {
            accountRef: "codex-a",
            accountRefHash: accountHashA,
            blockerRefs: [],
            homeState: "present",
            provider: "codex",
            readiness: { state: "ready" },
          },
        ],
      } as never,
      advertisedCodexAccounts: [
        {
          accountKey: "aaaaaaaaaaaa",
          accountRefHash: accountHashA,
          available: 1,
          busy: 0,
          queued: 0,
          ready: 1,
        },
      ],
      baseUrl: "https://openagents.example",
      fixture: true,
      objectives: repeatedKhalaSpawnObjectives({
        count: 1,
        objective: longObjective,
      }),
      targetPylonRef: "pylon.owner.codex",
    })

    const requestInput = result.slots[0]?.requestInput
    expect(requestInput?.prompt).toContain("x".repeat(1_100))
    expect(requestInput?.objectiveSummary).toHaveLength(1_000)
    expect(requestInput?.objectiveSummary?.endsWith("...")).toBe(true)
  })

  test("plans claude_agent_task slots against ready Claude accounts", () => {
    const result = buildPylonKhalaSpawnPlan({
      accounts: {
        accounts: [
          {
            accountRef: "codex-a",
            accountRefHash: accountHashA,
            blockerRefs: [],
            homeState: "present",
            provider: "codex",
            readiness: { state: "ready" },
          },
          {
            accountRef: "claude-a",
            accountRefHash: claudeAccountHash,
            blockerRefs: [],
            homeState: "present",
            provider: "claude_agent",
            readiness: { state: "ready" },
          },
        ],
      } as never,
      advertisedCodexAccounts: [
        {
          accountKey: "claudeaaaaaaaaaaaa",
          accountRefHash: claudeAccountHash,
          available: 2,
          busy: 0,
          queued: 0,
          ready: 2,
        },
      ],
      baseUrl: "https://openagents.example",
      fixture: true,
      objectives: repeatedKhalaSpawnObjectives({
        count: 2,
        objective: "Run the bounded fixture.",
      }),
      targetPylonRef: "pylon.owner.codex",
      workflow: "claude_agent_task",
    })

    expect(result.workerKind).toBe("claude")
    expect(result.workflow).toBe("claude_agent_task")
    expect(result.readyWorkerAccountCount).toBe(1)
    expect(result.slots.map(slot => slot.account.accountRef)).toEqual(["claude-a", "claude-a"])
    expect(result.slots[0]?.requestInput.workflow).toBe("claude_agent_task")
    expect(result.slots[0]?.commands.request).toContain("--workflow claude_agent_task")
    expect(result.slots[0]?.commands.request).toContain('--account-ref "claude-a"')
  })

  test("blocks batches that request more slots than advertised free capacity", () => {
    const result = buildPylonKhalaSpawnPlan({
      accounts: {
        accounts: [
          {
            accountRef: "codex-a",
            accountRefHash: accountHashA,
            blockerRefs: [],
            homeState: "present",
            provider: "codex",
            readiness: { state: "ready" },
          },
          {
            accountRef: "codex-b",
            accountRefHash: accountHashB,
            blockerRefs: [],
            homeState: "present",
            provider: "codex",
            readiness: { state: "ready" },
          },
        ],
      } as never,
      advertisedCodexAccounts: [
        {
          accountKey: "aaaaaaaaaaaa",
          accountRefHash: accountHashA,
          available: 1,
          busy: 4,
          queued: 0,
          ready: 5,
        },
        {
          accountKey: "bbbbbbbbbbbb",
          accountRefHash: accountHashB,
          available: 1,
          busy: 4,
          queued: 0,
          ready: 5,
        },
      ],
      baseUrl: "https://openagents.example",
      fixture: true,
      maxParallel: 5,
      objectives: repeatedKhalaSpawnObjectives({
        count: 5,
        objective: "Run the bounded fixture.",
      }),
      targetPylonRef: "pylon.owner.codex",
    })

    expect(result.advertisedCodexAvailability).toBe(2)
    expect(result.maxParallel).toBe(0)
    expect(result.slots).toEqual([])
    expect(result.blockerRefs).toContain(
      "blocker.khala_spawn.requested_count_exceeds_advertised_codex_availability",
    )
  })
})

const requestResult: PylonKhalaRequestResult = {
  assignmentRef: "assignment.public.khala_coding.test",
  diagnostics: [],
  durableRequestId: "durable.test",
  durableStreamUrl: null,
  frames: [],
  model: "openagents/khala",
  nextOffset: "0",
  ok: true,
  rawSse: "",
  schema: "openagents.pylon.khala_request.v1",
  streamClosed: true,
  streamUpToDate: true,
  text: "",
  workflow: "codex_agent_task",
}

const claudeRequestResult: PylonKhalaRequestResult = {
  ...requestResult,
  assignmentRef: "assignment.public.khala_claude.test",
  durableRequestId: "durable.claude.test",
  workflow: "claude_agent_task",
}

const exactProof: PylonKhalaProofResult = {
  assignmentRef: "assignment.public.khala_coding.test",
  generatedAt: "2026-06-27T13:30:00.000Z",
  ok: true,
  owner: {
    agentUserRef: "user.agent_owner",
    openauthUserRef: "user.owner",
  },
  proofChecklist: {
    blockerRefs: [],
    items: [],
    ok: true,
    schema: "openagents.pylon.khala_proof_checklist.v0.1",
  },
  pylonRef: "pylon.owner.codex",
  rawEvents: {
    byteLength: 100,
    count: 1,
    eventCount: 3,
    refs: ["raw.owner_only.event"],
    visibility: "owner_only",
  },
  schemaVersion: "openagents.pylon.codex_assignment_proof.v1",
  tokenUsage: {
    cacheReadTokens: 0,
    demandKind: "own_capacity",
    demandSource: "khala_coding_delegation",
    inputTokens: 10,
    model: "openagents/pylon-codex",
    outputTokens: 5,
    provider: "pylon-codex-own-capacity",
    reasoningTokens: 1,
    refs: ["event.inference.served-tokens.pylon-codex.owner"],
    rowCount: 1,
    totalTokens: 16,
    usageTruth: "exact",
  },
  traces: {
    count: 1,
    refs: ["trace.owner_only.test"],
    schemaVersion: "ATIF-v1.7",
    visibility: "owner_only",
  },
}

const claudeExactProof: PylonKhalaProofResult = {
  ...exactProof,
  assignmentRef: "assignment.public.khala_claude.test",
  pylonRef: "pylon.owner.claude",
  tokenUsage: {
    ...exactProof.tokenUsage,
    model: "openagents/pylon-claude",
    provider: "pylon-claude-own-capacity",
    refs: ["event.inference.served-tokens.pylon-claude.owner"],
    totalTokens: 18,
  },
}

describe("Khala spawn proof gate", () => {
  test("accepts exact own-capacity Pylon/Codex proof rows", async () => {
    const tokenCounts = [1000, 1016]
    const result = await runPylonKhalaSpawnPlan({
      deps: {
        readProof: async () => exactProof,
        readTokensServed: async () => tokenCounts.shift() ?? 1016,
        requestAssignment: async () => requestResult,
        runAssignment: async () => ({
          closeout: { status: "accepted" },
          ok: true,
        }),
      },
      network: {
        agentToken: "agent.public.test",
        baseUrl: "https://openagents.example",
      },
      plan,
      summary: {} as never,
    })

    expect(result.ok).toBe(true)
    expect(result.aggregate.acceptedCount).toBe(1)
    expect(result.aggregate.totalVerifiedTokens).toBe(16)
    expect(result.blockerRefs).toEqual([])
  })

  test("accepts exact own-capacity Pylon/Claude proof rows", async () => {
    const tokenCounts = [1000, 1018]
    const claudePlan = buildPylonKhalaSpawnPlan({
      accounts: {
        accounts: [
          {
            accountRef: "claude-a",
            accountRefHash: claudeAccountHash,
            blockerRefs: [],
            homeState: "present",
            provider: "claude_agent",
            readiness: { state: "ready" },
          },
        ],
      } as never,
      advertisedCodexAvailability: 1,
      baseUrl: "https://openagents.example",
      fixture: true,
      objectives: repeatedKhalaSpawnObjectives({ count: 1, objective: "Run Claude fixture." }),
      targetPylonRef: "pylon.owner.claude",
      workflow: "claude_agent_task",
    })

    const result = await runPylonKhalaSpawnPlan({
      deps: {
        readProof: async () => claudeExactProof,
        readTokensServed: async () => tokenCounts.shift() ?? 1018,
        requestAssignment: async () => claudeRequestResult,
        runAssignment: async () => ({
          closeout: { status: "accepted" },
          ok: true,
        }),
      },
      network: {
        agentToken: "agent.public.test",
        baseUrl: "https://openagents.example",
      },
      plan: claudePlan,
      summary: {} as never,
    })

    expect(result.ok).toBe(true)
    expect(result.plan.workflow).toBe("claude_agent_task")
    expect(result.aggregate.acceptedCount).toBe(1)
    expect(result.aggregate.totalVerifiedTokens).toBe(18)
    expect(result.blockerRefs).toEqual([])
  })

  test("blocks a slot when proof is not exact own-capacity", async () => {
    const tokenCounts = [1000, 1016]
    const inexactProof = {
      ...exactProof,
      tokenUsage: {
        ...exactProof.tokenUsage,
        usageTruth: "estimated",
      },
    } as unknown as PylonKhalaProofResult

    const result = await runPylonKhalaSpawnPlan({
      deps: {
        readProof: async () => inexactProof,
        readTokensServed: async () => tokenCounts.shift() ?? 1016,
        requestAssignment: async () => requestResult,
        runAssignment: async () => ({
          closeout: { status: "accepted" },
          ok: true,
        }),
      },
      network: {
        agentToken: "agent.public.test",
        baseUrl: "https://openagents.example",
      },
      plan,
      summary: {} as never,
    })

    expect(result.ok).toBe(false)
    expect(result.blockerRefs).toContain(
      "blocker.khala_spawn.proof_not_exact_own_capacity",
    )
    expect(result.results[0]?.state).toBe("failed")
  })

  test("keeps rejected closeouts rejected while aggregating exact proof rows", async () => {
    const tokenCounts = [2000, 2016]
    const result = await runPylonKhalaSpawnPlan({
      deps: {
        readProof: async () => exactProof,
        readTokensServed: async () => tokenCounts.shift() ?? 2016,
        requestAssignment: async () => requestResult,
        runAssignment: async () => ({
          closeout: {
            blockerRefs: ["blocker.assignment.codex_agent_test_failed"],
            status: "rejected",
          },
          ok: false,
        }),
      },
      network: {
        agentToken: "agent.public.test",
        baseUrl: "https://openagents.example",
      },
      plan,
      summary: {} as never,
    })

    expect(result.ok).toBe(false)
    expect(result.aggregate.acceptedCount).toBe(0)
    expect(result.aggregate.closeoutAcceptedCount).toBe(0)
    expect(result.aggregate.rejectedWithVerifiedTokensCount).toBe(1)
    expect(result.aggregate.totalTokenRows).toBe(1)
    expect(result.aggregate.totalVerifiedTokens).toBe(16)
    expect(result.aggregate.verifiedTokenAssignmentCount).toBe(1)
    expect(result.counter.state).toBe("increment_observed")
    expect(result.blockerRefs).toContain("blocker.assignment.codex_agent_test_failed")
    expect(result.results[0]?.proof?.totalTokens).toBe(16)
    expect(result.results[0]?.runAccepted).toBe(false)
    expect(result.results[0]?.state).toBe("rejected")
    expect(result.results[0]?.lifecycleEvents.map(event => event.state)).toContain("proof_checked")
  })

  test("backfills delayed exact proof rows for rejected closeouts", async () => {
    const tokenCounts = [3000, 3016]
    let proofAttempts = 0
    const result = await runPylonKhalaSpawnPlan({
      deps: {
        readProof: async () => {
          proofAttempts += 1
          if (proofAttempts <= 4) {
            throw new Error("proof rows not indexed yet")
          }
          return exactProof
        },
        readTokensServed: async () => tokenCounts.shift() ?? 3016,
        requestAssignment: async () => requestResult,
        runAssignment: async () => ({
          closeout: {
            blockerRefs: ["blocker.assignment.codex_agent_test_failed"],
            status: "rejected",
          },
          ok: false,
        }),
        sleep: async () => undefined,
      },
      network: {
        agentToken: "agent.public.test",
        baseUrl: "https://openagents.example",
      },
      plan,
      summary: {} as never,
    })

    expect(result.ok).toBe(false)
    expect(result.aggregate.acceptedCount).toBe(0)
    expect(result.aggregate.closeoutAcceptedCount).toBe(0)
    expect(result.aggregate.rejectedWithVerifiedTokensCount).toBe(1)
    expect(result.aggregate.totalTokenRows).toBe(1)
    expect(result.aggregate.totalVerifiedTokens).toBe(16)
    expect(result.aggregate.verifiedTokenAssignmentCount).toBe(1)
    expect(result.blockerRefs).toContain("blocker.assignment.codex_agent_test_failed")
    expect(result.results[0]?.proof?.totalTokens).toBe(16)
    expect(result.results[0]?.runAccepted).toBe(false)
    expect(result.results[0]?.state).toBe("rejected")
    expect(result.results[0]?.lifecycleEvents.at(-1)?.message).toBe("assignment proof backfilled")
  })

  test("recovers accepted slots when only proof indexing was late", async () => {
    const tokenCounts = [4000, 4016]
    let proofAttempts = 0
    const result = await runPylonKhalaSpawnPlan({
      deps: {
        readProof: async () => {
          proofAttempts += 1
          if (proofAttempts <= 4) {
            throw new Error("proof rows not indexed yet")
          }
          return exactProof
        },
        readTokensServed: async () => tokenCounts.shift() ?? 4016,
        requestAssignment: async () => requestResult,
        runAssignment: async () => ({
          closeout: { status: "accepted" },
          ok: true,
        }),
        sleep: async () => undefined,
      },
      network: {
        agentToken: "agent.public.test",
        baseUrl: "https://openagents.example",
      },
      plan,
      summary: {} as never,
    })

    expect(result.ok).toBe(true)
    expect(result.aggregate.acceptedCount).toBe(1)
    expect(result.aggregate.closeoutAcceptedCount).toBe(1)
    expect(result.aggregate.totalVerifiedTokens).toBe(16)
    expect(result.aggregate.verifiedTokenAssignmentCount).toBe(1)
    expect(result.blockerRefs).toEqual([])
    expect(result.results[0]?.failure).toBeNull()
    expect(result.results[0]?.proof?.totalTokens).toBe(16)
    expect(result.results[0]?.state).toBe("accepted")
    expect(result.results[0]?.lifecycleEvents.at(-1)?.message).toBe("assignment proof backfilled")
  })

  test("classifies request public-safety guard failures explicitly", async () => {
    const result = await runPylonKhalaSpawnPlan({
      deps: {
        readTokensServed: async () => 1000,
        requestAssignment: async () => {
          throw new Error("khala request prompt contains private, payment, credential, wallet, or raw material")
        },
      },
      network: {
        agentToken: "agent.public.test",
        baseUrl: "https://openagents.example",
      },
      plan,
      summary: {} as never,
    })

    expect(result.ok).toBe(false)
    expect(result.blockerRefs).toContain(
      "blocker.khala_spawn.request_public_safety_blocked",
    )
    expect(result.results[0]?.failure?.ref).toBe(
      "failure.khala_spawn.request_public_safety_blocked",
    )
  })

  test("retries transient HTTP 409 assignment creation and reports recovery", async () => {
    const tokenCounts = [5000, 5016]
    const sleeps: number[] = []
    let requestAttempts = 0
    const result = await runPylonKhalaSpawnPlan({
      deps: {
        readProof: async () => exactProof,
        readTokensServed: async () => tokenCounts.shift() ?? 5016,
        requestAssignment: async () => {
          requestAttempts += 1
          if (requestAttempts < 3) {
            throw new Error("pylon khala request failed (409): assignment slot conflict")
          }
          return requestResult
        },
        runAssignment: async () => ({
          closeout: { status: "accepted" },
          ok: true,
        }),
        sleep: async (ms) => {
          sleeps.push(ms)
        },
      },
      network: {
        agentToken: "agent.public.test",
        baseUrl: "https://openagents.example",
      },
      plan,
      summary: {} as never,
    })

    const statuses = result.results[0]?.lifecycleEvents.flatMap((event) =>
      event.status === undefined ? [] : [event.status]
    )
    expect(result.ok).toBe(true)
    expect(requestAttempts).toBe(3)
    expect(sleeps).toEqual([750, 2000])
    expect(statuses).toContain("retry.khala_spawn.assignment_http_409")
    expect(statuses).toContain("retry.khala_spawn.assignment_http_409_recovered")
    expect(statuses).toContain("retry.khala_spawn.assignment_http_409_succeeded")
    expect(result.results[0]?.failure).toBeNull()
    expect(result.results[0]?.state).toBe("accepted")
  })

  test("exhausts bounded HTTP 409 assignment creation retries with public-safe blocker", async () => {
    const sleeps: number[] = []
    let requestAttempts = 0
    const result = await runPylonKhalaSpawnPlan({
      deps: {
        readTokensServed: async () => null,
        requestAssignment: async () => {
          requestAttempts += 1
          throw new Error("pylon khala request failed (409): assignment slot conflict")
        },
        sleep: async (ms) => {
          sleeps.push(ms)
        },
      },
      network: {
        agentToken: "agent.public.test",
        baseUrl: "https://openagents.example",
      },
      plan,
      summary: {} as never,
    })

    const statuses = result.results[0]?.lifecycleEvents.flatMap((event) =>
      event.status === undefined ? [] : [event.status]
    )
    expect(result.ok).toBe(false)
    expect(requestAttempts).toBe(4)
    expect(sleeps).toEqual([750, 2000, 4000])
    expect(statuses).toContain("retry.khala_spawn.assignment_http_409")
    expect(statuses).toContain("failure.khala_spawn.assignment_http_409_retry_exhausted")
    expect(result.blockerRefs).toContain("blocker.khala_spawn.slot_http_409")
    expect(result.results[0]?.failure).toEqual({
      message: "worker failed because the OpenAgents API returned HTTP 409",
      phase: "requesting",
      ref: "failure.khala_spawn.http_409",
    })
  })
})
