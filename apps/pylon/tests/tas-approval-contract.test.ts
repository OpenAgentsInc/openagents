import { describe, expect, test } from "bun:test"

import {
  decideApproval,
  type ApprovalPolicy,
  type ApprovalRequest,
  type ApprovalSurface,
} from "../src/tas/approval-contract"

const surfaces: ApprovalSurface[] = [
  "headless",
  "background",
  "mobile",
  "pylon",
  "api",
]

const policy: ApprovalPolicy = {
  rules: [
    {
      decision: "deny",
      capability: "shell.danger",
      reason: "dangerous shell capability is denied",
    },
    {
      decision: "ask",
      capability: "workspace.write",
      reason: "workspace writes require approval",
    },
    {
      decision: "allow",
      capability: "workspace.read",
      reason: "workspace reads are allowed",
    },
  ],
  defaultDecision: "ask",
  defaultReason: "unknown capability requires approval",
}

function request(
  capability: string,
  readOnly: boolean,
  surface: ApprovalSurface = "pylon",
): ApprovalRequest {
  return {
    actionRef: `action.fixture.${capability}`,
    surface,
    capability,
    readOnly,
  }
}

describe("approval contract", () => {
  test("allows read-only capability with allow rule", () => {
    expect(decideApproval(policy, request("workspace.read", true))).toEqual({
      decision: "allow",
      reason: "workspace reads are allowed",
    })
  })

  test("denies capability with deny rule", () => {
    expect(decideApproval(policy, request("shell.danger", false))).toEqual({
      decision: "deny",
      reason: "dangerous shell capability is denied",
    })
  })

  test("asks for capability with ask rule", () => {
    expect(decideApproval(policy, request("workspace.write", false))).toEqual({
      decision: "ask",
      reason: "workspace writes require approval",
    })
  })

  test("asks by default when no rule matches", () => {
    expect(decideApproval(policy, request("network.fetch", true))).toEqual({
      decision: "ask",
      reason: "unknown capability requires approval",
    })
  })

  test("read-only approval contract never allows an effectful action", () => {
    expect(decideApproval(policy, request("workspace.read", false))).toEqual({
      decision: "ask",
      reason: "effectful action cannot be allowed by read-only approval contract",
    })

    expect(
      decideApproval(
        { ...policy, effectfulFallbackDecision: "deny" },
        request("workspace.read", false),
      ),
    ).toEqual({
      decision: "deny",
      reason: "effectful action cannot be allowed by read-only approval contract",
    })
  })

  test("same policy and request semantics produce same decision across surfaces", () => {
    const decisions = surfaces.map((surface) =>
      decideApproval(policy, request("workspace.write", false, surface)),
    )

    expect(new Set(decisions.map((decision) => decision.decision))).toEqual(
      new Set(["ask"]),
    )
    expect(new Set(decisions.map((decision) => decision.reason))).toEqual(
      new Set(["workspace writes require approval"]),
    )
  })
})
