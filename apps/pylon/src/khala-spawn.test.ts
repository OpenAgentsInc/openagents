import { describe, expect, test } from "bun:test"

import {
  PYLON_KHALA_SPAWN_PLAN_SCHEMA,
  runPylonKhalaSpawnPlan,
  type PylonKhalaSpawnPlan,
} from "./khala-spawn.js"
import type { PylonKhalaProofResult, PylonKhalaRequestResult } from "./khala-requester.js"

const plan: PylonKhalaSpawnPlan = {
  schema: PYLON_KHALA_SPAWN_PLAN_SCHEMA,
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
        accountRefHash: "account.pylon.codex.default",
      },
      commands: {
        proof: "pylon khala proof --assignment-ref <assignmentRef> --json",
        request: "pylon khala request --workflow codex_agent_task --fixture --json",
        runNoSpend:
          'pylon assignment run-no-spend --base-url "https://openagents.example" --account "codex" --assignment-ref <assignmentRef> --json',
      },
      objective: {
        objective: "Implement OpenAgents issue #6366 from the Khala roadmap.",
        objectiveRef: "objective.khala_spawn.01",
      },
      requestInput: {
        prompt: "Implement OpenAgents issue #6366 from the Khala roadmap.",
        targetPylonRef: "pylon.owner.codex",
        workflow: "codex_agent_task",
      },
      slotIndex: 0,
    },
  ],
  targetPylonRef: "pylon.owner.codex",
}

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
})
