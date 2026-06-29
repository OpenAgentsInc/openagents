import { describe, expect, test } from "bun:test"
import {
  activateTrainingWindow,
  admitTrainingRealGradientEvidence,
  buildTrainingEvidencePacket,
  claimTrainingWindowLease,
  fetchTrainingDashboard,
  fetchTrainingPromiseGates,
  fetchTrainingRuns,
  planTrainingRunWindow,
  readTrainingEvidencePacketSummary,
  reconcileTrainingWindow,
  requestTrainingBootstrapGrant,
} from "../src/bun/training-runs"

const sampleRun = {
  createdAtDisplay: "2 days ago",
  maxAllowedStale: 5,
  promiseRef: "pylon.first_real_model_training_run.v1",
  receiptRefs: ["receipt.1"],
  sealInFlight: false,
  sealPublicationCadenceWindows: 1,
  sourceRefs: ["route:/api/training/runs"],
  state: "planned",
  trainingRunRef: "run.cs336.a1.real_gradient.demo",
  updatedAtDisplay: "2 days ago",
}

const sampleSummary = {
  copyBoundaryRefs: ["no_unbounded_training_claim"],
  emptyState: { idle: false, reason: "" },
  metrics: {
    activeWindowCount: { provenanceLabel: "", sourceRefs: [], value: 0 },
    assignedContributorCount: { provenanceLabel: "", sourceRefs: [], value: 2 },
    pendingPayoutCount: { provenanceLabel: "", sourceRefs: [], value: 0 },
    plannedWindowCount: { provenanceLabel: "", sourceRefs: [], value: 0 },
    providerConfirmedSettledPayoutSats: {
      provenanceLabel: "",
      sourceRefs: [],
      value: 0,
    },
    receiptRefCount: { provenanceLabel: "", sourceRefs: ["receipt.1"], value: 1 },
    reconciledWindowCount: { provenanceLabel: "", sourceRefs: [], value: 2 },
    rejectedWorkCount: { provenanceLabel: "", sourceRefs: [], value: 0 },
    sealedWindowCount: { provenanceLabel: "", sourceRefs: [], value: 0 },
    verifiedWorkCount: { provenanceLabel: "", sourceRefs: [], value: 3 },
  },
  realGradient: {
    closeoutRequirement: {
      evalRef: "eval.1",
      freivaldsCommitmentRefs: ["freivalds.1"],
      gradientCloseoutRefs: ["closeout.1"],
      mergeRef: "merge.1",
      provenanceLabel: "",
      satisfied: true,
    },
    deviceRequirement: {
      observedDistinctContributorDevices: 2,
      provenanceLabel: "",
      requiredDistinctContributorDevices: 2,
      satisfied: true,
      sourceRefs: [],
    },
    externalAsk: {
      blockerRefs: [],
      psionicLaneRef: "psionic.lane.demo",
      requirementRefs: [],
      status: "observed",
    },
    lossUnderBudget: {
      budgetLabel: "tiny",
      budgetRef: "budget.1",
      finalValidationLoss: 3.1,
      maxValidationLoss: 4,
      provenanceLabel: "",
      satisfied: true,
      sourceRefs: [],
    },
    scopeBoundaryRefs: ["no_public_gradients"],
  },
  receiptRefs: ["receipt.1"],
  run: sampleRun,
  sourceRefs: ["route:/api/training/runs"],
  windows: [
    {
      datasetRefs: ["dataset.1"],
      homeworkKind: "cs336_a1_real_gradient",
      plannedAtDisplay: "2 days ago",
      priority: 10,
      receiptRefs: ["window.receipt.1"],
      sealMetadata: null,
      sourceRefs: [],
      state: "reconciled",
      trainingRunRef: sampleRun.trainingRunRef,
      updatedAtDisplay: "2 days ago",
      windowRef: "window.1",
    },
  ],
}

const sampleEvidencePacket = {
  budgetLabel: "desktop tiny loss budget",
  budgetRef: "budget.desktop.training.a1",
  evalRef: "eval.desktop.training.a1",
  freivaldsCommitmentRefs: ["freivalds.desktop.training.a1"],
  gradientCloseoutRefs: ["closeout.desktop.training.a1"],
  lossCurve: [
    { step: 0, validationLoss: 4.2 },
    { step: 1, validationLoss: 3.1 },
  ],
  maxValidationLoss: 4,
  mergeRef: "merge.desktop.training.a1",
  receiptRefs: ["receipt.desktop.training.a1"],
  shardContributions: [
    {
      dataUnitCount: 128,
      gradientCommitmentRef: "gradient.desktop.training.a1.0",
      pylonRef: "pylon.training.1",
      receiptRefs: ["receipt.desktop.training.shard.1"],
      shardIndex: 0,
      shardLoss: 3.2,
      stepIndex: 1,
      verificationRefs: ["verification.desktop.training.shard.1"],
    },
    {
      dataUnitCount: 128,
      gradientCommitmentRef: "gradient.desktop.training.a1.1",
      pylonRef: "pylon.training.2",
      receiptRefs: ["receipt.desktop.training.shard.2"],
      shardIndex: 1,
      shardLoss: 3.0,
      stepIndex: 1,
      verificationRefs: ["verification.desktop.training.shard.2"],
    },
  ],
  sourceRefs: ["source.desktop.training.a1"],
}

