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
  baseUrl: "https://openagents.example",
  blockerRefs: [],
  maxParallel: 1,
  objectiveCount: 1,
  readyCodexAccountCount: 1,
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
      "blocker.khala_spawn.requested_count_exceeds_advertised_availability",
    )
  })
})

const requestResult: PylonKhalaRequestResult = {
  assignmentRef: "assignment.public.khala_coding.test",
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
    expect(result.aggregate.totalTokenRows).toBe(1)
    expect(result.aggregate.totalVerifiedTokens).toBe(16)
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
    expect(result.aggregate.totalTokenRows).toBe(1)
    expect(result.aggregate.totalVerifiedTokens).toBe(16)
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
    expect(result.aggregate.totalVerifiedTokens).toBe(16)
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
})
