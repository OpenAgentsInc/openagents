import { describe, expect, test } from "vite-plus/test"

import {
  admitClaudePermission,
  issueClaudeOwnerLocalPermissionAuthority,
  projectClaudePermissionAudit,
  revokeClaudeOwnerLocalPermissionAuthority,
} from "./claude-owner-local-permission.js"

const now = new Date("2026-07-11T12:00:00.000Z")
const scope = {
  pylonRef: "pylon.owner.local",
  runRef: "fleet_run.owner.local",
  operationRef: "assignment.public.claude.owner_local",
  accountRefHash: "account.pylon.claude_agent.0123456789abcdef",
}

const authority = () =>
  issueClaudeOwnerLocalPermissionAuthority({
    authorizationRef: "authorization.pylon.claude_owner_local.0123456789abcdef01234567",
    ...scope,
    now,
  })

describe("Claude owner-local permission authority", () => {
  test("admits only the exact target, run, operation, and named account", () => {
    const admitted = admitClaudePermission({
      control: { authority: authority() },
      expected: scope,
      now,
    })
    expect(admitted).toMatchObject({
      kind: "owner_local",
      permissionMode: "bypassPermissions",
      authorityRef: expect.stringMatching(/^authority\.pylon\.claude_owner_local\./),
      auditReceiptRef: expect.stringMatching(/^proof\.pylon\.claude_owner_local_permission\./),
    })

    const mismatches = [
      { ...scope, pylonRef: "pylon.remote.other" },
      { ...scope, runRef: "fleet_run.other" },
      { ...scope, operationRef: "assignment.public.other" },
      { ...scope, accountRefHash: "account.pylon.claude_agent.other" },
    ]
    for (const expected of mismatches) {
      expect(admitClaudePermission({
        control: { authority: authority() },
        expected,
        now,
      })).toMatchObject({
        kind: "refused",
        permissionMode: "acceptEdits",
        blockerRef: "blocker.pylon.claude_owner_local_permission.scope_mismatch",
      })
    }
  })

  test("public absence stays bounded and expired or revoked grants refuse", () => {
    expect(admitClaudePermission({ expected: scope, now })).toEqual({
      kind: "bounded",
      permissionMode: "acceptEdits",
      authorityRef: null,
      auditReceiptRef: null,
    })
    expect(admitClaudePermission({
      control: { authority: authority() },
      expected: scope,
      now: new Date(now.getTime() + 31 * 60 * 1_000),
    })).toMatchObject({
      kind: "refused",
      blockerRef: "blocker.pylon.claude_owner_local_permission.expired",
    })
    expect(admitClaudePermission({
      control: {
        authority: revokeClaudeOwnerLocalPermissionAuthority(
          authority(),
          new Date(now.getTime() + 1_000),
        ),
      },
      expected: scope,
      now: new Date(now.getTime() + 2_000),
    })).toMatchObject({
      kind: "refused",
      blockerRef: "blocker.pylon.claude_owner_local_permission.revoked",
    })
  })

  test("serialized or process-reconstructed authority cannot be replayed", () => {
    const replay = JSON.parse(JSON.stringify(authority()))
    expect(admitClaudePermission({
      control: { authority: replay },
      expected: scope,
      now,
    })).toMatchObject({
      kind: "refused",
      blockerRef: "blocker.pylon.claude_owner_local_permission.invalid",
    })
  })

  test("the projected audit contains refs only and no permission mode", () => {
    const serialized = JSON.stringify(
      projectClaudePermissionAudit(admitClaudePermission({
        control: { authority: authority() },
        expected: scope,
        now,
      })),
    )
    expect(serialized).toContain("policy.pylon.claude.owner_local_bypass.v1")
    expect(serialized).not.toContain("bypassPermissions")
    expect(serialized).not.toContain("acceptEdits")
  })
})
