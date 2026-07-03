import { describe, expect, test } from "bun:test"
import {
  buildFleetRunSmokePlan,
  closeoutEvidenceFromPayload,
  runFleetRunSmokeFromEnv,
  type FleetRunSmokeCloseoutEvidence,
  type FleetRunSmokeManager,
  type FleetRunSmokeManagerStartInput,
  type FleetRunSmokeSnapshot,
} from "../src/fleet-run-live-smoke"

const commit = "0123456789abcdef0123456789abcdef01234567"

const liveEnv = {
  OPENAGENTS_AGENT_TOKEN: "oa_agent_secret",
  PYLON_FLEET_RUN_LIVE_ARM: "1",
  PYLON_FLEET_RUN_LIVE_COMMIT: commit,
  PYLON_FLEET_RUN_LIVE_ISSUES: "7854,7900",
  PYLON_FLEET_RUN_LIVE_POLL_MS: "100",
  PYLON_FLEET_RUN_LIVE_PYLON_REF: "pylon.local.test",
  PYLON_FLEET_RUN_LIVE_REPO: "OpenAgentsInc/openagents",
  PYLON_FLEET_RUN_LIVE_VERIFY: "bun test apps/pylon/tests/fleet-run-live-smoke.test.ts",
}

const sustainedEnv = {
  OPENAGENTS_AGENT_TOKEN: "oa_agent_secret",
  PYLON_FLEET_RUN_SUSTAINED_ARM: "1",
  PYLON_FLEET_RUN_SUSTAINED_COMMIT: commit,
  PYLON_FLEET_RUN_SUSTAINED_ISSUES: "7854,7900,7901,7902,7904,7905,7906",
  PYLON_FLEET_RUN_SUSTAINED_POLL_MS: "100",
  PYLON_FLEET_RUN_SUSTAINED_PYLON_REF: "pylon.local.test",
  PYLON_FLEET_RUN_SUSTAINED_REPO: "OpenAgentsInc/openagents",
  PYLON_FLEET_RUN_SUSTAINED_VERIFY: "bun test apps/pylon/tests/fleet-run-live-smoke.test.ts",
}

const sustainedPlanNodes = Array.from({ length: 7 }, (_, index) => ({
  ref: `node-${index + 1}`,
  title: `Sustained evidence node ${index + 1}`,
  objective: `Run bounded public-safe sustained smoke evidence node ${index + 1}. Do not open a PR.`,
}))

const sustainedPlanDagEnv = {
  ...sustainedEnv,
  PYLON_FLEET_RUN_SUSTAINED_WORK_SOURCE: "plan_dag",
  PYLON_FLEET_RUN_SUSTAINED_PLAN_REF: "plan.qa8034.test",
  PYLON_FLEET_RUN_SUSTAINED_PLAN_NODES_JSON: JSON.stringify(sustainedPlanNodes),
  PYLON_FLEET_RUN_SUSTAINED_ISSUES: undefined,
}

const snapshot = (input: {
  readonly assignmentRefs?: readonly string[]
  readonly completedAssignments?: number
  readonly dispatchFailures?: readonly string[]
  readonly failedAssignments?: number
  readonly state?: string
  readonly targetConcurrency?: number
  readonly workUnitRefs?: readonly string[]
}): FleetRunSmokeSnapshot => ({
  active: input.state !== "completed",
  lifecycle: [
    ...(input.assignmentRefs ?? []).map((assignmentRef, index) => ({
      assignmentRef,
      claimRef: `claim.test.${index}`,
      kind: "dispatch",
      status: "completed",
      workUnitRef: input.workUnitRefs?.[index] ?? `github.issue.${index}`,
    })),
    ...(input.dispatchFailures ?? []).map((summary, index) => ({
      assignmentRef: null,
      claimRef: `claim.failed.${index}`,
      kind: "dispatch",
      status: "failed",
      summary,
      workUnitRef: input.workUnitRefs?.[index] ?? `github.issue.failed.${index}`,
    })),
  ],
  pylonRef: "pylon.local.test",
  run: {
    counters: {
      activeAssignments: 0,
      blockedAssignments: 0,
      completedAssignments: input.completedAssignments ?? input.assignmentRefs?.length ?? 0,
      failedAssignments: input.failedAssignments ?? input.dispatchFailures?.length ?? 0,
      workUnitsTotal: (input.assignmentRefs?.length ?? 0) + (input.dispatchFailures?.length ?? 0),
    },
    runRef: "fleet_run.test",
    state: input.state ?? "completed",
    targetConcurrency: input.targetConcurrency ?? 2,
    workerKind: "codex",
    workSource: "issue_list",
  },
})

