import { describe, expect, test } from "bun:test"
import type { PylonAssignmentRunLifecycleEvent } from "@openagentsinc/agent-runtime-schema"

import type { PylonAccountRegistryEntry } from "../src/account-registry.js"
import { hashPylonAccountRef } from "../src/account-registry.js"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap.js"
import type {
  PylonKhalaAssignmentTraceStatusResult,
  PylonKhalaRequestInput,
} from "../src/khala-requester.js"
import {
  createPylonOwnedFleetRunSupervisorRunner,
  PYLON_OWNED_FLEET_RUNNER_BLOCKERS,
  PYLON_OWNED_FLEET_RUNNER_MAX_TERMINAL_RETENTION,
  type PylonOwnedFleetRunAssignmentReceipt,
} from "../src/orchestration/fleet-run-owned-runner.js"
import type {
  FleetRunSupervisorActiveAssignment,
  FleetRunSupervisorDispatchInput,
} from "../src/orchestration/fleet-run-supervisor.js"
import type { FleetRun, WorkClaim } from "../src/orchestration/store.js"

const fixedNow = new Date("2026-07-09T12:00:00.000Z")
const pylonRef = "pylon.owner.fleet.runner"
const commit = "5008856f577f38f0841c40142bb68d40e766df29"

const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
  PYLON_HOME: "/tmp/pylon-fleet-runner-test",
})

const account = (
  ref: string,
  provider: PylonAccountRegistryEntry["provider"],
): PylonAccountRegistryEntry => ({
  ref,
  provider,
  home: `/tmp/pylon-fleet-runner-test/accounts/${provider}/${ref}`,
  openAgentsProviderAccountRef: `provider_account.public.${ref}`,
  hourlyCap: null,
  weeklyCap: null,
  manualResetsRemaining: null,
  marginalCostClass: "subscription",
})

const run: FleetRun = {
  schema: "openagents.khala_code.fleet_run.v1",
  runRef: "fleet_run.runner.test",
  objective: "Implement the bounded public work unit and run its verifier.",
  workSource: "issue_list",
  targetConcurrency: 2,
  workerKind: "auto",
  refillPolicy: {
    maxPerAccount: 1,
    cooldownAware: true,
    stopCondition: "backlog_empty",
  },
  state: "running",
  dispatchKind: "supervised_dispatch",
  dagTracked: false,
  startedAt: fixedNow.toISOString(),
  counters: {
    workUnitsTotal: 2,
    activeAssignments: 0,
    completedAssignments: 0,
    failedAssignments: 0,
    blockedAssignments: 0,
  },
  createdAt: fixedNow.toISOString(),
  updatedAt: fixedNow.toISOString(),
}

const claim = (ref: string, accountRef: string, assignmentRef: string | null = null): WorkClaim => ({
  schema: "openagents.khala_code.work_claim.v1",
  claimRef: ref,
  workUnitRef: `work_unit.${ref}`,
  runRef: run.runRef,
  assignmentRef,
  workerAccountRef: accountRef,
  state: "in_progress",
  ttl: 60_000,
  claimedAt: fixedNow.toISOString(),
  expiresAt: new Date(fixedNow.getTime() + 60_000).toISOString(),
  updatedAt: fixedNow.toISOString(),
})

const dispatchInput = (
  workerKind: FleetRunSupervisorDispatchInput["workerKind"],
  accountRef: string,
  ordinal: number,
  kind: "fixture" | "github_issue" = "github_issue",
): FleetRunSupervisorDispatchInput => {
  const workClaim = claim(`claim.runner.${ordinal}`, accountRef)
  return {
    accountRef,
    claim: workClaim,
    run,
    taskId: `task.runner.${ordinal}`,
    workerKind,
    workUnit: kind === "fixture"
      ? {
          workUnitRef: workClaim.workUnitRef,
          kind: "fixture",
          title: `Fixture ${ordinal}`,
          source: "fixture",
          status: "claimable",
        }
      : {
          workUnitRef: workClaim.workUnitRef,
          kind: "github_issue",
          title: `Issue ${ordinal}`,
          source: "issue_list",
          status: "claimable",
          body: `Implement public issue ${ordinal} without changing unrelated surfaces.`,
          branch: "main",
          baseCommit: commit,
          repo: "OpenAgentsInc/openagents",
          number: ordinal,
          verify: `bun test apps/pylon/tests/fixture-${ordinal}.test.ts`,
        },
  }
}

