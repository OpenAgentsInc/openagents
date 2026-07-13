import { describe, expect, test } from "bun:test"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap.js"
import type { PylonDevCheckProjection } from "../src/dev-loop.js"
import type { ControlSessionExecutorInput, ControlSessionExecutorResult } from "../src/node/control-sessions.js"
import {
  createPylonManagedCloudFleetRunClaimedWorkPort,
  createPylonRemoteManagedCloudFleetRunClaimedWorkPort,
  openPylonManagedCloudStandingFleetRunExecutor,
  PYLON_MANAGED_CLOUD_FLEET_BLOCKERS,
  type PylonManagedCloudFleetExactTuple,
} from "../src/orchestration/fleet-run-managed-cloud-runner.js"
import { openPylonFleetRunRuntime } from "../src/orchestration/fleet-run-runtime.js"
import type {
  FleetRunSupervisorDispatchInput,
  FleetRunSupervisorObservedEvent,
} from "../src/orchestration/fleet-run-supervisor.js"
import type { FleetRun, WorkClaim } from "../src/orchestration/store.js"

const fixedNow = new Date("2026-07-10T09:30:00.000Z")
const commit = "e5fa32f58953d532c63eccc28235b2fd2f9a2c61"
const accountRef = "managed-cloud-capacity.codex"

const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
  PYLON_HOME: "/tmp/pylon-managed-cloud-fleet-test",
})

const run: FleetRun = {
  schema: "openagents.khala_code.fleet_run.v1",
  runRef: "fleet_run.hybrid.managed_cloud",
  objective: "Implement the bounded public issue through managed cloud.",
  workSource: "issue_list",
  targetConcurrency: 1,
  workerKind: "codex",
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
    workUnitsTotal: 1,
    activeAssignments: 0,
    completedAssignments: 0,
    failedAssignments: 0,
    blockedAssignments: 0,
  },
  createdAt: fixedNow.toISOString(),
  updatedAt: fixedNow.toISOString(),
}

const claim = (overrides: Partial<WorkClaim> = {}): WorkClaim => ({
  schema: "openagents.khala_code.work_claim.v1",
  claimRef: "claim.hybrid.managed_cloud.1",
  workUnitRef: "work_unit.issue.8636.cloud_adapter",
  runRef: run.runRef,
  assignmentRef: null,
  workerAccountRef: accountRef,
  marginalCostClass: "api_metered",
  state: "in_progress",
  ttl: 60_000,
  claimedAt: fixedNow.toISOString(),
  expiresAt: new Date(fixedNow.getTime() + 60_000).toISOString(),
  updatedAt: fixedNow.toISOString(),
  ...overrides,
})

const dispatch = (overrides: Partial<FleetRunSupervisorDispatchInput> = {}): FleetRunSupervisorDispatchInput => {
  const workClaim = overrides.claim ?? claim()
  return {
    accountRef,
    claim: workClaim,
    run,
    taskId: "task.hybrid.managed_cloud.1",
    workerKind: "codex",
    workUnit: {
      workUnitRef: workClaim.workUnitRef,
      kind: "github_issue",
      title: "FC-4 managed-cloud adapter",
      body: "Implement the bounded public #8636 adapter without widening authority.",
      source: "issue_list",
      status: "claimable",
      branch: "main",
      baseCommit: commit,
      repo: "OpenAgentsInc/openagents",
      number: 8636,
      verify: "bun test apps/pylon/tests/fleet-run-managed-cloud-runner.test.ts",
    },
    ...overrides,
  }
}

const devCheck = (state: PylonDevCheckProjection["state"] = "passed"): PylonDevCheckProjection => ({
  schema: "openagents.pylon.dev_check.v0.3",
  observedAt: fixedNow.toISOString(),
  action: "check",
  state,
  changeSummary: {
    repo: {
      state: "ready",
      rootRef: "root.public.managed_cloud",
      branch: "branch.main",
      commit: `commit.${commit}`,
    },
    dirty: {
      state: "clean",
      changedCount: 0,
      stagedCount: 0,
      unstagedCount: 0,
      untrackedCount: 0,
    },
    changedFileRefs: [],
    areaRefs: [],
    blockerRefs: state === "passed" ? [] : ["blocker.cloud.verify_failed"],
  },
  checkPlan: {
    state: "ready",
    commandRefs: ["command.public.verify"],
    blockerRefs: [],
  },
  commandResults: [],
  latestRecordRef: null,
  branchUntouched: true,
  commitUntouched: true,
  pushPerformed: false,
  blockerRefs: state === "passed" ? [] : ["blocker.cloud.verify_failed"],
})