const managerReturning = (snapshots: readonly FleetRunSmokeSnapshot[]): FleetRunSmokeManager => {
  let index = 0
  return {
    start: async () => snapshots[0] ?? snapshot({}),
    status: async () => {
      index = Math.min(index + 1, snapshots.length - 1)
      return snapshots[index] ?? snapshots[0] ?? snapshot({})
    },
  }
}

const closeout = (
  assignmentRef: string,
  overrides: Partial<FleetRunSmokeCloseoutEvidence> = {},
): FleetRunSmokeCloseoutEvidence => ({
  assignmentRef,
  blockerRefs: [],
  closeoutChecklistOk: true,
  demandSource: "khala_coding_delegation",
  ok: true,
  proofChecklistOk: true,
  rawEventCount: 1,
  statusState: "closed_out",
  tokenRefs: [`d1:token_usage_events:${assignmentRef}`],
  tokenRows: 1,
  totalTokens: 100,
  traceCount: 1,
  usageTruth: "exact",
  ...overrides,
})

const counterFetch = (values: number[]): typeof fetch => async () =>
  new Response(JSON.stringify({ tokensServed: values.shift() ?? values.at(-1) ?? 0 }), {
    headers: { "Content-Type": "application/json" },
  })

describe("fleet run live smoke", () => {
  test("skips by default without constructing a live supervisor", async () => {
    const result = await runFleetRunSmokeFromEnv("live", {
      createManager: () => {
        throw new Error("manager should not be constructed for an unarmed smoke")
      },
      env: {},
    })

    expect(result.ok).toBe(true)
    expect(result.skipped).toBe(true)
    expect(result.message).toContain("Skipped by default")
  })

  test("validates live pins and issue cardinality before dispatch", async () => {
    const result = await runFleetRunSmokeFromEnv("live", {
      env: {
        ...liveEnv,
        PYLON_FLEET_RUN_LIVE_COMMIT: "not-a-sha",
        PYLON_FLEET_RUN_LIVE_ISSUES: "7854,7854,7900",
        PYLON_FLEET_RUN_LIVE_TARGET: "3",
      },
    })

    expect(result.ok).toBe(false)
    expect(result.failures).toContain("PYLON_FLEET_RUN_LIVE_COMMIT must be a pinned 40-character commit SHA")
    expect(result.failures).toContain("PYLON_FLEET_RUN_LIVE_ISSUES must name distinct issues")
    expect(result.failures).toContain("PYLON_FLEET_RUN_LIVE_ISSUES must contain exactly 2 positive issue numbers")
    expect(result.failures).toContain("PYLON_FLEET_RUN_LIVE_TARGET must be exactly 2")
  })

  test("passes live smoke only with exact closeouts and public counter reconciliation", async () => {
    const result = await runFleetRunSmokeFromEnv("live", {
      closeoutReader: async assignmentRef => closeout(assignmentRef),
      env: liveEnv,
      fetch: counterFetch([1_000, 1_250]),
      manager: managerReturning([
        snapshot({
          assignmentRefs: ["assignment.public.one", "assignment.public.two"],
          completedAssignments: 2,
          targetConcurrency: 2,
          workUnitRefs: ["issue.7854", "issue.7900"],
        }),
      ]),
    })

    expect(result.ok).toBe(true)
    expect(result.skipped).toBe(false)
    expect(result.evidence?.closeouts).toHaveLength(2)
    expect(result.evidence?.tokenRows).toBe(2)
    expect(result.evidence?.totalTokens).toBe(200)
    expect(result.evidence?.publicCounterReconciliation).toMatchObject({
      delta: 250,
      expectedMinimumDelta: 200,
      ok: true,
      state: "checked",
    })
  })

  test("fails when duplicate work-unit claims are observed", async () => {
    const result = await runFleetRunSmokeFromEnv("live", {
      closeoutReader: async assignmentRef => closeout(assignmentRef),
      env: liveEnv,
      fetch: counterFetch([1_000, 1_300]),
      manager: managerReturning([
        snapshot({
          assignmentRefs: ["assignment.public.one", "assignment.public.two"],
          completedAssignments: 2,
          workUnitRefs: ["issue.7854", "issue.7854"],
        }),
      ]),
    })

    expect(result.ok).toBe(false)
    expect(result.failures.join("\n")).toContain("expected zero duplicate work-unit claims")
  })

  test("does not accept public counter movement without exact closeout rows", async () => {
    const result = await runFleetRunSmokeFromEnv("live", {
      closeoutReader: async assignmentRef => closeout(assignmentRef, {
        tokenRefs: [],
        tokenRows: 0,
        totalTokens: 0,
      }),
      env: liveEnv,
      fetch: counterFetch([1_000, 2_000]),
      manager: managerReturning([
        snapshot({
          assignmentRefs: ["assignment.public.one", "assignment.public.two"],
          completedAssignments: 2,
        }),
      ]),
    })

    expect(result.ok).toBe(false)
    expect(result.failures.join("\n")).toContain("expected positive exact token row evidence")
    expect(result.failures.join("\n")).toContain("expected aggregate exact token rows and verified tokens to be positive")
  })

  test("surfaces failed dispatch summaries in the smoke result", async () => {
    const result = await runFleetRunSmokeFromEnv("live", {
      env: liveEnv,
      fetch: counterFetch([1_000, 1_000]),
      manager: managerReturning([
        snapshot({
          dispatchFailures: [
            "pylon khala request failed (409): stale heartbeat",
          ],
          failedAssignments: 1,
          state: "stopped",
          workUnitRefs: ["github:OpenAgentsInc/openagents:issue:8060"],
        }),
      ]),
    })

    expect(result.ok).toBe(false)
    expect(result.evidence?.dispatchFailures).toContain(
      "github:OpenAgentsInc/openagents:issue:8060: pylon khala request failed (409): stale heartbeat",
    )
    expect(result.failures.join("\n")).toContain("dispatch failure: github:OpenAgentsInc/openagents:issue:8060")
  })

  test("enforces sustained thresholds before dispatch", async () => {
    const result = await runFleetRunSmokeFromEnv("sustained", {
      env: {
        ...sustainedEnv,
        PYLON_FLEET_RUN_SUSTAINED_DURATION_MINUTES: "29",
        PYLON_FLEET_RUN_SUSTAINED_ISSUES: "7854,7900,7901,7902,7904,7905",
        PYLON_FLEET_RUN_SUSTAINED_MIN_REFILLS: "1",
        PYLON_FLEET_RUN_SUSTAINED_TARGET: "4",
      },
    })

    expect(result.ok).toBe(false)
    expect(result.failures).toContain("PYLON_FLEET_RUN_SUSTAINED_TARGET must be at least 5")
    expect(result.failures).toContain("PYLON_FLEET_RUN_SUSTAINED_MIN_REFILLS must be at least 2")
    expect(result.failures).toContain("PYLON_FLEET_RUN_SUSTAINED_DURATION_MINUTES must be at least 30")
    expect(result.failures).toContain("PYLON_FLEET_RUN_SUSTAINED_ISSUES must contain at least 7 distinct issue numbers")
  })

  test("arms sustained plan-DAG source without requiring issue-list cardinality", async () => {
    let started: FleetRunSmokeManagerStartInput | null = null
    let current = new Date("2026-07-02T00:00:00.000Z")
    const refs = Array.from({ length: 7 }, (_, index) => `assignment.public.plan.${index + 1}`)
    const result = await runFleetRunSmokeFromEnv("sustained", {
      closeoutReader: async assignmentRef => closeout(assignmentRef),
      env: sustainedPlanDagEnv,
      fetch: counterFetch([10_000, 10_900]),
      manager: {
        start: async input => {
          started = input
          return snapshot({
            assignmentRefs: refs.slice(0, 5),
            completedAssignments: 5,
            state: "running",
            targetConcurrency: 5,
            workUnitRefs: refs.slice(0, 5).map((_, index) => `plan_dag:plan.qa8034.test:node:node-${index + 1}`),
          })
        },
        status: async () => snapshot({
          assignmentRefs: refs,
          completedAssignments: 7,
          state: "completed",
          targetConcurrency: 5,
          workUnitRefs: refs.map((_, index) => `plan_dag:plan.qa8034.test:node:node-${index + 1}`),
        }),
      },
      now: () => current,
      sleep: async () => {
        current = new Date(current.getTime() + 30 * 60 * 1000)
      },
    })

    expect(result.ok).toBe(true)
    expect(result.workSource).toBe("plan_dag")
    expect(result.planRef).toBe("plan.qa8034.test")
    expect(started?.workSource).toBe("plan_dag")
    expect(started?.planRef).toBe("plan.qa8034.test")
    expect(started?.planNodes).toHaveLength(7)
    expect(started?.issues).toEqual([])
  })

  test("validates sustained plan-DAG node count and structure before dispatch", async () => {
    const result = await runFleetRunSmokeFromEnv("sustained", {
      env: {
        ...sustainedPlanDagEnv,
        PYLON_FLEET_RUN_SUSTAINED_PLAN_NODES_JSON: JSON.stringify([
          { ref: "node-1", title: "one", objective: "one" },
          { ref: "node-1", title: "duplicate", objective: "duplicate" },
          { ref: "node-3", title: "missing objective" },
        ]),
      },
    })

    expect(result.ok).toBe(false)
    expect(result.failures).toContain("PYLON_FLEET_RUN_SUSTAINED_PLAN_NODES_JSON must contain at least 7 distinct plan node(s)")
    expect(result.failures).toContain("PYLON_FLEET_RUN_SUSTAINED_PLAN_NODES_JSON must name distinct plan node refs")
    expect(result.failures).toContain("PYLON_FLEET_RUN_SUSTAINED_PLAN_NODES_JSON[2].objective is required")
  })

  test("passes sustained smoke after duration, refill, closeout, and counter evidence", async () => {
    let current = new Date("2026-07-02T00:00:00.000Z")
    const refs = Array.from({ length: 7 }, (_, index) => `assignment.public.${index + 1}`)
    const result = await runFleetRunSmokeFromEnv("sustained", {
      closeoutReader: async assignmentRef => closeout(assignmentRef),
      env: sustainedEnv,
      fetch: counterFetch([10_000, 10_900]),
      manager: managerReturning([
        snapshot({
          assignmentRefs: refs.slice(0, 5),
          completedAssignments: 5,
          state: "running",
          targetConcurrency: 5,
        }),
        snapshot({
          assignmentRefs: refs,
          completedAssignments: 7,
          state: "completed",
          targetConcurrency: 5,
        }),
      ]),
      now: () => current,
      sleep: async () => {
        current = new Date(current.getTime() + 30 * 60 * 1000)
      },
    })

    expect(result.ok).toBe(true)
    expect(result.evidence?.refillsObserved).toBe(2)
    expect(result.evidence?.runObservedDurationMs).toBe(30 * 60 * 1000)
    expect(result.evidence?.closeouts).toHaveLength(7)
  })

  test("parses closeout checklist evidence from pylon khala closeout output", () => {
    const evidence = closeoutEvidenceFromPayload("assignment.public.one", {
      closeoutChecklist: { blockerRefs: [], ok: true },
      ok: true,
      proof: {
        proofChecklist: { blockerRefs: [], ok: true },
        rawEvents: { eventCount: 1 },
        tokenUsage: {
          demandSource: "khala_coding_delegation",
          refs: ["d1:token_usage_events:one"],
          rowCount: 1,
          totalTokens: 123,
          usageTruth: "exact",
        },
        traces: { count: 1 },
      },
      status: { progress: { state: "closed_out" } },
    })

    expect(evidence).toMatchObject({
      assignmentRef: "assignment.public.one",
      closeoutChecklistOk: true,
      demandSource: "khala_coding_delegation",
      statusState: "closed_out",
      tokenRows: 1,
      totalTokens: 123,
      usageTruth: "exact",
    })
  })

  test("builds sustained plan with the documented defaults", () => {
    const plan = buildFleetRunSmokePlan("sustained", {}, new Date("2026-07-02T00:00:00.000Z"))

    expect(plan.armed).toBe(false)
    expect(plan.targetWorkers).toBe(5)
    expect(plan.minDurationMinutes).toBe(30)
    expect(plan.minRefills).toBe(2)
    expect(plan.requiredCloseouts).toBe(7)
  })
})