const lifecycle = (
  assignmentRef: string,
  accountRefHash: string,
): PylonAssignmentRunLifecycleEvent[] => [
  {
    schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
    event: "assignment_run.runtime_started",
    observedAt: fixedNow.toISOString(),
    assignmentRef,
    accountRefHash,
  },
  {
    schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
    event: "assignment_run.completed",
    observedAt: fixedNow.toISOString(),
    assignmentRef,
    status: "accepted",
  },
]

const acceptedAssignment = (
  assignmentRef: string,
  accountRefHash: string,
): PylonOwnedFleetRunAssignmentReceipt => ({
  accountRefHash,
  assignmentRef,
  closeout: {
    paymentMode: "no-spend",
    payoutClaimAllowed: false,
    settlementState: "not_applicable",
    status: "accepted",
  },
  lifecycle: lifecycle(assignmentRef, accountRefHash),
  ok: true,
})

const requestReceipt = (
  request: PylonKhalaRequestInput,
  assignmentRef = `assignment.public.${request.workflow}`,
  overrides: { pylonRef?: string; workflow?: string } = {},
) => ({
  assignmentRef,
  workflow: request.workflow ?? null,
  frames: [{
    data: "delegated",
    parsed: {
      openagents: {
        coding_delegation: {
          assignmentRef,
          pylonRef: overrides.pylonRef ?? request.targetPylonRef,
          workflowClass: overrides.workflow ?? request.workflow,
        },
      },
    },
  }],
})

const trace = (
  assignmentRef: string,
  state: PylonKhalaAssignmentTraceStatusResult["progress"]["state"],
  overrides: Partial<PylonKhalaAssignmentTraceStatusResult> = {},
): PylonKhalaAssignmentTraceStatusResult => ({
  assignmentRef,
  closeoutPolicy: state === "closed_out"
    ? {
        paymentMode: "no-spend",
        payoutClaimAllowed: false,
        settlementState: "not_applicable",
        source: "worker_closeout_event",
      }
    : {
        paymentMode: "unknown",
        payoutClaimAllowed: null,
        settlementState: "unknown",
        source: "unavailable",
      },
  events: {
    count: 1,
    latestEventKind: null,
    latestObservedAt: fixedNow.toISOString(),
    latestStatus: null,
    progressCount: 0,
  },
  generatedAt: fixedNow.toISOString(),
  lifecycle: {
    acceptedWorkRefs: [],
    artifactRefs: [],
    closeoutRefs: [],
    createdAt: fixedNow.toISOString(),
    proofRefs: [],
    rejectionRefs: state === "rejected" ? ["rejection.public.fixture"] : [],
    state,
    updatedAt: fixedNow.toISOString(),
  },
  ok: true,
  owner: {
    agentUserRef: "agent:owner",
    openauthUserRef: "user:owner",
  },
  progress: {
    closeoutReady: state === "closed_out",
    hasFinalTrace: state === "closed_out",
    hasLiveChunks: false,
    hasTokenUsage: state === "closed_out",
    missingReadinessRefs: [],
    state,
  },
  pylonRef,
  rawEventChunks: {
    byteLength: 0,
    count: 0,
    eventCount: 0,
    latestChunkRef: null,
    latestObservedAt: null,
    visibility: "owner_only",
  },
  rawEvents: {
    byteLength: 0,
    count: 0,
    eventCount: 0,
    latestObservedAt: null,
    latestRawEventRef: null,
    refs: [],
    visibility: "owner_only",
  },
  schemaVersion: "openagents.pylon.codex_assignment_trace_status.v1",
  tokenUsage: {
    cacheReadTokens: 0,
    demandKind: "own_capacity",
    demandSource: "khala_coding_delegation",
    inputTokens: 0,
    model: "openagents/pylon-codex",
    outputTokens: 0,
    provider: "pylon-codex-own-capacity",
    reasoningTokens: 0,
    refs: [],
    rowCount: 0,
    status: state === "closed_out" ? "recorded" : "pending",
    totalTokens: 0,
    usageTruth: "exact",
  },
  traces: {
    count: 0,
    finalTraceUuid: null,
    latestTraceUuid: null,
    refs: [],
    schemaVersion: "ATIF-v1.7",
    visibility: "owner_only",
  },
  ...overrides,
})

