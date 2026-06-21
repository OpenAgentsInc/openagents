// VCODE-09 (#5926): code-mode Decisions UI.
//
// Pins the explicit one-shot actions, the visible scope grid, and the
// fail-closed persistent approval affordance.

import { describe, expect, test } from "bun:test"

import type { ApprovalRow, NodeStateMessage } from "../src/shared/rpc"
import { initialModel, Model } from "../src/ui/model"
import { ClickedResolveApproval, GotNodeState } from "../src/ui/message"
import { projectApprovalDecision } from "../src/ui/approval-decision-projection"
import { update } from "../src/ui/update"
import { view } from "../src/ui/view"

const serializeView = (node: unknown): string => {
  const seen = new WeakSet<object>()
  return JSON.stringify(node, (_key, value) => {
    if (typeof value === "function") return "[fn]"
    if (typeof value === "object" && value !== null) {
      if (seen.has(value)) return "[cycle]"
      seen.add(value)
    }
    return value
  })
}

const approval = (input: Partial<ApprovalRow> = {}): ApprovalRow => ({
  approvalRef: input.approvalRef ?? "approval.codex.exec.1",
  kind: input.kind ?? "exec",
  prompt: input.prompt ?? "Run bun test?",
  createdAt: input.createdAt ?? "2026-06-21T20:00:00.000Z",
  sessionRef: input.sessionRef,
  workspaceRef: input.workspaceRef,
  commandClass: input.commandClass,
  accountRefHash: input.accountRefHash,
  expiresAt: input.expiresAt,
  lane: input.lane,
  source: input.source,
  assignmentPath: input.assignmentPath,
  persistentApprovalSupported: input.persistentApprovalSupported,
})

const nodeWithApproval = (row: ApprovalRow): NodeStateMessage => ({
  ok: true,
  schema: "openagents.pylon.control.v0.3",
  sessions: [],
  approvals: [row],
})

describe("approval decision UI (#5926)", () => {
  test("projects current approvals as reject/allow-once only when scope is missing", () => {
    const projected = projectApprovalDecision(approval())

    expect(projected.actions.map((action) => action.label)).toEqual([
      "Reject",
      "Allow once",
      "Scoped always",
    ])
    expect(projected.actions.find((action) => action.kind === "reject")?.decision).toBe("deny")
    expect(projected.actions.find((action) => action.kind === "allow_once")?.decision).toBe("approve")
    expect(projected.scopedAlwaysEnabled).toBe(false)
    expect(projected.scopedAlwaysBlockers).toContain("session scope not published")
    expect(projected.scopedAlwaysBlockers).toContain("desktop has no persistent approval control verb")
    expect(projected.scopeRows.map((row) => `${row.label}: ${row.value}`)).toContain(
      "Expiration: one decision only",
    )
  })

  test("keeps persistent approval blocked on public/provider lanes even with visible scope", () => {
    const projected = projectApprovalDecision(
      approval({
        sessionRef: "session.pylon.codex.live",
        workspaceRef: "workspace.openagents.desktop",
        commandClass: "exec",
        accountRefHash: "account.pylon.codex.work.abcdef0123456789",
        expiresAt: "2026-06-21T20:15:00.000Z",
        lane: "public_assignment",
        persistentApprovalSupported: true,
      }),
    )

    expect(projected.scopeRows.map((row) => row.value)).toContain("workspace.openagents.desktop")
    expect(projected.scopeRows.map((row) => row.value)).toContain("codex ...23456789")
    expect(projected.scopedAlwaysEnabled).toBe(false)
    expect(projected.scopedAlwaysBlockers).toContain(
      "public assignment, market, and provider lanes cannot use local danger modes",
    )
  })

  test("renders scope before explicit decision actions", () => {
    let model = Model.make({ ...initialModel, pane: "decisions" })
    ;[model] = update(model, GotNodeState({ node: nodeWithApproval(approval()) }))

    const tree = serializeView(view(model).body)
    expect(tree).toContain("Decisions")
    expect(tree).toContain("Run bun test?")
    expect(tree).toContain("Session")
    expect(tree).toContain("Workspace")
    expect(tree).toContain("Command class")
    expect(tree).toContain("Account")
    expect(tree).toContain("Expiration")
    expect(tree).toContain("Reject")
    expect(tree).toContain("Allow once")
    expect(tree).toContain("Scoped always")
    expect(tree).toContain("Scoped always unavailable")
  })

  test("reject and allow once still call the existing resolve command", () => {
    const [allow, allowCommands] = update(
      initialModel,
      ClickedResolveApproval({ approvalRef: "ap-allow", decision: "approve" }),
    )
    expect(allow.resolvedApprovals).toEqual(["ap-allow"])
    expect(allowCommands.map((command) => command.name)).toEqual(["ResolveApproval"])

    const [reject, rejectCommands] = update(
      initialModel,
      ClickedResolveApproval({ approvalRef: "ap-reject", decision: "deny" }),
    )
    expect(reject.resolvedApprovals).toEqual(["ap-reject"])
    expect(rejectCommands.map((command) => command.name)).toEqual(["ResolveApproval"])
  })
})
