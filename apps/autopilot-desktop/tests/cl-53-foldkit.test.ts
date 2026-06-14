// CL-53: pure-helper + reducer tests for the Foldkit desktop rewrite.
//
// The view itself needs a DOM/runtime to assert against, but the logic that
// used to live in the deleted hand-DOM panes/cards now lives in pure helpers
// (helpers.ts) and the pure reducer (update.ts). These cover the same behavior
// the deleted cl-47..cl-58 tests covered, without a DOM.

import { describe, expect, test } from "bun:test"
import type {
  AssignmentRow,
  NodeStateMessage,
  WalletStatusRow,
} from "../src/shared/rpc"
import {
  approvalLabel,
  artifactLineText,
  assignmentMeta,
  connectionSummary,
  coordinatorToggleLabel,
  nodeStatusLine,
  parseVerifyLines,
  shipStatusLine,
  stateBreakdown,
  verifyLineText,
  walletSummary,
} from "../src/ui/helpers"
import {
  initialModel,
  Model,
  modelNode,
  modelTrainingBootstrap,
  modelTrainingDashboard,
  modelTrainingPromiseGates,
} from "../src/ui/model"
import {
  ChangedAskTitle,
  ClickedActivateTrainingWindow,
  ClickedClaimTrainingLease,
  ClickedPlanTrainingWindow,
  ClickedQueueTrainingCloseout,
  ClickedRefreshTrainingRuns,
  ClickedReconcileTrainingWindow,
  ClickedResolveApproval,
  ClickedRequestTrainingBootstrap,
  ClickedSubmitIntent,
  GotTrainingDashboard,
  GotTrainingPromiseGates,
  GotNodeState,
  NavigatedTo,
  SelectedSession,
  SettledActivateTrainingWindow,
  SettledClaimTrainingLease,
  SettledPlanTrainingWindow,
  SettledQueueTrainingCloseout,
  SettledReconcileTrainingWindow,
  SettledRequestTrainingBootstrap,
  SettledResolveApproval,
  SettledSubmitIntent,
  ToggledEvent,
} from "../src/ui/message"
import { update } from "../src/ui/update"

const session = (sessionRef: string, state: string) =>
  ({
    sessionRef,
    adapter: "codex",
    state,
    accountRefHash: null,
    updatedAt: "2026-06-13T00:00:00.000Z",
  }) as never

describe("helpers (CL-47..CL-58 parity, pure)", () => {
  test("nodeStatusLine summarizes connection + breakdown", () => {
    expect(nodeStatusLine({ ok: true, sessions: [] })).toBe("connected · 0 sessions")
    expect(
      nodeStatusLine({
        ok: true,
        sessions: [session("a", "running"), session("b", "failed")],
      }),
    ).toBe("connected · 2 sessions · 1 running · 1 failed")
    expect(nodeStatusLine({ ok: false, sessions: [session("a", "running")] })).toBe(
      "offline · 1 session · 1 running",
    )
  })

  test("stateBreakdown groups by state", () => {
    expect(stateBreakdown([])).toBe("")
    expect(
      stateBreakdown([{ state: "running" }, { state: "running" }, { state: "failed" }]),
    ).toBe("2 running · 1 failed")
  })

  test("approvalLabel falls back to kind when prompt empty", () => {
    expect(approvalLabel({ prompt: "Approve deploy?", kind: "deploy" })).toBe(
      "Approve deploy?",
    )
    expect(approvalLabel({ prompt: "   ", kind: "deploy" })).toBe("deploy")
  })

  test("shipStatusLine maps intent status to label + terminal flag", () => {
    expect(shipStatusLine("shipped")).toEqual({
      text: "✓ shipped",
      terminal: true,
      dotColor: "#3fb950",
    })
    expect(shipStatusLine("planning").terminal).toBe(false)
    expect(shipStatusLine("mystery").text).toBe("mystery")
  })

  test("walletSummary derives value + summary", () => {
    const wallet: WalletStatusRow = {
      configured: true,
      daemonOnline: true,
      balanceSats: 12345,
      receiveReady: true,
      sendReady: false,
      readiness: "ready",
    }
    expect(walletSummary(wallet)).toEqual({
      value: "12,345 sats",
      summary: "wallet online · ready · receive ✓",
    })
    expect(walletSummary({ ...wallet, balanceSats: null, daemonOnline: false, receiveReady: false }).value).toBe(
      "—",
    )
  })

  test("assignmentMeta derives goal + meta", () => {
    const row: AssignmentRow = {
      assignmentRef: "assign-abcdef123456",
      leaseRef: "lease-1",
      goal: "Fix the bug",
      paymentMode: "fixed",
      expiresAt: "2026-07-01T00:00:00.000Z",
    }
    const { goal, meta } = assignmentMeta(row)
    expect(goal).toBe("Fix the bug")
    expect(meta).toContain("fixed")
    expect(meta).toContain("expires 2026-07-01")
  })

  test("connectionSummary + coordinatorToggleLabel", () => {
    expect(connectionSummary(null)).toBe("connecting…")
    expect(connectionSummary({ ok: true })).toBe("online")
    expect(connectionSummary({ ok: false })).toBe("offline")
    expect(coordinatorToggleLabel(true)).toBe("▶ Resume")
    expect(coordinatorToggleLabel(false)).toBe("⏸ Pause")
  })

  test("parseVerifyLines trims + drops empties", () => {
    expect(parseVerifyLines("bun test\n\n  bun run typecheck  \n")).toEqual([
      "bun test",
      "bun run typecheck",
    ])
  })

  test("verifyLineText + artifactLineText", () => {
    expect(verifyLineText(session("a", "completed")).toneClass).toBe("verify-completed")
    expect(verifyLineText(session("a", "failed")).text).toContain("✗")
    expect(artifactLineText(null)).toBe("")
    expect(
      artifactLineText({
        kind: "diff",
        outcome: "applied",
        editedFileCount: 3,
        commandCount: 2,
        totalTokens: 100,
      }),
    ).toBe("artifact: applied · 3 files · 2 cmds · 100 tok")
  })
})