describe("Pylon-owned FleetRun runner", () => {
  test("runs Codex and Claude claims simultaneously on exact named accounts and pinned workspaces", async () => {
    const registry = [account("codex-a", "codex"), account("claude-a", "claude_agent")]
    const requests: PylonKhalaRequestInput[] = []
    const runs: Array<{ accountRef: string; assignmentRef: string }> = []
    let started = 0
    let release!: () => void
    const bothStarted = new Promise<void>(resolve => {
      release = resolve
    })
    const runner = createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      loadRegistry: async () => registry,
      request: async request => {
        requests.push(request)
        return requestReceipt(request)
      },
      runAssignment: async input => {
        runs.push(input)
        started += 1
        if (started === 2) release()
        await bothStarted
        const provider = input.accountRef.startsWith("claude") ? "claude_agent" : "codex"
        return acceptedAssignment(input.assignmentRef, hashPylonAccountRef(provider, input.accountRef))
      },
    })

    const [codex, claude] = await Promise.all([
      runner.dispatch(dispatchInput("codex", "codex-a", 1)),
      runner.dispatch(dispatchInput("claude", "claude-a", 2)),
    ])

    expect(codex.status).toBe("completed")
    expect(claude.status).toBe("completed")
    expect(requests.map(entry => entry.workflow).sort()).toEqual([
      "claude_agent_task",
      "codex_agent_task",
    ])
    expect(requests.map(entry => entry.targetAccountRefHash).sort()).toEqual([
      hashPylonAccountRef("claude_agent", "claude-a"),
      hashPylonAccountRef("codex", "codex-a"),
    ].sort())
    expect(requests.every(entry => entry.targetPylonRef === pylonRef)).toBe(true)
    expect(requests.every(entry => entry.workspace?.repository.commitSha === commit)).toBe(true)
    expect(requests.every(entry => entry.workspace?.repository.branch === "main")).toBe(true)
    expect(runs.map(entry => entry.accountRef).sort()).toEqual(["claude-a", "codex-a"])
  })

  test("coalesces a duplicate durable claim into one request and one assignment run", async () => {
    let requestCount = 0
    let runCount = 0
    const input = dispatchInput("codex", "codex-a", 3, "fixture")
    const runner = createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      loadRegistry: async () => [account("codex-a", "codex")],
      request: async request => {
        requestCount += 1
        return requestReceipt(request)
      },
      runAssignment: async request => {
        runCount += 1
        return acceptedAssignment(request.assignmentRef, hashPylonAccountRef("codex", "codex-a"))
      },
    })

    const [first, second] = await Promise.all([runner.dispatch(input), runner.dispatch(input)])

    expect(first).toEqual(second)
    expect(requestCount).toBe(1)
    expect(runCount).toBe(1)
  })

  test("rejects conflicting reuse of one claim ref instead of inheriting another account result", async () => {
    let requestCount = 0
    let runCount = 0
    const original = dispatchInput("codex", "codex-a", 31, "fixture")
    const conflicting = {
      ...original,
      accountRef: "codex-b",
      claim: { ...original.claim, workerAccountRef: "codex-b" },
    } satisfies FleetRunSupervisorDispatchInput
    const runner = createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      loadRegistry: async () => [account("codex-a", "codex"), account("codex-b", "codex")],
      request: async request => {
        requestCount += 1
        return requestReceipt(request)
      },
      runAssignment: async request => {
        runCount += 1
        return acceptedAssignment(request.assignmentRef, hashPylonAccountRef("codex", "codex-a"))
      },
    })

    const first = await runner.dispatch(original)
    const conflict = await runner.dispatch(conflicting)

    expect(first.status).toBe("completed")
    expect(conflict).toMatchObject({
      status: "failed",
      lifecycle: [{
        event: "assignment_run.no_assignment",
        blockerRefs: [PYLON_OWNED_FLEET_RUNNER_BLOCKERS.dispatchConflict],
      }],
    })
    expect(requestCount).toBe(1)
    expect(runCount).toBe(1)
  })

  test("bounds terminal single-flight retention while durable state remains the replay authority", async () => {
    const runner = createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      loadRegistry: async () => [],
    })
    await Promise.all(
      Array.from(
        { length: PYLON_OWNED_FLEET_RUNNER_MAX_TERMINAL_RETENTION + 20 },
        (_, index) => runner.dispatch(dispatchInput("codex", "codex-a", 1_000 + index, "fixture")),
      ),
    )

    expect(runner.retainedDispatchCount()).toBe(PYLON_OWNED_FLEET_RUNNER_MAX_TERMINAL_RETENTION)
  })

  test("never substitutes providers, default accounts, or Grok claims", async () => {
    let requestCount = 0
    const runner = createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      defaultHomes: {
        claudeAgent: "/tmp/default-claude",
        codex: "/tmp/default-codex",
      },
      loadRegistry: async () => [
        account("codex-a", "codex"),
        account("duplicate", "codex"),
        account("duplicate", "claude_agent"),
        { ...account("claude-a", "claude_agent"), home: "/tmp/default-claude" },
      ],
      request: async request => {
        requestCount += 1
        return requestReceipt(request)
      },
    })

    const [missingClaude, duplicate, defaultClaude, grok] = await Promise.all([
      runner.dispatch(dispatchInput("claude", "codex-a", 4, "fixture")),
      runner.dispatch(dispatchInput("codex", "duplicate", 41, "fixture")),
      runner.dispatch(dispatchInput("claude", "claude-a", 5, "fixture")),
      runner.dispatch(dispatchInput("grok", "grok-a", 6, "fixture")),
    ])

    expect(missingClaude.status).toBe("blocked")
    expect(duplicate.status).toBe("blocked")
    expect(defaultClaude.status).toBe("blocked")
    expect(grok).toMatchObject({
      assignmentRef: null,
      status: "blocked",
      summary: "Grok claimed-work custody is unavailable on this Pylon.",
    })
    expect(requestCount).toBe(0)
  })

  test("fails closed on missing, malformed, or mismatched assignment custody", async () => {
    const runCalls: string[] = []
    const makeRunner = (options: {
      request: (input: PylonKhalaRequestInput) => ReturnType<typeof requestReceipt>
      runAssignment?: (assignmentRef: string) => PylonOwnedFleetRunAssignmentReceipt
    }) => createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      loadRegistry: async () => [account("codex-a", "codex")],
      request: async input => options.request(input),
      runAssignment: async input => {
        runCalls.push(input.assignmentRef)
        return options.runAssignment?.(input.assignmentRef) ??
          acceptedAssignment(input.assignmentRef, hashPylonAccountRef("codex", "codex-a"))
      },
    })

    const missing = await makeRunner({
      request: request => requestReceipt(request, ""),
    }).dispatch(dispatchInput("codex", "codex-a", 7, "fixture"))
    const wrongDelegation = await makeRunner({
      request: request => requestReceipt(request, "assignment.public.wrong-target", {
        pylonRef: "pylon.other.owner",
      }),
    }).dispatch(dispatchInput("codex", "codex-a", 8, "fixture"))
    const wrongAssignment = await makeRunner({
      request: request => requestReceipt(request, "assignment.public.requested"),
      runAssignment: () => acceptedAssignment(
        "assignment.public.different",
        hashPylonAccountRef("codex", "codex-a"),
      ),
    }).dispatch(dispatchInput("codex", "codex-a", 9, "fixture"))
    const wrongAccount = await makeRunner({
      request: request => requestReceipt(request, "assignment.public.wrong-account"),
      runAssignment: assignmentRef => acceptedAssignment(
        assignmentRef,
        hashPylonAccountRef("codex", "codex-b"),
      ),
    }).dispatch(dispatchInput("codex", "codex-a", 10, "fixture"))

    expect(missing.status).toBe("failed")
    expect(wrongDelegation.status).toBe("failed")
    expect(wrongDelegation.lifecycle).toContainEqual(expect.objectContaining({
      blockerRefs: [PYLON_OWNED_FLEET_RUNNER_BLOCKERS.delegationMismatch],
    }))
    expect(wrongAssignment.status).toBe("failed")
    expect(wrongAssignment.lifecycle).toContainEqual(expect.objectContaining({
      blockerRefs: [PYLON_OWNED_FLEET_RUNNER_BLOCKERS.assignmentMismatch],
    }))
    expect(wrongAccount.status).toBe("failed")
    expect(wrongAccount.lifecycle).toContainEqual(expect.objectContaining({
      blockerRefs: [PYLON_OWNED_FLEET_RUNNER_BLOCKERS.accountMismatch],
    }))
    expect(runCalls).toEqual([
      "assignment.public.requested",
      "assignment.public.wrong-account",
    ])
  })

  test("isolates provider failures and never returns raw network or runner errors", async () => {
    const rawError = "SECRET bearer token from /Users/owner/private"
    const runner = createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      loadRegistry: async () => [account("codex-a", "codex"), account("claude-a", "claude_agent")],
      request: async request => {
        if (request.workflow === "codex_agent_task") throw new Error(rawError)
        return requestReceipt(request)
      },
      runAssignment: async request =>
        acceptedAssignment(request.assignmentRef, hashPylonAccountRef("claude_agent", request.accountRef)),
    })

    const [codex, claude] = await Promise.all([
      runner.dispatch(dispatchInput("codex", "codex-a", 11, "fixture")),
      runner.dispatch(dispatchInput("claude", "claude-a", 12, "fixture")),
    ])

    expect(codex.status).toBe("failed")
    expect(claude.status).toBe("completed")
    expect(JSON.stringify([codex, claude])).not.toContain(rawError)
    expect(JSON.stringify([codex, claude])).not.toContain("/Users")
    expect(codex.lifecycle).toContainEqual(expect.objectContaining({
      blockerRefs: [PYLON_OWNED_FLEET_RUNNER_BLOCKERS.requestFailed],
    }))
  })

  test("validates fixed authority and dispatch envelopes before any injected port", async () => {
    let portCalls = 0
    expect(() => createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef: "invalid pylon",
      baseUrl: "https://openagents.test",
    })).toThrow("pylonRef is invalid")
    expect(() => createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://owner:secret@openagents.test",
    })).toThrow("base URL")
    expect(() => createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      now: () => new Date("invalid"),
    })).toThrow("clock is invalid")

    const runner = createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      loadRegistry: async () => {
        portCalls += 1
        return [account("codex-a", "codex")]
      },
      request: async request => {
        portCalls += 1
        return requestReceipt(request)
      },
    })
    const valid = dispatchInput("codex", "codex-a", 42, "fixture")
    const invalid = {
      ...valid,
      accountRef: "/Users/owner/private",
      claim: { ...valid.claim, workerAccountRef: "/Users/owner/private" },
    } satisfies FleetRunSupervisorDispatchInput
    const result = await runner.dispatch(invalid)

    expect(result).toMatchObject({
      status: "blocked",
      lifecycle: [{ blockerRefs: [PYLON_OWNED_FLEET_RUNNER_BLOCKERS.dispatchInvalid] }],
    })
    expect(portCalls).toBe(0)
    expect(JSON.stringify(result)).not.toContain("/Users")

    let grokCalls = 0
    const grokRunner = createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      grok: {
        dispatch: async () => {
          grokCalls += 1
          return { assignmentRef: null, lifecycle: [], status: "accepted" }
        },
      },
    })
    const grokInput = dispatchInput("grok", "grok-a", 43, "fixture")
    const unsafeGrokInput = {
      ...grokInput,
      run: { ...grokInput.run, objective: "Read /Users/owner/private before running." },
    } satisfies FleetRunSupervisorDispatchInput
    const unsafeGrok = await grokRunner.dispatch(unsafeGrokInput)
    expect(unsafeGrok.status).toBe("blocked")
    expect(grokCalls).toBe(0)
  })

  test("reconciles exact assignment refs by inspection without requesting or rerunning", async () => {
    let requestCount = 0
    let runCount = 0
    const inspected: string[] = []
    const runner = createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      loadRegistry: async () => [],
      request: async request => {
        requestCount += 1
        return requestReceipt(request)
      },
      runAssignment: async request => {
        runCount += 1
        return acceptedAssignment(request.assignmentRef, "account.pylon.codex.fixture")
      },
      inspectAssignment: async assignmentRef => {
        inspected.push(assignmentRef)
        if (assignmentRef.endsWith("closed")) return trace(assignmentRef, "closed_out")
        if (assignmentRef.endsWith("rejected")) return trace(assignmentRef, "rejected")
        return trace(assignmentRef, "streaming_chunks")
      },
    })
    const active = (suffix: string): FleetRunSupervisorActiveAssignment => ({
      accountRef: "codex-a",
      claim: claim(`claim.reconcile.${suffix}`, "codex-a", `assignment.public.${suffix}`),
      contextId: `context.${suffix}`,
      taskId: `task.${suffix}`,
    })

    const reconciled = await runner.reconcile?.({
      activeAssignments: [active("closed"), active("active"), active("rejected")],
      now: fixedNow,
      run,
    })

    expect(reconciled?.map(entry => entry.status)).toEqual(["completed", "accepted", "failed"])
    expect(inspected).toEqual([
      "assignment.public.closed",
      "assignment.public.active",
      "assignment.public.rejected",
    ])
    expect(requestCount).toBe(0)
    expect(runCount).toBe(0)
  })

  test("keeps reconciliation inspection failures active and terminates mismatched custody safely", async () => {
    const runner = createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      inspectAssignment: async assignmentRef => {
        if (assignmentRef.endsWith("offline")) throw new Error("raw private network failure")
        return trace(assignmentRef, "streaming_chunks", { pylonRef: "pylon.other.owner" })
      },
    })
    const active = (suffix: string): FleetRunSupervisorActiveAssignment => ({
      accountRef: "codex-a",
      claim: claim(`claim.reconcile.${suffix}`, "codex-a", `assignment.public.${suffix}`),
      contextId: `context.${suffix}`,
      taskId: `task.${suffix}`,
    })

    const reconciled = await runner.reconcile?.({
      activeAssignments: [active("offline"), active("mismatch")],
      now: fixedNow,
      run,
    })

    expect(reconciled?.[0]).toMatchObject({
      status: "accepted",
      lifecycle: [{
        event: "assignment_run.runtime_progress",
        blockerRefs: [PYLON_OWNED_FLEET_RUNNER_BLOCKERS.requestFailed],
      }],
    })
    expect(reconciled?.[1]).toMatchObject({
      status: "failed",
      lifecycle: [{ event: "assignment_run.completed", status: "rejected" }],
    })
    expect(JSON.stringify(reconciled)).not.toContain("raw private network failure")
  })
})
