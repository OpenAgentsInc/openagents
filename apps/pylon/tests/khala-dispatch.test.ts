import { describe, expect, test } from "bun:test"

import {
  buildPylonKhalaDispatchPlan,
  classifyKhalaDispatchLifecycle,
  enforceSingleKhalaDispatchController,
  normalizeKhalaDispatchCandidateRefs,
  projectKhalaDispatchRecord,
  type KhalaDispatchAccountTarget,
} from "../src/khala-dispatch"
import {
  PYLON_DISPATCH_BREAKER_SCHEMA,
  type PylonDispatchBreakerSnapshot,
} from "../src/dispatch-failure-taxonomy"

const commit = "7ab7cb401803f6e04a6c93b7aa9102405de66419"
const verifier = {
  commit,
  command: "bun scripts/check-conflict-markers.mjs",
  repository: "OpenAgentsInc/openagents",
}

const account = (ref: string, hash: string): KhalaDispatchAccountTarget => ({
  accountRef: ref,
  accountRefHash: `account.pylon.codex.${hash}`,
  provider: "codex",
})

const accountDispatchBreaker = (
  accountRefHash: string,
): PylonDispatchBreakerSnapshot => ({
  accountRefHash,
  blockerRefs: [
    "blocker.pylon.dispatch.account_rate_limited",
    "blocker.pylon.dispatch.cooldown_active",
  ],
  contextId: "context.public.dispatch-breaker",
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

describe("pylon khala dispatch planning", () => {
  test("accepts structured candidate refs, account targets, concurrency, verifier, and priority lane", () => {
    const plan = buildPylonKhalaDispatchPlan({
      accountTargets: [
        account("codex-3", "abcdef123456"),
        account("codex-4", "fedcba654321"),
      ],
      candidateRefs: normalizeKhalaDispatchCandidateRefs(["pr:7557", "issue:7598"]),
      concurrency: 2,
      priorityLane: "rate-limit",
      targetPylonRef: "pylon.public.local",
      verifier,
    })

    expect(plan.schema).toBe("openagents.pylon.khala_dispatch_plan.v0.1")
    expect(plan.blockerRefs).toEqual([])
    expect(plan.concurrency).toBe(2)
    expect(plan.priorityLane).toBe("rate-limit")
    expect(plan.slots.map((slot) => slot.candidate.ref)).toEqual([
      "pr:7557",
      "issue:7598",
    ])
    expect(plan.slots.map((slot) => slot.account.accountRef)).toEqual([
      "codex-3",
      "codex-4",
    ])
    expect(plan.slots[0]?.requestInput).toMatchObject({
      targetAccountRefHash: "account.pylon.codex.abcdef123456",
      targetPylonRef: "pylon.public.local",
      workflow: "codex_agent_task",
    })
    expect(plan.slots[0]?.requestInput.workspace).toMatchObject({
      repository: {
        commitSha: commit,
        fullName: "OpenAgentsInc/openagents",
      },
      verificationCommand: {
        args: ["bun", "scripts/check-conflict-markers.mjs"],
      },
    })
  })

  test("filters active account-lane breakers before selecting dispatch slots", () => {
    // background_agents.dispatch.lane_account_breaker.v1
    const blocked = account("codex-3", "abcdef123456")
    const plan = buildPylonKhalaDispatchPlan({
      accountTargets: [
        blocked,
        account("codex-4", "fedcba654321"),
      ],
      candidateRefs: normalizeKhalaDispatchCandidateRefs(["issue:7598", "issue:7599"]),
      concurrency: 2,
      dispatchBreakers: [accountDispatchBreaker(blocked.accountRefHash)],
      priorityLane: "rate-limit",
      targetPylonRef: "pylon.public.local",
      verifier,
    })

    expect(plan.dispatchBreakers).toHaveLength(1)
    expect(plan.slots).toHaveLength(1)
    expect(plan.blockerRefs).toEqual([])
    expect(plan.slots[0]?.account.accountRef).toBe("codex-4")
    expect(plan.slots[0]?.candidate.ref).toBe("issue:7598")
  })
})

describe("pylon khala dispatch lifecycle", () => {
  test("keeps codex-3 filenames out of PR number and account lifecycle state", () => {
    const candidate = normalizeKhalaDispatchCandidateRefs(["pr:7557"])[0]
    const projection = projectKhalaDispatchRecord({
      account: account("codex-3", "abcdef123456"),
      candidate,
      events: [{ kind: "assignment_run.accepted" }],
      legacyFilename: "pr-review-20260629-codex-3-7557-rate-limit.log",
    }, "rate-limit")

    expect(projection).toMatchObject({
      accountRef: "codex-3",
      action: "hold",
      candidateRef: "pr:7557",
      lifecycle: "accepted_running",
      number: 7557,
      priorityLane: "rate-limit",
    })
  })

  test("releases a lock when an accepted run later completes rejected", () => {
    expect(
      classifyKhalaDispatchLifecycle([
        { kind: "assignment_run.accepted" },
        { kind: "assignment_run.completed", status: "rejected" },
      ]),
    ).toEqual({
      action: "release",
      finalStatus: "rejected",
      state: "completed_rejected",
    })
  })

  test("enforces a single active controller per namespace", () => {
    expect(
      enforceSingleKhalaDispatchController({
        activeControllerIds: ["controller.a"],
        namespace: "pr-review",
        requestedControllerId: "controller.b",
      }),
    ).toEqual({
      ok: false,
      blockerRefs: ["blocker.khala_dispatch.controller_conflict.pr-review"],
    })
    expect(
      enforceSingleKhalaDispatchController({
        activeControllerIds: ["controller.a"],
        namespace: "pr-review",
        requestedControllerId: "controller.a",
      }),
    ).toEqual({ ok: true, controllerId: "controller.a" })
  })
})
