import { describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  FleetRunDurablePlannerError,
  createPylonDurableFleetRunPlanner,
} from "../src/orchestration/fleet-run-durable-planner.js"
import { openPylonFleetRunRuntime } from "../src/orchestration/fleet-run-runtime.js"
import {
  fleetRunWorkSourceDescriptorFrom,
  type FleetRunWorkSourceDescriptor,
} from "../src/orchestration/fleet-run-work-source.js"
import { createPylonOrchestrationStore } from "../src/orchestration/store.js"

const fixedNow = new Date("2026-07-09T22:00:00.000Z")
const commit = "0123456789abcdef0123456789abcdef01234567"
const verify = "bun test apps/pylon/tests/fleet-run-durable-planner.test.ts"

const planDagDescriptor = (): FleetRunWorkSourceDescriptor =>
  fleetRunWorkSourceDescriptorFrom({
    kind: "plan_dag",
    planRef: "plan.fc2.durable",
    repo: "OpenAgentsInc/openagents",
    branch: "main",
    baseCommit: commit,
    verify,
    nodes: [
      {
        ref: "root",
        title: "Root durable unit",
        objective: "Implement the root durable unit.",
        issue: 8633,
      },
      {
        ref: "dependent",
        title: "Dependent durable unit",
        objective: "Implement the dependent durable unit.",
        dependsOn: ["root"],
      },
    ],
  })