const sampleWorkerReceiptsBundle = {
  budgetLabel: "desktop tiny loss budget",
  budgetRef: "budget.desktop.training.a1",
  evalRef: "eval.desktop.training.a1",
  lossCurve: [
    { step: 0, validationLoss: 4.2 },
    { step: 1, validationLoss: 3.1 },
  ],
  maxValidationLoss: 4,
  mergeRef: "merge.desktop.training.a1",
  sourceRefs: ["source.desktop.training.worker_receipts"],
  workerReceipts: [
    {
      schema: "openagents.psionic.training_worker_receipt.v0.3",
      receiptRef: "receipt.psionic.training_worker.1",
      assignmentRef: "assignment.public.psionic_training.1",
      workerRef: "pylon.training.1",
      runRef: "run.cs336.a1.real_gradient.demo",
      artifactRefs: ["artifact.psionic.training.output.1"],
      checkpointRefs: ["checkpoint.psionic.training.1"],
      metricRefs: ["metric.psionic.training.loss_curve.1"],
      proofRefs: ["proof.psionic.training.freivalds.1"],
      signature: {
        signatureRef: "signature.psionic.worker_receipt.1",
        signerRef: "signer.psionic.release.authority.v1",
        verificationRef: "verification.psionic.worker_receipt.1",
      },
    },
    {
      schema: "openagents.psionic.training_worker_receipt.v0.3",
      receiptRef: "receipt.psionic.training_worker.2",
      assignmentRef: "assignment.public.psionic_training.2",
      workerRef: "pylon.training.2",
      runRef: "run.cs336.a1.real_gradient.demo",
      artifactRefs: ["artifact.psionic.training.output.2"],
      checkpointRefs: ["checkpoint.psionic.training.2"],
      metricRefs: ["metric.psionic.training.loss_curve.2"],
      proofRefs: ["proof.psionic.training.freivalds.2"],
      signature: {
        signatureRef: "signature.psionic.worker_receipt.2",
        signerRef: "signer.psionic.release.authority.v1",
        verificationRef: "verification.psionic.worker_receipt.2",
      },
    },
  ],
}

describe("fetchTrainingRuns", () => {
  test("decodes public run summaries", async () => {
    const result = await fetchTrainingRuns({
      baseUrl: "https://openagents.test",
      nowIso: () => "2026-06-14T00:00:00.000Z",
      fetchFn: async () =>
        new Response(JSON.stringify({ runs: [sampleRun], summaries: [sampleSummary] })),
    })

    expect(result.ok).toBe(true)
    expect(result.sourceUrl).toBe("https://openagents.test/api/training/runs")
    expect(result.runs).toHaveLength(1)
    expect(result.summaries).toHaveLength(1)
    expect(result.summaries[0]?.metrics.verifiedWorkCount.value).toBe(3)
    expect(
      result.summaries[0]?.realGradient.deviceRequirement.satisfied,
    ).toBe(true)
  })

  test("falls back to run-only summaries", async () => {
    const result = await fetchTrainingRuns({
      baseUrl: "https://openagents.test/",
      fetchFn: async () => new Response(JSON.stringify({ runs: [sampleRun] })),
    })

    expect(result.ok).toBe(true)
    expect(result.summaries).toHaveLength(1)
    expect(result.summaries[0]?.run.trainingRunRef).toBe(sampleRun.trainingRunRef)
    expect(result.summaries[0]?.metrics.receiptRefCount.value).toBe(1)
  })

  test("returns a typed error projection on HTTP failure", async () => {
    const result = await fetchTrainingRuns({
      baseUrl: "https://openagents.test",
      fetchFn: async () => new Response("nope", { status: 503 }),
    })

    expect(result.ok).toBe(false)
    expect(result.runs).toEqual([])
    expect(result.error).toBe("training runs 503")
  })

  test("keeps the Tassadar summary when the legacy training-runs aggregate stalls", async () => {
    const result = await fetchTrainingRuns({
      baseUrl: "https://openagents.test",
      timeoutMs: 5,
      fetchFn: async (url) => {
        const href = String(url)
        if (href.endsWith("/api/public/tassadar-run-summary")) {
          return new Response(
            JSON.stringify({
              runRef: "run.tassadar.executor.20260615",
              runState: "active",
              metrics: { assignedContributorCount: { value: 11 } },
            }),
          )
        }
        return new Promise<Response>(() => {})
      },
    })

    expect(result.ok).toBe(false)
    expect(result.error).toBe("training runs timeout")
    expect(result.tassadarSummary?.runRef).toBe("run.tassadar.executor.20260615")
    expect(result.tassadarSummary?.runState).toBe("active")
  })
})

