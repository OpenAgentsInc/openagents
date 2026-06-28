import { describe, expect, test } from "bun:test"

import {
  createPylonCapabilityDelegation,
  evaluatePylonCapabilityDelegation,
  verifyPylonCapabilityProof,
} from "../src/capability-delegation"

const signingKey = new TextEncoder().encode("public-fixture-delegation-key")

describe("Pylon revocable capability delegation", () => {
  test("creates a signed, time-bounded least-privilege delegation", () => {
    const token = createPylonCapabilityDelegation({
      issuerRef: "agent.owner.artanis",
      audienceRef: "pylon.owner.codex",
      subjectRef: "assignment.public.issue.6422",
      capabilityRefs: [
        "capability.pylon.local_codex",
        "capability.pylon.workspace_materializer",
      ],
      caveats: {
        allowedToolRefs: ["tool.pylon.codex_agent_task", "tool.pylon.git_checkout"],
        sandboxProfileRef: "sandbox.pylon.owner_local.danger_full_access",
        maxUses: 2,
      },
      notBefore: "2026-06-27T00:00:00.000Z",
      expiresAt: "2026-06-27T00:10:00.000Z",
      nonceRef: "nonce.public.issue.6422.root",
      signingKey,
    })

    expect(verifyPylonCapabilityProof(token, signingKey)).toBe(true)
    expect(token.proofRef).toStartWith("proof.pylon.capability_delegation.")
    expect(
      evaluatePylonCapabilityDelegation({
        token,
        signingKey,
        requiredCapabilityRefs: ["capability.pylon.local_codex"],
        requiredToolRef: "tool.pylon.codex_agent_task",
        now: new Date("2026-06-27T00:05:00.000Z"),
      }),
    ).toEqual({
      admitted: true,
      selectedCapabilityRefs: ["capability.pylon.local_codex"],
      blockerRefs: [],
      expiresAt: "2026-06-27T00:10:00.000Z",
    })
  })

  test("attenuates child delegation to a subset of parent capability and tool scope", () => {
    const parent = createPylonCapabilityDelegation({
      issuerRef: "agent.owner.artanis",
      audienceRef: "pylon.owner.codex",
      subjectRef: "assignment.public.issue.6422",
      capabilityRefs: [
        "capability.pylon.local_codex",
        "capability.pylon.workspace_materializer",
      ],
      caveats: {
        allowedToolRefs: ["tool.pylon.codex_agent_task", "tool.pylon.git_checkout"],
        sandboxProfileRef: "sandbox.pylon.owner_local.danger_full_access",
        maxUses: 3,
      },
      notBefore: "2026-06-27T00:00:00.000Z",
      expiresAt: "2026-06-27T00:30:00.000Z",
      nonceRef: "nonce.public.issue.6422.parent",
      signingKey,
    })

    const child = createPylonCapabilityDelegation({
      issuerRef: "agent.owner.artanis",
      audienceRef: "pylon.owner.codex",
      subjectRef: "assignment.public.issue.6422",
      capabilityRefs: ["capability.pylon.local_codex"],
      caveats: {
        allowedToolRefs: ["tool.pylon.codex_agent_task"],
        sandboxProfileRef: "sandbox.pylon.owner_local.danger_full_access",
        maxUses: 1,
      },
      notBefore: "2026-06-27T00:05:00.000Z",
      expiresAt: "2026-06-27T00:20:00.000Z",
      nonceRef: "nonce.public.issue.6422.child",
      signingKey,
      parent,
    })

    expect(child.parentDigestRef).toStartWith("digest.pylon.capability_delegation.")
    expect(child.capabilityRefs).toEqual(["capability.pylon.local_codex"])
    expect(child.caveats.allowedToolRefs).toEqual(["tool.pylon.codex_agent_task"])
  })

  test("rejects child delegations that widen parent authority", () => {
    const parent = createPylonCapabilityDelegation({
      issuerRef: "agent.owner.artanis",
      audienceRef: "pylon.owner.codex",
      subjectRef: "assignment.public.issue.6422",
      capabilityRefs: ["capability.pylon.local_codex"],
      caveats: {
        allowedToolRefs: ["tool.pylon.codex_agent_task"],
        sandboxProfileRef: "sandbox.pylon.owner_local.danger_full_access",
        maxUses: 1,
      },
      notBefore: "2026-06-27T00:00:00.000Z",
      expiresAt: "2026-06-27T00:10:00.000Z",
      nonceRef: "nonce.public.issue.6422.parent",
      signingKey,
    })

    expect(() =>
      createPylonCapabilityDelegation({
        issuerRef: "agent.owner.artanis",
        audienceRef: "pylon.owner.codex",
        subjectRef: "assignment.public.issue.6422",
        capabilityRefs: [
          "capability.pylon.local_codex",
          "capability.pylon.workspace_materializer",
        ],
        caveats: {
          allowedToolRefs: ["tool.pylon.codex_agent_task", "tool.pylon.git_checkout"],
          sandboxProfileRef: "sandbox.pylon.owner_local.danger_full_access",
          maxUses: 2,
        },
        notBefore: "2026-06-26T23:55:00.000Z",
        expiresAt: "2026-06-27T00:20:00.000Z",
        nonceRef: "nonce.public.issue.6422.bad_child",
        signingKey,
        parent,
      }),
    ).toThrow(/capability_widened/)
  })

  test("blocks revoked, expired, tampered, and wrong-tool tokens", () => {
    const token = createPylonCapabilityDelegation({
      issuerRef: "agent.owner.artanis",
      audienceRef: "pylon.owner.codex",
      subjectRef: "assignment.public.issue.6422",
      capabilityRefs: ["capability.pylon.local_codex"],
      caveats: {
        allowedToolRefs: ["tool.pylon.codex_agent_task"],
        sandboxProfileRef: "sandbox.pylon.owner_local.danger_full_access",
        maxUses: 1,
      },
      notBefore: "2026-06-27T00:00:00.000Z",
      expiresAt: "2026-06-27T00:10:00.000Z",
      nonceRef: "nonce.public.issue.6422.revocable",
      signingKey,
    })

    expect(
      evaluatePylonCapabilityDelegation({
        token,
        signingKey,
        requiredCapabilityRefs: ["capability.pylon.local_codex"],
        requiredToolRef: "tool.pylon.codex_agent_task",
        revocations: { revokedRefs: [token.proofRef] },
        now: new Date("2026-06-27T00:05:00.000Z"),
      }).blockerRefs,
    ).toContain("blocker.pylon.capability_delegation.revoked")

    expect(
      evaluatePylonCapabilityDelegation({
        token,
        signingKey,
        requiredCapabilityRefs: ["capability.pylon.local_codex"],
        requiredToolRef: "tool.pylon.codex_agent_task",
        now: new Date("2026-06-27T00:11:00.000Z"),
      }).blockerRefs,
    ).toContain("blocker.pylon.capability_delegation.expired")

    expect(
      evaluatePylonCapabilityDelegation({
        token: { ...token, audienceRef: "pylon.other.codex" },
        signingKey,
        requiredCapabilityRefs: ["capability.pylon.local_codex"],
        requiredToolRef: "tool.pylon.codex_agent_task",
        now: new Date("2026-06-27T00:05:00.000Z"),
      }).blockerRefs,
    ).toContain("blocker.pylon.capability_delegation.bad_proof")

    expect(
      evaluatePylonCapabilityDelegation({
        token,
        signingKey,
        requiredCapabilityRefs: ["capability.pylon.local_codex"],
        requiredToolRef: "tool.pylon.git_checkout",
        now: new Date("2026-06-27T00:05:00.000Z"),
      }).blockerRefs,
    ).toContain("blocker.pylon.capability_delegation.tool_not_allowed")
  })
})