const cloudResult = (overrides: Partial<ControlSessionExecutorResult> = {}): ControlSessionExecutorResult => ({
  commandCount: 1,
  devCheck: devCheck(),
  editedFileCount: 1,
  eventCount: 2,
  externalSessionRef: "session.pylon.cloud.external_run.aaaaaaaaaaaaaaaaaaaaaaaa",
  responseDigestRef: "digest.pylon.cloud.receipt.aaaaaaaaaaaaaaaaaaaaaaaa",
  totalTokens: 0,
  cloudRunner: {
    lane: "cloud-gcp",
    providerLane: "gcp",
    runnerId: "raw-private-runner-name",
    externalRunId: "raw-private-external-run-id",
  },
  resourceUsageReceiptRef: "receipt.openagents.resource_usage_receipt.v1.aaaaaaaaaaaaaaaaaaaaaaaa",
  ...overrides,
})

const validBinding = {
  authGrantRef: "grant.owner.codex.aaaaaaaa",
  providerAccountRef: "provider_account.owner.codex.aaaaaaaa",
  ownerRef: "owner://sha256/private-owner-ref",
} as const

const blockerRefs = (result: {
  lifecycle: readonly { blockerRefs?: readonly string[] | undefined }[]
}): readonly string[] => result.lifecycle.flatMap((event) => event.blockerRefs ?? [])

