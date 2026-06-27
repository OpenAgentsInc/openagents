import { describe, expect, test } from "bun:test"

import {
  attenuatePylonCapabilityDelegation,
  issuePylonCapabilityDelegation,
  verifyPylonCapabilityDelegation,
  type PylonCapabilityDelegation,
} from "./capability-delegation.js"

const signingKeys = new Map([["key.pylon.security.test", "test-signing-key"]])
const signingKey = signingKeys.get("key.pylon.security.test")!

const rootCapability = () =>
  issuePylonCapabilityDelegation(
    {
      audienceRef: "pylon.local.owner",
      issuerRef: "owner.openagents.test",
      subjectRef: "agent.artanis.test",
      keyRef: "key.pylon.security.test",
      issuedAt: "2026-06-27T00:00:00.000Z",
      expiresAt: "2026-06-27T01:00:00.000Z",
      actionRefs: [
        "action.pylon.assignment.read",
        "action.pylon.assignment.run_no_spend",
      ],
      scopeRefs: [
        "scope.repo.OpenAgentsInc.openagents",
        "scope.assignment.public.issue_6422",
      ],
      caveatRefs: ["caveat.public_repo_only"],
    },
    signingKey,
  )

describe("Pylon capability delegation (#6422)", () => {
  test("accepts an attenuated, time-bounded delegation chain", () => {
    const root = rootCapability()
    const child = attenuatePylonCapabilityDelegation(
      root,
      {
        audienceRef: "pylon.local.owner",
        subjectRef: "agent.worker.codex.test",
        keyRef: "key.pylon.security.test",
        issuedAt: "2026-06-27T00:05:00.000Z",
        expiresAt: "2026-06-27T00:30:00.000Z",
        actionRefs: ["action.pylon.assignment.run_no_spend"],
        scopeRefs: ["scope.assignment.public.issue_6422"],
        caveatRefs: ["caveat.no_payment_spend"],
      },
      signingKey,
    )

    const verdict = verifyPylonCapabilityDelegation({
      token: child,
      ancestors: [root],
      actionRef: "action.pylon.assignment.run_no_spend",
      scopeRef: "scope.assignment.public.issue_6422",
      audienceRef: "pylon.local.owner",
      now: new Date("2026-06-27T00:10:00.000Z"),
      signingKeys,
    })

    expect(verdict).toMatchObject({
      ok: true,
      tokenRef: child.tokenRef,
      rootRef: root.rootRef,
    })
    expect(child.actionRefs).toEqual(["action.pylon.assignment.run_no_spend"])
    expect(child.scopeRefs).toEqual(["scope.assignment.public.issue_6422"])
    expect(child.caveatRefs).toEqual([
      "caveat.no_payment_spend",
      "caveat.public_repo_only",
    ])
  })

  test("refuses attenuation that expands authority", () => {
    const root = rootCapability()

    expect(() =>
      attenuatePylonCapabilityDelegation(
        root,
        {
          audienceRef: "pylon.local.owner",
          subjectRef: "agent.worker.codex.test",
          keyRef: "key.pylon.security.test",
          issuedAt: "2026-06-27T00:05:00.000Z",
          expiresAt: "2026-06-27T00:30:00.000Z",
          actionRefs: [
            "action.pylon.assignment.run_no_spend",
            "action.pylon.payment.spend",
          ],
          scopeRefs: ["scope.assignment.public.issue_6422"],
        },
        signingKey,
      ),
    ).toThrow("cannot add action refs")
  })

  test("revokes a full chain when any ancestor is revoked", () => {
    const root = rootCapability()
    const child = attenuatePylonCapabilityDelegation(
      root,
      {
        audienceRef: "pylon.local.owner",
        subjectRef: "agent.worker.codex.test",
        keyRef: "key.pylon.security.test",
        issuedAt: "2026-06-27T00:05:00.000Z",
        expiresAt: "2026-06-27T00:30:00.000Z",
        actionRefs: ["action.pylon.assignment.run_no_spend"],
        scopeRefs: ["scope.assignment.public.issue_6422"],
      },
      signingKey,
    )

    const verdict = verifyPylonCapabilityDelegation({
      token: child,
      ancestors: [root],
      actionRef: "action.pylon.assignment.run_no_spend",
      scopeRef: "scope.assignment.public.issue_6422",
      audienceRef: "pylon.local.owner",
      now: new Date("2026-06-27T00:10:00.000Z"),
      revokedTokenRefs: new Set([root.tokenRef]),
      signingKeys,
    })

    expect(verdict).toEqual({
      ok: false,
      tokenRef: child.tokenRef,
      blockerRefs: ["blocker.pylon.capability_delegation.revoked"],
    })
  })

  test("rejects tampered token payloads and derived refs", () => {
    const root = rootCapability()
    const child = attenuatePylonCapabilityDelegation(
      root,
      {
        audienceRef: "pylon.local.owner",
        subjectRef: "agent.worker.codex.test",
        keyRef: "key.pylon.security.test",
        issuedAt: "2026-06-27T00:05:00.000Z",
        expiresAt: "2026-06-27T00:30:00.000Z",
        actionRefs: ["action.pylon.assignment.run_no_spend"],
        scopeRefs: ["scope.assignment.public.issue_6422"],
      },
      signingKey,
    )
    const tampered: PylonCapabilityDelegation = {
      ...child,
      actionRefs: [
        ...child.actionRefs,
        "action.pylon.payment.spend",
      ],
    }

    const verdict = verifyPylonCapabilityDelegation({
      token: tampered,
      ancestors: [root],
      actionRef: "action.pylon.payment.spend",
      scopeRef: "scope.assignment.public.issue_6422",
      audienceRef: "pylon.local.owner",
      now: new Date("2026-06-27T00:10:00.000Z"),
      signingKeys,
    })

    expect(verdict).toEqual({
      ok: false,
      tokenRef: child.tokenRef,
      blockerRefs: [
        "blocker.pylon.capability_delegation.action_expansion",
        "blocker.pylon.capability_delegation.bad_signature",
        "blocker.pylon.capability_delegation.bad_token_ref",
      ],
    })
  })

  test("rejects expired or wrong-audience delegations", () => {
    const root = rootCapability()

    expect(
      verifyPylonCapabilityDelegation({
        token: root,
        actionRef: "action.pylon.assignment.read",
        scopeRef: "scope.repo.OpenAgentsInc.openagents",
        audienceRef: "pylon.other",
        now: new Date("2026-06-27T02:00:00.000Z"),
        signingKeys,
      }),
    ).toEqual({
      ok: false,
      tokenRef: root.tokenRef,
      blockerRefs: [
        "blocker.pylon.capability_delegation.expired",
        "blocker.pylon.capability_delegation.wrong_audience",
      ],
    })
  })
})
