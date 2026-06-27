import { describe, expect, test } from "bun:test"

import {
  attenuateCapabilityDelegation,
  authorizeCapabilityToolRequest,
  mintCapabilityDelegation,
  signatureRef,
  type CapabilityToolRequest,
} from "../src/tas/capability-delegation"

const issuerSecret = "fixture-local-issuer-secret"
const issuedAt = "2026-06-27T00:00:00.000Z"
const expiresAt = "2026-06-27T00:10:00.000Z"
const now = "2026-06-27T00:02:00.000Z"

const baseRequest: CapabilityToolRequest = {
  audienceRef: "pylon.owner.fixture",
  capabilityRef: "capability.pylon.tool.workspace",
  scopeRef: "scope.repo.public_checkout.read",
  toolRef: "tool.pylon.workspace.read",
  effect: "read_only",
  sourceTrust: "trusted_control",
}

function rootDelegation() {
  return mintCapabilityDelegation({
    issuerRef: "agent.owner.fixture",
    subjectRef: "agent.runner.fixture",
    audienceRef: "pylon.owner.fixture",
    capabilityRefs: [
      "capability.pylon.tool.workspace",
      "capability.pylon.tool.shell",
    ],
    scopeRefs: [
      "scope.repo.public_checkout.read",
      "scope.repo.public_checkout.write",
    ],
    issuedAt,
    expiresAt,
    nonceRef: "nonce.fixture.root",
    issuerSecret,
  })
}

describe("Pylon capability delegation", () => {
  test("authorizes an attenuated least-privilege tool request", () => {
    const root = rootDelegation()
    const leaf = attenuateCapabilityDelegation({
      parent: root,
      subjectRef: "agent.tool.fixture",
      capabilityRefs: ["capability.pylon.tool.workspace"],
      scopeRefs: ["scope.repo.public_checkout.read"],
      caveats: [
        { kind: "tool_ref", toolRef: "tool.pylon.workspace.read" },
        { kind: "effect", effect: "read_only" },
      ],
      issuedAt,
      expiresAt: "2026-06-27T00:05:00.000Z",
      nonceRef: "nonce.fixture.leaf",
    })

    expect(
      authorizeCapabilityToolRequest({
        chain: [root, leaf],
        issuerSecret,
        request: baseRequest,
        now,
      }),
    ).toEqual({
      ok: true,
      evidenceRefs: [
        "receipt.pylon.capability_delegation.chain_verified",
        signatureRef(leaf.signature),
      ],
    })
  })

  test("rejects attenuation that expands parent scope", () => {
    const root = rootDelegation()

    expect(() =>
      attenuateCapabilityDelegation({
        parent: root,
        subjectRef: "agent.tool.fixture",
        scopeRefs: [
          "scope.repo.public_checkout.read",
          "scope.repo.private_checkout.write",
        ],
        issuedAt,
        expiresAt: "2026-06-27T00:05:00.000Z",
        nonceRef: "nonce.fixture.bad_scope",
      }),
    ).toThrow(/expand scope/)
  })

  test("rejects revoked delegated capabilities", () => {
    const root = rootDelegation()
    const revokedSignatureRefs = new Set([signatureRef(root.signature)])

    expect(
      authorizeCapabilityToolRequest({
        chain: [root],
        issuerSecret,
        request: {
          ...baseRequest,
          scopeRef: "scope.repo.public_checkout.write",
        },
        now,
        revokedSignatureRefs,
      }),
    ).toEqual({
      ok: false,
      blockerRefs: ["blocker.pylon.capability_delegation.revoked"],
    })
  })

  test("rejects expired delegated capabilities", () => {
    const root = rootDelegation()

    expect(
      authorizeCapabilityToolRequest({
        chain: [root],
        issuerSecret,
        request: baseRequest,
        now: "2026-06-27T00:11:00.000Z",
      }),
    ).toEqual({
      ok: false,
      blockerRefs: ["blocker.pylon.capability_delegation.expired"],
    })
  })

  test("rejects untrusted data driving an effectful tool", () => {
    const root = rootDelegation()

    expect(
      authorizeCapabilityToolRequest({
        chain: [root],
        issuerSecret,
        request: {
          ...baseRequest,
          capabilityRef: "capability.pylon.tool.shell",
          scopeRef: "scope.repo.public_checkout.write",
          toolRef: "tool.pylon.shell.exec",
          effect: "effectful",
          sourceTrust: "untrusted_data",
        },
        now,
      }),
    ).toEqual({
      ok: false,
      blockerRefs: ["blocker.pylon.capability_delegation.untrusted_effectful_tool"],
    })
  })
})
