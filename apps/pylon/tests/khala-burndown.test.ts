import { describe, expect, test } from "bun:test"

import type { BootstrapSummary } from "../src/bootstrap"
import type { PylonAccountsListProjection } from "../src/account-usage"
import {
  buildPylonKhalaBurndownPlan,
  parseKhalaBurndownIssueNumbers,
  parseKhalaRoadmapActiveIssueNumbers,
  runPylonKhalaBurndownPlan,
} from "../src/khala-burndown"
import type {
  PylonKhalaProofResult,
  PylonKhalaRequestResult,
} from "../src/khala-requester"
import {
  PYLON_DISPATCH_BREAKER_SCHEMA,
  type PylonDispatchBreakerSnapshot,
} from "../src/dispatch-failure-taxonomy"

const commit = "7ab7cb401803f6e04a6c93b7aa9102405de66419"
const verificationCommand = "bun test apps/pylon/tests/khala-burndown.test.ts"

const readyCodexAccount = (
  accountRef: string,
  accountRefHash: string,
): PylonAccountsListProjection["accounts"][number] => ({
  accountRef,
  accountRefHash,
  blockerRefs: [],
  homeRef: `home.public.${accountRefHash}`,
  homeState: "present",
  provider: "codex",
  readiness: {
    blockerRefs: [],
    capabilityRefs: ["capability.pylon.local_codex"],
    credentialSourceRef: "credential.public.codex_login",
    enabled: true,
    schema: "openagents.pylon.codex_agent_readiness.v0.3",
    state: "ready",
  },
  selector: "registry_ref",
})

const readyClaudeAccount = (
  accountRef: string,
  accountRefHash: string,
): PylonAccountsListProjection["accounts"][number] => ({
  accountRef,
  accountRefHash,
  blockerRefs: [],
  homeRef: `home.public.${accountRefHash}`,
  homeState: "present",
  provider: "claude_agent",
  readiness: {
    blockerRefs: [],
    capabilityRefs: ["capability.pylon.local_claude"],
    credentialSourceRef: "credential.public.claude_setup_token",
    enabled: true,
    schema: "openagents.pylon.claude_agent_readiness.v0.3",
    state: "ready",
  },
  selector: "registry_ref",
})

const accountsProjection = (): PylonAccountsListProjection => ({
  accounts: [
    readyCodexAccount("account.public.codex.one", "accthash_one"),
    readyCodexAccount("account.public.codex.two", "accthash_two"),
    {
      ...readyCodexAccount("account.public.codex.blocked", "accthash_blocked"),
      blockerRefs: ["blocker.codex.not_ready"],
      readiness: {
        blockerRefs: ["blocker.codex.not_ready"],
        capabilityRefs: [],
        credentialSourceRef: null,
        enabled: true,
        schema: "openagents.pylon.codex_agent_readiness.v0.3",
        state: "credentials_missing",
      },
    },
  ],
  blockerRefs: [],
  observedAt: "2026-06-26T12:00:00.000Z",
  schema: "openagents.pylon.accounts_list.v0.3",
})

const accountDispatchBreaker = (
  accountRefHash: string,
): PylonDispatchBreakerSnapshot => ({
  accountRefHash,
  blockerRefs: [
    "blocker.pylon.dispatch.account_rate_limited",
    "blocker.pylon.dispatch.cooldown_active",
  ],
  contextId: "context.public.burndown-breaker",
  cooldownUntil: "2099-01-01T00:30:00.000Z",
  failureCount: 1,
  failureKind: "transient",
  firstObservedAt: "2099-01-01T00:00:00.000Z",
  lane: "codex",
  lastObservedAt: "2099-01-01T00:00:00.000Z",
  reason: "account_rate_limited",
  schema: PYLON_DISPATCH_BREAKER_SCHEMA,
  scopeKey: `dispatch-breaker.account-lane.codex.${accountRefHash}`,
  sourceDigestRef: "digest.pylon.dispatch_failure.test",
})

const requestResult = (assignmentRef: string): PylonKhalaRequestResult => ({
  assignmentRef,
  durableRequestId: `durable.public.${assignmentRef.replaceAll(".", "_")}`,
  durableStreamUrl: null,
  frames: [],
  model: "openagents/khala",
  nextOffset: "0",
  ok: true,
  rawSse: "",
  schema: "openagents.pylon.khala_request.v1",
  streamClosed: true,
  streamUpToDate: true,
  text: "delegated",
  workflow: "codex_agent_task",
})