describe("managed-cloud FleetRun claimed-work adapter (#8636)", () => {
  test("composes the real managed-cloud adapter and fails closed at the exact-usage gate", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-managed-cloud-standing-"))
    const composedSummary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
      PYLON_HOME: join(root, "pylon-home"),
    })
    const runRef = `fleet_run.sarah.${"4".repeat(20)}`
    const composedPylonRef = "pylon.public.managed_cloud_composed"
    let grantCalls = 0
    let executorCalls = 0
    const observed: FleetRunSupervisorObservedEvent[] = []
    try {
      const seed = await openPylonFleetRunRuntime({
        bootstrap: composedSummary,
        now: () => fixedNow,
      })
      seed.store.createFleetRun({
        runRef,
        objective: "Implement the bounded managed-cloud issue and verify it.",
        workSource: "issue_list",
        workSourceDescriptor: {
          schema: "openagents.pylon.fleet_run_work_source.v1",
          kind: "issue_list",
          repo: "OpenAgentsInc/openagents",
          branch: "main",
          baseCommit: commit,
          verify: "bun test apps/pylon/tests/fleet-run-managed-cloud-runner.test.ts",
          issues: [
            {
              number: 8547,
              title: "Compose managed-cloud execution",
              body: "Use the exact accepted Sarah authority and cloud grant.",
            },
          ],
        },
        authorityBinding: {
          schema: "openagents.pylon.fleet_run_authority_binding.v1",
          source: "sarah_authority",
          authorityFingerprint: "e".repeat(64),
          claimRef: `claim.sarah_fleet_run.${"2".repeat(24)}`,
          pylonRef: composedPylonRef,
          targetPreference: "managed_cloud",
          phase: "accepted",
        },
        targetConcurrency: 1,
        workerKind: "codex",
        state: "running",
        now: fixedNow,
      })
      await seed.close()

      const standing = await openPylonManagedCloudStandingFleetRunExecutor({
        summary: composedSummary,
        pylonRef: composedPylonRef,
        runRef,
        capacity: {
          accounts: async () => [
            {
              accountRef,
              advertisedCapacity: 1,
              marginalCostClass: "api_metered",
              workerKind: "codex",
            },
          ],
        },
        livenessProbe: () => "unknown",
        resolveGrantBinding: async (tuple) => {
          grantCalls += 1
          expect(tuple.runRef).toBe(runRef)
          expect(tuple.workUnitRef).toContain("8547")
          return validBinding
        },
        createExecutor: ({ binding, tuple }) => {
          expect(binding).toEqual(validBinding)
          expect(tuple.workerKind).toBe("codex")
          return async () => {
            executorCalls += 1
            return cloudResult()
          }
        },
        now: () => fixedNow,
        clock: {
          now: () => fixedNow,
          sleep: () => new Promise<void>(() => {}),
        },
        onLifecycle: (event) => {
          observed.push(event)
        },
        startImmediately: false,
      })

      try {
        for (let attempt = 0; attempt < 100 && executorCalls === 0; attempt += 1) {
          await Bun.sleep(2)
        }
        const taskFailed = (): boolean =>
          standing.runtime.store.listTasks().some((candidate) => candidate.status === "failed")
        for (let attempt = 0; attempt < 100 && !taskFailed(); attempt += 1) {
          await Bun.sleep(2)
        }
        expect(grantCalls).toBe(1)
        expect(executorCalls).toBe(1)
        expect(standing.runtime.store.listWorkClaims({ runRef })).toEqual([
          expect.objectContaining({
            runRef,
            state: "released",
            workerAccountRef: accountRef,
            assignmentRef: expect.stringMatching(/^assignment\.pylon\.managed_cloud\.[a-f0-9]{24}$/u),
          }),
        ])
        expect(standing.runtime.store.listTasks("failed")).toEqual([
          expect.objectContaining({
            spec: expect.objectContaining({ fleetRunRef: runRef }),
          }),
        ])
        expect(observed).toContainEqual(
          expect.objectContaining({
            kind: "dispatch",
            runRef,
            status: "failed",
            blockerRefs: ["blocker.pylon.fleet_run.usage_evidence_required"],
          }),
        )
      } finally {
        await standing.close()
      }
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("binds the exact tuple to the existing cloud executor and returns refs-only managed-cloud evidence", async () => {
    let resolvedTuple: PylonManagedCloudFleetExactTuple | null = null
    let executorTuple: PylonManagedCloudFleetExactTuple | null = null
    let executorInput: ControlSessionExecutorInput | null = null
    const observedLifecycle: string[] = []

    const port = createPylonManagedCloudFleetRunClaimedWorkPort({
      summary,
      now: () => fixedNow,
      resolveGrantBinding: async (tuple) => {
        resolvedTuple = tuple
        return validBinding
      },
      createExecutor: ({ binding, sessionRef, tuple }) => {
        expect(binding).toEqual(validBinding)
        expect(sessionRef).toMatch(/^session\.pylon\.managed_cloud\.[a-f0-9]{24}$/u)
        executorTuple = tuple
        return async (input) => {
          executorInput = input
          input.emit({
            phase: "composer_event",
            message: "raw /private/cloud/topology bearer-token should never project",
          })
          input.emit({
            phase: "composer_event",
            message: "raw runner id should never project",
          })
          return cloudResult()
        }
      },
    })

    const result = await port.dispatch({
      targetPreference: "managed_cloud",
      dispatch: dispatch({
        onLifecycle: (event) => {
          observedLifecycle.push(event.event)
        },
      }),
    })

    expect(result.status).toBe("completed")
    expect(result.assignmentRef).toMatch(/^assignment\.pylon\.managed_cloud\.[a-f0-9]{24}$/u)
    expect(result.closeoutRef).toBeNull()
    expect(result.target).toEqual({
      schema: "openagents.pylon.managed_cloud_fleet_target.v1",
      targetPreference: "managed_cloud",
      capacityClass: "managed_cloud",
      executionTargetRef: expect.stringMatching(/^execution_target\.pylon\.managed_cloud\.[a-f0-9]{24}$/u),
      targetEvidenceRef: expect.stringMatching(/^evidence\.public\.pylon\.managed_cloud\.target\.[a-f0-9]{24}$/u),
      fallbackRefs: [],
    })
    expect(result.verification).toEqual({
      truth: "passed",
      verifierRef: expect.stringMatching(/^verifier\.public\.pylon\.managed_cloud\.[a-f0-9]{24}$/u),
      evidenceRefs: [
        result.target.targetEvidenceRef,
        "receipt.openagents.resource_usage_receipt.v1.aaaaaaaaaaaaaaaaaaaaaaaa",
      ],
    })
    expect(result.authorityReceiptRefs).toEqual([
      "receipt.openagents.resource_usage_receipt.v1.aaaaaaaaaaaaaaaaaaaaaaaa",
    ])
    expect(result.accountRefHash).toBeNull()
    expect(result.usageEvidence).toBeNull()
    expect(observedLifecycle).toEqual([
      "assignment_run.runtime_started",
      "assignment_run.runtime_progress",
      "assignment_run.runtime_progress",
      "assignment_run.completed",
    ])

    expect(resolvedTuple).not.toBeNull()
    expect(executorTuple).toEqual(resolvedTuple)
    expect(resolvedTuple).toMatchObject({
      schema: "openagents.pylon.managed_cloud_fleet_tuple.v1",
      targetPreference: "managed_cloud",
      runRef: run.runRef,
      taskId: "task.hybrid.managed_cloud.1",
      claimRef: "claim.hybrid.managed_cloud.1",
      workUnitRef: "work_unit.issue.8636.cloud_adapter",
      workerAccountRef: accountRef,
      workerKind: "codex",
      repository: {
        fullName: "OpenAgentsInc/openagents",
        branch: "main",
        commit,
      },
    })
    expect(resolvedTuple?.fingerprint).toMatch(/^[a-f0-9]{64}$/u)
    expect(executorInput).toMatchObject({
      adapter: "codex",
      account: null,
      lane: "auto",
      cwd: ".",
      env: {},
      verify: ["bun", "test", "apps/pylon/tests/fleet-run-managed-cloud-runner.test.ts"],
    })

    const projected = JSON.stringify(result)
    expect(projected).not.toContain("raw-private-runner-name")
    expect(projected).not.toContain("raw-private-external-run-id")
    expect(projected).not.toContain("private-owner-ref")
    expect(projected).not.toContain(validBinding.authGrantRef)
    expect(projected).not.toContain(validBinding.providerAccountRef)
    expect(projected).not.toContain("bearer-token")
    expect(projected).not.toContain("/private/cloud/topology")
  })

  test("keeps broker credentials server-side and accepts only terminal refs from remote dispatch", async () => {
    let posted: Record<string, unknown> | null = null
    const port = createPylonRemoteManagedCloudFleetRunClaimedWorkPort({
      agentToken: "agent_test_public_transport_token",
      baseUrl: "https://openagents.example",
      pylonRef: "pylon.fc4.test",
      now: () => fixedNow,
      fetchImpl: async (_url, init) => {
        posted = JSON.parse(String(init?.body)) as Record<string, unknown>
        return Response.json({
          schema: "openagents.pylon.managed_cloud_fleet_dispatch.result.v1",
          state: "completed",
          placementRef: "placement.cloud-coding.fc4",
          agentComputerRef: "agent-computer.fc4",
          agentComputerState: "reclaimed",
          lifecycleReceiptRefs: ["receipt.lifecycle.fc4"],
          resourceUsageReceiptRefs: ["receipt.resource_usage.fc4"],
          artifactRef: "artifact.fc4",
          workContextRef: "work_context.fc4",
        })
      },
    })
    const result = await port.dispatch({
      targetPreference: "managed_cloud",
      dispatch: dispatch({ executionTarget: "managed_cloud" }),
    })
    expect(result.status).toBe("completed")
    expect(result.authorityReceiptRefs).toEqual(["receipt.resource_usage.fc4"])
    expect(result.artifactRefs).toEqual(["artifact.fc4"])
    expect(posted).toMatchObject({
      schema: "openagents.pylon.managed_cloud_fleet_dispatch.request.v1",
      targetPreference: "managed_cloud",
      repository: {
        fullName: "OpenAgentsInc/openagents",
        branch: "main",
        commit,
      },
    })
    expect(JSON.stringify(posted)).not.toContain("agent_test_public_transport_token")
    expect(JSON.stringify(result)).not.toContain("agent_test_public_transport_token")
  })

  test("rejects a non-managed target before either authority port runs", async () => {
    let calls = 0
    const port = createPylonManagedCloudFleetRunClaimedWorkPort({
      summary,
      now: () => fixedNow,
      resolveGrantBinding: async () => {
        calls += 1
        return validBinding
      },
      createExecutor: () => {
        calls += 1
        return async () => cloudResult()
      },
    })

    const result = await port.dispatch({
      targetPreference: "owner_local" as never,
      dispatch: dispatch(),
    })

    expect(result.status).toBe("blocked")
    expect(result.assignmentRef).toBeNull()
    expect(result.target.targetPreference).toBe("managed_cloud")
    expect(result.target.fallbackRefs).toEqual([])
    expect(blockerRefs(result)).toContain(PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.targetRequired)
    expect(calls).toBe(0)
  })

  test("rejects unsupported Claude and Grok workers without substituting Codex", async () => {
    let calls = 0
    const port = createPylonManagedCloudFleetRunClaimedWorkPort({
      summary,
      now: () => fixedNow,
      resolveGrantBinding: async () => {
        calls += 1
        return validBinding
      },
      createExecutor: () => {
        calls += 1
        return async () => cloudResult()
      },
    })

    for (const workerKind of ["claude", "grok"] as const) {
      const result = await port.dispatch({
        targetPreference: "managed_cloud",
        dispatch: dispatch({ workerKind }),
      })
      expect(result.status).toBe("blocked")
      expect(blockerRefs(result)).toContain(PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.workerUnsupported)
      expect(result.target.fallbackRefs).toEqual([])
    }
    expect(calls).toBe(0)
  })

  test("rejects a mismatched claim tuple before resolving a grant", async () => {
    let calls = 0
    const mismatched = claim({ runRef: "fleet_run.other" })
    const port = createPylonManagedCloudFleetRunClaimedWorkPort({
      summary,
      now: () => fixedNow,
      resolveGrantBinding: async () => {
        calls += 1
        return validBinding
      },
      createExecutor: () => {
        calls += 1
        return async () => cloudResult()
      },
    })

    const result = await port.dispatch({
      targetPreference: "managed_cloud",
      dispatch: dispatch({ claim: mismatched }),
    })

    expect(result.status).toBe("blocked")
    expect(result.assignmentRef).toBeNull()
    expect(blockerRefs(result)).toContain(PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.tupleInvalid)
    expect(calls).toBe(0)
  })

  test("fails closed when the owner grant is absent or its resolver throws", async () => {
    for (const resolveGrantBinding of [
      async () => null,
      async () => ({
        authGrantRef: validBinding.authGrantRef,
        providerAccountRef: validBinding.providerAccountRef,
      }),
      async () => {
        throw new Error("raw bearer credential from /private/owner/grant must not project")
      },
    ]) {
      let executorCalls = 0
      const port = createPylonManagedCloudFleetRunClaimedWorkPort({
        summary,
        now: () => fixedNow,
        resolveGrantBinding,
        createExecutor: () => {
          executorCalls += 1
          return async () => cloudResult()
        },
      })

      const result = await port.dispatch({
        targetPreference: "managed_cloud",
        dispatch: dispatch(),
      })
      expect(result.status).toBe("blocked")
      expect(result.assignmentRef).toMatch(/^assignment\.pylon\.managed_cloud\.[a-f0-9]{24}$/u)
      expect(blockerRefs(result)).toContain(PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.grantUnavailable)
      expect(executorCalls).toBe(0)
      expect(JSON.stringify(result)).not.toContain("bearer credential")
      expect(JSON.stringify(result)).not.toContain("/private/owner/grant")
    }
  })

  test("never accepts a local-looking executor result as managed cloud", async () => {
    const port = createPylonManagedCloudFleetRunClaimedWorkPort({
      summary,
      now: () => fixedNow,
      resolveGrantBinding: async () => validBinding,
      createExecutor: () => async (input) => {
        input.emit({
          phase: "composer_event",
          message: "local fallback attempted",
        })
        return cloudResult({
          cloudRunner: undefined,
          resourceUsageReceiptRef: null,
        })
      },
    })

    const result = await port.dispatch({
      targetPreference: "managed_cloud",
      dispatch: dispatch(),
    })

    expect(result.status).toBe("failed")
    expect(result.closeoutRef).toBeNull()
    expect(result.target.targetEvidenceRef).toBeNull()
    expect(result.target.fallbackRefs).toEqual([])
    expect(blockerRefs(result)).toContain(PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.cloudEvidenceInvalid)
  })

  test("rejects syntactically valid evidence refs that fail the public-safety tripwire", async () => {
    const port = createPylonManagedCloudFleetRunClaimedWorkPort({
      summary,
      now: () => fixedNow,
      resolveGrantBinding: async () => validBinding,
      createExecutor: () => async () =>
        cloudResult({
          resourceUsageReceiptRef: "receipt.private_repo.owner",
        }),
    })

    const result = await port.dispatch({
      targetPreference: "managed_cloud",
      dispatch: dispatch(),
    })

    expect(result.status).toBe("failed")
    expect(result.target.targetEvidenceRef).toBeNull()
    expect(blockerRefs(result)).toContain(PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.cloudEvidenceInvalid)
    expect(JSON.stringify(result)).not.toContain("private_repo")
  })

  test("redacts executor failures and records no false closeout", async () => {
    const port = createPylonManagedCloudFleetRunClaimedWorkPort({
      summary,
      now: () => fixedNow,
      resolveGrantBinding: async () => validBinding,
      createExecutor: () => async (input) => {
        input.emit({
          phase: "composer_event",
          message: "raw private topology",
        })
        throw new Error("runner raw-private-host leaked token=owner-provider-secret")
      },
    })

    const result = await port.dispatch({
      targetPreference: "managed_cloud",
      dispatch: dispatch(),
    })

    expect(result.status).toBe("failed")
    expect(result.closeoutRef).toBeNull()
    expect(blockerRefs(result)).toContain(PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.executorFailed)
    const projected = JSON.stringify(result)
    expect(projected).not.toContain("raw-private-host")
    expect(projected).not.toContain("owner-provider-secret")
    expect(projected).not.toContain("raw private topology")
  })

  test("retains refs-only compute evidence on a failed cloud verifier", async () => {
    const port = createPylonManagedCloudFleetRunClaimedWorkPort({
      summary,
      now: () => fixedNow,
      resolveGrantBinding: async () => validBinding,
      createExecutor: () => async () => cloudResult({ devCheck: devCheck("failed") }),
    })

    const result = await port.dispatch({
      targetPreference: "managed_cloud",
      dispatch: dispatch(),
    })

    expect(result.status).toBe("failed")
    expect(result.closeoutRef).toBeNull()
    expect(result.verification?.truth).toBe("failed")
    expect(result.authorityReceiptRefs).toEqual([
      "receipt.openagents.resource_usage_receipt.v1.aaaaaaaaaaaaaaaaaaaaaaaa",
    ])
    expect(result.target.capacityClass).toBe("managed_cloud")
    expect(result.target.fallbackRefs).toEqual([])
  })

  test("fails closed when lifecycle projection rejects", async () => {
    const port = createPylonManagedCloudFleetRunClaimedWorkPort({
      summary,
      now: () => fixedNow,
      resolveGrantBinding: async () => validBinding,
      createExecutor: () => async () => cloudResult(),
    })

    const result = await port.dispatch({
      targetPreference: "managed_cloud",
      dispatch: dispatch({
        onLifecycle: async () => {
          throw new Error("owner-private projection sink detail")
        },
      }),
    })

    expect(result.status).toBe("failed")
    expect(result.closeoutRef).toBeNull()
    expect(blockerRefs(result)).toContain(PYLON_MANAGED_CLOUD_FLEET_BLOCKERS.lifecycleProjectionFailed)
    expect(JSON.stringify(result)).not.toContain("owner-private")
  })
})