describe("fetchTrainingDashboard", () => {
  test("decodes public leaderboard and CS336 dashboard summaries", async () => {
    const result = await fetchTrainingDashboard({
      baseUrl: "https://openagents.test/",
      nowIso: () => "2026-06-14T00:00:00.000Z",
      fetchFn: async (url) => {
        const href = String(url)
        if (href.endsWith("/api/training/leaderboards")) {
          return new Response(
            JSON.stringify({
              blockerRefs: ["blocker.all"],
              lanes: [
                {
                  blockerRefs: [],
                  lane: "a1_loss",
                  rows: [
                    {
                      contributorRef: "pylon.training.1",
                      rank: 1,
                      score: 3.1,
                      scoreLabel: "validation_loss=3.1",
                      settledPayoutSats: 21,
                      trainingRunRef: "training.run.1",
                    },
                  ],
                  title: "A1 Loss Under Budget",
                },
                {
                  blockerRefs: ["blocker.a2"],
                  lane: "a2_throughput",
                  rows: [],
                  title: "A2 Throughput",
                },
              ],
            }),
          )
        }
        if (href.endsWith("/api/training/device-capabilities/a2")) {
          return new Response(
            JSON.stringify({
              blockerRefs: [],
              classDistributions: [
                { verified: true },
                { verified: false },
                { verified: true },
              ],
              observedDeviceClassCount: 2,
              observedMeasurementCount: 3,
            }),
          )
        }
        if (href.endsWith("/api/training/isoflop/a3")) {
          return new Response(
            JSON.stringify({
              blockerRefs: ["blocker.a3"],
              cells: [{ verified: true }, { verified: false }],
              fitArtifacts: [{}],
            }),
          )
        }
        if (href.endsWith("/api/training/refinery/a4")) {
          return new Response(
            JSON.stringify({
              blockerRefs: [],
              evalDeltaBonusBlockerRefs: ["blocker.bonus"],
              observedVerifiedStages: ["pii_masking", "gopher_rules"],
              requiredVerifiedStageCount: 3,
              shards: [{}, {}],
            }),
          )
        }
        return new Response(
          JSON.stringify({
            blockerRefs: [],
            evalSuites: [{ verificationRefs: ["v1"] }, { verificationRefs: [] }],
            updateBoundaryRef: "issue.github.openagents.4669",
          }),
        )
      },
    })

    expect(result.ok).toBe(true)
    expect(result.sourceUrl).toBe(
      "https://openagents.test/api/training/leaderboards",
    )
    expect(result.leaderboards.blockerRefs).toEqual(["blocker.all"])
    expect(result.leaderboards.lanes).toHaveLength(2)
    expect(result.leaderboards.lanes[0]?.topRow).toMatchObject({
      contributorRef: "pylon.training.1",
      rank: 1,
      trainingRunRef: "training.run.1",
    })
    expect(result.a2.verifiedMeasurementCount).toBe(2)
    expect(result.a3.verifiedCellCount).toBe(1)
    expect(result.a4.shardCount).toBe(2)
    expect(result.a5.verifiedSuiteCount).toBe(1)
  })

  test("returns a typed error projection when a dashboard endpoint fails", async () => {
    const result = await fetchTrainingDashboard({
      baseUrl: "https://openagents.test",
      fetchFn: async (url) =>
        String(url).endsWith("/api/training/isoflop/a3")
          ? new Response(JSON.stringify({ reason: "a3 offline" }), {
              status: 503,
            })
          : new Response(JSON.stringify({ lanes: [], blockerRefs: [] })),
    })

    expect(result.ok).toBe(false)
    expect(result.error).toBe("a3: a3 offline")
    expect(result.leaderboards.lanes).toEqual([])
    expect(result.a3.cellCount).toBe(0)
  })
})