describe("Pylon durable FleetRun work-source planner", () => {
  test("preserves exact Sarah authority unit refs across issue and DAG claims", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const authorityBinding = (claimRef: string) => ({
      schema: "openagents.pylon.fleet_run_authority_binding.v1" as const,
      source: "sarah_authority" as const,
      authorityFingerprint: "a".repeat(64),
      claimRef,
      pylonRef: "pylon.public.durable_planner",
      targetPreference: "owner_local" as const,
      phase: "accepted" as const,
    })
    const issueRun = store.createFleetRun({
      runRef: "fleet_run.sarah.11111111111111111111",
      objective: "Preserve the authority issue unit identity.",
      workSource: "issue_list",
      workSourceDescriptor: fleetRunWorkSourceDescriptorFrom({
        kind: "issue_list",
        repo: "OpenAgentsInc/openagents",
        branch: "main",
        baseCommit: commit,
        verify,
        issues: [8633],
      }),
      authorityBinding: authorityBinding(
        "claim.sarah_fleet_run.111111111111111111111111",
      ),
      targetConcurrency: 1,
      workerKind: "codex",
      state: "running",
      now: fixedNow,
    })
    const issuePlanner = createPylonDurableFleetRunPlanner({ store })
    const firstIssuePlan = await issuePlanner.plan({
      run: issueRun,
      now: fixedNow,
    })
    expect(firstIssuePlan.claimable.map(unit => unit.workUnitRef)).toEqual([
      "issue.8633",
    ])
    expect(store.tryClaimWorkUnit({
      claimRef: "claim.public.authority.issue.8633",
      workUnitRef: "issue.8633",
      runRef: issueRun.runRef,
      workerAccountRef: "codex-owner-isolated",
      ttl: 60_000,
      now: fixedNow,
    })).not.toBeNull()
    const claimedIssuePlan = await issuePlanner.plan({
      run: issueRun,
      now: fixedNow,
    })
    expect(claimedIssuePlan.claimable).toEqual([])
    expect(claimedIssuePlan.skipped).toEqual([
      expect.objectContaining({
        workUnitRef: "issue.8633",
        skipReason: "already_claimed",
      }),
    ])

    const dagRun = store.createFleetRun({
      runRef: "fleet_run.sarah.22222222222222222222",
      objective: "Preserve authority DAG identities and dependencies.",
      workSource: "plan_dag",
      workSourceDescriptor: planDagDescriptor(),
      authorityBinding: authorityBinding(
        "claim.sarah_fleet_run.222222222222222222222222",
      ),
      targetConcurrency: 2,
      workerKind: "auto",
      state: "running",
      now: fixedNow,
    })
    const dagPlanner = createPylonDurableFleetRunPlanner({ store })
    const firstDagPlan = await dagPlanner.plan({ run: dagRun, now: fixedNow })
    expect(firstDagPlan.claimable.map(unit => unit.workUnitRef)).toEqual([
      "root",
    ])
    expect(firstDagPlan.skipped).toEqual([
      expect.objectContaining({
        workUnitRef: "dependent",
        skipReason: "dependency_pending",
      }),
    ])
    const rootClaim = store.tryClaimWorkUnit({
      claimRef: "claim.public.authority.dag.root",
      workUnitRef: "root",
      runRef: dagRun.runRef,
      workerAccountRef: "claude-owner-isolated",
      ttl: 60_000,
      now: fixedNow,
    })
    expect(rootClaim).not.toBeNull()
    store.updateWorkClaimState(rootClaim!.claimRef, "closeout", fixedNow)
    const completedRootPlan = await dagPlanner.plan({
      run: dagRun,
      now: fixedNow,
    })
    expect(completedRootPlan.claimable.map(unit => unit.workUnitRef)).toEqual([
      "dependent",
    ])
    expect(completedRootPlan.skipped).toEqual([
      expect.objectContaining({ workUnitRef: "root", skipReason: "completed" }),
    ])
  })

  test("reopens the same pinned plan and refuses a duplicate live claim", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-durable-planner-"))
    const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
    const runRef = "fleet_run.fc2.durable_source"

    try {
      const first = await openPylonFleetRunRuntime({ env, now: () => fixedNow })
      const descriptor = planDagDescriptor()
      const run = first.store.createFleetRun({
        runRef,
        objective: "Execute the persisted FC-2 plan.",
        workSource: "plan_dag",
        workSourceDescriptor: descriptor,
        targetConcurrency: 2,
        workerKind: "auto",
        state: "running",
        now: fixedNow,
      })
      const before = await createPylonDurableFleetRunPlanner({ store: first.store }).plan({
        run,
        now: fixedNow,
      })
      expect(before.claimable.map((unit) => unit.workUnitRef)).toEqual([
        "plan_dag:plan.fc2.durable:node:root",
      ])
      expect(before.skipped).toEqual([
        expect.objectContaining({
          workUnitRef: "plan_dag:plan.fc2.durable:node:dependent",
          skipReason: "dependency_pending",
        }),
      ])
      expect(before.units[0]).toMatchObject({
        repo: "OpenAgentsInc/openagents",
        branch: "main",
        baseCommit: commit,
        verify,
      })
      await first.close()

      const reopened = await openPylonFleetRunRuntime({ env, now: () => fixedNow })
      try {
        const reopenedRun = reopened.store.getFleetRun(runRef)
        expect(reopenedRun?.workSourceDescriptor).toEqual(descriptor)
        if (reopenedRun === null) throw new Error("expected reopened FleetRun")
        const planner = createPylonDurableFleetRunPlanner({ store: reopened.store })
        const after = await planner.plan({ run: reopenedRun, now: fixedNow })
        expect(after).toEqual(before)

        const rootUnitRef = "plan_dag:plan.fc2.durable:node:root"
        const firstClaim = reopened.store.tryClaimWorkUnit({
          claimRef: "claim.fc2.durable.root",
          workUnitRef: rootUnitRef,
          runRef,
          workerAccountRef: "codex-owner-isolated",
          ttl: 60_000,
          now: fixedNow,
        })
        expect(firstClaim).not.toBeNull()
        expect(reopened.store.tryClaimWorkUnit({
          claimRef: "claim.fc2.durable.root.duplicate",
          workUnitRef: rootUnitRef,
          runRef,
          workerAccountRef: "claude-owner-isolated",
          ttl: 60_000,
          now: fixedNow,
        })).toBeNull()
        const claimedPlan = await planner.plan({ run: reopenedRun, now: fixedNow })
        expect(claimedPlan.claimable).toEqual([])
        expect(claimedPlan.skipped).toEqual(expect.arrayContaining([
          expect.objectContaining({ workUnitRef: rootUnitRef, skipReason: "already_claimed" }),
        ]))
        expect(reopened.store.listWorkClaims({ runRef })).toHaveLength(1)
      } finally {
        await reopened.close()
      }
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("carries checkout and verifier pins through durable issue-list and GitHub planners", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const pins = {
      repo: "OpenAgentsInc/openagents",
      branch: "main",
      baseCommit: commit,
      verify,
    }
    const issueRun = store.createFleetRun({
      runRef: "fleet_run.fc2.issue_list_pins",
      objective: "Plan a pinned issue list.",
      workSource: "issue_list",
      workSourceDescriptor: fleetRunWorkSourceDescriptorFrom({
        kind: "issue_list",
        ...pins,
        issues: [8633],
      }),
      targetConcurrency: 1,
      workerKind: "codex",
      state: "running",
      now: fixedNow,
    })
    const issuePlan = await createPylonDurableFleetRunPlanner({ store }).plan({
      run: issueRun,
      now: fixedNow,
    })
    expect(issuePlan.claimable[0]).toMatchObject(pins)

    const githubRun = store.createFleetRun({
      runRef: "fleet_run.fc2.github_pins",
      objective: "Plan a pinned GitHub backlog.",
      workSource: "github_backlog",
      workSourceDescriptor: fleetRunWorkSourceDescriptorFrom({
        kind: "github_backlog",
        ...pins,
        limit: 10,
      }),
      targetConcurrency: 1,
      workerKind: "codex",
      state: "running",
      now: fixedNow,
    })
    const githubPlan = await createPylonDurableFleetRunPlanner({
      store,
      gh: async (args) => args[0] === "issue"
        ? JSON.stringify([{
            number: 8634,
            title: "Pinned GitHub unit",
            state: "OPEN",
            labels: [],
            body: "",
            url: "https://github.com/OpenAgentsInc/openagents/issues/8634",
          }])
        : "[]",
    }).plan({ run: githubRun, now: fixedNow })
    expect(githubPlan.claimable[0]).toMatchObject(pins)
  })

  test("legacy kind-only rows load but the durable planner fails with a typed public blocker", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const run = store.createFleetRun({
      runRef: "fleet_run.fc2.legacy_kind_only",
      objective: "Retain a backwards-compatible legacy row.",
      workSource: "fixture",
      targetConcurrency: 1,
      workerKind: "codex",
      state: "running",
      now: fixedNow,
    })

    expect(store.getFleetRun(run.runRef)).toMatchObject({
      runRef: run.runRef,
      workSource: "fixture",
    })
    expect(store.getFleetRun(run.runRef)).not.toHaveProperty("workSourceDescriptor")
    const error = await createPylonDurableFleetRunPlanner({ store }).plan({ run, now: fixedNow })
      .then(() => null, (cause: unknown) => cause)
    expect(error).toBeInstanceOf(FleetRunDurablePlannerError)
    expect(error).toMatchObject({
      failure: "missing_descriptor",
      blockerRefs: ["blocker.pylon.fleet_run.work_source_missing_descriptor"],
      runRef: run.runRef,
    })
    expect(new FleetRunDurablePlannerError("/Users/owner/private/run", "unknown_run"))
      .toMatchObject({ runRef: "fleet_run.unavailable" })
  })

  test("rejects mismatched, excess, invalid, private, and corrupt descriptors without leaking local material", async () => {
    expect(() => fleetRunWorkSourceDescriptorFrom({
      kind: "fixture",
      count: 1,
      worktreePath: "/Users/owner/private/fleet-worktree",
    })).toThrow()
    expect(() => fleetRunWorkSourceDescriptorFrom({
      kind: "plan_dag",
      planRef: "plan.fc2.private",
      repo: "OpenAgentsInc/openagents",
      branch: "main",
      baseCommit: commit,
      verify: "/Users/owner/private/verify.sh",
      nodes: [{ ref: "root", title: "Root", objective: "Run root." }],
    })).toThrow(/private|absolute paths/i)
    expect(() => fleetRunWorkSourceDescriptorFrom({
      kind: "plan_dag",
      planRef: "plan.fc2.invalid",
      nodes: [{ ref: "root", title: "Root", objective: "Run root." }],
    })).toThrow(/commit.*required|verify.*required/i)

    const db = new Database(":memory:")
    const store = createPylonOrchestrationStore(db)
    expect(() => store.createFleetRun({
      runRef: "fleet_run.fc2.kind_mismatch",
      objective: "Reject mismatched source kinds.",
      workSource: "fixture",
      workSourceDescriptor: planDagDescriptor(),
      targetConcurrency: 1,
      workerKind: "codex",
      state: "running",
      now: fixedNow,
    })).toThrow(/must match/)
    expect(store.getFleetRun("fleet_run.fc2.kind_mismatch")).toBeNull()

    const run = store.createFleetRun({
      runRef: "fleet_run.fc2.corrupt_descriptor",
      objective: "Fail closed after descriptor corruption.",
      workSource: "plan_dag",
      workSourceDescriptor: planDagDescriptor(),
      targetConcurrency: 1,
      workerKind: "codex",
      state: "running",
      now: fixedNow,
    })
    const row = db.query(
      "SELECT record_json FROM pylon_orchestration_fleet_runs WHERE run_ref = $runRef",
    ).get({ $runRef: run.runRef }) as { record_json: string }
    const corrupted = JSON.parse(row.record_json) as Record<string, unknown>
    const descriptor = corrupted.workSourceDescriptor as Record<string, unknown>
    descriptor.verify = "/Users/owner/private/do-not-project.sh"
    db.query(
      "UPDATE pylon_orchestration_fleet_runs SET record_json = $recordJson WHERE run_ref = $runRef",
    ).run({ $recordJson: JSON.stringify(corrupted), $runRef: run.runRef })

    const error = await createPylonDurableFleetRunPlanner({ store }).plan({ run, now: fixedNow })
      .then(() => null, (cause: unknown) => cause)
    expect(error).toBeInstanceOf(FleetRunDurablePlannerError)
    expect(error).toMatchObject({
      failure: "corrupt_descriptor",
      blockerRefs: ["blocker.pylon.fleet_run.work_source_corrupt_descriptor"],
    })
    expect(JSON.stringify(error)).not.toContain("/Users/owner/private")
    expect(error instanceof Error ? error.message : "").not.toContain("/Users/owner/private")
  })

  test("rejects hostile descriptor bounds before persistence", () => {
    const pinnedIssueSource = {
      kind: "issue_list" as const,
      repo: "OpenAgentsInc/openagents",
      branch: "main",
      baseCommit: commit,
      verify,
    }
    expect(() => fleetRunWorkSourceDescriptorFrom({ kind: "fixture", count: 1_001 }))
      .toThrow(/at most|no greater/i)
    expect(() => fleetRunWorkSourceDescriptorFrom({
      kind: "fixture",
      units: [
        { ref: "duplicate" },
        { ref: "duplicate" },
      ],
    })).toThrow(/duplicate unit ref/i)
    expect(() => fleetRunWorkSourceDescriptorFrom({
      ...pinnedIssueSource,
      issues: Array.from({ length: 1_001 }, (_, index) => index + 1),
    })).toThrow(/at most/i)
    expect(() => fleetRunWorkSourceDescriptorFrom({
      ...pinnedIssueSource,
      issues: [{ number: 1, title: "x".repeat(241) }],
    })).toThrow(/title/i)
    expect(() => fleetRunWorkSourceDescriptorFrom({
      ...pinnedIssueSource,
      issues: [{ number: 1, body: "x".repeat(8_001) }],
    })).toThrow(/body/i)
    expect(() => fleetRunWorkSourceDescriptorFrom({
      ...pinnedIssueSource,
      issues: [{ number: 1, labels: Array.from({ length: 51 }, (_, index) => `label-${index}`) }],
    })).toThrow(/labels/i)
    expect(() => fleetRunWorkSourceDescriptorFrom({
      ...pinnedIssueSource,
      issues: [{ number: 1, url: `https://github.com/${"x".repeat(2_100)}` }],
    })).toThrow(/url/i)
    expect(() => fleetRunWorkSourceDescriptorFrom({
      kind: "plan_dag",
      planRef: "plan.fc2.too_many_nodes",
      repo: "OpenAgentsInc/openagents",
      branch: "main",
      baseCommit: commit,
      verify,
      nodes: Array.from({ length: 501 }, (_, index) => ({
        ref: `node-${index}`,
        title: `Node ${index}`,
        objective: `Run node ${index}.`,
      })),
    })).toThrow(/at most/i)
    expect(() => fleetRunWorkSourceDescriptorFrom({
      kind: "plan_dag",
      planRef: "plan.fc2.too_many_dependencies",
      repo: "OpenAgentsInc/openagents",
      branch: "main",
      baseCommit: commit,
      verify,
      nodes: [
        ...Array.from({ length: 101 }, (_, index) => ({
          ref: `dependency-${index}`,
          title: `Dependency ${index}`,
          objective: `Run dependency ${index}.`,
        })),
        {
          ref: "dependent",
          title: "Dependent",
          objective: "Run the dependent node.",
          dependsOn: Array.from({ length: 101 }, (_, index) => `dependency-${index}`),
        },
      ],
    })).toThrow(/dependencies/i)
    expect(() => fleetRunWorkSourceDescriptorFrom({
      kind: "plan_dag",
      planRef: "plan.fc2.oversized",
      repo: "OpenAgentsInc/openagents",
      branch: "main",
      baseCommit: commit,
      verify,
      nodes: [{ ref: "root", title: "Root", objective: "x".repeat(140_000) }],
    })).toThrow(/bytes/i)
  })

  test("validates optional plan-level pins even when every node overrides them", () => {
    const node = {
      ref: "root",
      title: "Root",
      objective: "Run root.",
      repo: "OpenAgentsInc/openagents",
      branch: "main",
      baseCommit: commit,
      verify,
    }
    const invalidRootPins = [
      { repo: "not-a-github-repository" },
      { branch: " " },
      { baseCommit: "deadbeef" },
      { verify: "/Users/owner/private/root-verify.sh" },
    ] as const

    for (const pins of invalidRootPins) {
      expect(() => fleetRunWorkSourceDescriptorFrom({
        kind: "plan_dag",
        planRef: "plan.fc2.invalid_overridden_root",
        ...pins,
        nodes: [node],
      })).toThrow()
    }
  })
})
