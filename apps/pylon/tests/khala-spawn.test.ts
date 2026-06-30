import { describe, expect, test } from "bun:test"

import type { BootstrapSummary } from "../src/bootstrap"
import type { PylonAccountsListProjection } from "../src/account-usage"
import {
  buildPylonKhalaSpawnPlan,
  repeatedKhalaSpawnObjectives,
  runPylonKhalaSpawnPlan,
} from "../src/khala-spawn"
import type {
  PylonKhalaProofResult,
  PylonKhalaRequestResult,
} from "../src/khala-requester"

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

const accountsProjection = (): PylonAccountsListProjection => ({
  accounts: [
    readyCodexAccount("account.public.codex.one", "accthash_one"),
    readyCodexAccount("account.public.codex.two", "accthash_two"),
    readyCodexAccount("account.public.codex.three", "accthash_three"),
  ],
  blockerRefs: [],
  observedAt: "2026-06-27T12:00:00.000Z",
  schema: "openagents.pylon.accounts_list.v0.3",
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
  generatedAt: "2026-06-27T12:01:00.000Z",
  ok: true,
  owner: {
    agentUserRef: "agent_user.public.owner",
    openauthUserRef: "openauth.public.owner",
  },
  pylonRef: "pylon.public.local",
  proofChecklist: {
    blockerRefs: [],
    items: [],
    ok: true,
    schema: "openagents.pylon.khala_proof_checklist.v0.1",
  },
  rawEvents: {
    byteLength: 800,
    count: 2,
    eventCount: 4,
    refs: ["raw_event.public.one", "raw_event.public.two"],
    visibility: "owner_only",
  },
  schemaVersion: "openagents.pylon.codex_assignment_proof.v1",
  tokenUsage: {
    cacheReadTokens: 3,
    demandKind: "own_capacity",
    demandSource: "khala_coding_delegation",
    inputTokens: totalTokens - 25,
    model: "openagents/pylon-codex",
    outputTokens: 20,
    provider: "pylon-codex-own-capacity",
    reasoningTokens: 5,
    refs: [`event.inference.served-tokens.pylon-codex.${assignmentRef}`],
    rowCount: 1,
    totalTokens,
    usageTruth: "exact",
  },
  traces: {
    count: 2,
    refs: ["trace.public.one", "trace.public.two"],
    schemaVersion: "trace.v1",
    visibility: "owner_only",
  },
})

describe("pylon khala spawn planner", () => {
  test("plans generic objective slots against ready accounts and advertised capacity", () => {
    const plan = buildPylonKhalaSpawnPlan({
      accounts: accountsProjection(),
      advertisedCodexAvailability: 5,
      baseUrl: "https://openagents.test",
      maxParallel: 2,
      objectives: repeatedKhalaSpawnObjectives({
        count: 5,
        objective: "audit the public checkout workflow",
      }),
      targetPylonRef: "pylon.public.local",
    })

    expect(plan.schema).toBe("openagents.pylon.khala_spawn_plan.v0.1")
    expect(plan.maxParallel).toBe(2)
    expect(plan.readyCodexAccountCount).toBe(3)
    expect(plan.advertisedCodexAvailability).toBe(5)
    expect(plan.slots).toHaveLength(5)
    expect(plan.slots.map((slot) => slot.account.accountRefHash)).toEqual([
      "accthash_one",
      "accthash_two",
      "accthash_one",
      "accthash_two",
      "accthash_one",
    ])
    expect(plan.slots[0]?.commands.request).toContain("pylon khala request")
    expect(plan.slots[0]?.commands.runNoSpend).toContain("assignment run-no-spend")
    expect(plan.slots[0]?.commands.proof).toContain("pylon khala proof")
  })

  test("plans multiple advertised slots per ready account", () => {
    const plan = buildPylonKhalaSpawnPlan({
      accounts: accountsProjection(),
      advertisedCodexAvailability: 5,
      baseUrl: "https://openagents.test",
      maxParallel: 5,
      objectives: repeatedKhalaSpawnObjectives({
        count: 5,
        objective: "run a live-capacity smoke task",
      }),
      targetPylonRef: "pylon.public.local",
    })

    expect(plan.maxParallel).toBe(5)
    expect(plan.readyCodexAccountCount).toBe(3)
    expect(plan.slots.map((slot) => slot.account.accountRefHash)).toEqual([
      "accthash_one",
      "accthash_two",
      "accthash_three",
      "accthash_one",
      "accthash_two",
    ])
  })
})