describe("fetchTrainingPromiseGates", () => {
  test("decodes public training promise gates", async () => {
    const result = await fetchTrainingPromiseGates({
      baseUrl: "https://openagents.test/",
      nowIso: () => "2026-06-14T00:00:00.000Z",
      fetchFn: async () =>
        new Response(
          JSON.stringify({
            registryVersion: "2026-06-12.8",
            promises: [
              {
                blockerRefs: [
                  "blocker.product_promises.public_distributed_training_run_receipts_missing",
                ],
                claim: "Pylons participate in public distributed model-training runs.",
                evidenceRefs: ["docs/transcripts/236.md"],
                productArea: "training",
                promiseId: "training.public_distributed_training_run.v1",
                safeCopy: "Not green yet.",
                state: "red",
                verification: "Requires run, work, validation, and settlement refs.",
              },
              {
                blockerRefs: [],
                claim: "The code map is public.",
                evidenceRefs: ["https://github.com/OpenAgentsInc/openagents"],
                productArea: "source transparency",
                promiseId: "repo.open_source_code_map.v1",
                safeCopy: "Use the public code map.",
                state: "green",
                verification: "Fetch the code map.",
              },
              {
                blockerRefs: ["blocker.model_spec_missing"],
                claim: "Tassadar executor-model direction.",
                evidenceRefs: ["docs/transcripts/236.md"],
                productArea: "models",
                promiseId: "models.tassadar_percepta_executor.v1",
                safeCopy: "Research candidate only.",
                state: "red",
                verification: "Requires model spec and public evidence refs.",
              },
            ],
          }),
        ),
    })

    expect(result.ok).toBe(true)
    expect(result.sourceUrl).toBe(
      "https://openagents.test/api/public/product-promises",
    )
    expect(result.registryVersion).toBe("2026-06-12.8")
    expect(result.promises.map(promise => promise.promiseId)).toEqual([
      "models.tassadar_percepta_executor.v1",
      "training.public_distributed_training_run.v1",
    ])
    expect(result.stateCounts.red).toBe(2)
    expect(result.blockerRefs).toEqual([
      "blocker.model_spec_missing",
      "blocker.product_promises.public_distributed_training_run_receipts_missing",
    ])
  })

  test("returns a typed error projection on registry failure", async () => {
    const result = await fetchTrainingPromiseGates({
      baseUrl: "https://openagents.test",
      fetchFn: async () =>
        new Response(JSON.stringify({ reason: "registry unavailable" }), {
          status: 503,
        }),
    })

    expect(result.ok).toBe(false)
    expect(result.error).toBe("registry unavailable")
    expect(result.promises).toEqual([])
    expect(result.stateCounts.red).toBe(0)
  })
})

