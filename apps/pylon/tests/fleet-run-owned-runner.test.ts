import { describe, expect, test } from "bun:test"
import { createHash } from "node:crypto"
import type { PylonAssignmentRunLifecycleEvent } from "@openagentsinc/agent-runtime-schema"

import type { PylonAccountRegistryEntry } from "../src/account-registry.js"
import { hashPylonAccountRef } from "../src/account-registry.js"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap.js"
import type {
  PylonKhalaAssignmentTraceStatusResult,
  PylonKhalaCloseoutResult,
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
  marginalCostClass: "subscription",
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

const exactCloseout = (
  assignmentRef: string,
  harnessKind: "codex" | "claude" = assignmentRef.includes("claude") ? "claude" : "codex",
): PylonKhalaCloseoutResult => {
  const provider = harnessKind === "claude"
    ? "pylon-claude-own-capacity" as const
    : "pylon-codex-own-capacity" as const
  const model = harnessKind === "claude"
    ? "openagents/pylon-claude" as const
    : "openagents/pylon-codex" as const
  const tokenRef = `token_usage_event.public.${harnessKind}.${assignmentRef}`
  const closeoutRef = `closeout.public.${assignmentRef}`
  const proofRef = `proof.public.${assignmentRef}`
  const tokenUsage = {
    cacheReadTokens: 1,
    demandKind: "own_capacity" as const,
    demandSource: "khala_coding_delegation" as const,
    inputTokens: 5,
    model,
    outputTokens: 3,
    provider,
    reasoningTokens: 1,
    refs: [tokenRef],
    rowCount: 1,
    totalTokens: 8,
    usageTruth: "exact" as const,
  }
  const workerCloseout = {
    artifactRefs: ["artifact.public.fixture"],
    authorityReceiptRefs: ["receipt.public.fixture"],
    closeoutRefs: [closeoutRef],
    eventRef: "event.public.worker_closeout.fixture",
    observedAt: fixedNow.toISOString(),
    projectionBlockerRefs: [],
    proofRefs: [proofRef],
    resultRefs: ["result.public.fixture"],
    source: "worker_closeout_event",
    status: "closeout_submitted",
    testRefs: ["test.public.fixture"],
    verificationRefs: ["test.public.fixture"],
    visibility: "owner_only",
  } as const
  const status = {
    ...trace(assignmentRef, "closed_out", {
    lifecycle: {
      acceptedWorkRefs: ["work.public.fixture"],
      artifactRefs: ["artifact.public.fixture"],
      closeoutRefs: [closeoutRef],
      createdAt: fixedNow.toISOString(),
      proofRefs: [proofRef],
      rejectionRefs: [],
      state: "closed_out",
      updatedAt: fixedNow.toISOString(),
    },
    tokenUsage: { ...tokenUsage, status: "recorded" },
    }),
    workerCloseout,
  } as PylonKhalaAssignmentTraceStatusResult
  const checklistItem = { ok: true, ref: "check.public.fixture" }
  const proof = {
    assignmentRef,
    closeoutPolicy: status.closeoutPolicy,
    generatedAt: fixedNow.toISOString(),
    ok: true as const,
    owner: status.owner,
    pylonRef,
    rawEvents: {
      byteLength: 1,
      count: 1,
      eventCount: 1,
      refs: ["raw_event.public.fixture"],
      visibility: "owner_only" as const,
    },
    proofChecklist: {
      blockerRefs: [],
      items: [checklistItem],
      ok: true,
      schema: "openagents.pylon.khala_proof_checklist.v0.1" as const,
    },
    schemaVersion: "openagents.pylon.codex_assignment_proof.v1" as const,
    tokenUsage,
    traces: {
      count: 1,
      refs: ["trace.public.fixture"],
      schemaVersion: "ATIF-v1.7",
      visibility: "owner_only" as const,
    },
    workerCloseout,
  }
  return {
    assignmentRef,
    closeoutChecklist: {
      blockerRefs: [],
      caveatRefs: [],
      items: [checklistItem],
      ok: true,
      schema: "openagents.pylon.khala_closeout_checklist.v0.1",
    },
    ok: true,
    proof,
    schema: "openagents.pylon.khala_closeout.v0.1",
    status,
  }
}

const readExactCloseout = async (assignmentRef: string): Promise<PylonKhalaCloseoutResult> =>
  exactCloseout(assignmentRef)

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
      readCloseout: readExactCloseout,
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
    expect(codex).toMatchObject({
      accountRefHash: hashPylonAccountRef("codex", "codex-a"),
      closeoutRef: expect.stringMatching(/^closeout\.public\./),
      usageEvidence: {
        truth: "exact",
        harnessKind: "codex",
        provider: "pylon-codex-own-capacity",
        tokenRows: 1,
        totalTokens: 8,
        tokenUsageRefs: [expect.stringMatching(/^token_usage_event\.public\./)],
      },
    })
    expect(claude).toMatchObject({
      accountRefHash: hashPylonAccountRef("claude_agent", "claude-a"),
      closeoutRef: expect.stringMatching(/^closeout\.public\./),
      usageEvidence: {
        truth: "exact",
        harnessKind: "claude",
        provider: "pylon-claude-own-capacity",
      },
    })
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
    expect(JSON.stringify([codex, claude])).not.toMatch(/private|\/Users|bearer|token-secret/i)
  })

  test("serializes and joins slow lifecycle delivery before returning terminal evidence", async () => {
    const accountRefHash = hashPylonAccountRef("codex", "codex-a")
    let release!: () => void
    let entered!: () => void
    const gate = new Promise<void>(resolve => {
      release = resolve
    })
    const firstDelivery = new Promise<void>(resolve => {
      entered = resolve
    })
    const delivered: string[] = []
    let activeDeliveries = 0
    let maxActiveDeliveries = 0
    const runner = createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      readCloseout: readExactCloseout,
      loadRegistry: async () => [account("codex-a", "codex")],
      request: async request => requestReceipt(request),
      runAssignment: async input => {
        const started = {
          schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1" as const,
          event: "assignment_run.runtime_started" as const,
          observedAt: fixedNow.toISOString(),
          assignmentRef: input.assignmentRef,
          accountRefHash,
        }
        const progress = {
          schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1" as const,
          event: "assignment_run.runtime_progress" as const,
          observedAt: fixedNow.toISOString(),
          assignmentRef: input.assignmentRef,
          accountRefHash,
          status: "running" as const,
          phase: "runtime_active" as const,
        }
        void input.onLifecycle?.(started)
        void input.onLifecycle?.(progress)
        return acceptedAssignment(input.assignmentRef, accountRefHash)
      },
    })
    let settled = false
    const dispatched = runner.dispatch({
      ...dispatchInput("codex", "codex-a", 207, "fixture"),
      onLifecycle: async event => {
        activeDeliveries += 1
        maxActiveDeliveries = Math.max(maxActiveDeliveries, activeDeliveries)
        delivered.push(event.event)
        if (delivered.length === 1) {
          entered()
          await gate
        }
        activeDeliveries -= 1
      },
    }).finally(() => {
      settled = true
    })

    await firstDelivery
    await Bun.sleep(2)
    expect(settled).toBe(false)
    expect(delivered).toEqual(["assignment_run.runtime_started"])
    release()
    const result = await dispatched
    expect(result.status).toBe("completed")
    expect(maxActiveDeliveries).toBe(1)
    expect(delivered).toEqual([
      "assignment_run.runtime_started",
      "assignment_run.runtime_progress",
    ])
    await Bun.sleep(2)
    expect(delivered).toHaveLength(2)
  })

  test("fails closed when exact closeout evidence is missing or mismatched", async () => {
    const makeRunner = (readCloseout: (assignmentRef: string) => Promise<PylonKhalaCloseoutResult>) =>
      createPylonOwnedFleetRunSupervisorRunner({
        summary,
        pylonRef,
        baseUrl: "https://openagents.test",
        loadRegistry: async () => [account("codex-a", "codex")],
        request: async request => requestReceipt(request),
        runAssignment: async request => acceptedAssignment(
          request.assignmentRef,
          hashPylonAccountRef("codex", "codex-a"),
        ),
        readCloseout,
      })
    const missingChecklist = await makeRunner(async assignmentRef => ({
      ...exactCloseout(assignmentRef),
      closeoutChecklist: {
        ...exactCloseout(assignmentRef).closeoutChecklist,
        blockerRefs: ["blocker.public.fixture"],
        ok: false,
      },
    })).dispatch(dispatchInput("codex", "codex-a", 201, "fixture"))
    const mismatched = await makeRunner(async assignmentRef => ({
      ...exactCloseout(assignmentRef),
      assignmentRef: "assignment.public.different",
    })).dispatch(dispatchInput("codex", "codex-a", 202, "fixture"))
    const missingWorkerEvidence = await makeRunner(async assignmentRef => {
      const closeout = exactCloseout(assignmentRef)
      const { workerCloseout: _statusWorkerCloseout, ...status } = closeout.status as
        PylonKhalaAssignmentTraceStatusResult & { workerCloseout?: unknown }
      const { workerCloseout: _proofWorkerCloseout, ...proof } = closeout.proof as
        typeof closeout.proof & { workerCloseout?: unknown }
      return { ...closeout, status, proof }
    }).dispatch(dispatchInput("codex", "codex-a", 205, "fixture"))
    const incoherentUsage = await makeRunner(async assignmentRef => {
      const closeout = exactCloseout(assignmentRef)
      const tokenUsage = { ...closeout.proof.tokenUsage, totalTokens: 9 }
      return {
        ...closeout,
        proof: { ...closeout.proof, tokenUsage },
        status: {
          ...closeout.status,
          tokenUsage: { ...tokenUsage, status: "recorded" as const },
        },
      }
    }).dispatch(dispatchInput("codex", "codex-a", 203, "fixture"))
    const reasoningOverflow = await makeRunner(async assignmentRef => {
      const closeout = exactCloseout(assignmentRef)
      const tokenUsage = { ...closeout.proof.tokenUsage, reasoningTokens: 4 }
      return {
        ...closeout,
        proof: { ...closeout.proof, tokenUsage },
        status: {
          ...closeout.status,
          tokenUsage: { ...tokenUsage, status: "recorded" as const },
        },
      }
    }).dispatch(dispatchInput("codex", "codex-a", 204, "fixture"))

    for (const result of [
      missingChecklist,
      mismatched,
      missingWorkerEvidence,
      incoherentUsage,
      reasoningOverflow,
    ]) {
      expect(result).toMatchObject({
        accountRefHash: null,
        closeoutRef: null,
        status: "failed",
        usageEvidence: null,
      })
      expect(result.lifecycle).toContainEqual(expect.objectContaining({
        blockerRefs: [PYLON_OWNED_FLEET_RUNNER_BLOCKERS.usageEvidenceInvalid],
      }))
    }
  })

  test("opaque-digests path-like worker evidence before attempt projection", async () => {
    const runner = createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      loadRegistry: async () => [account("codex-a", "codex")],
      request: async request => requestReceipt(request),
      runAssignment: async request => acceptedAssignment(
        request.assignmentRef,
        hashPylonAccountRef("codex", "codex-a"),
      ),
      readCloseout: async assignmentRef => {
        const closeout = exactCloseout(assignmentRef)
        const pathLikes = [
          "Users/owner/private/worktree/artifact-a",
          "Users/owner/private/worktree/artifact-b",
        ]
        const status = closeout.status as PylonKhalaAssignmentTraceStatusResult & {
          workerCloseout: { artifactRefs: string[] }
        }
        const proof = closeout.proof as typeof closeout.proof & {
          workerCloseout: { artifactRefs: string[] }
        }
        return {
          ...closeout,
          status: {
            ...status,
            workerCloseout: { ...status.workerCloseout, artifactRefs: pathLikes },
          },
          proof: {
            ...proof,
            workerCloseout: { ...proof.workerCloseout, artifactRefs: pathLikes },
          },
        }
      },
    })

    const result = await runner.dispatch(dispatchInput("codex", "codex-a", 206, "fixture"))
    expect(result.status).toBe("completed")
    expect(result.artifactRefs).toHaveLength(2)
    expect(new Set(result.artifactRefs).size).toBe(2)
    expect(result.artifactRefs).toEqual(expect.arrayContaining([
      expect.stringMatching(/^artifact\.public\.pylon\.opaque\.[a-f0-9]{24}$/),
      expect.stringMatching(/^artifact\.public\.pylon\.opaque\.[a-f0-9]{24}$/),
    ]))
    expect(result.verification?.evidenceRefs).toEqual(["test.public.fixture"])
    expect(JSON.stringify(result)).not.toContain("Users/owner/private")
  })

  test("fails closed instead of truncating or deduplicating within-role worker evidence refs", async () => {
    const withWorkerEvidence = (
      closeout: PylonKhalaCloseoutResult,
      update: (worker: Record<string, unknown>) => Record<string, unknown>,
    ): PylonKhalaCloseoutResult => {
      const status = closeout.status as PylonKhalaAssignmentTraceStatusResult & {
        workerCloseout: Record<string, unknown>
      }
      const proof = closeout.proof as typeof closeout.proof & {
        workerCloseout: Record<string, unknown>
      }
      const workerCloseout = update(status.workerCloseout)
      return {
        ...closeout,
        status: { ...status, workerCloseout },
        proof: { ...proof, workerCloseout },
      } as PylonKhalaCloseoutResult
    }
    const invalidEvidence = [
      (closeout: PylonKhalaCloseoutResult) => withWorkerEvidence(closeout, worker => ({
        ...worker,
        artifactRefs: ["artifact.public.duplicate", "artifact.public.duplicate"],
      })),
      (closeout: PylonKhalaCloseoutResult) => withWorkerEvidence(closeout, worker => ({
        ...worker,
        testRefs: Array.from(
          { length: 33 },
          (_, index) => `test.public.combined_overflow.${index}`,
        ),
        verificationRefs: Array.from(
          { length: 33 },
          (_, index) => `verification.public.combined_overflow.${index}`,
        ),
      })),
      (closeout: PylonKhalaCloseoutResult) => {
        const unsafe = "Users/owner/private/colliding-artifact"
        const collision = `artifact.public.pylon.opaque.${createHash("sha256")
          .update(unsafe)
          .digest("hex")
          .slice(0, 24)}`
        return withWorkerEvidence(closeout, worker => ({
          ...worker,
          artifactRefs: [unsafe],
          proofRefs: [collision],
        }))
      },
    ]

    for (const [index, mutate] of invalidEvidence.entries()) {
      const runner = createPylonOwnedFleetRunSupervisorRunner({
        summary,
        pylonRef,
        baseUrl: "https://openagents.test",
        loadRegistry: async () => [account("codex-a", "codex")],
        request: async request => requestReceipt(request),
        runAssignment: async request => acceptedAssignment(
          request.assignmentRef,
          hashPylonAccountRef("codex", "codex-a"),
        ),
        readCloseout: async assignmentRef => mutate(exactCloseout(assignmentRef)),
      })

      const result = await runner.dispatch(
        dispatchInput("codex", "codex-a", 260 + index, "fixture"),
      )
      expect(result).toMatchObject({
        accountRefHash: null,
        closeoutRef: null,
        status: "failed",
        usageEvidence: null,
      })
      expect(result.lifecycle).toContainEqual(expect.objectContaining({
        blockerRefs: [PYLON_OWNED_FLEET_RUNNER_BLOCKERS.usageEvidenceInvalid],
      }))
    }

    const manyArtifactsRunner = createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      loadRegistry: async () => [account("codex-a", "codex")],
      request: async request => requestReceipt(request),
      runAssignment: async request => acceptedAssignment(
        request.assignmentRef,
        hashPylonAccountRef("codex", "codex-a"),
      ),
      readCloseout: async assignmentRef => withWorkerEvidence(
        exactCloseout(assignmentRef),
        worker => ({
          ...worker,
          artifactRefs: Array.from(
            { length: 60 },
            (_, index) => `artifact.public.role_bounded.${index}`,
          ),
          closeoutRefs: Array.from(
            { length: 100 },
            (_, index) => `closeout.public.source_role.${index}`,
          ),
          resultRefs: Array.from(
            { length: 100 },
            (_, index) => `result.public.source_role.${index}`,
          ),
        }),
      ),
    })
    const manyArtifacts = await manyArtifactsRunner.dispatch(
      dispatchInput("codex", "codex-a", 269, "fixture"),
    )
    expect(manyArtifacts).toMatchObject({ status: "completed" })
    expect(manyArtifacts.artifactRefs).toHaveLength(60)
  })

  test("coalesces a duplicate durable claim into one request and one assignment run", async () => {
    let requestCount = 0
    let runCount = 0
    const input = dispatchInput("codex", "codex-a", 3, "fixture")
    const runner = createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      readCloseout: readExactCloseout,
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
      readCloseout: readExactCloseout,
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
      readCloseout: readExactCloseout,
      loadRegistry: async () => [account("codex-a", "codex")],
    })
    await Promise.all(
      Array.from(
        { length: PYLON_OWNED_FLEET_RUNNER_MAX_TERMINAL_RETENTION + 20 },
        (_, index) => runner.dispatch(dispatchInput("codex", "codex-a", 1_000 + index, "fixture")),
      ),
    )

    expect(runner.retainedDispatchCount()).toBe(PYLON_OWNED_FLEET_RUNNER_MAX_TERMINAL_RETENTION)
  })

  test("exposes honest exact-attempt steering control from the production-owned runner", async () => {
    let state: PylonKhalaAssignmentTraceStatusResult["progress"]["state"] = "streaming_chunks"
    const runner = createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      readCloseout: readExactCloseout,
      loadRegistry: async () => [account("codex-a", "codex")],
      inspectAssignment: async assignmentRef => trace(assignmentRef, state),
    })
    const attempt = {
      pylonRef,
      runRef: run.runRef,
      claimRef: "claim.sarah_fleet_run.111111111111111111111111",
      workUnitRef: "work_unit.runner.steering",
      workClaimRef: "claim.runner.steering",
      assignmentRef: "assignment.runner.steering",
    }
    expect(await runner.steeringControl.applySteer({
      ...attempt,
      body: "owner-private direction",
      bodyRef: null,
    })).toEqual({
      state: "failed",
      failureRef: "blocker.pylon.fleet_steering.next_turn_control_unavailable",
    })
    expect(await runner.steeringControl.observeStop({
      pylonRef,
      runRef: run.runRef,
      claimRef: attempt.claimRef,
      attempts: [attempt],
    })).toEqual({
      state: "retry",
      failureRef: "blocker.pylon.fleet_steering.stop_waiting_for_terminal_attempts",
    })
    state = "closed_out"
    expect(await runner.steeringControl.applyApproval({
      ...attempt,
      approvalRef: "approval.public.runner.steering",
      decision: "allow",
    })).toEqual({
      state: "stale",
      failureRef: "blocker.pylon.fleet_steering.attempt_terminal",
    })
  })

  test("never substitutes providers, default accounts, or Grok claims", async () => {
    let requestCount = 0
    const runner = createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      readCloseout: readExactCloseout,
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
      readCloseout: readExactCloseout,
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
      readCloseout: readExactCloseout,
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
      readCloseout: readExactCloseout,
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
      readCloseout: readExactCloseout,
      now: () => new Date("invalid"),
    })).toThrow("clock is invalid")

    const runner = createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      readCloseout: readExactCloseout,
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
      readCloseout: readExactCloseout,
      grok: {
        dispatch: async () => {
          grokCalls += 1
          return {
            accountRefHash: null,
            assignmentRef: null,
            closeoutRef: null,
            lifecycle: [],
            status: "accepted",
            usageEvidence: null,
          }
        },
        reconcile: async ({ active }) => ({
          accountRefHash: null,
          assignmentRef: active.claim.assignmentRef,
          closeoutRef: null,
          lifecycle: [],
          status: "accepted",
          taskId: active.taskId,
          usageEvidence: null,
        }),
        probeLiveness: async () => "unknown",
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
      readCloseout: readExactCloseout,
      loadRegistry: async () => [{
        ...account("codex-a", "codex"),
        marginalCostClass: "api_metered",
      }],
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
        if (assignmentRef.endsWith("closed")) return exactCloseout(assignmentRef).status
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
    expect(reconciled?.[0]).toMatchObject({
      accountRefHash: hashPylonAccountRef("codex", "codex-a"),
      closeoutRef: expect.stringMatching(/^closeout\.public\./),
      usageEvidence: { truth: "exact", harnessKind: "codex", totalTokens: 8 },
      marginalCostClass: "subscription",
    })
    expect(reconciled?.[1]).toMatchObject({
      accountRefHash: null,
      closeoutRef: null,
      usageEvidence: null,
    })
    expect(inspected).toEqual([
      "assignment.public.closed",
      "assignment.public.active",
      "assignment.public.rejected",
    ])
    expect(requestCount).toBe(0)
    expect(runCount).toBe(0)
  })

  test("derives the same verifier ref before and after restart from worker closeout evidence", async () => {
    const runner = createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      loadRegistry: async () => [account("codex-a", "codex")],
      request: async request => requestReceipt(request),
      runAssignment: async request => acceptedAssignment(
        request.assignmentRef,
        hashPylonAccountRef("codex", "codex-a"),
      ),
      readCloseout: readExactCloseout,
      inspectAssignment: async assignmentRef => exactCloseout(assignmentRef).status,
    })
    const input = dispatchInput("codex", "codex-a", 55, "fixture")
    const dispatched = await runner.dispatch(input)
    if (dispatched.assignmentRef === null) {
      throw new Error("expected exact dispatched assignment")
    }
    const reconciled = await runner.reconcile({
      activeAssignments: [{
        accountRef: input.accountRef,
        claim: { ...input.claim, assignmentRef: dispatched.assignmentRef },
        contextId: "context.verifier.restart",
        taskId: input.taskId,
      }],
      now: fixedNow,
      run,
    })

    expect(dispatched.status).toBe("completed")
    expect(reconciled[0]?.status).toBe("completed")
    expect(reconciled[0]?.verification?.verifierRef).toBe(
      dispatched.verification?.verifierRef,
    )
  })

  test("never restart-promotes a closed verifier rejection to accepted", async () => {
    const assignmentRef = "assignment.public.rejected_verifier_restart"
    const closeout = exactCloseout(assignmentRef)
    const rejectWorker = <T extends { workerCloseout?: unknown }>(value: T): T => ({
      ...value,
      workerCloseout: {
        ...(value.workerCloseout as Record<string, unknown>),
        status: "rejected",
      },
    })
    const rejectedStatus = rejectWorker(closeout.status as
      PylonKhalaAssignmentTraceStatusResult & { workerCloseout?: unknown })
    const runner = createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      loadRegistry: async () => [account("codex-a", "codex")],
      inspectAssignment: async () => rejectedStatus,
      readCloseout: async () => ({
        ...closeout,
        status: rejectedStatus,
        proof: rejectWorker(closeout.proof as typeof closeout.proof & { workerCloseout?: unknown }),
      }),
    })
    const active: FleetRunSupervisorActiveAssignment = {
      accountRef: "codex-a",
      claim: claim(
        "claim.reconcile.rejected_verifier_restart",
        "codex-a",
        assignmentRef,
      ),
      contextId: "context.rejected_verifier_restart",
      taskId: "task.rejected_verifier_restart",
    }

    const [result] = await runner.reconcile?.({
      activeAssignments: [active],
      now: fixedNow,
      run,
    }) ?? []
    expect(result).toMatchObject({
      status: "failed",
      usageEvidence: null,
      lifecycle: [{ status: "rejected" }],
    })
  })

  test("keeps reconciliation inspection failures active and terminates mismatched custody safely", async () => {
    const runner = createPylonOwnedFleetRunSupervisorRunner({
      summary,
      pylonRef,
      baseUrl: "https://openagents.test",
      readCloseout: readExactCloseout,
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