const proofResult = (assignmentRef: string, totalTokens: number): PylonKhalaProofResult => ({
  assignmentRef,
  generatedAt: "2026-06-26T12:01:00.000Z",
  ok: true,
  owner: {
    agentUserRef: "agent_user.public.owner",
    openauthUserRef: "openauth.public.owner",
  },
  pylonRef: "pylon.public.local",
  rawEvents: {
    byteLength: 800,
    count: 3,
    eventCount: 3,
    refs: ["raw_event.public.one"],
    visibility: "owner_only",
  },
  schemaVersion: "openagents.pylon.codex_assignment_proof.v1",
  tokenUsage: {
    cacheReadTokens: 0,
    demandKind: "own_capacity",
    demandSource: "khala_coding_delegation",
    inputTokens: totalTokens - 20,
    model: "openagents/pylon-codex",
    outputTokens: 20,
    provider: "pylon-codex-own-capacity",
    reasoningTokens: 0,
    refs: [`event.inference.served-tokens.pylon-codex.${assignmentRef}`],
    rowCount: 1,
    totalTokens,
    usageTruth: "exact",
  },
  traces: {
    count: 1,
    refs: ["trace.public.one"],
    schemaVersion: "trace.v1",
    visibility: "owner_only",
  },
})

describe("pylon khala burndown planner", () => {
  test("parses explicit issue lists without duplicates", () => {
    expect(parseKhalaBurndownIssueNumbers("#6355, 6356, #6355, nope, 0")).toEqual([
      6355,
      6356,
    ])
  })

  test("reads active issue numbers from the roadmap section", () => {
    const roadmap = [
      "# Khala roadmap",
      "## Remaining active sequence",
      "1. **Open** - #6355 - burndown loop.",
      "2. **Open** - #6356 - next item.",
      "## Notes",
      "Ignore closed #1 here.",
    ].join("\n")

    expect(parseKhalaRoadmapActiveIssueNumbers(roadmap)).toEqual([6355, 6356])
  })

  test("maps ready Codex accounts to parallel issue slots", () => {
    const plan = buildPylonKhalaBurndownPlan({
      accounts: accountsProjection(),
      baseUrl: "https://openagents.test",
      commit,
      issueNumbers: [6355, 6356, 6357],
      maxParallel: 2,
      repository: "OpenAgentsInc/openagents",
      targetPylonRef: "pylon.public.local",
      verificationCommand,
    })

    expect(plan.schema).toBe("openagents.pylon.khala_burndown_plan.v0.1")
    expect(plan.mergePolicy).toBe("operator_review_required")
    expect(plan.readyCodexAccountCount).toBe(2)
    expect(plan.issueCount).toBe(3)
    expect(plan.slots).toHaveLength(2)
    expect(plan.slots.map((slot) => slot.issue.issueRef)).toEqual(["#6355", "#6356"])
    expect(plan.slots.map((slot) => slot.account.accountRefHash)).toEqual([
      "accthash_one",
      "accthash_two",
    ])
    expect(plan.slots[0]?.commands.request).toContain("pylon khala request")
    expect(plan.slots[0]?.commands.runNoSpend).toContain("assignment run-no-spend")
    expect(plan.slots[0]?.commands.proof).toContain("pylon khala proof")
  })

  test("maps ready Claude accounts to claude_agent_task issue slots", () => {
    const plan = buildPylonKhalaBurndownPlan({
      accounts: {
        ...accountsProjection(),
        accounts: [
          readyCodexAccount("account.public.codex.one", "accthash_codex"),
          readyClaudeAccount("account.public.claude.one", "accthash_claude"),
        ],
      },
      advertisedCodexAvailability: 2,
      baseUrl: "https://openagents.test",
      commit,
      issueNumbers: [6355, 6356],
      maxParallel: 2,
      repository: "OpenAgentsInc/openagents",
      targetPylonRef: "pylon.public.local",
      verificationCommand,
      workerKind: "claude",
    })

    expect(plan.workerKind).toBe("claude")
    expect(plan.workflow).toBe("claude_agent_task")
    expect(plan.readyWorkerAccountCount).toBe(1)
    expect(plan.slots).toHaveLength(2)
    expect(plan.slots.map((slot) => slot.account.accountRefHash)).toEqual([
      "accthash_claude",
      "accthash_claude",
    ])
    expect(plan.slots[0]?.requestInput.workflow).toBe("claude_agent_task")
    expect(plan.slots[0]?.commands.request).toContain("--workflow claude_agent_task")
  })

  test("skips cooled account-lane breakers while planning issue slots", () => {
    // background_agents.dispatch.lane_account_breaker.v1
    const plan = buildPylonKhalaBurndownPlan({
      accounts: accountsProjection(),
      baseUrl: "https://openagents.test",
      commit,
      dispatchBreakers: [accountDispatchBreaker("accthash_one")],
      issueNumbers: [6355, 6356],
      maxParallel: 2,
      repository: "OpenAgentsInc/openagents",
      targetPylonRef: "pylon.public.local",
      verificationCommand,
    })

    expect(plan.dispatchBreakers).toHaveLength(1)
    expect(plan.readyCodexAccountCount).toBe(1)
    expect(plan.advertisedCodexAvailability).toBe(1)
    expect(plan.slots).toHaveLength(1)
    expect(plan.slots[0]?.account.accountRefHash).toBe("accthash_two")
  })
})

