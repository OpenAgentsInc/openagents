import { describe, expect, test } from "bun:test"

import {
  decodeOpenAgentsMcpContractStatus,
  decodeOpenAgentsMcpGrant,
  filterOpenAgentsMcpDescriptorsByGrantSet,
  isOpenAgentsMcpHighRiskAuthority,
  openAgentsMcpAuthorityClasses,
  openAgentsMcpContractStatus,
  openAgentsMcpHighRiskAuthorityClasses,
  type OpenAgentsMcpGrant,
} from "./index.js"

describe("@openagentsinc/mcp-contract", () => {
  test("exports a phase 0 status without exposing runtime transports", () => {
    expect(decodeOpenAgentsMcpContractStatus(openAgentsMcpContractStatus)).toEqual({
      schemaVersion: "openagents.mcp.phase0.v1",
      packageName: "@openagentsinc/mcp-contract",
      phase: "phase_0_contract_groundwork",
      runtimeTransportExposed: false,
    })
  })

  test("decodes every authority class and classifies high-risk grants", () => {
    expect(openAgentsMcpAuthorityClasses).toHaveLength(13)
    for (const authorityClass of openAgentsMcpAuthorityClasses) {
      const decoded = decodeOpenAgentsMcpGrant({
        grantRef: `grant.test.${authorityClass}`,
        subjectRef: "client.test.operator",
        authorityClass,
        decision: "granted",
        scopeRefs: ["scope.test"],
        grantedAt: "2026-06-22T00:00:00.000Z",
        sourceRefs: ["github:OpenAgentsInc/openagents#5936"],
      })
      expect(decoded.authorityClass).toBe(authorityClass)
    }

    expect(openAgentsMcpHighRiskAuthorityClasses).toEqual([
      "workspace_write",
      "payment_spend",
      "deployment",
      "admin",
    ])
    expect(isOpenAgentsMcpHighRiskAuthority("payment_spend")).toBe(true)
    expect(isOpenAgentsMcpHighRiskAuthority("payment_receive")).toBe(false)
  })

  test("filters ungranted descriptors out of list results", () => {
    const descriptors = [
      { name: "pylon.health", requiredAuthorities: ["public_read"] as const },
      { name: "pylon.account.read", requiredAuthorities: ["private_account_read"] as const },
      { name: "pylon.session.cancel", requiredAuthorities: ["coding_session_control"] as const },
    ]
    const grants: ReadonlyArray<OpenAgentsMcpGrant> = [
      {
        grantRef: "grant.test.public_read",
        subjectRef: "client.test.read_only",
        authorityClass: "public_read",
        decision: "granted",
        scopeRefs: ["scope.test"],
        grantedAt: "2026-06-22T00:00:00.000Z",
        sourceRefs: ["github:OpenAgentsInc/openagents#5936"],
      },
      {
        grantRef: "grant.test.private_account_read.denied",
        subjectRef: "client.test.read_only",
        authorityClass: "private_account_read",
        decision: "denied",
        scopeRefs: ["scope.test"],
        grantedAt: "2026-06-22T00:00:00.000Z",
        sourceRefs: ["github:OpenAgentsInc/openagents#5936"],
      },
    ]

    expect(filterOpenAgentsMcpDescriptorsByGrantSet(descriptors, grants).map((d) => d.name))
      .toEqual(["pylon.health"])
  })

  test("keeps high-risk tools absent without explicit grants", () => {
    const descriptors = [
      { name: "pylon.wallet.status", requiredAuthorities: ["payment_read"] as const },
      { name: "pylon.wallet.spend", requiredAuthorities: ["payment_spend"] as const },
      { name: "autopilot.deploy.start", requiredAuthorities: ["deployment"] as const },
      { name: "openagents.admin.reconcile", requiredAuthorities: ["admin"] as const },
    ]
    const grants: ReadonlyArray<OpenAgentsMcpGrant> = [
      {
        grantRef: "grant.test.payment_read",
        subjectRef: "client.test.wallet_read",
        authorityClass: "payment_read",
        decision: "granted",
        scopeRefs: ["scope.test"],
        grantedAt: "2026-06-22T00:00:00.000Z",
        sourceRefs: ["github:OpenAgentsInc/openagents#5936"],
      },
    ]

    expect(filterOpenAgentsMcpDescriptorsByGrantSet(descriptors, grants).map((d) => d.name))
      .toEqual(["pylon.wallet.status"])
  })
})