describe("update reducer (CL-53)", () => {
  test("GotNodeState stores the projection", () => {
    const node: NodeStateMessage = { ok: true, schema: "x", sessions: [] }
    const [model] = update(initialModel, GotNodeState({ node }))
    expect(modelNode(model)?.ok).toBe(true)
  })

  test("NavigatedTo switches pane and resets expanded events", () => {
    const start = Model.make({ ...initialModel, expandedEvents: [1, 2] })
    const [model] = update(start, NavigatedTo({ pane: "settings" }))
    expect(model.pane).toBe("settings")
    expect(model.expandedEvents).toEqual([])
  })

  test("SelectedSession focuses the detail pane", () => {
    const [model] = update(initialModel, SelectedSession({ sessionRef: "sess-1" }))
    expect(model.pane).toBe("session-detail")
    expect(model.selectedSessionRef).toBe("sess-1")
  })

  test("ToggledEvent toggles membership", () => {
    const [a] = update(initialModel, ToggledEvent({ eventIndex: 3 }))
    expect(a.expandedEvents).toEqual([3])
    const [b] = update(a, ToggledEvent({ eventIndex: 3 }))
    expect(b.expandedEvents).toEqual([])
  })

  test("approval resolve optimistically hides, un-hides on rejection", () => {
    const [hidden, commands] = update(
      initialModel,
      ClickedResolveApproval({ approvalRef: "ap-1", decision: "approve" }),
    )
    expect(hidden.resolvedApprovals).toEqual(["ap-1"])
    expect(commands).toHaveLength(1)

    // Confirmed → stays hidden.
    const [ok] = update(hidden, SettledResolveApproval({ approvalRef: "ap-1", ok: true }))
    expect(ok.resolvedApprovals).toEqual(["ap-1"])

    // Rejected → un-hidden.
    const [back] = update(hidden, SettledResolveApproval({ approvalRef: "ap-1", ok: false }))
    expect(back.resolvedApprovals).toEqual([])
  })

  test("ask submit validation blocks empty title (no command)", () => {
    const [model, commands] = update(initialModel, ClickedSubmitIntent())
    expect(model.askStatus.tone).toBe("error")
    expect(commands).toHaveLength(0)
  })

  test("ask submit dispatches a command with a valid title", () => {
    const withTitle = update(initialModel, ChangedAskTitle({ value: "Ship it" }))[0]
    const [model, commands] = update(withTitle, ClickedSubmitIntent())
    expect(model.askPending).toBe(true)
    expect(commands).toHaveLength(1)
  })

  test("SettledSubmitIntent clears the form on success", () => {
    const withTitle = update(initialModel, ChangedAskTitle({ value: "Ship it" }))[0]
    const [model] = update(withTitle, SettledSubmitIntent({ ok: true, text: "sent · received" }))
    expect(model.askTitle).toBe("")
    expect(model.askStatus.tone).toBe("success")
  })

  test("training plan action dispatches and stores the public-safe result", () => {
    const [pending, commands] = update(initialModel, ClickedPlanTrainingWindow())
    expect(pending.trainingPlanPending).toBe(true)
    expect(pending.trainingPlanStatus.tone).toBe("info")
    expect(commands).toHaveLength(1)

    const [settled, followups] = update(
      pending,
      SettledPlanTrainingWindow({
        projection: {
          ok: true,
          enabled: true,
          fetchedAt: "2026-06-14T00:00:00.000Z",
          sourceUrl: "https://openagents.test/api/training/windows/plan",
          trainingRunRef: "training.run.desktop.r1.test",
          windowRef: "training.window.desktop.r1.test",
          run: null,
          window: null,
          runPlanned: true,
          windowPlanned: true,
          reason: "planned",
          message: "planned training.run.desktop.r1.test / training.window.desktop.r1.test",
        },
      }),
    )
    expect(settled.trainingPlanPending).toBe(false)
    expect(settled.trainingPlanStatus.tone).toBe("success")
    expect(settled.trainingPlan).toMatchObject({
      trainingRunRef: "training.run.desktop.r1.test",
      windowRef: "training.window.desktop.r1.test",
    })
    expect(followups).toHaveLength(3)
  })

  test("training refresh loads run, dashboard, and promise projections", () => {
    const [pending, commands] = update(initialModel, ClickedRefreshTrainingRuns())
    expect(pending.trainingRunsPending).toBe(true)
    expect(pending.trainingDashboardPending).toBe(true)
    expect(pending.trainingPromiseGatesPending).toBe(true)
    expect(commands).toHaveLength(3)
  })

  test("training dashboard projection is stored with a lane summary", () => {
    const [settled] = update(
      initialModel,
      GotTrainingDashboard({
        projection: {
          ok: true,
          fetchedAt: "2026-06-14T00:00:00.000Z",
          sourceUrl: "https://openagents.test/api/training/leaderboards",
          leaderboards: {
            blockerRefs: [],
            lanes: [
              {
                blockerRefs: [],
                lane: "a1_loss",
                rowCount: 1,
                title: "A1 Loss Under Budget",
                topRow: {
                  contributorRef: "pylon.training.1",
                  rank: 1,
                  score: 3.1,
                  scoreLabel: "validation_loss=3.1",
                  settledPayoutSats: 21,
                  trainingRunRef: "training.run.1",
                },
              },
            ],
          },
          a2: {
            blockerRefs: [],
            observedDeviceClassCount: 2,
            observedMeasurementCount: 3,
            verifiedMeasurementCount: 2,
          },
          a3: {
            blockerRefs: [],
            cellCount: 2,
            fitArtifactCount: 1,
            verifiedCellCount: 1,
          },
          a4: {
            blockerRefs: [],
            evalDeltaBonusBlockerRefs: [],
            observedVerifiedStages: ["pii_masking"],
            requiredVerifiedStageCount: 3,
            shardCount: 1,
          },
          a5: {
            blockerRefs: [],
            evalSuiteCount: 1,
            updateBoundaryRef: "issue.github.openagents.4669",
            verifiedSuiteCount: 1,
          },
        },
      }),
    )

    expect(settled.trainingDashboardPending).toBe(false)
    expect(settled.trainingDashboardStatus.tone).toBe("success")
    expect(modelTrainingDashboard(settled)?.leaderboards.lanes[0]?.rowCount).toBe(1)
  })

  test("training promise gates projection is stored with blocker status", () => {
    const [settled] = update(
      initialModel,
      GotTrainingPromiseGates({
        projection: {
          ok: true,
          fetchedAt: "2026-06-14T00:00:00.000Z",
          registryVersion: "2026-06-12.8",
          sourceUrl: "https://openagents.test/api/public/product-promises",
          blockerRefs: [
            "blocker.product_promises.public_distributed_training_run_receipts_missing",
          ],
          promises: [
            {
              blockerRefs: [
                "blocker.product_promises.public_distributed_training_run_receipts_missing",
              ],
              claim: "Pylons participate in public distributed model-training runs.",
              evidenceRefCount: 3,
              productArea: "training",
              promiseId: "training.public_distributed_training_run.v1",
              safeCopy: "Not green yet.",
              state: "red",
              verification: "Requires run, work, validation, and settlement refs.",
            },
          ],
          stateCounts: {
            degraded: 0,
            green: 0,
            planned: 0,
            red: 1,
            withdrawn: 0,
            yellow: 0,
            unknown: 0,
          },
        },
      }),
    )

    expect(settled.trainingPromiseGatesPending).toBe(false)
    expect(settled.trainingPromiseGatesStatus.tone).toBe("info")
    expect(modelTrainingPromiseGates(settled)?.promises[0]?.state).toBe("red")
  })

  test("training activation action dispatches and stores the public-safe result", () => {
    const [pending, commands] = update(
      initialModel,
      ClickedActivateTrainingWindow({
        windowRef: "training.window.desktop.r1.test",
      }),
    )
    expect(pending.trainingActivationPending).toBe(true)
    expect(pending.trainingActivationStatus.tone).toBe("info")
    expect(commands).toHaveLength(1)

    const [settled, followups] = update(
      pending,
      SettledActivateTrainingWindow({
        projection: {
          ok: true,
          enabled: true,
          fetchedAt: "2026-06-14T00:00:00.000Z",
          sourceUrl:
            "https://openagents.test/api/training/windows/training.window.desktop.r1.test/activate",
          windowRef: "training.window.desktop.r1.test",
          window: null,
          reason: "activated",
          message: "activated training.window.desktop.r1.test",
        },
      }),
    )
    expect(settled.trainingActivationPending).toBe(false)
    expect(settled.trainingActivationStatus.tone).toBe("success")
    expect(settled.trainingActivation).toMatchObject({
      windowRef: "training.window.desktop.r1.test",
    })
    expect(followups).toHaveLength(3)
  })

  test("training lease claim action dispatches and stores the public-safe result", () => {
    const [pending, commands] = update(initialModel, ClickedClaimTrainingLease())
    expect(pending.trainingLeasePending).toBe(true)
    expect(pending.trainingLeaseStatus.tone).toBe("info")
    expect(commands).toHaveLength(1)

    const [settled, followups] = update(
      pending,
      SettledClaimTrainingLease({
        projection: {
          ok: true,
          enabled: true,
          fetchedAt: "2026-06-14T00:00:00.000Z",
          sourceUrl: "https://openagents.test/api/training/leases/claim",
          pylonRef: "pylon.training.1",
          lease: {
            claimedAtDisplay: "now",
            leaseExpiresInSeconds: 900,
            leaseRef: "training.lease.1",
            pylonRef: "pylon.training.1",
            receiptRefs: ["receipt.training.lease"],
            state: "active",
            trainingRunRef: "training.run.desktop.r1.test",
            windowRef: "training.window.desktop.r1.test",
          },
          reason: "claimed",
          message: "claimed training.lease.1 for training.window.desktop.r1.test",
        },
      }),
    )
    expect(settled.trainingLeasePending).toBe(false)
    expect(settled.trainingLeaseStatus.tone).toBe("success")
    expect(settled.trainingLease).toMatchObject({
      lease: { leaseRef: "training.lease.1" },
    })
    expect(followups).toHaveLength(3)
  })

  test("training bootstrap action dispatches and stores the public-safe result", () => {
    const [pending, commands] = update(
      initialModel,
      ClickedRequestTrainingBootstrap({
        trainingRunRef: "training.run.4850",
      }),
    )
    expect(pending.trainingBootstrapPending).toBe(true)
    expect(pending.trainingBootstrapStatus.tone).toBe("info")
    expect(commands).toHaveLength(1)

    const [settled, followups] = update(
      pending,
      SettledRequestTrainingBootstrap({
        projection: {
          ok: true,
          fetchedAt: "2026-06-14T00:00:00.000Z",
          sourceUrl:
            "https://openagents.test/api/training/runs/training.run.4850/bootstrap-grant",
          pylonRef: "pylon.training.1",
          trainingRunRef: "training.run.4850",
          outcome: {
            grant: {
              checkpointDigestRef: "checkpoint.digest.1",
              grantRef: "training.bootstrap.grant.1",
              joinerReceiptRefs: ["receipt.bootstrap.1"],
              joinerRef: "pylon.training.1",
              sealReceiptRefs: ["receipt.seal.1"],
              sealedAtDisplay: "now",
              sealedWindowRef: "training.window.sealed.1",
              trainingRunRef: "training.run.4850",
            },
            kind: "granted",
          },
          reason: "granted",
          message: "bootstrap grant training.bootstrap.grant.1",
        },
      }),
    )

    expect(settled.trainingBootstrapPending).toBe(false)
    expect(settled.trainingBootstrapStatus.tone).toBe("success")
    expect(modelTrainingBootstrap(settled)?.outcome?.kind).toBe("granted")
    expect(followups).toHaveLength(3)
  })

  test("training bootstrap queue feedback refreshes public projections", () => {
    const [settled, followups] = update(
      initialModel,
      SettledRequestTrainingBootstrap({
        projection: {
          ok: false,
          fetchedAt: "2026-06-14T00:00:00.000Z",
          sourceUrl:
            "https://openagents.test/api/training/runs/training.run.4850/bootstrap-grant",
          pylonRef: "pylon.training.1",
          trainingRunRef: "training.run.4850",
          outcome: {
            joinerRef: "pylon.training.1",
            kind: "queued",
            reasonCode: "seal_in_flight",
            trainingRunRef: "training.run.4850",
          },
          reason: "queued",
          message: "bootstrap queued: seal_in_flight",
        },
      }),
    )

    expect(settled.trainingBootstrapPending).toBe(false)
    expect(settled.trainingBootstrapStatus).toEqual({
      text: "bootstrap queued: seal_in_flight",
      tone: "info",
    })
    expect(modelTrainingBootstrap(settled)?.outcome?.kind).toBe("queued")
    expect(followups).toHaveLength(3)
  })

  test("training closeout packet action dispatches and stores local queue feedback", () => {
    const [pending, commands] = update(
      initialModel,
      ClickedQueueTrainingCloseout({
        trainingRunRef: "training.run.4850",
        windowRef: "training.window.4850.active",
        leaseRef: "training.lease.4850",
        bootstrapGrantRef: "training.bootstrap.grant.4850",
      }),
    )
    expect(pending.trainingCloseoutPending).toBe(true)
    expect(pending.trainingCloseoutStatus.tone).toBe("info")
    expect(commands).toHaveLength(1)

    const [settled, followups] = update(
      pending,
      SettledQueueTrainingCloseout({
        ok: true,
        text: "queued · accepted",
      }),
    )

    expect(settled.trainingCloseoutPending).toBe(false)
    expect(settled.trainingCloseoutStatus).toEqual({
      text: "queued · accepted",
      tone: "success",
    })
    expect(followups).toHaveLength(3)
  })

  test("training reconcile action dispatches and stores the public-safe result", () => {
    const [pending, commands] = update(
      initialModel,
      ClickedReconcileTrainingWindow({
        windowRef: "training.window.desktop.r1.test",
      }),
    )
    expect(pending.trainingReconcilePending).toBe(true)
    expect(pending.trainingReconcileStatus.tone).toBe("info")
    expect(commands).toHaveLength(1)

    const [settled, followups] = update(
      pending,
      SettledReconcileTrainingWindow({
        projection: {
          ok: true,
          enabled: true,
          fetchedAt: "2026-06-14T00:00:00.000Z",
          sourceUrl:
            "https://openagents.test/api/training/windows/training.window.desktop.r1.test/reconcile",
          windowRef: "training.window.desktop.r1.test",
          window: null,
          reason: "reconciled",
          message: "reconciled training.window.desktop.r1.test",
        },
      }),
    )
    expect(settled.trainingReconcilePending).toBe(false)
    expect(settled.trainingReconcileStatus.tone).toBe("success")
    expect(settled.trainingReconcile).toMatchObject({
      windowRef: "training.window.desktop.r1.test",
    })
    expect(followups).toHaveLength(3)
  })
})