describe("pylon khala spawn runner", () => {
  test("runs fixture-backed assignment slots in parallel without exceeding maxParallel", async () => {
    const plan = buildPylonKhalaSpawnPlan({
      accounts: accountsProjection(),
      advertisedCodexAvailability: 4,
      baseUrl: "https://openagents.test",
      maxParallel: 2,
      objectives: repeatedKhalaSpawnObjectives({
        count: 4,
        objective: "run a fixture-backed Codex assignment",
      }),
      targetPylonRef: "pylon.public.local",
    })
    let active = 0
    let maxSeen = 0
    const requestedAssignments: string[] = []
    const lifecycleStates: string[] = []
    const counterSnapshots = [50_000, 54_800]

    const result = await runPylonKhalaSpawnPlan({
      deps: {
        onWorkerLifecycle: async (event) => {
          lifecycleStates.push(event.state)
        },
        readProof: async (_network, assignmentRef) => proofResult(assignmentRef, 1200),
        readTokensServed: async () => counterSnapshots.shift() ?? null,
        requestAssignment: async (_network, _input, slot) => {
          const assignmentRef = `assignment.public.spawn.${slot.slotIndex}`
          requestedAssignments.push(assignmentRef)
          return requestResult(assignmentRef)
        },
        runAssignment: async (_summary, options) => {
          active += 1
          maxSeen = Math.max(maxSeen, active)
          await options.onLifecycleEvent?.({
            schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
            assignmentRef: options.assignmentRef,
            event: "assignment_run.runtime_started",
            leaseRef: `lease.public.${options.assignmentRef}`,
            observedAt: "2026-06-27T12:00:01.000Z",
          })
          await Bun.sleep(25)
          await options.onLifecycleEvent?.({
            schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
            assignmentRef: options.assignmentRef,
            closeoutRef: `closeout.public.${options.assignmentRef}`,
            event: "assignment_run.closeout_submitted",
            leaseRef: `lease.public.${options.assignmentRef}`,
            observedAt: "2026-06-27T12:00:02.000Z",
            status: "accepted",
          })
          active -= 1
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
    expect(maxSeen).toBe(2)
    expect(requestedAssignments).toEqual([
      "assignment.public.spawn.0",
      "assignment.public.spawn.1",
      "assignment.public.spawn.2",
      "assignment.public.spawn.3",
    ])
    expect(result.aggregate).toMatchObject({
      acceptedCount: 4,
      ownerOnlyRawEventCount: 16,
      ownerOnlyTraceCount: 8,
      totalTokenRows: 4,
      totalVerifiedTokens: 4800,
    })
    expect(result.counter).toMatchObject({
      after: 54_800,
      before: 50_000,
      delta: 4800,
      expectedMinimumDelta: 4800,
      state: "increment_observed",
    })
    expect(result.results.every((slot) => slot.proof?.provider === "pylon-codex-own-capacity")).toBe(true)
    expect(result.results.every((slot) => slot.proof?.model === "openagents/pylon-codex")).toBe(true)
    expect(result.results.every((slot) => slot.proof?.usageTruth === "exact")).toBe(true)
    expect(result.results.every((slot) => slot.proof?.demandKind === "own_capacity")).toBe(true)
    expect(result.results.every((slot) => slot.proof?.demandSource === "khala_coding_delegation")).toBe(true)
    expect(result.results.every((slot) => slot.failure === null)).toBe(true)
    expect(lifecycleStates).toContain("queued")
    expect(lifecycleStates).toContain("assignment_created")
    expect(lifecycleStates).toContain("running")
    expect(lifecycleStates).toContain("closeout_submitted")
    expect(lifecycleStates).toContain("proof_checked")
    expect(lifecycleStates).toContain("accepted")
  })

  test("retries proof reads after an accepted closeout", async () => {
    const plan = buildPylonKhalaSpawnPlan({
      accounts: accountsProjection(),
      advertisedCodexAvailability: 1,
      baseUrl: "https://openagents.test",
      maxParallel: 1,
      objectives: repeatedKhalaSpawnObjectives({
        count: 1,
        objective: "run a fixture-backed Codex assignment",
      }),
      targetPylonRef: "pylon.public.local",
    })
    let proofAttempts = 0
    const sleeps: number[] = []

    const result = await runPylonKhalaSpawnPlan({
      deps: {
        readProof: async (_network, assignmentRef) => {
          proofAttempts += 1
          if (proofAttempts < 3) throw new Error("proof unavailable")
          return proofResult(assignmentRef, 900)
        },
        readTokensServed: async () => null,
        requestAssignment: async (_network, _input, slot) =>
          requestResult(`assignment.public.spawn.${slot.slotIndex}`),
        runAssignment: async () => ({ closeout: { status: "accepted" }, ok: true }),
        sleep: async (ms) => {
          sleeps.push(ms)
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
    expect(proofAttempts).toBe(3)
    expect(sleeps).toEqual([500, 1500])
    expect(result.results[0]?.failure).toBeNull()
    expect(result.results[0]?.proof?.totalTokens).toBe(900)
  })

  test("projects public-safe timeout failures from slots", async () => {
    const plan = buildPylonKhalaSpawnPlan({
      accounts: accountsProjection(),
      advertisedCodexAvailability: 1,
      baseUrl: "https://openagents.test",
      maxParallel: 1,
      objectives: repeatedKhalaSpawnObjectives({
        count: 1,
        objective: "run a fixture-backed Codex assignment",
      }),
      targetPylonRef: "pylon.public.local",
    })

    const result = await runPylonKhalaSpawnPlan({
      deps: {
        readTokensServed: async () => null,
        requestAssignment: async (_network, _input, slot) =>
          requestResult(`assignment.public.spawn.${slot.slotIndex}`),
        runAssignment: async () => {
          throw new Error("command timed out")
        },
      },
      network: {
        agentToken: "public-agent-token",
        baseUrl: "https://openagents.test",
      },
      plan,
      summary: {} as BootstrapSummary,
    })

    expect(result.ok).toBe(false)
    expect(result.blockerRefs).toContain("blocker.khala_spawn.slot_timeout")
    expect(result.results[0]?.failure).toEqual({
      message: "worker failed because a bounded operation timed out",
      phase: "assignment_created",
      ref: "failure.khala_spawn.timeout",
    })
  })
})
