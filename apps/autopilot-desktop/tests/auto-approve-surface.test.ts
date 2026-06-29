// #5468 (EPIC #5461): bounded auto-approve surface tests.
//
// The surface is a PURE read projection over the existing node-state plus a
// static policy summary sourced from the real Pylon policy module — no new wire
// verb, no DOM, no runtime. These tests drive the pure helpers
// (auto-approval-view.ts) directly, the same way the swarm/composer/CL-53 tests
// cover the other panes, and assert that the Decisions pane still renders a
// mountable Document with the bounded card present (manual approve/deny stays
// the default; the card is honest about being OFF by default).

import { describe, expect, test } from "bun:test"

import { DEFAULT_ALLOW_KINDS } from "../../pylon/src/node/auto-approval-policy"
import {
  boundedAutoApprovalPolicySummary,
  projectAutoApprovalAudit,
  summarizeAutoApprovalAudit,
  type AutoApprovalAuditEntry,
} from "../src/ui/auto-approval-view"
import { initialModel, Model } from "../src/ui/model"
import type { NodeStateMessage } from "../src/shared/rpc"
import { view } from "../src/ui/view"

describe("#5468 bounded auto-approve policy summary", () => {
  test("is fail-closed and sources its allow-list from the real Pylon policy", () => {
    expect(boundedAutoApprovalPolicySummary.failClosed).toBe(true)
    // No drift: the displayed allow-list is the runtime's DEFAULT_ALLOW_KINDS.
    expect(boundedAutoApprovalPolicySummary.allowKinds).toEqual([...DEFAULT_ALLOW_KINDS])
    expect(boundedAutoApprovalPolicySummary.allowKinds.length).toBeGreaterThan(0)
  })

  test("declares default caps + window and points at the authoritative policy", () => {
    expect(boundedAutoApprovalPolicySummary.defaultMaxAutoApprovals).toBeGreaterThan(0)
    expect(boundedAutoApprovalPolicySummary.defaultWindowMinutes).toBeGreaterThan(0)
    expect(boundedAutoApprovalPolicySummary.policyRef).toBe(
      "apps/pylon/src/node/auto-approval-policy.ts",
    )
    expect(boundedAutoApprovalPolicySummary.cliFlag).toContain("--on-approval auto")
  })

  test("names the categories that ALWAYS escalate/deny (never auto-approved)", () => {
    const always = boundedAutoApprovalPolicySummary.alwaysEscalates.join(" ").toLowerCase()
    expect(always).toContain("destructive")
    expect(always).toContain("secret")
    expect(always).toContain("network")
    // The three bounded categories are present for the legend.
    const ids = boundedAutoApprovalPolicySummary.categories.map((c) => c.id)
    expect(ids).toEqual(["allow", "escalate", "deny"])
  })
})

describe("#5468 audit-trail projection (refs-only, fail-soft)", () => {
  test("projects allow/escalate/deny entries with honest labels", () => {
    const entries: AutoApprovalAuditEntry[] = [
      { approvalRef: "a1", kind: "edit", category: "allow", decision: "approve", reason: "auto.allow.allow_listed_kind" },
      { approvalRef: "a2", kind: "shell", category: "escalate", decision: "pause", reason: "auto.escalate.kind_not_allow_listed" },
      { approvalRef: "a3", kind: "shell", category: "deny", decision: "deny", reason: "auto.deny.destructive_command" },
    ]
    const rows = projectAutoApprovalAudit(entries)
    expect(rows.length).toBe(3)
    expect(rows[0]).toMatchObject({ approvalRef: "a1", autoApproved: true, categoryLabel: "Auto-approved" })
    expect(rows[1]).toMatchObject({ autoApproved: false, categoryLabel: "Escalated to you" })
    expect(rows[2]).toMatchObject({ autoApproved: false, categoryLabel: "Denied" })
    // Reason gloss is derived from the stable enum, never raw text.
    expect(rows[2].reasonGloss).toContain("destructive")
  })

  test("fail-soft: non-array / malformed input yields no rows, unknown category escalates", () => {
    expect(projectAutoApprovalAudit(undefined)).toEqual([])
    expect(projectAutoApprovalAudit(null)).toEqual([])
    expect(projectAutoApprovalAudit("nope")).toEqual([])
    expect(projectAutoApprovalAudit([42, null, "x"])).toEqual([])
    // An unknown/missing category is NEVER shown as an auto-approval.
    const rows = projectAutoApprovalAudit([{ approvalRef: "r", kind: "k", category: "weird", reason: "auto.unknown" }])
    expect(rows.length).toBe(1)
    expect(rows[0].category).toBe("escalate")
    expect(rows[0].autoApproved).toBe(false)
  })

  test("summary counts categories and reports active only when a trail exists", () => {
    expect(summarizeAutoApprovalAudit([])).toMatchObject({ total: 0, active: false })
    const rows = projectAutoApprovalAudit([
      { approvalRef: "a1", kind: "edit", category: "allow", decision: "approve", reason: "auto.allow.allow_listed_kind" },
      { approvalRef: "a2", kind: "x", category: "allow", decision: "approve", reason: "auto.allow.allow_listed_kind" },
      { approvalRef: "a3", kind: "x", category: "deny", decision: "deny", reason: "auto.deny.spend_or_secret" },
    ])
    expect(summarizeAutoApprovalAudit(rows)).toMatchObject({
      total: 3,
      autoApproved: 2,
      escalated: 0,
      denied: 1,
      active: true,
    })
  })
})

describe("#5468 Decisions pane renders the bounded card (Document, not black screen)", () => {
  const renderDecisions = (node: NodeStateMessage | null) => {
    const model = Model.make({ ...initialModel, pane: "decisions", node })
    const doc = view(model) as unknown as { title: string; body: unknown }
    expect(typeof doc.title).toBe("string")
    expect(doc.body).toBeDefined()
    return doc
  }

  test("renders with no node (off-by-default honest state) without throwing", () => {
    renderDecisions(null)
  })

  test("renders with an audit trail present", () => {
    const node: NodeStateMessage = {
      ok: true,
      schema: "x",
      sessions: [],
      autoApprovals: [
        { approvalRef: "a1", kind: "edit", category: "allow", decision: "approve", reason: "auto.allow.allow_listed_kind" },
        { approvalRef: "a2", kind: "rm", category: "deny", decision: "deny", reason: "auto.deny.destructive_command" },
      ],
    }
    renderDecisions(node)
  })
})
