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
import { initialModel, Model, modelNode } from "../src/ui/model"
import {
  ChangedAskTitle,
  ClickedPlanTrainingWindow,
  ClickedResolveApproval,
  ClickedSubmitIntent,
  GotNodeState,
  NavigatedTo,
  SelectedSession,
  SettledPlanTrainingWindow,
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
    expect(followups).toHaveLength(1)
  })
})