describe("planTrainingRunWindow", () => {
  test("does not call admin routes when disabled", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const result = await planTrainingRunWindow({
      adminToken: "admin-token",
      baseUrl: "https://openagents.test",
      enabled: false,
      fetchFn: async (url, init) => {
        calls.push({ url: String(url), init })
        return new Response("{}")
      },
      nowIso: () => "2026-06-14T00:00:00.000Z",
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("disabled")
    expect(calls).toHaveLength(0)
  })

  test("does not call admin routes without a token", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const result = await planTrainingRunWindow({
      adminToken: null,
      baseUrl: "https://openagents.test",
      enabled: true,
      fetchFn: async (url, init) => {
        calls.push({ url: String(url), init })
        return new Response("{}")
      },
      nowIso: () => "2026-06-14T00:00:00.000Z",
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("admin_token_missing")
    expect(calls).toHaveLength(0)
  })

  test("plans a run and window through admin routes", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const expectedRunRef =
      "training.run.desktop.r1.2026.06.14t00.00.00.000z"
    const expectedWindowRef =
      "training.window.desktop.r1.2026.06.14t00.00.00.000z"
    const result = await planTrainingRunWindow({
      adminToken: "admin-token",
      baseUrl: "https://openagents.test/",
      enabled: true,
      fetchFn: async (url, init) => {
        calls.push({ url: String(url), init })
        if (String(url).endsWith("/api/training/runs")) {
          return new Response(
            JSON.stringify({
              run: { ...sampleRun, trainingRunRef: expectedRunRef },
            }),
          )
        }
        return new Response(
          JSON.stringify({
            window: {
              ...sampleSummary.windows[0],
              trainingRunRef: expectedRunRef,
              windowRef: expectedWindowRef,
            },
          }),
        )
      },
      nowIso: () => "2026-06-14T00:00:00.000Z",
    })

    expect(result.ok).toBe(true)
    expect(result.trainingRunRef).toBe(expectedRunRef)
    expect(result.windowRef).toBe(expectedWindowRef)
    expect(result.runPlanned).toBe(true)
    expect(result.windowPlanned).toBe(true)
    expect(calls.map(call => call.url)).toEqual([
      "https://openagents.test/api/training/runs",
      "https://openagents.test/api/training/windows/plan",
    ])
    expect((calls[0]?.init?.headers as Record<string, string>).authorization).toBe(
      "Bearer admin-token",
    )
    expect(
      JSON.parse(String(calls[1]?.init?.body)).trainingRunRef,
    ).toBe(expectedRunRef)
  })

  test("reports a partial plan if window planning fails", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const result = await planTrainingRunWindow({
      adminToken: "admin-token",
      baseUrl: "https://openagents.test",
      enabled: true,
      fetchFn: async (url, init) => {
        calls.push({ url: String(url), init })
        if (String(url).endsWith("/api/training/runs")) {
          return new Response(
            JSON.stringify({ run: { ...sampleRun, trainingRunRef: "run.ok" } }),
          )
        }
        return new Response(JSON.stringify({ reason: "window duplicate" }), {
          status: 409,
        })
      },
      nowIso: () => "2026-06-14T00:00:00.000Z",
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("window_plan_failed")
    expect(result.trainingRunRef).toBe("run.ok")
    expect(result.runPlanned).toBe(true)
    expect(result.windowPlanned).toBe(false)
    expect(result.error).toBe("window duplicate")
    expect(calls).toHaveLength(2)
  })
})

describe("activateTrainingWindow", () => {
  test("does not call admin routes when disabled", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const result = await activateTrainingWindow({
      adminToken: "admin-token",
      baseUrl: "https://openagents.test",
      enabled: false,
      fetchFn: async (url, init) => {
        calls.push({ url: String(url), init })
        return new Response("{}")
      },
      nowIso: () => "2026-06-14T00:00:00.000Z",
      windowRef: "training.window.desktop.r1.test",
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("disabled")
    expect(calls).toHaveLength(0)
  })

  test("does not call admin routes without a token", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const result = await activateTrainingWindow({
      adminToken: "",
      baseUrl: "https://openagents.test",
      enabled: true,
      fetchFn: async (url, init) => {
        calls.push({ url: String(url), init })
        return new Response("{}")
      },
      nowIso: () => "2026-06-14T00:00:00.000Z",
      windowRef: "training.window.desktop.r1.test",
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("admin_token_missing")
    expect(calls).toHaveLength(0)
  })

  test("rejects invalid window refs before calling the Worker", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const result = await activateTrainingWindow({
      adminToken: "admin-token",
      baseUrl: "https://openagents.test",
      enabled: true,
      fetchFn: async (url, init) => {
        calls.push({ url: String(url), init })
        return new Response("{}")
      },
      nowIso: () => "2026-06-14T00:00:00.000Z",
      windowRef: "../private",
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("invalid_window_ref")
    expect(calls).toHaveLength(0)
  })

  test("activates a planned training window through the admin route", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const result = await activateTrainingWindow({
      adminToken: "admin-token",
      baseUrl: "https://openagents.test/",
      enabled: true,
      fetchFn: async (url, init) => {
        calls.push({ url: String(url), init })
        return new Response(
          JSON.stringify({
            window: {
              ...sampleSummary.windows[0],
              state: "active",
              windowRef: "training.window.desktop.r1.test",
            },
          }),
        )
      },
      nowIso: () => "2026-06-14T00:00:00.000Z",
      windowRef: "training.window.desktop.r1.test",
    })

    expect(result.ok).toBe(true)
    expect(result.reason).toBe("activated")
    expect(result.window?.state).toBe("active")
    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toBe(
      "https://openagents.test/api/training/windows/training.window.desktop.r1.test/activate",
    )
    expect((calls[0]?.init?.headers as Record<string, string>).authorization).toBe(
      "Bearer admin-token",
    )
    expect(JSON.parse(String(calls[0]?.init?.body)).receiptRef).toBe(
      "receipt.desktop.training.window.activate.2026.06.14t00.00.00.000z",
    )
  })

  test("reports activation failures without leaking credentials", async () => {
    const result = await activateTrainingWindow({
      adminToken: "admin-token",
      baseUrl: "https://openagents.test",
      enabled: true,
      fetchFn: async () =>
        new Response(JSON.stringify({ reason: "Training window not found." }), {
          status: 404,
        }),
      nowIso: () => "2026-06-14T00:00:00.000Z",
      windowRef: "training.window.missing",
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("transition_failed")
    expect(result.error).toBe("Training window not found.")
    expect(result.message).not.toContain("admin-token")
  })
})

describe("reconcileTrainingWindow", () => {
  test("does not call admin routes when disabled", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const result = await reconcileTrainingWindow({
      adminToken: "admin-token",
      baseUrl: "https://openagents.test",
      enabled: false,
      fetchFn: async (url, init) => {
        calls.push({ url: String(url), init })
        return new Response("{}")
      },
      nowIso: () => "2026-06-14T00:00:00.000Z",
      windowRef: "training.window.desktop.r1.test",
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("disabled")
    expect(calls).toHaveLength(0)
  })

  test("reconciles a sealed training window through the admin route", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const result = await reconcileTrainingWindow({
      adminToken: "admin-token",
      baseUrl: "https://openagents.test/",
      enabled: true,
      fetchFn: async (url, init) => {
        calls.push({ url: String(url), init })
        return new Response(
          JSON.stringify({
            window: {
              ...sampleSummary.windows[0],
              state: "reconciled",
              windowRef: "training.window.desktop.r1.test",
            },
          }),
        )
      },
      nowIso: () => "2026-06-14T00:00:00.000Z",
      windowRef: "training.window.desktop.r1.test",
    })

    expect(result.ok).toBe(true)
    expect(result.reason).toBe("reconciled")
    expect(result.window?.state).toBe("reconciled")
    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toBe(
      "https://openagents.test/api/training/windows/training.window.desktop.r1.test/reconcile",
    )
    expect(JSON.parse(String(calls[0]?.init?.body)).receiptRef).toBe(
      "receipt.desktop.training.window.reconcile.2026.06.14t00.00.00.000z",
    )
  })

  test("reports reconciliation failures without leaking credentials", async () => {
    const result = await reconcileTrainingWindow({
      adminToken: "admin-token",
      baseUrl: "https://openagents.test",
      enabled: true,
      fetchFn: async () =>
        new Response(JSON.stringify({ reason: "Invalid transition." }), {
          status: 400,
        }),
      nowIso: () => "2026-06-14T00:00:00.000Z",
      windowRef: "training.window.active",
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("transition_failed")
    expect(result.error).toBe("Invalid transition.")
    expect(result.message).not.toContain("admin-token")
  })
})

describe("claimTrainingWindowLease", () => {
  test("does not call the Worker when lease claiming is disabled", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const result = await claimTrainingWindowLease({
      baseUrl: "https://openagents.test",
      enabled: false,
      fetchFn: async (url, init) => {
        calls.push({ url: String(url), init })
        return new Response("{}")
      },
      nowIso: () => "2026-06-14T00:00:00.000Z",
      pylonRef: "pylon.training.1",
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("disabled")
    expect(calls).toHaveLength(0)
  })

  test("does not call the Worker without a Pylon ref", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const result = await claimTrainingWindowLease({
      baseUrl: "https://openagents.test",
      enabled: true,
      fetchFn: async (url, init) => {
        calls.push({ url: String(url), init })
        return new Response("{}")
      },
      nowIso: () => "2026-06-14T00:00:00.000Z",
      pylonRef: null,
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("pylon_ref_missing")
    expect(calls).toHaveLength(0)
  })

  test("rejects invalid Pylon refs before calling the Worker", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const result = await claimTrainingWindowLease({
      baseUrl: "https://openagents.test",
      enabled: true,
      fetchFn: async (url, init) => {
        calls.push({ url: String(url), init })
        return new Response("{}")
      },
      nowIso: () => "2026-06-14T00:00:00.000Z",
      pylonRef: "Pylon.Bad",
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("invalid_pylon_ref")
    expect(calls).toHaveLength(0)
  })

  test("claims a training lease and decodes raw Worker records", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const result = await claimTrainingWindowLease({
      baseUrl: "https://openagents.test/",
      enabled: true,
      fetchFn: async (url, init) => {
        calls.push({ url: String(url), init })
        return new Response(
          JSON.stringify({
            lease: {
              claimedAt: "2026-06-14T00:00:00.000Z",
              leaseExpiresAt: "2026-06-14T00:15:00.000Z",
              leaseRef: "training.lease.1",
              pylonRef: "pylon.training.1",
              receiptRefs: ["receipt.training.lease"],
              state: "active",
              trainingRunRef: "training.run.desktop.r1.test",
              windowRef: "training.window.desktop.r1.test",
            },
          }),
        )
      },
      nowIso: () => "2026-06-14T00:00:00.000Z",
      pylonRef: "pylon.training.1",
    })

    expect(result.ok).toBe(true)
    expect(result.reason).toBe("claimed")
    expect(result.lease?.leaseRef).toBe("training.lease.1")
    expect(result.lease?.leaseExpiresInSeconds).toBe(900)
    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toBe(
      "https://openagents.test/api/training/leases/claim",
    )
    expect(JSON.parse(String(calls[0]?.init?.body))).toEqual({
      pylonRef: "pylon.training.1",
      receiptRefs: ["receipt.desktop.training.lease.claim.2026.06.14t00.00.00.000z"],
    })
  })

  test("reports lease claim failures without leaking local state", async () => {
    const result = await claimTrainingWindowLease({
      baseUrl: "https://openagents.test",
      enabled: true,
      fetchFn: async () =>
        new Response(
          JSON.stringify({
            reason: "No active training window is currently claimable.",
          }),
          { status: 404 },
        ),
      nowIso: () => "2026-06-14T00:00:00.000Z",
      pylonRef: "pylon.training.1",
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("claim_failed")
    expect(result.error).toBe("No active training window is currently claimable.")
    expect(result.message).not.toContain("identity.json")
  })
})

describe("requestTrainingBootstrapGrant", () => {
  test("requests a bootstrap grant for the local Pylon ref", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const result = await requestTrainingBootstrapGrant({
      baseUrl: "https://openagents.test/",
      pylonRef: "pylon.training.1",
      trainingRunRef: "training.run.4850",
      fetchFn: async (url, init) => {
        calls.push({ url: String(url), init })
        return new Response(
          JSON.stringify({
            outcome: {
              grant: {
                checkpointDigestRef: "checkpoint.digest.1",
                grantRef: "training.bootstrap.grant.1",
                joinerReceiptRefs: ["receipt.desktop.training.bootstrap.request.1"],
                joinerRef: "pylon.training.1",
                sealReceiptRefs: ["receipt.seal.1"],
                sealedAtDisplay: "1 minute ago",
                sealedWindowRef: "training.window.sealed.1",
                trainingRunRef: "training.run.4850",
              },
              kind: "granted",
            },
          }),
        )
      },
      nowIso: () => "2026-06-14T00:00:00.000Z",
    })

    expect(result.ok).toBe(true)
    expect(result.reason).toBe("granted")
    expect(result.outcome?.kind).toBe("granted")
    expect(calls.map(call => call.url)).toEqual([
      "https://openagents.test/api/training/runs/training.run.4850/bootstrap-grant",
    ])
    expect(JSON.parse(String(calls[0]?.init?.body))).toEqual({
      joinerRef: "pylon.training.1",
      receiptRefs: ["receipt.desktop.training.bootstrap.request.2026.06.14t00.00.00.000z"],
    })
  })

  test("reports queued bootstrap grants as public-safe feedback", async () => {
    const result = await requestTrainingBootstrapGrant({
      baseUrl: "https://openagents.test",
      pylonRef: "pylon.training.1",
      trainingRunRef: "training.run.4851",
      fetchFn: async () =>
        new Response(
          JSON.stringify({
            outcome: {
              joinerRef: "pylon.training.1",
              kind: "queued",
              reasonCode: "join_lifecycle.public.join_deferred_seal_in_flight",
              trainingRunRef: "training.run.4851",
            },
          }),
        ),
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("queued")
    expect(result.message).toContain("join_deferred_seal_in_flight")
  })

  test("does not call the Worker without a local Pylon ref", async () => {
    const calls: string[] = []
    const result = await requestTrainingBootstrapGrant({
      baseUrl: "https://openagents.test",
      pylonRef: null,
      trainingRunRef: "training.run.4850",
      fetchFn: async (url) => {
        calls.push(String(url))
        return new Response("{}")
      },
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("pylon_ref_missing")
    expect(calls).toHaveLength(0)
  })
})

describe("admitTrainingRealGradientEvidence", () => {
  test("does not build an evidence packet when packet writing is disabled", () => {
    const result = buildTrainingEvidencePacket({
      enabled: false,
      evidencePacketPath: "/tmp/evidence.json",
      readBundle: () => {
        throw new Error("should not read receipts")
      },
      trainingRunRef: "training.run.4855",
      workerReceiptsPath: "/tmp/worker-receipts.json",
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("disabled")
    expect(result.blockerRefs).toContain(
      "env.OPENAGENTS_DESKTOP_TRAINING_EVIDENCE_WRITE_ENABLE",
    )
  })

  test("builds an evidence packet candidate from worker receipt refs", () => {
    const writes: Array<{ packet: unknown; path: string }> = []
    const result = buildTrainingEvidencePacket({
      enabled: true,
      evidencePacketPath: "/tmp/training-evidence.json",
      nowIso: () => "2026-06-14T00:00:00.000Z",
      readBundle: () => sampleWorkerReceiptsBundle,
      trainingRunRef: sampleRun.trainingRunRef,
      workerReceiptsPath: "/tmp/worker-receipts.json",
      writePacket: (path, packet) => {
        writes.push({ path, packet })
      },
    })

    expect(result.ok).toBe(true)
    expect(result.reason).toBe("written")
    expect(result.inputSource).toBe("local.training_worker_receipts")
    expect(result.packetSource).toBe(
      "env.OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH",
    )
    expect(result.summary?.receiptRefCount).toBe(2)
    expect(result.summary?.distinctPylonCount).toBe(2)
    expect(result.summary?.freivaldsCommitmentRefCount).toBe(2)
    expect(result.summary?.gradientCloseoutRefCount).toBe(2)
    expect(writes).toHaveLength(1)
    expect(JSON.stringify(writes[0]?.packet)).not.toContain("/tmp/")
    expect(JSON.stringify(result)).not.toContain("/tmp/")
  })

  test("returns packet blockers for incomplete worker receipt bundles", () => {
    const writes: unknown[] = []
    const result = buildTrainingEvidencePacket({
      enabled: true,
      evidencePacketPath: "/tmp/training-evidence.json",
      readBundle: () => ({
        ...sampleWorkerReceiptsBundle,
        budgetRef: null,
        workerReceipts: [sampleWorkerReceiptsBundle.workerReceipts[0]],
      }),
      trainingRunRef: sampleRun.trainingRunRef,
      workerReceiptsPath: "/tmp/worker-receipts.json",
      writePacket: (_path, packet) => {
        writes.push(packet)
      },
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("packet_blocked")
    expect(result.summary?.blockerRefs).toContain(
      "training.evidence_packet.budget_ref_missing",
    )
    expect(result.summary?.blockerRefs).toContain(
      "training.evidence_packet.requires_two_distinct_pylons",
    )
    expect(writes).toHaveLength(1)
  })

  test("summarizes missing evidence packet configuration without reading", () => {
    const result = readTrainingEvidencePacketSummary({
      evidencePacketPath: null,
      readPacket: () => {
        throw new Error("should not read packet")
      },
    })

    expect(result.ok).toBe(false)
    expect(result.configured).toBe(false)
    expect(result.packetSource).toBe(null)
    expect(result.blockerRefs).toContain(
      "env.OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH",
    )
  })

  test("summarizes packet read failures without leaking the local path", () => {
    const result = readTrainingEvidencePacketSummary({
      evidencePacketPath: "/Users/chris/private/evidence.json",
      readPacket: () => {
        throw new Error("/Users/chris/private/evidence.json: denied")
      },
    })

    expect(result.ok).toBe(false)
    expect(result.configured).toBe(true)
    expect(result.error).toBe("training evidence packet read failed")
    expect(result.blockerRefs).toContain("training.evidence_packet.read_failed")
    expect(JSON.stringify(result)).not.toContain("/Users/chris/private")
  })

  test("summarizes a ready evidence packet before admission", () => {
    const result = readTrainingEvidencePacketSummary({
      evidencePacketPath: "/tmp/training-evidence.json",
      nowIso: () => "2026-06-14T00:00:00.000Z",
      readPacket: () => sampleEvidencePacket,
    })

    expect(result.ok).toBe(true)
    expect(result.packetSource).toBe(
      "env.OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH",
    )
    expect(result.receiptRefCount).toBe(3)
    expect(result.distinctPylonCount).toBe(2)
    expect(result.shardContributionCount).toBe(2)
    expect(result.lossPointCount).toBe(2)
    expect(result.finalValidationLoss).toBe(3.1)
    expect(result.maxValidationLoss).toBe(4)
    expect(result.blockerRefs).toEqual([])
    expect(JSON.stringify(result)).not.toContain("/tmp/training-evidence.json")
  })

  test("summarizes packet blockers before admission", () => {
    const result = readTrainingEvidencePacketSummary({
      evidencePacketPath: "/tmp/training-evidence.json",
      readPacket: () => ({
        ...sampleEvidencePacket,
        maxValidationLoss: 3,
        shardContributions: [sampleEvidencePacket.shardContributions[0]],
      }),
    })

    expect(result.ok).toBe(false)
    expect(result.blockerRefs).toContain(
      "training.evidence_packet.loss_exceeds_budget",
    )
    expect(result.blockerRefs).toContain(
      "training.evidence_packet.requires_two_distinct_pylons",
    )
  })

  test("does not read or call the Worker when evidence admission is disabled", async () => {
    const calls: string[] = []
    const result = await admitTrainingRealGradientEvidence({
      adminToken: "admin-token",
      baseUrl: "https://openagents.test",
      enabled: false,
      evidencePacketPath: "/Users/chris/private/evidence.json",
      fetchFn: async (url) => {
        calls.push(String(url))
        return new Response("{}")
      },
      readPacket: () => {
        throw new Error("should not read packet")
      },
      trainingRunRef: "training.run.4855",
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("disabled")
    expect(calls).toHaveLength(0)
  })

  test("requires an explicit evidence packet path", async () => {
    const calls: string[] = []
    const result = await admitTrainingRealGradientEvidence({
      adminToken: "admin-token",
      baseUrl: "https://openagents.test",
      enabled: true,
      evidencePacketPath: null,
      fetchFn: async (url) => {
        calls.push(String(url))
        return new Response("{}")
      },
      trainingRunRef: "training.run.4855",
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("packet_path_missing")
    expect(calls).toHaveLength(0)
  })

  test("reports packet read failures without leaking the local path", async () => {
    const result = await admitTrainingRealGradientEvidence({
      adminToken: "admin-token",
      baseUrl: "https://openagents.test",
      enabled: true,
      evidencePacketPath: "/Users/chris/private/evidence.json",
      readPacket: () => {
        throw new Error("/Users/chris/private/evidence.json: denied")
      },
      trainingRunRef: "training.run.4855",
    })

    expect(result.ok).toBe(false)
    expect(result.reason).toBe("packet_read_failed")
    expect(result.message).toBe("training evidence packet read failed")
    expect(JSON.stringify(result)).not.toContain("/Users/chris/private")
  })

  test("admits a packet through the admin real-gradient evidence route", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const result = await admitTrainingRealGradientEvidence({
      adminToken: "admin-token",
      baseUrl: "https://openagents.test/",
      enabled: true,
      evidencePacketPath: "/tmp/training-evidence.json",
      fetchFn: async (url, init) => {
        calls.push({ url: String(url), init })
        return new Response(
          JSON.stringify({
            realGradient: sampleSummary.realGradient,
            run: sampleRun,
          }),
        )
      },
      nowIso: () => "2026-06-14T00:00:00.000Z",
      readPacket: () => sampleEvidencePacket,
      trainingRunRef: sampleRun.trainingRunRef,
    })

    expect(result.ok).toBe(true)
    expect(result.reason).toBe("admitted")
    expect(result.trainingRunRef).toBe(sampleRun.trainingRunRef)
    expect(result.receiptRefCount).toBe(3)
    expect(result.shardContributionCount).toBe(2)
    expect(result.distinctPylonCount).toBe(2)
    expect(result.realGradient?.closeoutRequirement.satisfied).toBe(true)
    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toBe(
      "https://openagents.test/api/training/runs/run.cs336.a1.real_gradient.demo/real-gradient-evidence",
    )
    expect((calls[0]?.init?.headers as Record<string, string>).authorization).toBe(
      "Bearer admin-token",
    )
    expect(JSON.parse(String(calls[0]?.init?.body))).toEqual(sampleEvidencePacket)
    expect(JSON.stringify(result)).not.toContain("admin-token")
    expect(JSON.stringify(result)).not.toContain("/tmp/training-evidence.json")
  })
})