describe("pylon khala burndown runner", () => {
  test("dispatches, runs, and proof-checks each planned slot", async () => {
    const plan = buildPylonKhalaBurndownPlan({
      accounts: accountsProjection(),
      baseUrl: "https://openagents.test",
      commit,
      issueNumbers: [6355, 6356],
      maxParallel: 2,
      repository: "OpenAgentsInc/openagents",
      targetPylonRef: "pylon.public.local",
      verificationCommand,
    })
    const requestedIssues: string[] = []
    const runIssues: string[] = []
    const proofAssignments: string[] = []
    const counterSnapshots = [10_000, 12_400]

    const result = await runPylonKhalaBurndownPlan({
      deps: {
        issueRequest: async (_network, _input, slot) => {
          requestedIssues.push(slot.issue.issueRef)
          return requestResult(`assignment.public.${slot.issue.issueNumber}`)
        },
        readProof: async (_network, assignmentRef) => {
          proofAssignments.push(assignmentRef)
          return proofResult(assignmentRef, 1200)
        },
        readTokensServed: async () => counterSnapshots.shift() ?? null,
        runAssignment: async (_summary, options, slot) => {
          runIssues.push(`${slot.issue.issueRef}:${options.accountRef ?? "default"}`)
          return { closeout: { status: "accepted" }, ok: true }
        },
      },
      network: {
        agentToken: "public-agent-token",
        baseUrl: "https://openagents.test",
      },
      plan,
      summary: {} as BootstrapSummary,
    })

    expect(result.ok).toBe(true)
    expect(result.totalVerifiedTokens).toBe(2400)
    expect(result.counter).toMatchObject({
      after: 12_400,
      before: 10_000,
      delta: 2400,
      expectedMinimumDelta: 2400,
      state: "increment_observed",
    })
    expect(result.results.every((slot) => slot.proof?.usageTruth === "exact")).toBe(true)
    expect(requestedIssues).toEqual(["#6355", "#6356"])
    expect(runIssues).toEqual([
      "#6355:account.public.codex.one",
      "#6356:account.public.codex.two",
    ])
    expect(proofAssignments).toEqual([
      "assignment.public.6355",
      "assignment.public.6356",
    ])
  })

  test("fails closed when proof succeeds but the public counter does not increment", async () => {
    const plan = buildPylonKhalaBurndownPlan({
      accounts: accountsProjection(),
      baseUrl: "https://openagents.test",
      commit,
      issueNumbers: [6355],
      maxParallel: 1,
      repository: "OpenAgentsInc/openagents",
      targetPylonRef: "pylon.public.local",
      verificationCommand,
    })
    const counterSnapshots = [10_000, 10_000]

    const result = await runPylonKhalaBurndownPlan({
      deps: {
        issueRequest: async (_network, _input, slot) =>
          requestResult(`assignment.public.${slot.issue.issueNumber}`),
        readProof: async (_network, assignmentRef) => proofResult(assignmentRef, 1200),
        readTokensServed: async () => counterSnapshots.shift() ?? null,
        runAssignment: async () => ({ closeout: { status: "accepted" }, ok: true }),
      },
      network: {
        agentToken: "public-agent-token",
        baseUrl: "https://openagents.test",
      },
      plan,
      summary: {} as BootstrapSummary,
    })

    expect(result.ok).toBe(false)
    expect(result.counter.state).toBe("unchanged")
    expect(result.blockerRefs).toContain("blocker.khala_burndown.counter_not_incremented")
  })
})
