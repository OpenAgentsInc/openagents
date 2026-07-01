import { describe, expect, test } from "bun:test"

import {
  khalaCodeDesktopCodexApprovalResponsePayload,
} from "../src/shared/codex-approval-decisions"

describe("Codex approval decision payloads", () => {
  test("builds command approval responses with Codex decision tags", () => {
    expect(khalaCodeDesktopCodexApprovalResponsePayload({
      action: "accept",
      method: "item/commandExecution/requestApproval",
    })).toEqual({ decision: "accept" })

    expect(khalaCodeDesktopCodexApprovalResponsePayload({
      action: "acceptWithExecpolicyAmendment",
      execpolicyAmendment: ["git status"],
      method: "item/commandExecution/requestApproval",
    })).toEqual({
      decision: {
        acceptWithExecpolicyAmendment: {
          execpolicy_amendment: ["git status"],
        },
      },
    })

    expect(khalaCodeDesktopCodexApprovalResponsePayload({
      action: "applyNetworkPolicyAmendment",
      method: "item/commandExecution/requestApproval",
      networkPolicyAmendment: {
        action: "allow",
        host: "api.example.com",
      },
    })).toEqual({
      decision: {
        applyNetworkPolicyAmendment: {
          network_policy_amendment: {
            action: "allow",
            host: "api.example.com",
          },
        },
      },
    })
  })

  test("builds file-change approval responses", () => {
    expect(khalaCodeDesktopCodexApprovalResponsePayload({
      action: "acceptForSession",
      method: "item/fileChange/requestApproval",
    })).toEqual({ decision: "acceptForSession" })

    expect(khalaCodeDesktopCodexApprovalResponsePayload({
      action: "cancel",
      method: "item/fileChange/requestApproval",
    })).toEqual({ decision: "cancel" })
  })

  test("builds permission approval and empty decline responses", () => {
    expect(khalaCodeDesktopCodexApprovalResponsePayload({
      action: "grantPermissionsWithStrictReview",
      method: "item/permissions/requestApproval",
      permissions: {
        network: { enabled: true },
        fileSystem: {
          read: ["/workspace"],
          write: ["/workspace/out"],
        },
      },
    })).toEqual({
      permissions: {
        network: { enabled: true },
        fileSystem: {
          read: ["/workspace"],
          write: ["/workspace/out"],
        },
      },
      scope: "turn",
      strictAutoReview: true,
    })

    expect(khalaCodeDesktopCodexApprovalResponsePayload({
      action: "decline",
      method: "item/permissions/requestApproval",
      permissions: {
        network: { enabled: true },
      },
    })).toEqual({
      permissions: {},
      scope: "turn",
    })
  })

  test("rejects invalid decision and amendment combinations", () => {
    expect(() => khalaCodeDesktopCodexApprovalResponsePayload({
      action: "grantPermissions",
      method: "item/commandExecution/requestApproval",
      permissions: {},
    })).toThrow("not valid for Codex command approval")

    expect(() => khalaCodeDesktopCodexApprovalResponsePayload({
      action: "acceptWithExecpolicyAmendment",
      method: "item/commandExecution/requestApproval",
    })).toThrow("requires an execpolicy amendment")
  })
})
