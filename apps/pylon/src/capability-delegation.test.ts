import { generateKeyPairSync, sign } from "node:crypto"

import { describe, expect, test } from "bun:test"

import {
  PYLON_CAPABILITY_DELEGATION_SCHEMA,
  type PylonCapabilityDelegationEnvelope,
  pylonCapabilityDelegationRef,
  pylonCapabilityDelegationSigningPayload,
  verifyPylonCapabilityDelegation,
} from "./capability-delegation.js"

const { privateKey, publicKey } = generateKeyPairSync("ed25519")
const issuerRef = "issuer.public.openagents.security"
const publicKeyPem = publicKey.export({ format: "pem", type: "spki" }).toString()
const trustedIssuerPublicKeys = new Map([[issuerRef, publicKeyPem]])

const operation = {
  capabilityRef: "capability.pylon.local_codex",
  filesystem: "workspace_write",
  maxWallClockMs: 60_000,
  network: "public_internet",
  sandboxMode: "workspace_write",
  toolRef: "tool.pylon.codex.run",
  workflowRef: "workflow.pylon.codex_agent_task",
} as const

function signedEnvelope(
  input: Omit<PylonCapabilityDelegationEnvelope, "delegationRef" | "signature">,
): PylonCapabilityDelegationEnvelope {
  const unsigned = {
    ...input,
    delegationRef: "delegation.pylon.capability.pending",
    signature: {
      alg: "Ed25519",
      publicKeyPem,
      signatureBase64: "",
    },
  } satisfies PylonCapabilityDelegationEnvelope
  const withRef = {
    ...unsigned,
    delegationRef: pylonCapabilityDelegationRef(unsigned),
  }
  return {
    ...withRef,
    signature: {
      alg: "Ed25519",
      publicKeyPem,
      signatureBase64: sign(
        null,
        pylonCapabilityDelegationSigningPayload(withRef),
        privateKey,
      ).toString("base64"),
    },
  }
}

function rootEnvelope(
  overrides: Partial<Omit<PylonCapabilityDelegationEnvelope, "delegationRef" | "signature">> = {},
): PylonCapabilityDelegationEnvelope {
  return signedEnvelope({
    schema: PYLON_CAPABILITY_DELEGATION_SCHEMA,
    audienceRef: "audience.pylon.assignment",
    capabilityRefs: [
      "capability.pylon.local_codex",
      "capability.pylon.workspace_materializer.v1",
    ],
    constraints: {
      filesystem: "workspace_write",
      maxWallClockMs: 120_000,
      network: "public_internet",
      sandboxMode: "workspace_write",
    },
    expiresAt: "2026-06-27T13:00:00.000Z",
    issuedAt: "2026-06-27T12:00:00.000Z",
    issuerRef,
    subjectRef: "pylon.owner.codex",
    toolRefs: ["tool.pylon.codex.run", "tool.pylon.workspace.materialize"],
    workflowRefs: ["workflow.pylon.codex_agent_task"],
    ...overrides,
  })
}

describe("#6422 Pylon capability delegation verifier", () => {
  test("accepts a signed, unrevoked, least-privilege delegation", () => {
    const delegation = rootEnvelope()

    expect(
      verifyPylonCapabilityDelegation({
        delegation,
        now: new Date("2026-06-27T12:30:00.000Z"),
        operation,
        trustedIssuerPublicKeys,
      }),
    ).toEqual({
      blockerRefs: [],
      delegationRef: delegation.delegationRef,
      ok: true,
    })
  })

  test("rejects expired and revoked delegations", () => {
    const delegation = rootEnvelope()

    const result = verifyPylonCapabilityDelegation({
      delegation,
      now: new Date("2026-06-27T13:00:00.000Z"),
      operation,
      revokedDelegationRefs: new Set([delegation.delegationRef]),
      trustedIssuerPublicKeys,
    })

    expect(result.ok).toBe(false)
    expect(result.blockerRefs).toContain(
      "blocker.pylon.capability_delegation.expired",
    )
    expect(result.blockerRefs).toContain(
      "blocker.pylon.capability_delegation.revoked",
    )
  })

  test("rejects child delegations that broaden parent scope", () => {
    const parent = rootEnvelope({
      capabilityRefs: ["capability.pylon.local_codex"],
      constraints: {
        filesystem: "read_only",
        maxWallClockMs: 30_000,
        network: "loopback",
        sandboxMode: "read_only",
      },
      toolRefs: ["tool.pylon.codex.run"],
    })
    const child = rootEnvelope({
      constraints: {
        filesystem: "workspace_write",
        maxWallClockMs: 60_000,
        network: "public_internet",
        sandboxMode: "workspace_write",
      },
      parentRef: parent.delegationRef,
    })

    const result = verifyPylonCapabilityDelegation({
      delegation: child,
      now: new Date("2026-06-27T12:15:00.000Z"),
      operation,
      parentChain: [parent],
      trustedIssuerPublicKeys,
    })

    expect(result.ok).toBe(false)
    expect(result.blockerRefs).toEqual(
      expect.arrayContaining([
        "blocker.pylon.capability_delegation.parent_capability_exceeded",
        "blocker.pylon.capability_delegation.parent_filesystem_scope_exceeded",
        "blocker.pylon.capability_delegation.parent_network_scope_exceeded",
        "blocker.pylon.capability_delegation.parent_sandbox_scope_exceeded",
        "blocker.pylon.capability_delegation.parent_wall_clock_scope_exceeded",
      ]),
    )
  })

  test("rejects operation requests outside delegated tool and sandbox scope", () => {
    const delegation = rootEnvelope({
      constraints: {
        filesystem: "read_only",
        maxWallClockMs: 10_000,
        network: "loopback",
        sandboxMode: "read_only",
      },
      toolRefs: ["tool.pylon.codex.inspect"],
    })

    const result = verifyPylonCapabilityDelegation({
      delegation,
      now: new Date("2026-06-27T12:15:00.000Z"),
      operation,
      trustedIssuerPublicKeys,
    })

    expect(result.ok).toBe(false)
    expect(result.blockerRefs).toEqual(
      expect.arrayContaining([
        "blocker.pylon.capability_delegation.tool_not_granted",
        "blocker.pylon.capability_delegation.filesystem_scope_exceeded",
        "blocker.pylon.capability_delegation.network_scope_exceeded",
        "blocker.pylon.capability_delegation.sandbox_scope_exceeded",
        "blocker.pylon.capability_delegation.wall_clock_scope_exceeded",
      ]),
    )
  })

  test("rejects tampered signatures and untrusted issuers", () => {
    const delegation = rootEnvelope()
    const tampered = {
      ...delegation,
      toolRefs: ["tool.pylon.codex.inspect"],
    }

    const result = verifyPylonCapabilityDelegation({
      delegation: tampered,
      now: new Date("2026-06-27T12:15:00.000Z"),
      operation,
      trustedIssuerPublicKeys,
    })

    expect(result.ok).toBe(false)
    expect(result.blockerRefs).toContain(
      "blocker.pylon.capability_delegation.ref_mismatch",
    )
    expect(result.blockerRefs).toContain(
      "blocker.pylon.capability_delegation.signature_untrusted",
    )
  })
})
